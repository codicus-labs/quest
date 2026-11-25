use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use chrono_tz::Tz;
use clap::Parser;
use quest_server::{
    AiConfig, AuthConfig, CertFingerprint, ClientAddress, Config, DEFAULT_AI_TIMEOUT, McpConfig, OidcSettings,
    OpenAiCompatible, OpenAiSettings, OpenRouterOptions, ReasoningEffort, start,
};
use tracing_subscriber::EnvFilter;
use url::Url;

/// Quest sync and auth server.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Address to listen on.
    #[arg(long, env = "QUEST_BIND", default_value = "127.0.0.1:8787")]
    bind: SocketAddr,
    /// SQLite database URL; the file is created if missing.
    #[arg(long, env = "QUEST_DATABASE_URL", default_value = "sqlite://quest-server.sqlite")]
    database_url: String,
    /// Log lines as text (for a terminal) or as one JSON object per line (for a log collector).
    #[arg(long, env = "QUEST_LOG_FORMAT", value_enum, default_value_t = LogFormat::Text)]
    log_format: LogFormat,
    #[command(flatten)]
    ai: AiArgs,
    #[command(flatten)]
    auth: AuthArgs,
    #[command(flatten)]
    mcp: McpArgs,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum LogFormat {
    Text,
    Json,
}

/// Sign-in: the OpenID Connect provider and this server's public origin.
#[derive(clap::Args)]
struct AuthArgs {
    /// The https origin clients reach this server at, e.g. https://quest.example.com (behind a TLS
    /// proxy). Required unless listening on loopback (then http://<bind>).
    #[arg(long, env = "QUEST_PUBLIC_URL")]
    public_url: Option<Url>,
    /// Issuer of the OpenID Connect provider, e.g. https://id.example.com or
    /// http://127.0.0.1:8091/realms/quest (local Keycloak). Discovery at /.well-known/openid-configuration.
    #[arg(long, env = "QUEST_OIDC_ISSUER")]
    oidc_issuer: Option<Url>,
    /// This server's client id at the provider.
    #[arg(long, env = "QUEST_OIDC_CLIENT_ID")]
    oidc_client_id: Option<String>,
    /// This server's client secret at the provider (prefer the environment variable over the flag).
    #[arg(long, env = "QUEST_OIDC_CLIENT_SECRET", hide_env_values = true)]
    oidc_client_secret: Option<String>,
    /// Rate-limit OAuth requests by the last X-Forwarded-For entry instead of the connecting address.
    /// Only when every request comes through your reverse proxy and it sets that header.
    #[arg(long, env = "QUEST_TRUST_FORWARDED_FOR", conflicts_with = "client_ip_header")]
    trust_forwarded_for: bool,
    /// Rate-limit OAuth requests by the one address in this header, e.g. x-real-ip set by the
    /// proxy in front. Only when every request comes through the edge that sets it. A missing or
    /// unreadable header counts the connecting address.
    #[arg(long, env = "QUEST_CLIENT_IP_HEADER")]
    client_ip_header: Option<axum::http::HeaderName>,
    /// SHA-256 fingerprints (AB:CD:…, comma-separated) of the Android app's signing certificates,
    /// served in /.well-known/assetlinks.json for an optional App Link sign-in. Unset: no such file.
    #[arg(long, env = "QUEST_ANDROID_CERT_SHA256", value_delimiter = ',')]
    android_cert_sha256: Vec<CertFingerprint>,
}

impl AuthArgs {
    fn into_config(self, bind: SocketAddr) -> Result<AuthConfig, String> {
        let (Some(issuer), Some(client_id), Some(client_secret)) =
            (self.oidc_issuer, self.oidc_client_id, self.oidc_client_secret)
        else {
            return Err("sign-in is not configured: set QUEST_OIDC_ISSUER, QUEST_OIDC_CLIENT_ID and \
                        QUEST_OIDC_CLIENT_SECRET (for local development: run `cargo run -p quest-oidc-fake` and use \
                        the values it prints; see docs/auth.md)"
                .to_owned());
        };
        if self.public_url.is_none() && !bind.ip().is_loopback() {
            return Err("set QUEST_PUBLIC_URL to the server's https origin (it is the OAuth issuer and the \
                        sign-in redirect)"
                .to_owned());
        }
        let mut config = AuthConfig::with_oidc(self.public_url, OidcSettings { issuer, client_id, client_secret })?;
        config.client_address = match (self.client_ip_header, self.trust_forwarded_for) {
            (Some(header), _) => ClientAddress::Header(header),
            (None, true) => ClientAddress::LastForwardedFor,
            (None, false) => ClientAddress::Peer,
        };
        config.android_cert_fingerprints = self.android_cert_sha256;
        Ok(config)
    }
}

/// The MCP server at /mcp.
#[derive(clap::Args)]
struct McpArgs {
    /// IANA time zone for MCP tool calls that do not name one, e.g. Europe/Berlin.
    #[arg(long, env = "QUEST_TIME_ZONE", default_value = "UTC")]
    time_zone: Tz,
    /// Switch MCP off (the OAuth endpoints stay on for the apps).
    #[arg(long, env = "QUEST_MCP_DISABLED")]
    mcp_disabled: bool,
}

impl McpArgs {
    fn into_config(self) -> Option<McpConfig> {
        (!self.mcp_disabled).then_some(McpConfig { default_time_zone: self.time_zone })
    }
}

/// AI capture. Off unless base URL, key and model are all set.
#[derive(clap::Args)]
struct AiArgs {
    /// OpenAI-compatible API root, e.g. https://openrouter.ai/api/v1 or https://api.openai.com/v1.
    #[arg(long, env = "QUEST_AI_BASE_URL")]
    ai_base_url: Option<String>,
    /// API key of the AI provider (prefer the environment variable over the flag).
    #[arg(long, env = "QUEST_AI_API_KEY", hide_env_values = true)]
    ai_api_key: Option<String>,
    /// Model name, e.g. openai/gpt-4.1-mini (OpenRouter) or gpt-4.1-mini (OpenAI).
    #[arg(long, env = "QUEST_AI_MODEL")]
    ai_model: Option<String>,
    /// Timeout of one AI call in seconds.
    #[arg(long, env = "QUEST_AI_TIMEOUT_SECS", default_value_t = DEFAULT_AI_TIMEOUT.as_secs())]
    ai_timeout_secs: u64,
    /// OpenRouter: `HTTP-Referer` header (your app's URL).
    #[arg(long, env = "QUEST_AI_HTTP_REFERER")]
    ai_http_referer: Option<String>,
    /// OpenRouter: `X-Title` header (your app's name).
    #[arg(long, env = "QUEST_AI_APP_TITLE")]
    ai_app_title: Option<String>,
    /// Allow plain http:// to a non-loopback AI host (loopback, e.g. a local mock, is always allowed).
    #[arg(long, env = "QUEST_AI_ALLOW_HTTP")]
    ai_allow_http: bool,
    /// OpenRouter: models to try in order when the main one fails, comma-separated.
    #[arg(long, env = "QUEST_AI_FALLBACK_MODELS", value_delimiter = ',')]
    ai_fallback_models: Vec<String>,
    /// OpenRouter: reasoning effort (none, minimal, low, medium, high), or "default" to leave it to the model.
    #[arg(long, env = "QUEST_AI_REASONING_EFFORT", default_value = "none")]
    ai_reasoning_effort: ReasoningEffort,
    /// Send OpenRouter's request fields; detected from the base URL when not set.
    #[arg(long, env = "QUEST_AI_OPENROUTER")]
    ai_openrouter: Option<bool>,
}

impl AiArgs {
    fn into_config(self) -> Result<Option<AiConfig>, String> {
        let (base_url, api_key, model) = match (self.ai_base_url, self.ai_api_key, self.ai_model) {
            (None, None, None) => return Ok(None),
            (Some(base_url), Some(api_key), Some(model)) => (base_url, api_key, model),
            (base_url, api_key, model) => {
                let missing: Vec<&str> = [
                    (base_url.is_none(), "QUEST_AI_BASE_URL"),
                    (api_key.is_none(), "QUEST_AI_API_KEY"),
                    (model.is_none(), "QUEST_AI_MODEL"),
                ]
                .into_iter()
                .filter_map(|(is_missing, name)| is_missing.then_some(name))
                .collect();
                return Err(format!(
                    "AI capture is partly configured; also set {} or unset all QUEST_AI_* variables",
                    missing.join(", ")
                ));
            }
        };
        let extra_headers = [("HTTP-Referer", self.ai_http_referer), ("X-Title", self.ai_app_title)]
            .into_iter()
            .filter_map(|(name, value)| value.map(|value| (name.to_owned(), value)))
            .collect();
        let openrouter = OpenRouterOptions::for_base_url(
            &base_url,
            self.ai_openrouter,
            self.ai_fallback_models,
            self.ai_reasoning_effort,
        )
        .map_err(|error| error.to_string())?;
        let settings = OpenAiSettings {
            base_url,
            api_key,
            model,
            extra_headers,
            allow_insecure_http: self.ai_allow_http,
            openrouter,
        };
        let provider = OpenAiCompatible::new(settings).map_err(|error| error.to_string())?;
        let mut config = AiConfig::new(Arc::new(provider));
        config.timeout = Duration::from_secs(self.ai_timeout_secs.max(1));
        Ok(Some(config))
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    init_logging(args.log_format);
    let ai = match args.ai.into_config() {
        Ok(ai) => ai,
        Err(error) => {
            tracing::error!("{error}");
            return ExitCode::FAILURE;
        }
    };
    match &ai {
        Some(ai) => tracing::info!(provider = ?ai.provider, timeout_secs = ai.timeout.as_secs(), "AI capture enabled"),
        None => tracing::info!("AI capture disabled (QUEST_AI_* not set)"),
    }
    let auth = match args.auth.into_config(args.bind) {
        Ok(auth) => auth,
        Err(error) => {
            tracing::error!("{error}");
            return ExitCode::FAILURE;
        }
    };
    tracing::info!(identity = ?auth.identity, android_certificates = auth.android_cert_fingerprints.len(), "sign-in configured");
    let mcp = args.mcp.into_config();
    let server = match start(Config { bind: args.bind, database_url: args.database_url, ai, auth, mcp }).await {
        Ok(server) => server,
        Err(error) => {
            tracing::error!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let signal = stop_signal().await;
    tracing::info!(signal, "shutting down");
    if tokio::time::timeout(SHUTDOWN_LIMIT, server.shutdown()).await.is_err() {
        tracing::warn!(limit_secs = SHUTDOWN_LIMIT.as_secs(), "requests still running; stopping without them");
        // SQLite's WAL keeps the data consistent without closing the pool; the exit code shows the cut.
        return ExitCode::FAILURE;
    }
    tracing::info!("stopped");
    ExitCode::SUCCESS
}

/// How long running requests (e.g. an AI capture, up to its timeout) may take after SIGTERM; below
/// Kubernetes' default grace period of 30 s.
const SHUTDOWN_LIMIT: Duration = Duration::from_secs(20);

fn init_logging(format: LogFormat) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,rmcp=warn"));
    let logs = tracing_subscriber::fmt().with_env_filter(filter);
    match format {
        LogFormat::Text => logs.init(),
        LogFormat::Json => logs.json().flatten_event(true).with_current_span(false).init(),
    }
}

/// Waits for Ctrl-C or, on Unix, SIGTERM (what Kubernetes sends before it kills the pod).
async fn stop_signal() -> &'static str {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "cannot wait for Ctrl-C");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut terminate) => {
                terminate.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "cannot wait for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => "SIGINT",
        () = terminate => "SIGTERM",
    }
}
