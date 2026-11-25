//! The boundary to AI providers: one structured completion per capture.

use std::fmt::Debug;

use futures_util::future::BoxFuture;
use serde_json::Value;

/// Asks a model for JSON that follows a schema.
///
/// Object-safe (boxed future instead of `async fn`) so the server holds an `Arc<dyn LlmProvider>`
/// chosen at runtime from its configuration, and tests can swap in a fake without making
/// [`crate::AppState`] generic. The boxing costs nothing next to a network round trip.
///
/// `Debug` must not print secrets such as API keys.
pub trait LlmProvider: Debug + Send + Sync {
    fn complete_structured<'a>(
        &'a self,
        request: &'a StructuredRequest,
    ) -> BoxFuture<'a, Result<Completion, ProviderError>>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructuredRequest {
    /// Instructions (system message).
    pub system: String,
    /// The data to work on (user message).
    pub user: String,
    /// Name of the schema (or of the function when the provider falls back to tool calling).
    pub schema_name: String,
    /// JSON schema the answer must follow.
    pub schema: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Completion {
    /// The answer, parsed as JSON but otherwise untrusted.
    pub value: Value,
    pub usage: Option<Usage>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    /// In US dollars, if the provider reports it (OpenRouter does).
    pub cost: Option<f64>,
}

/// Why a completion failed. Carries no provider text: error bodies may echo the user's input.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    #[error("the AI provider cannot be reached")]
    Unreachable,
    #[error("the AI provider did not answer in time")]
    Timeout,
    #[error("the AI provider rate-limited the server")]
    RateLimited,
    #[error("the AI provider answered with something that is not the requested JSON")]
    InvalidResponse,
    #[error("the AI provider rejected the request with HTTP {status}")]
    Rejected { status: u16 },
}
