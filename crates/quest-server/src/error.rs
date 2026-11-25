use std::net::SocketAddr;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use quest_protocol::{ApiError, ErrorCode};

/// Failures while starting the server.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("cannot open database {url:?}: {source}; check QUEST_DATABASE_URL and that the directory is writable")]
    OpenDatabase { url: String, source: sqlx::Error },
    #[error("database migration failed: {0}; the database may belong to a newer server version")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("cannot listen on {addr}: {source}; pick another address with QUEST_BIND")]
    Bind { addr: SocketAddr, source: std::io::Error },
    #[error("sign-in cannot start: {0}")]
    Auth(String),
}

/// An [`ApiError`] together with the HTTP status it is sent with.
#[derive(Debug)]
pub struct HttpFailure {
    status: StatusCode,
    error: ApiError,
}

impl HttpFailure {
    pub fn new(status: StatusCode, code: ErrorCode, message: impl Into<String>) -> Self {
        HttpFailure { status, error: ApiError { code, message: message.into() } }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        HttpFailure::new(StatusCode::BAD_REQUEST, ErrorCode::InvalidRequest, message)
    }

    pub fn internal(context: &str, error: impl std::fmt::Display) -> Self {
        tracing::error!(%error, "{context}");
        HttpFailure::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Internal,
            "Internal server error; try again later",
        )
    }
}

impl IntoResponse for HttpFailure {
    fn into_response(self) -> Response {
        (self.status, Json(self.error)).into_response()
    }
}
