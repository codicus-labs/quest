//! Quest sync and auth server. Implements the protocol in [`quest_protocol`].
//!
//! Run it in-process (e.g. from tests) with [`start`]; `main.rs` does the same from CLI/env config.

mod account;
mod ai;
mod auth;
mod connections;
mod error;
mod health;
mod hub;
mod mcp;
mod oauth;
mod oidc;
mod policy;
mod repo;
mod sync;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::http::header::WWW_AUTHENTICATE;
use axum::http::{HeaderName, Method};
use axum::middleware;
use axum::routing::{delete, get, post, put};
use quest_protocol::{
    ACCOUNT_PATH, ACCOUNT_SETTINGS_PATH, AI_CAPTURE_PATH, AI_CONNECTIONS_PATH, ANDROID_APP_LINK_PATH, ASSET_LINKS_PATH,
    LOGGED_OUT_PATH, LOGOUT_PATH, OIDC_CALLBACK_PATH, SYNC_PATH, UserId,
};
use tokio::sync::{broadcast, oneshot, watch};
use tokio::task::JoinHandle;
use tower_http::cors::{Any, CorsLayer};

pub use ai::{
    AiConfig, AiConfigError, Completion, DEFAULT_AI_TIMEOUT, LlmProvider, OpenAiCompatible, OpenAiSettings,
    OpenRouterOptions, ProviderError, RateLimits, ReasoningEffort, StructuredRequest, Usage, is_openrouter,
    structured_request,
};
pub use error::ServerError;
pub use mcp::McpConfig;
pub use oauth::{
    AuthConfig, CertFingerprint, ClientAddress, ClientMetadata, ClientMetadataSource, FetchError, Fetched,
    HttpsMetadataSource, OAuthRateLimits, Window,
};
pub use oidc::{Identity, IdentityProvider, LoginError, OidcSettings, OpenIdConnect, PendingLogin, StartedLogin};

use ai::AiService;
use hub::Hub;
use oauth::OAuthService;
use repo::Repository;

#[derive(Clone, Debug)]
pub struct Config {
    /// Use port 0 to get an ephemeral port; see [`RunningServer::addr`].
    pub bind: SocketAddr,
    /// e.g. `sqlite://quest-server.sqlite`; the file is created if missing.
    pub database_url: String,
    /// AI capture; `None` switches it off (the route answers `AiNotConfigured`).
    pub ai: Option<AiConfig>,
    /// Sign-in at the identity provider and the OAuth authorization server (always on).
    pub auth: AuthConfig,
    /// The MCP server at `/mcp`; `None` leaves it out (the OAuth endpoints stay for the apps).
    pub mcp: Option<McpConfig>,
}

/// Liveness limits of sync connections (see the `quest_protocol` docs).
#[derive(Clone, Copy, Debug)]
pub struct SyncTimeouts {
    /// The client must send `Hello` within this time after connecting.
    pub hello: Duration,
    /// A connection is dropped if the client sends nothing (not even a pong) for this long, or
    /// does not take a frame within it.
    pub idle: Duration,
    /// Interval of the server's pings.
    pub ping: Duration,
    /// How long a closing connection may take to flush its last frames.
    pub writer_drain: Duration,
}

impl Default for SyncTimeouts {
    fn default() -> Self {
        SyncTimeouts {
            hello: Duration::from_secs(10),
            idle: Duration::from_secs(45),
            ping: Duration::from_secs(15),
            writer_drain: Duration::from_secs(2),
        }
    }
}

/// Revocations an open sync connection may fall behind on before it re-checks its grant.
const REVOKED_GRANTS_BACKLOG: usize = 64;

/// Shared state of one server instance.
#[derive(Clone)]
pub struct AppState {
    repo: Repository,
    hub: Hub,
    timeouts: SyncTimeouts,
    shutdown: Arc<watch::Sender<bool>>,
    ai: Option<Arc<AiService>>,
    oauth: Arc<OAuthService>,
    mcp: Option<McpConfig>,
    /// Ids of grants revoked by this instance, so open sync connections of them close at once.
    revoked_grants: broadcast::Sender<uuid::Uuid>,
}

impl AppState {
    /// Opens (and migrates) the database. `bound` is the listening address, the public URL when
    /// `auth` names none.
    pub async fn open(
        database_url: &str,
        timeouts: SyncTimeouts,
        auth: AuthConfig,
        bound: SocketAddr,
    ) -> Result<AppState, ServerError> {
        let oauth = OAuthService::new(auth, bound).map_err(ServerError::Auth)?;
        let repo = Repository::open(database_url).await?;
        Ok(AppState {
            repo,
            hub: Hub::default(),
            timeouts,
            shutdown: Arc::new(watch::Sender::new(false)),
            ai: None,
            oauth: Arc::new(oauth),
            mcp: None,
            revoked_grants: broadcast::Sender::new(REVOKED_GRANTS_BACKLOG),
        })
    }

    /// Switches AI capture on with `config`, or off with `None`.
    pub fn with_ai(mut self, config: Option<AiConfig>) -> Self {
        self.ai = config.map(|config| Arc::new(AiService::new(config)));
        self
    }

    /// Switches the MCP server on with `config`, or off with `None`.
    pub fn with_mcp(mut self, config: Option<McpConfig>) -> Self {
        self.mcp = config;
        self
    }

    /// Closes all open sync connections, now and in the future.
    pub fn close_connections(&self) {
        self.shutdown.send_replace(true);
    }

    fn is_shutting_down(&self) -> bool {
        *self.shutdown.borrow()
    }

    fn shutdown_signal(&self) -> watch::Receiver<bool> {
        self.shutdown.subscribe()
    }

    /// Revokes the grant (with `user`, only if it is theirs) and tells its open sync connections.
    /// Returns false if there was no such active grant.
    pub(crate) async fn revoke_grant(&self, grant: uuid::Uuid, user: Option<UserId>) -> sqlx::Result<bool> {
        let revoked = self.repo.revoke_grant(grant, user, unix_ms()).await?;
        if revoked {
            // No receivers is fine: then no sync connection uses the grant.
            let _ = self.revoked_grants.send(grant);
        }
        Ok(revoked)
    }

    fn revoked_grants(&self) -> broadcast::Receiver<uuid::Uuid> {
        self.revoked_grants.subscribe()
    }
}

pub fn app(state: AppState) -> Router {
    let limit = middleware::from_fn_with_state(state.clone(), oauth::limit_requests);
    let router = Router::new()
        .route(health::LIVE_PATH, get(health::live))
        .route(health::READY_PATH, get(health::ready))
        .route(SYNC_PATH, get(sync::upgrade))
        .route(ACCOUNT_PATH, get(account::get))
        .route(ACCOUNT_SETTINGS_PATH, put(account::update_settings))
        .route(AI_CAPTURE_PATH, post(ai::capture))
        .route(AI_CONNECTIONS_PATH, get(connections::list))
        .route(&format!("{AI_CONNECTIONS_PATH}/{{id}}"), delete(connections::revoke))
        .route(OIDC_CALLBACK_PATH, get(oauth::callback).layer(limit.clone()))
        .route(LOGOUT_PATH, get(oauth::logout))
        .route(LOGGED_OUT_PATH, get(oauth::logged_out))
        .route(ANDROID_APP_LINK_PATH, get(oauth::android_app_link_page))
        .route(ASSET_LINKS_PATH, get(oauth::asset_links_file))
        .route(oauth::AUTHORIZE_PATH, get(oauth::authorize_page).post(oauth::authorize_form).layer(limit))
        .merge(oauth_routes(&state));
    let router = match state.mcp.clone() {
        Some(config) => router.merge(mcp_routes(&state, &config)),
        None => router,
    };
    router.with_state(state)
}

/// CORS for the endpoints browser-based MCP clients call; tokens travel in headers, never cookies.
fn cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any)
        .expose_headers([
            WWW_AUTHENTICATE,
            HeaderName::from_static("mcp-session-id"),
            HeaderName::from_static("mcp-protocol-version"),
        ])
}

/// The OAuth endpoints (always on). `/register` and `/token` are rate-limited per client address.
fn oauth_routes(state: &AppState) -> Router<AppState> {
    let limit = middleware::from_fn_with_state(state.clone(), oauth::limit_requests);
    Router::new()
        .route(oauth::AUTHORIZATION_SERVER_PATH, get(oauth::authorization_server))
        .route(oauth::TOKEN_PATH, post(oauth::token).layer(limit.clone()))
        .route(oauth::REGISTER_PATH, post(oauth::register).layer(limit))
        .route(oauth::REVOKE_PATH, post(oauth::revoke))
        .layer(cors())
}

/// `/mcp` behind the access-token check, and its protected resource metadata.
fn mcp_routes(state: &AppState, config: &McpConfig) -> Router<AppState> {
    let mcp = Router::new()
        .route_service(oauth::policy::MCP_PATH, mcp::service(state.clone(), &state.oauth, config))
        .layer(middleware::from_fn_with_state(state.clone(), mcp::require_token));
    let resource_metadata = get(oauth::protected_resource);
    Router::new()
        .merge(mcp)
        .route(oauth::PROTECTED_RESOURCE_PATH, resource_metadata.clone())
        .route(&format!("{}{}", oauth::PROTECTED_RESOURCE_PATH, oauth::policy::MCP_PATH), resource_metadata)
        .layer(cors())
}

/// Opens the database, binds `config.bind` and serves in a background task until
/// [`RunningServer::shutdown`] is called or the handle is dropped.
pub async fn start(config: Config) -> Result<RunningServer, ServerError> {
    start_with(config, SyncTimeouts::default()).await
}

/// [`start`] with custom connection timeouts, e.g. short ones in tests.
pub async fn start_with(config: Config, timeouts: SyncTimeouts) -> Result<RunningServer, ServerError> {
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|source| ServerError::Bind { addr: config.bind, source })?;
    let addr = listener.local_addr().map_err(|source| ServerError::Bind { addr: config.bind, source })?;
    let state = AppState::open(&config.database_url, timeouts, config.auth, addr)
        .await?
        .with_ai(config.ai)
        .with_mcp(config.mcp);
    if state.mcp.is_some() {
        tracing::info!(resource = %state.oauth.resource, "MCP enabled");
    }
    let (stop, stopped) = oneshot::channel::<()>();
    let router = app(state.clone()).into_make_service_with_connect_info::<SocketAddr>();
    let task = tokio::spawn(async move {
        let served = axum::serve(listener, router).with_graceful_shutdown(async {
            let _ = stopped.await;
        });
        if let Err(error) = served.await {
            tracing::error!(%error, "server stopped with an error");
        }
    });
    tracing::info!(%addr, "quest-server listening");
    Ok(RunningServer { addr, state, stop: Some(stop), task: Some(task) })
}

/// A server started with [`start`]. Dropping it stops the server without waiting.
pub struct RunningServer {
    addr: SocketAddr,
    state: AppState,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl RunningServer {
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Base URL for the HTTP endpoints, e.g. `http://127.0.0.1:8787`.
    pub fn http_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// The OAuth issuer and origin the resources are named after (the public URL, or the
    /// `http_url` without one).
    pub fn issuer(&self) -> &str {
        &self.state.oauth.issuer
    }

    /// Full URL of the sync WebSocket, e.g. `ws://127.0.0.1:8787/v1/sync`.
    pub fn ws_url(&self) -> String {
        format!("ws://{}{SYNC_PATH}", self.addr)
    }

    /// Closes all connections, stops listening and waits until the database is closed.
    pub async fn shutdown(mut self) {
        self.signal_stop();
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
        self.state.repo.close().await;
    }

    fn signal_stop(&mut self) {
        self.state.close_connections();
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.signal_stop();
    }
}

/// The wall clock in Unix milliseconds, as stored in the database.
pub(crate) fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
}
