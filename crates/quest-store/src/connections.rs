//! AI clients connected over MCP, with the store's session: the adapter for
//! `GET /v1/ai/connections` and `DELETE /v1/ai/connections/{id}`. Translates transport and server
//! errors into `ConnectionsError`s at this boundary.

use std::sync::Arc;
use std::time::Duration;

use quest_protocol::{AI_CONNECTIONS_PATH, AiConnection, AiConnections, ApiError, ErrorCode, ai_connection_path};
use url::Url;
use uuid::Uuid;

use crate::Store;
use crate::remote::{http_client, invalid_url};
use crate::session::{AccountSession, RequestFailure};
use crate::shared::Shared;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Why listing or revoking connections failed. Messages say what to do.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConnectionsError {
    #[error("connected AI clients need an account. Sign in to your Quest server first.")]
    SignedOut,
    #[error("your sign-in has expired. Sign in again.")]
    SessionExpired,
    #[error("could not reach your Quest server: {message}. Check your connection and try again.")]
    Unreachable { message: String },
    #[error("this AI client is no longer connected. Reload the list.")]
    NotFound,
    /// The server refused the request, or it could not be sent (e.g. an invalid server URL).
    #[error("the request failed: {message}")]
    Server { message: String },
}

impl Store {
    /// A handle for the account's connected AI clients with this store's session. Its calls
    /// block: not for the UI thread.
    pub fn connections(&self) -> ConnectionsClient {
        ConnectionsClient { shared: self.shared.clone() }
    }
}

/// Lists and revokes the AI clients connected to the account. Cheap to clone; every call blocks.
#[derive(Clone)]
pub struct ConnectionsClient {
    shared: Arc<Shared>,
}

impl ConnectionsClient {
    /// The active connections, most recently authorised first.
    pub fn list(&self) -> Result<Vec<AiConnection>, ConnectionsError> {
        self.call(|client, session| async move {
            let endpoint = endpoint(&session.server_url, AI_CONNECTIONS_PATH)?;
            let response = session
                .send_authorized(|token| client.get(endpoint.clone()).bearer_auth(token).send())
                .await
                .map_err(request_failure)?;
            if response.status().is_success() {
                let body: AiConnections = response
                    .json()
                    .await
                    .map_err(|e| ConnectionsError::Server { message: format!("unreadable answer: {e}") })?;
                return Ok(body.connections);
            }
            Err(failure(response).await)
        })
    }

    /// Revokes one connection: its tokens stop working at once.
    pub fn revoke(&self, id: Uuid) -> Result<(), ConnectionsError> {
        self.call(|client, session| async move {
            let endpoint = endpoint(&session.server_url, &ai_connection_path(id))?;
            let response = session
                .send_authorized(|token| client.delete(endpoint.clone()).bearer_auth(token).send())
                .await
                .map_err(request_failure)?;
            if response.status().is_success() {
                return Ok(());
            }
            Err(failure(response).await)
        })
    }

    fn call<T, F>(&self, request: impl FnOnce(reqwest::Client, Arc<AccountSession>) -> F) -> Result<T, ConnectionsError>
    where
        F: Future<Output = Result<T, ConnectionsError>>,
    {
        let session = self.shared.session().ok_or(ConnectionsError::SignedOut)?;
        // Not the network's fault: saying "offline" would send the user looking in the wrong place.
        let client = http_client(&session.server_url, REQUEST_TIMEOUT).map_err(local)?;
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(local)?;
        runtime.block_on(request(client, session))
    }
}

fn endpoint(server_url: &Url, path: &str) -> Result<Url, ConnectionsError> {
    server_url.join(path).map_err(|e| local(invalid_url(server_url, e)))
}

fn local(error: impl std::fmt::Display) -> ConnectionsError {
    ConnectionsError::Server { message: error.to_string() }
}

fn request_failure(failure: RequestFailure) -> ConnectionsError {
    match failure {
        RequestFailure::SessionExpired => ConnectionsError::SessionExpired,
        RequestFailure::Unreachable(message) => ConnectionsError::Unreachable { message },
        RequestFailure::Local(message) => ConnectionsError::Server { message },
    }
}

async fn failure(response: reqwest::Response) -> ConnectionsError {
    let status = response.status();
    match response.json::<ApiError>().await {
        Ok(error) => api_error(error),
        Err(_) => ConnectionsError::Server { message: format!("unexpected HTTP status {status}") },
    }
}

fn api_error(error: ApiError) -> ConnectionsError {
    match error.code {
        ErrorCode::Unauthorized => ConnectionsError::SessionExpired,
        ErrorCode::NotFound => ConnectionsError::NotFound,
        ErrorCode::InvalidRequest
        | ErrorCode::UnsupportedProtocol
        | ErrorCode::InsufficientScope
        | ErrorCode::Internal
        | ErrorCode::AiDisabled
        | ErrorCode::AiNotConfigured
        | ErrorCode::AiUnavailable
        | ErrorCode::AiInvalidResponse
        | ErrorCode::RateLimited
        | ErrorCode::TextTooLong => ConnectionsError::Server { message: error.message },
    }
}
