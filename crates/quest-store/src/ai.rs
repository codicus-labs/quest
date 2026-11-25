//! AI capture over HTTP with the store's session: the adapter for `POST /v1/ai/capture`. Translates
//! transport and server errors into `CaptureError`s at this boundary.

use std::sync::Arc;
use std::time::Duration;

use quest_protocol::{
    AI_CAPTURE_PATH, ApiError, CAPTURE_VERSION, CaptureProposal, CaptureRequest, CaptureResponse, ErrorCode,
};

use crate::remote::{http_client, invalid_url};
use crate::session::{AccountSession, RequestFailure};
use crate::shared::Shared;

/// Longer than the server's provider timeout, so the server's own answer arrives first.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(75);

/// Why a capture failed. Messages say what to do.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CaptureError {
    #[error("AI capture needs an account. Sign in to your Quest server first.")]
    SignedOut,
    #[error("your sign-in has expired. Sign in again.")]
    SessionExpired,
    #[error("AI features are off for your account. Turn them on in Settings.")]
    Disabled,
    #[error("AI capture is not set up on your Quest server. Ask its admin to configure an AI provider.")]
    NotConfigured,
    #[error("could not reach your Quest server: {message}. Check your connection and try again.")]
    Unreachable { message: String },
    #[error("the AI provider is not available right now. Try again in a moment.")]
    ProviderUnavailable,
    #[error("the AI answer could not be used. Try again, maybe with other words.")]
    InvalidResponse,
    #[error("too many AI requests. Wait a minute and try again.")]
    RateLimited,
    #[error("the text is too long. Shorten it and try again.")]
    TextTooLong,
    #[error("the server rejected the request: {message}")]
    Server { message: String },
}

/// Sends capture requests with the store's current session. Cheap to clone; every call blocks.
#[derive(Clone)]
pub struct AiClient {
    shared: Arc<Shared>,
}

impl AiClient {
    pub(crate) fn new(shared: Arc<Shared>) -> AiClient {
        AiClient { shared }
    }

    /// Asks the server to structure `request.text`. Blocks for up to about a minute: never call it
    /// on the UI thread.
    pub fn capture(&self, request: &CaptureRequest) -> Result<CaptureProposal, CaptureError> {
        let session = self.shared.session().ok_or(CaptureError::SignedOut)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| CaptureError::Unreachable { message: e.to_string() })?;
        runtime.block_on(post(&session, request))
    }
}

async fn post(session: &AccountSession, request: &CaptureRequest) -> Result<CaptureProposal, CaptureError> {
    let server_url = &session.server_url;
    let unreachable = |message: String| CaptureError::Unreachable { message };
    let endpoint = server_url.join(AI_CAPTURE_PATH).map_err(|e| unreachable(invalid_url(server_url, e).to_string()))?;
    let client = http_client(server_url, CAPTURE_TIMEOUT).map_err(|e| unreachable(e.to_string()))?;
    let response = session
        .send_authorized(|token| client.post(endpoint.clone()).bearer_auth(token).json(request).send())
        .await
        .map_err(|failure| match failure {
            RequestFailure::SessionExpired => CaptureError::SessionExpired,
            RequestFailure::Unreachable(message) => CaptureError::Unreachable { message },
            RequestFailure::Local(message) => CaptureError::Server { message },
        })?;
    let status = response.status();
    if status.is_success() {
        let body: CaptureResponse = response.json().await.map_err(|_| CaptureError::InvalidResponse)?;
        if body.version != CAPTURE_VERSION {
            return Err(CaptureError::Server { message: format!("unsupported capture version {}", body.version) });
        }
        return Ok(body.proposal);
    }
    match response.json::<ApiError>().await {
        Ok(error) => Err(api_error(error)),
        Err(_) => Err(CaptureError::Server { message: format!("unexpected HTTP status {status}") }),
    }
}

fn api_error(error: ApiError) -> CaptureError {
    match error.code {
        ErrorCode::Unauthorized => CaptureError::SessionExpired,
        ErrorCode::AiDisabled => CaptureError::Disabled,
        ErrorCode::AiNotConfigured => CaptureError::NotConfigured,
        ErrorCode::AiUnavailable => CaptureError::ProviderUnavailable,
        ErrorCode::AiInvalidResponse => CaptureError::InvalidResponse,
        ErrorCode::RateLimited => CaptureError::RateLimited,
        ErrorCode::TextTooLong => CaptureError::TextTooLong,
        ErrorCode::InvalidRequest
        | ErrorCode::UnsupportedProtocol
        | ErrorCode::InsufficientScope
        | ErrorCode::Internal
        | ErrorCode::NotFound => CaptureError::Server { message: error.message },
    }
}
