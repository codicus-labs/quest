//! AI clients connected over MCP (OAuth grants), as the apps' account settings list them.
//!
//! `GET /v1/ai/connections` with the app's `Authorization: Bearer <access token>` returns
//! [`AiConnections`]. `DELETE /v1/ai/connections/{id}` revokes one: its access and refresh tokens
//! stop working at once and the client has to be authorised again. Answers `204 No Content`, or
//! `404` with [`crate::ErrorCode::NotFound`] if the id is unknown or already revoked.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const AI_CONNECTIONS_PATH: &str = "/v1/ai/connections";

/// `DELETE` path of one connection.
pub fn ai_connection_path(id: Uuid) -> String {
    format!("{AI_CONNECTIONS_PATH}/{id}")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiConnections {
    /// Active connections, most recently authorised first.
    pub connections: Vec<AiConnection>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiConnection {
    pub id: Uuid,
    /// The name the client registered with, e.g. "Claude Code"; not verified.
    pub client_name: String,
    /// Host the client was redirected to after consent, e.g. `claude.ai` or `localhost`.
    pub redirect_host: String,
    pub created_at: DateTime<Utc>,
    /// Last time the client used the connection (token refresh or MCP call), if ever.
    pub last_used_at: Option<DateTime<Utc>>,
    /// Granted scope: [`crate::SCOPE_READ`] ("Read only") or [`crate::SCOPE_WRITE`] ("Read and
    /// write").
    pub scope: String,
}
