//! The contract between Quest clients and the server.
//!
//! HTTP (JSON):
//! - Sign-in is OAuth 2.1 with OpenID Connect behind it; see the [`auth`] module docs. App routes
//!   take `Authorization: Bearer <access token>` for the app resource; failures return an
//!   [`ApiError`] with a 4xx/5xx status.
//! - `GET /v1/sync` upgrades to a WebSocket. Authenticate with `Authorization: Bearer <token>`.
//!
//! Sync (JSON text frames, [`ClientMessage`] / [`ServerMessage`]):
//! 1. The client sends [`ClientMessage::Hello`] with the [`Cursor`] it has applied up to.
//! 2. The server replies with [`ServerMessage::Changes`] frames holding every stored change after
//!    that cursor; the last catch-up frame has `caught_up: true`. After that it streams new
//!    changes from other devices as they are accepted, also with `caught_up: true`.
//! 3. The client sends local changes in [`ClientMessage::Push`] batches. The server merges them
//!    field by field (last writer wins by HLC), records the winners and answers
//!    [`ServerMessage::Ack`]. Winning changes are broadcast to all of the user's connections,
//!    including the sender's, so its cursor advances past its own writes.
//!
//! The first server frame is always [`ServerMessage::Welcome`]; see its docs for cursor resets.
//! Both sides send WebSocket pings every 15 s and drop a connection that is silent for 45 s.
//!
//! Merging is idempotent and order-independent, so a batch may be retried after a reconnect.
//!
//! AI capture (`POST /v1/ai/capture`): see [`CaptureRequest`].

pub mod auth;
mod capture;
mod connections;

pub use auth::*;
pub use capture::*;
pub use connections::*;
use quest_domain::FieldChange;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use quest_domain::NodeId;

pub const SYNC_PATH: &str = "/v1/sync";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId(pub Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    UnsupportedProtocol,
    /// The access token is missing, unknown, expired or for another resource; refresh it or sign
    /// in again.
    Unauthorized,
    /// The token lacks a scope the request needs (e.g. `quest:write`).
    InsufficientScope,
    Internal,
    /// AI capture is switched off: the server has no AI provider configured.
    AiNotConfigured,
    /// AI features are switched off for this account (opt-in, see [`AccountInfo::ai_enabled`]).
    AiDisabled,
    /// The AI provider could not be reached, timed out or failed; worth a retry later.
    AiUnavailable,
    /// The AI provider answered with something unusable; worth a retry, maybe with other words.
    AiInvalidResponse,
    /// Too many AI requests from this account; retry after a minute.
    RateLimited,
    /// The capture text is longer than [`MAX_CAPTURE_TEXT_CHARS`].
    TextTooLong,
    /// The addressed record (e.g. an AI connection) does not exist.
    NotFound,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}

/// Position in the user's server-side change sequence. `Cursor(0)` means "nothing yet".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Cursor(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BatchId(pub Uuid);

/// Identifies one incarnation of a user's server-side change sequence. If the server's history is
/// reset or restored, the epoch changes and cursors from the old one are meaningless.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Epoch(pub Uuid);

/// Bumped on incompatible protocol changes; the server rejects other versions.
pub const PROTOCOL_VERSION: u32 = 2;

/// Upper bounds both sides enforce. A client splits pushes to stay below them.
pub const MAX_PUSH_CHANGES: usize = 1000;
pub const MAX_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
/// How far in the future a change's HLC wall time may be relative to the receiver's clock.
pub const MAX_CLOCK_DRIFT_MS: u64 = 5 * 60 * 1000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// `epoch` is the epoch the client's `cursor` belongs to (`None` before the first sync).
    Hello {
        protocol: u32,
        node: NodeId,
        cursor: Cursor,
        epoch: Option<Epoch>,
    },
    Push {
        batch: BatchId,
        changes: Vec<FieldChange>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// First reply to `Hello`. With `cursor_reset` the server did not recognise the client's
    /// epoch or cursor: catch-up starts from zero, and the client must re-push its full local
    /// state because the server may have lost changes it had acknowledged.
    Welcome {
        epoch: Epoch,
        cursor_reset: bool,
    },
    /// Changes in server order. `cursor` is the position after the last one.
    Changes {
        changes: Vec<FieldChange>,
        cursor: Cursor,
        caught_up: bool,
    },
    Ack {
        batch: BatchId,
    },
    /// The batch can never be accepted (invalid or too large). The client sets it aside and
    /// continues with the next one; retrying would fail again.
    Rejected {
        batch: BatchId,
        error: ApiError,
    },
    /// Connection-level failure (bad auth, unsupported protocol, internal error). The server
    /// closes the connection afterwards; `Internal` is worth a retry with backoff.
    Error {
        error: ApiError,
    },
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_tagged_json() {
        let hello =
            ClientMessage::Hello { protocol: PROTOCOL_VERSION, node: NodeId(7), cursor: Cursor(42), epoch: None };
        let json = serde_json::to_string(&hello).unwrap();
        assert_eq!(json, r#"{"type":"hello","protocol":2,"node":7,"cursor":42,"epoch":null}"#);
        assert_eq!(serde_json::from_str::<ClientMessage>(&json).unwrap(), hello);
    }
}
