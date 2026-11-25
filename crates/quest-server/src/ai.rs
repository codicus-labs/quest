//! `POST /v1/ai/capture`: structures free text into a proposal with the configured AI provider.
//!
//! Flow: auth, configured?, opted in (account setting, else `AiDisabled`)?, rate limit per account, body size, request limits
//! ([`quest_capture::check_request`]), provider call with timeout, strict validation of the answer
//! ([`quest_capture::validate`]).
//!
//! Privacy: the text, the prompt and the answer are never logged, and neither are provider error
//! bodies (they may echo the input). Each call logs only user id, duration, token counts and outcome.

mod openai;
mod provider;
mod rate_limit;

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use chrono_tz::Tz;
use futures_util::StreamExt;
use quest_capture::{InvalidOutput, RequestError, SCHEMA_NAME};
use quest_protocol::{CAPTURE_VERSION, CaptureProposal, CaptureRequest, CaptureResponse, ErrorCode, UserId};

pub(crate) use openai::tls_config;
pub use openai::{AiConfigError, OpenAiCompatible, OpenAiSettings, OpenRouterOptions, ReasoningEffort, is_openrouter};
pub use provider::{Completion, LlmProvider, ProviderError, StructuredRequest, Usage};
pub use rate_limit::RateLimits;

use crate::AppState;
use crate::auth::AuthenticatedUser;
use crate::error::HttpFailure;
use rate_limit::RateLimiter;

pub const DEFAULT_AI_TIMEOUT: Duration = Duration::from_secs(45);

/// Largest request body. A full request (4000 characters of text plus the maximal context, all of
/// it multi-byte) stays well below this.
const MAX_BODY_BYTES: usize = 512 * 1024;

/// AI capture settings; `None` in [`crate::Config`] switches the feature off.
#[derive(Clone)]
pub struct AiConfig {
    pub provider: Arc<dyn LlmProvider>,
    /// For the whole provider call, including the tool-calling fallback.
    pub timeout: Duration,
    pub limits: RateLimits,
}

impl AiConfig {
    /// Default timeout and limits.
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        AiConfig { provider, timeout: DEFAULT_AI_TIMEOUT, limits: RateLimits::default() }
    }
}

impl fmt::Debug for AiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The provider's own Debug redacts its secrets.
        f.debug_struct("AiConfig")
            .field("provider", &self.provider)
            .field("timeout", &self.timeout)
            .field("limits", &self.limits)
            .finish()
    }
}

pub(crate) struct AiService {
    provider: Arc<dyn LlmProvider>,
    timeout: Duration,
    limiter: Mutex<RateLimiter>,
}

impl AiService {
    pub fn new(config: AiConfig) -> Self {
        AiService {
            provider: config.provider,
            timeout: config.timeout,
            limiter: Mutex::new(RateLimiter::new(config.limits)),
        }
    }

    fn try_acquire(&self, user: UserId) -> bool {
        self.limiter.lock().unwrap_or_else(PoisonError::into_inner).try_acquire(user, Instant::now())
    }

    async fn capture(&self, user: UserId, body: Body) -> Outcome {
        if !self.try_acquire(user) {
            return Outcome::failed(CaptureFailure::RateLimited);
        }
        let request = match parse_body(body).await {
            Ok(request) => request,
            Err(failure) => return Outcome::failed(failure),
        };
        self.run(&request).await
    }

    /// A capture that does not come as an HTTP body (MCP's `capture_text`): the same rate limit,
    /// request limits, provider call and validation.
    pub(crate) async fn capture_request(&self, user: UserId, request: &CaptureRequest) -> Outcome {
        if !self.try_acquire(user) {
            return Outcome::failed(CaptureFailure::RateLimited);
        }
        self.run(request).await
    }

    /// The protocol's request limits ([`quest_capture::check_request`]) before anything goes to
    /// the provider, then the provider call and the strict validation of its answer.
    async fn run(&self, request: &CaptureRequest) -> Outcome {
        let time_zone = match quest_capture::check_request(request) {
            Ok(time_zone) => time_zone,
            Err(error) => return Outcome::failed(CaptureFailure::Request(error)),
        };
        let completion = match self.complete(request, time_zone).await {
            Ok(completion) => completion,
            Err(failure) => return Outcome::failed(failure),
        };
        let result = quest_capture::validate(&completion.value, request, time_zone)
            .map_err(|_: InvalidOutput| CaptureFailure::Invalid);
        Outcome { result, usage: completion.usage }
    }

    async fn complete(&self, request: &CaptureRequest, time_zone: Tz) -> Result<Completion, CaptureFailure> {
        let structured = structured_request(request, time_zone);
        tokio::time::timeout(self.timeout, self.provider.complete_structured(&structured))
            .await
            .map_err(|_| CaptureFailure::Provider(ProviderError::Timeout))?
            .map_err(CaptureFailure::Provider)
    }
}

/// What the provider gets for a checked capture request: prompt and output schema.
pub fn structured_request(request: &CaptureRequest, time_zone: Tz) -> StructuredRequest {
    let prompt = quest_capture::prompt(request, time_zone);
    StructuredRequest {
        system: prompt.system,
        user: prompt.user,
        schema_name: SCHEMA_NAME.to_owned(),
        schema: quest_capture::output_schema(),
    }
}

async fn parse_body(body: Body) -> Result<CaptureRequest, CaptureFailure> {
    let bytes = read_body(body).await?;
    serde_json::from_slice(&bytes).map_err(|error| CaptureFailure::MalformedBody(error.to_string()))
}

async fn read_body(body: Body) -> Result<Vec<u8>, CaptureFailure> {
    let mut stream = body.into_data_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|_| CaptureFailure::MalformedBody("the request body could not be read".to_owned()))?;
        if bytes.len() + chunk.len() > MAX_BODY_BYTES {
            return Err(CaptureFailure::BodyTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(crate) struct Outcome {
    pub result: Result<CaptureProposal, CaptureFailure>,
    pub usage: Option<Usage>,
}

impl Outcome {
    fn failed(failure: CaptureFailure) -> Self {
        Outcome { result: Err(failure), usage: None }
    }
}

#[derive(Debug)]
pub(crate) enum CaptureFailure {
    RateLimited,
    BodyTooLarge,
    /// The serde message goes back to the client only (it may quote the body).
    MalformedBody(String),
    Request(RequestError),
    Provider(ProviderError),
    /// The reason is not kept: it may quote the model's answer.
    Invalid,
}

impl CaptureFailure {
    /// Stable name for the log line.
    fn log_code(&self) -> &'static str {
        match self {
            CaptureFailure::RateLimited => "rate_limited",
            CaptureFailure::BodyTooLarge => "body_too_large",
            CaptureFailure::MalformedBody(_) => "malformed_body",
            CaptureFailure::Request(RequestError::TextTooLong { .. }) => "text_too_long",
            CaptureFailure::Request(_) => "invalid_request",
            CaptureFailure::Provider(ProviderError::Unreachable) => "provider_unreachable",
            CaptureFailure::Provider(ProviderError::Timeout) => "provider_timeout",
            CaptureFailure::Provider(ProviderError::RateLimited) => "provider_rate_limited",
            CaptureFailure::Provider(ProviderError::InvalidResponse) => "provider_invalid_response",
            CaptureFailure::Provider(ProviderError::Rejected { .. }) => "provider_rejected",
            CaptureFailure::Invalid => "invalid_proposal",
        }
    }

    fn into_http(self) -> HttpFailure {
        let unavailable = |status| {
            HttpFailure::new(
                status,
                ErrorCode::AiUnavailable,
                "The AI provider is not available right now; try again in a few minutes",
            )
        };
        match self {
            CaptureFailure::RateLimited => HttpFailure::new(
                StatusCode::TOO_MANY_REQUESTS,
                ErrorCode::RateLimited,
                "Too many AI requests from this account; try again in a minute",
            ),
            CaptureFailure::BodyTooLarge => HttpFailure::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                ErrorCode::TextTooLong,
                format!("The request is larger than {} KiB; shorten the text", MAX_BODY_BYTES / 1024),
            ),
            CaptureFailure::MalformedBody(message) => {
                HttpFailure::invalid_request(format!("The capture request is malformed: {message}"))
            }
            CaptureFailure::Request(error @ RequestError::TextTooLong { .. }) => {
                HttpFailure::new(StatusCode::PAYLOAD_TOO_LARGE, ErrorCode::TextTooLong, capitalized(&error))
            }
            CaptureFailure::Request(error) => HttpFailure::invalid_request(capitalized(&error)),
            CaptureFailure::Provider(ProviderError::Timeout) => unavailable(StatusCode::GATEWAY_TIMEOUT),
            CaptureFailure::Provider(ProviderError::RateLimited) => unavailable(StatusCode::SERVICE_UNAVAILABLE),
            CaptureFailure::Provider(ProviderError::Unreachable) => unavailable(StatusCode::BAD_GATEWAY),
            CaptureFailure::Provider(ProviderError::Rejected { .. }) => HttpFailure::new(
                StatusCode::BAD_GATEWAY,
                ErrorCode::AiUnavailable,
                "The AI provider refused the request; the server operator can find the reason in its logs",
            ),
            CaptureFailure::Provider(ProviderError::InvalidResponse) | CaptureFailure::Invalid => HttpFailure::new(
                StatusCode::BAD_GATEWAY,
                ErrorCode::AiInvalidResponse,
                "The AI answer could not be used; try again, maybe with other words",
            ),
        }
    }
}

pub(crate) fn capitalized(error: &impl fmt::Display) -> String {
    let message = error.to_string();
    let mut chars = message.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

pub async fn capture(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    body: Body,
) -> Result<Json<CaptureResponse>, HttpFailure> {
    let Some(ai) = state.ai.as_deref() else {
        return Err(HttpFailure::new(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::AiNotConfigured,
            "AI capture is not set up on this server; ask its operator to configure QUEST_AI_*",
        ));
    };
    let enabled = state
        .repo
        .ai_enabled(user)
        .await
        .map_err(|error| HttpFailure::internal("reading the AI setting failed", error))?;
    if !enabled {
        return Err(HttpFailure::new(
            StatusCode::FORBIDDEN,
            ErrorCode::AiDisabled,
            "AI features are off for this account; turn them on in Settings → AI features",
        ));
    }
    let started = Instant::now();
    let outcome = ai.capture(user, body).await;
    log_outcome(user, started.elapsed(), &outcome);
    let proposal = outcome.result.map_err(CaptureFailure::into_http)?;
    Ok(Json(CaptureResponse { version: CAPTURE_VERSION, proposal }))
}

fn log_outcome(user: UserId, elapsed: Duration, outcome: &Outcome) {
    let duration_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    let prompt_tokens = outcome.usage.map(|usage| usage.prompt_tokens);
    let completion_tokens = outcome.usage.map(|usage| usage.completion_tokens);
    let cost_usd = outcome.usage.and_then(|usage| usage.cost);
    let provider_status = match &outcome.result {
        Err(CaptureFailure::Provider(ProviderError::Rejected { status })) => Some(*status),
        _ => None,
    };
    let result = outcome.result.as_ref().map_or_else(CaptureFailure::log_code, |_| "ok");
    tracing::info!(
        user = %user.0,
        duration_ms,
        prompt_tokens,
        completion_tokens,
        cost_usd,
        provider_status,
        result,
        "ai capture"
    );
}

#[cfg(test)]
mod tests;
