//! [`LlmProvider`] for OpenAI-compatible Chat Completions APIs (OpenAI, OpenRouter, ...).
//!
//! Asks for structured output (`response_format: json_schema`, strict). If the provider rejects that
//! for the model, it falls back once to a forced tool call whose arguments are the answer, and keeps
//! using tool calls from then on if that works.
//!
//! OpenRouter gets extra fields ([`OpenRouterOptions`]): `provider.require_parameters` so a provider
//! that cannot do `json_schema` is never picked (and would otherwise ignore it silently), the model
//! fallback list `models`, and `reasoning`. Other APIs (OpenAI) never see them.

use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures_util::future::BoxFuture;
use reqwest::Url;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderName, HeaderValue};
use rustls::ClientConfig;
use rustls_platform_verifier::BuilderVerifierExt;
use serde::Deserialize;
use serde_json::{Value, json};

use super::provider::{Completion, LlmProvider, ProviderError, StructuredRequest, Usage};

/// Answers larger than this are not read to the end; a capture answer is a few KiB.
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// What [`OpenAiCompatible::new`] needs; usually from `QUEST_AI_*`.
#[derive(Clone)]
pub struct OpenAiSettings {
    /// API root without `/chat/completions`, e.g. `https://openrouter.ai/api/v1`.
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    /// Sent with every request, e.g. OpenRouter's `HTTP-Referer` and `X-Title`.
    pub extra_headers: Vec<(String, String)>,
    /// Allows `http://` to hosts other than loopback. Loopback (for a local mock) is always allowed.
    pub allow_insecure_http: bool,
    /// OpenRouter-only request fields; `None` for any other API. See [`is_openrouter`].
    pub openrouter: Option<OpenRouterOptions>,
}

/// Request fields only OpenRouter understands (OpenAI rejects unknown fields).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenRouterOptions {
    /// Tried in order when the main model fails (sent as `models`, after the main model).
    pub fallback_models: Vec<String>,
    pub reasoning: ReasoningEffort,
}

/// How much the model may think before answering. Capture needs no reasoning; it only adds
/// latency and cost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReasoningEffort {
    /// Sends no `reasoning` field: the model's default. Needed for models without reasoning
    /// support, because `require_parameters` would otherwise find no provider for them.
    ModelDefault,
    #[default]
    None,
    Minimal,
    Low,
    Medium,
    High,
}

impl ReasoningEffort {
    fn wire_name(self) -> Option<&'static str> {
        match self {
            ReasoningEffort::ModelDefault => None,
            ReasoningEffort::None => Some("none"),
            ReasoningEffort::Minimal => Some("minimal"),
            ReasoningEffort::Low => Some("low"),
            ReasoningEffort::Medium => Some("medium"),
            ReasoningEffort::High => Some("high"),
        }
    }
}

impl FromStr for ReasoningEffort {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim().to_ascii_lowercase().as_str() {
            "default" => Ok(ReasoningEffort::ModelDefault),
            "none" => Ok(ReasoningEffort::None),
            "minimal" => Ok(ReasoningEffort::Minimal),
            "low" => Ok(ReasoningEffort::Low),
            "medium" => Ok(ReasoningEffort::Medium),
            "high" => Ok(ReasoningEffort::High),
            _ => Err(format!("unknown reasoning effort {text:?}; use none, minimal, low, medium, high or default")),
        }
    }
}

impl OpenRouterOptions {
    /// The options for `base_url`: OpenRouter is recognised by its host unless `force` says otherwise
    /// (e.g. a proxy in front of OpenRouter). Fallback models need OpenRouter; the reasoning setting
    /// is simply not sent elsewhere.
    pub fn for_base_url(
        base_url: &str,
        force: Option<bool>,
        fallback_models: Vec<String>,
        reasoning: ReasoningEffort,
    ) -> Result<Option<Self>, AiConfigError> {
        let fallback_models: Vec<String> =
            fallback_models.into_iter().map(|model| model.trim().to_owned()).filter(|m| !m.is_empty()).collect();
        if !force.unwrap_or_else(|| is_openrouter(base_url)) {
            if !fallback_models.is_empty() {
                return Err(AiConfigError::FallbackNeedsOpenRouter);
            }
            return Ok(None);
        }
        Ok(Some(OpenRouterOptions { fallback_models, reasoning }))
    }
}

/// Whether `base_url` points at OpenRouter (`openrouter.ai` or a subdomain).
pub fn is_openrouter(base_url: &str) -> bool {
    Url::parse(base_url.trim())
        .ok()
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
        .is_some_and(|host| host == "openrouter.ai" || host.ends_with(".openrouter.ai"))
}

impl fmt::Debug for OpenAiSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiSettings")
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .field("model", &self.model)
            .field("extra_headers", &self.extra_headers)
            .field("allow_insecure_http", &self.allow_insecure_http)
            .field("openrouter", &self.openrouter)
            .finish()
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum AiConfigError {
    #[error("QUEST_AI_BASE_URL {0:?} is not a valid http(s) URL; use e.g. https://openrouter.ai/api/v1")]
    InvalidBaseUrl(String),
    #[error(
        "QUEST_AI_BASE_URL {0:?} uses plain http to a non-loopback host, which would send the API key \
         unencrypted; use https or set QUEST_AI_ALLOW_HTTP=true"
    )]
    InsecureHttp(String),
    #[error("QUEST_AI_BASE_URL must not contain a user name or password; put the key in QUEST_AI_API_KEY")]
    CredentialsInBaseUrl,
    #[error("{0} contains characters that are not allowed in an HTTP header; check its value")]
    InvalidHeader(String),
    #[error(
        "QUEST_AI_FALLBACK_MODELS only works with OpenRouter; unset it, or set QUEST_AI_OPENROUTER=true if \
         QUEST_AI_BASE_URL is a proxy in front of OpenRouter"
    )]
    FallbackNeedsOpenRouter,
    #[error("cannot set up TLS for the AI provider: {0}")]
    Tls(String),
}

pub struct OpenAiCompatible {
    endpoint: Url,
    model: String,
    /// Authorization (marked sensitive) and the extra headers.
    headers: HeaderMap,
    extra_header_names: Vec<String>,
    openrouter: Option<OpenRouterOptions>,
    client: reqwest::Client,
    /// Set once the provider rejected `response_format: json_schema` and tool calling worked.
    use_tool_calling: AtomicBool,
}

impl fmt::Debug for OpenAiCompatible {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiCompatible")
            .field("endpoint", &self.endpoint.as_str())
            .field("model", &self.model)
            .field("api_key", &"<redacted>")
            .field("extra_headers", &self.extra_header_names)
            .field("openrouter", &self.openrouter)
            .finish()
    }
}

impl OpenAiCompatible {
    pub fn new(settings: OpenAiSettings) -> Result<Self, AiConfigError> {
        let endpoint = chat_completions_url(&settings.base_url, settings.allow_insecure_http)?;
        let mut headers = HeaderMap::new();
        let mut authorization = HeaderValue::from_str(&format!("Bearer {}", settings.api_key))
            .map_err(|_| AiConfigError::InvalidHeader("QUEST_AI_API_KEY".into()))?;
        authorization.set_sensitive(true);
        headers.insert(AUTHORIZATION, authorization);
        for (name, value) in &settings.extra_headers {
            let invalid = || AiConfigError::InvalidHeader(format!("header {name}"));
            let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| invalid())?;
            headers.insert(name, HeaderValue::from_str(value).map_err(|_| invalid())?);
        }
        let client = reqwest::Client::builder()
            .tls_backend_preconfigured(tls_config()?)
            .connect_timeout(CONNECT_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| AiConfigError::Tls(error.to_string()))?;
        Ok(OpenAiCompatible {
            endpoint,
            model: settings.model,
            headers,
            extra_header_names: settings.extra_headers.into_iter().map(|(name, _)| name).collect(),
            openrouter: settings.openrouter,
            client,
            use_tool_calling: AtomicBool::new(false),
        })
    }

    async fn complete(&self, request: &StructuredRequest) -> Result<Completion, ProviderError> {
        if self.use_tool_calling.load(Ordering::Relaxed) {
            return self.complete_with_tool_call(request).await;
        }
        match self.send(&self.json_schema_body(request)).await? {
            Reply::Ok(body) => parse_answer(&body),
            Reply::Failed { status, body } if rejects_json_schema(status, &body, self.openrouter.is_some()) => {
                let completion = self.complete_with_tool_call(request).await.inspect_err(|error| {
                    if self.openrouter.is_some() && *error == (ProviderError::Rejected { status: 404 }) {
                        tracing::warn!(
                            "OpenRouter has no provider for this model with these parameters, with json_schema \
                             nor with tool calling; if the model has no reasoning support, set \
                             QUEST_AI_REASONING_EFFORT=default"
                        );
                    }
                })?;
                tracing::info!("AI provider rejects json_schema for this model; using tool calling");
                self.use_tool_calling.store(true, Ordering::Relaxed);
                Ok(completion)
            }
            Reply::Failed { status, .. } => Err(status_error(status)),
        }
    }

    async fn complete_with_tool_call(&self, request: &StructuredRequest) -> Result<Completion, ProviderError> {
        match self.send(&self.tool_call_body(request)).await? {
            Reply::Ok(body) => parse_answer(&body),
            Reply::Failed { status, .. } => Err(status_error(status)),
        }
    }

    fn messages(request: &StructuredRequest) -> Value {
        json!([
            { "role": "system", "content": request.system },
            { "role": "user", "content": request.user },
        ])
    }

    fn json_schema_body(&self, request: &StructuredRequest) -> Value {
        self.with_openrouter_fields(json!({
            "model": self.model,
            "messages": Self::messages(request),
            "response_format": {
                "type": "json_schema",
                "json_schema": { "name": request.schema_name, "strict": true, "schema": request.schema },
            },
        }))
    }

    fn tool_call_body(&self, request: &StructuredRequest) -> Value {
        self.with_openrouter_fields(json!({
            "model": self.model,
            "messages": Self::messages(request),
            "tools": [{
                "type": "function",
                "function": {
                    "name": request.schema_name,
                    "description": "Return the structured result.",
                    "parameters": request.schema,
                },
            }],
            "tool_choice": { "type": "function", "function": { "name": request.schema_name } },
        }))
    }

    pub(crate) fn with_openrouter_fields(&self, mut body: Value) -> Value {
        let Some(options) = &self.openrouter else { return body };
        body["provider"] = json!({ "require_parameters": true });
        if !options.fallback_models.is_empty() {
            let models: Vec<&str> = std::iter::once(self.model.as_str())
                .chain(options.fallback_models.iter().map(String::as_str))
                .collect();
            body["models"] = json!(models);
        }
        if let Some(effort) = options.reasoning.wire_name() {
            body["reasoning"] = json!({ "effort": effort });
        }
        body
    }

    async fn send(&self, body: &Value) -> Result<Reply, ProviderError> {
        let response = self
            .client
            .post(self.endpoint.clone())
            .headers(self.headers.clone())
            .json(body)
            .send()
            .await
            .map_err(transport_error)?;
        let status = response.status().as_u16();
        let body = read_limited(response).await?;
        if (200..300).contains(&status) {
            return Ok(Reply::Ok(body));
        }
        Ok(Reply::Failed { status, body })
    }
}

impl LlmProvider for OpenAiCompatible {
    fn complete_structured<'a>(
        &'a self,
        request: &'a StructuredRequest,
    ) -> BoxFuture<'a, Result<Completion, ProviderError>> {
        Box::pin(self.complete(request))
    }
}

enum Reply {
    Ok(Vec<u8>),
    /// The body is only inspected, never logged: it may echo the prompt.
    Failed {
        status: u16,
        body: Vec<u8>,
    },
}

pub(crate) fn tls_config() -> Result<ClientConfig, AiConfigError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .and_then(|builder| builder.with_platform_verifier())
        .map(|builder| builder.with_no_client_auth())
        .map_err(|error| AiConfigError::Tls(error.to_string()))
}

pub(crate) fn chat_completions_url(base_url: &str, allow_insecure_http: bool) -> Result<Url, AiConfigError> {
    let invalid = || AiConfigError::InvalidBaseUrl(base_url.to_owned());
    let mut url = Url::parse(base_url.trim()).map_err(|_| invalid())?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AiConfigError::CredentialsInBaseUrl);
    }
    match url.scheme() {
        "https" => {}
        "http" if allow_insecure_http || is_loopback(&url) => {}
        "http" => return Err(AiConfigError::InsecureHttp(base_url.to_owned())),
        _ => return Err(invalid()),
    }
    if url.cannot_be_a_base() || url.host().is_none() {
        return Err(invalid());
    }
    let path = format!("{}/chat/completions", url.path().trim_end_matches('/'));
    url.set_path(&path);
    Ok(url)
}

fn is_loopback(url: &Url) -> bool {
    let Some(host) = url.host_str() else { return false };
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let ip = host.trim_start_matches('[').trim_end_matches(']');
    ip.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

async fn read_limited(mut response: reqwest::Response) -> Result<Vec<u8>, ProviderError> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(ProviderError::InvalidResponse);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn transport_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() { ProviderError::Timeout } else { ProviderError::Unreachable }
}

pub(crate) fn status_error(status: u16) -> ProviderError {
    match status {
        429 => ProviderError::RateLimited,
        408 | 504 => ProviderError::Timeout,
        _ => ProviderError::Rejected { status },
    }
}

/// Whether a failed request means the model cannot do structured output, as OpenAI and OpenRouter
/// word it: a 400 that blames structured output, or (OpenRouter with `require_parameters`) a 404
/// because no provider of the model supports all parameters.
/// Not every 400 naming `response_format` counts: "Invalid schema for response_format" means our
/// schema is wrong, and falling back would hide that.
pub(crate) fn rejects_json_schema(status: u16, body: &[u8], openrouter: bool) -> bool {
    let text = String::from_utf8_lossy(body).to_ascii_lowercase();
    match status {
        400 => {
            let names_feature =
                ["response_format", "json_schema", "structured output"].iter().any(|hint| text.contains(hint));
            let unsupported =
                ["not supported", "unsupported", "does not support"].iter().any(|hint| text.contains(hint));
            names_feature && unsupported
        }
        404 => openrouter && text.contains("no endpoints found that can handle the requested parameters"),
        _ => false,
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
    usage: Option<WireUsage>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    /// A string, or (some providers) an array of `{ "type": "text", "text": ... }` parts.
    content: Option<Value>,
    /// Some providers send `null` instead of leaving it out.
    tool_calls: Option<Vec<ToolCall>>,
    refusal: Option<String>,
}

#[derive(Deserialize)]
struct ToolCall {
    function: FunctionCall,
}

#[derive(Deserialize)]
struct FunctionCall {
    /// A JSON string per the spec; some providers send the object itself.
    arguments: Value,
}

#[derive(Deserialize)]
struct WireUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
    /// OpenRouter: what the call cost, in credits (US dollars).
    cost: Option<f64>,
}

/// The answer from a tool call if there is one, else from the message content.
pub(crate) fn parse_answer(body: &[u8]) -> Result<Completion, ProviderError> {
    let response: ChatResponse = serde_json::from_slice(body).map_err(|_| ProviderError::InvalidResponse)?;
    let usage = response.usage.map(|u| Usage {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        cost: u.cost,
    });
    let message = response.choices.into_iter().next().ok_or(ProviderError::InvalidResponse)?.message;
    if message.refusal.is_some_and(|refusal| !refusal.trim().is_empty()) {
        return Err(ProviderError::InvalidResponse);
    }
    let first_call = message.tool_calls.unwrap_or_default().into_iter().next();
    let value = match (first_call, message.content.as_ref().and_then(content_text)) {
        (Some(call), _) => match call.function.arguments {
            Value::String(arguments) => parse_json_text(&arguments),
            arguments @ Value::Object(_) => Some(arguments),
            _ => None,
        },
        (None, Some(content)) => parse_json_text(&content),
        (None, None) => None,
    };
    value.map(|value| Completion { value, usage }).ok_or(ProviderError::InvalidResponse)
}

/// The text of a message's content: a string, or the text parts of an array joined.
fn content_text(content: &Value) -> Option<String> {
    match content {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => {
            let text: String = parts.iter().filter_map(|part| part["text"].as_str()).collect();
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}

/// JSON object from model text that may be wrapped in a code fence or surrounded by prose.
pub(crate) fn parse_json_text(text: &str) -> Option<Value> {
    let text = text.trim();
    let object = |candidate: &str| serde_json::from_str::<Value>(candidate).ok().filter(Value::is_object);
    if let Some(value) = object(text) {
        return Some(value);
    }
    if let Some(fenced) = text.strip_prefix("```") {
        let body = fenced.split_once('\n').map_or("", |(_, rest)| rest);
        if let Some(value) = object(body.trim_end().trim_end_matches("```")) {
            return Some(value);
        }
    }
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (start < end).then(|| object(&text[start..=end])).flatten()
}
