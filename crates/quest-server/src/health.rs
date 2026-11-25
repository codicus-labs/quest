//! Probes for the orchestrator: `/health/live` while the process serves requests,
//! `/health/ready` while it can also reach its database and is not shutting down.

use axum::extract::State;
use axum::http::StatusCode;

use crate::AppState;

pub const LIVE_PATH: &str = "/health/live";
pub const READY_PATH: &str = "/health/ready";

pub async fn live() -> StatusCode {
    StatusCode::OK
}

pub async fn ready(State(state): State<AppState>) -> StatusCode {
    if state.is_shutting_down() {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    match state.repo.ping().await {
        Ok(()) => StatusCode::OK,
        Err(error) => {
            tracing::warn!(%error, "not ready: the database does not answer");
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}
