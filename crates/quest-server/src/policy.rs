//! Pure decisions about requests and frames; no IO, the time is passed in.

use quest_domain::{FieldChange, NodeId};
use quest_protocol::{
    ApiError, BatchId, ClientMessage, Cursor, Epoch, ErrorCode, MAX_CLOCK_DRIFT_MS, MAX_FRAME_BYTES, MAX_PUSH_CHANGES,
    MAX_VALUE_BYTES, PROTOCOL_VERSION, ServerMessage,
};

use crate::repo::{SequenceState, Sequenced};

pub const MAX_FIELD_NAME_LEN: usize = 64;
/// Changes per catch-up frame; frames are also cut by size.
pub const CATCH_UP_FRAME_CHANGES: usize = 500;
/// Room for the frame's envelope (type, cursor, flags) around the changes.
const FRAME_ENVELOPE_BYTES: usize = 1024;

pub fn invalid_request(message: impl Into<String>) -> ApiError {
    ApiError { code: ErrorCode::InvalidRequest, message: message.into() }
}

pub fn check_protocol(protocol: u32) -> Result<(), ApiError> {
    if protocol == PROTOCOL_VERSION {
        return Ok(());
    }
    Err(ApiError {
        code: ErrorCode::UnsupportedProtocol,
        message: format!(
            "Protocol version {protocol} is not supported; this server speaks {PROTOCOL_VERSION}. Update the app."
        ),
    })
}

/// Where catch-up starts for a client resuming at `cursor` of `epoch`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resume {
    pub cursor_reset: bool,
    pub from: Cursor,
}

/// A cursor from another epoch, or beyond anything stored, means the server's history was reset
/// or restored: start over from zero. A first sync (no epoch) is a reset too.
pub fn resume(epoch: Option<Epoch>, cursor: Cursor, server: &SequenceState) -> Resume {
    let cursor_reset = epoch != Some(server.epoch) || cursor > server.last;
    if cursor_reset {
        Resume { cursor_reset: true, from: Cursor(0) }
    } else {
        Resume { cursor_reset: false, from: cursor }
    }
}

/// Checks a push from the client that announced `node`, at server time `now_ms`.
pub fn validate_push(changes: &[FieldChange], node: NodeId, now_ms: u64) -> Result<(), ApiError> {
    if changes.len() > MAX_PUSH_CHANGES {
        return Err(invalid_request(format!(
            "Push has {} changes; send at most {MAX_PUSH_CHANGES} per batch",
            changes.len()
        )));
    }
    changes.iter().try_for_each(|change| validate_change(change, node, now_ms))
}

fn validate_change(change: &FieldChange, node: NodeId, now_ms: u64) -> Result<(), ApiError> {
    let at = || format!("{} {} field {:?}", change.kind.as_str(), change.id, change.field);
    if change.field.is_empty() || change.field.len() > MAX_FIELD_NAME_LEN {
        return Err(invalid_request(format!("Field name of {} must have 1 to {MAX_FIELD_NAME_LEN} bytes", at())));
    }
    let value_bytes = change.value.to_string().len();
    if value_bytes > MAX_VALUE_BYTES {
        return Err(invalid_request(format!(
            "Value of {} has {value_bytes} bytes; at most {MAX_VALUE_BYTES} are allowed",
            at()
        )));
    }
    if change.hlc.node != node {
        return Err(invalid_request(format!(
            "Change to {} is stamped by node {}, but this connection said hello as node {}; \
             each device must use its own node id",
            at(),
            change.hlc.node.0,
            node.0
        )));
    }
    if !change.hlc.is_plausible(now_ms, MAX_CLOCK_DRIFT_MS) {
        return Err(invalid_request(format!(
            "Change to {} is timestamped {} ms in the future; fix the device clock",
            at(),
            change.hlc.wall_ms.saturating_sub(now_ms)
        )));
    }
    Ok(())
}

/// How many of `changes` (at least one, at most `max_count`) fit into one frame of at most
/// `MAX_FRAME_BYTES`. A single change always fits because values and field names are bounded.
pub fn frame_len(changes: &[Sequenced], max_count: usize) -> usize {
    let budget = MAX_FRAME_BYTES - FRAME_ENVELOPE_BYTES;
    let mut used = 0;
    let mut count = 0;
    for sequenced in changes.iter().take(max_count) {
        used += serialized_len(&sequenced.change) + 1;
        if count > 0 && used > budget {
            break;
        }
        count += 1;
    }
    count
}

fn serialized_len(change: &FieldChange) -> usize {
    serde_json::to_string(change).map_or(usize::MAX / 2, |json| json.len())
}

/// Frames to forward for a live batch, without changes the connection already sent (catch-up
/// may have covered them). Empty if nothing is new.
pub fn live_frames(batch: &[Sequenced], sent: Cursor) -> Vec<ServerMessage> {
    let mut unsent = &batch[batch.partition_point(|s| s.seq <= sent)..];
    let mut frames = Vec::new();
    while !unsent.is_empty() {
        let (frame, rest) = unsent.split_at(frame_len(unsent, usize::MAX));
        frames.push(changes_frame(frame, true));
        unsent = rest;
    }
    frames
}

/// A `Changes` frame holding `changes`; the caller ensures `changes` is not empty.
pub fn changes_frame(changes: &[Sequenced], caught_up: bool) -> ServerMessage {
    ServerMessage::Changes {
        changes: changes.iter().map(|s| s.change.clone()).collect(),
        cursor: changes.last().map_or(Cursor(0), |last| last.seq),
        caught_up,
    }
}

#[cfg(test)]
mod tests;

/// A client text frame after decoding.
#[derive(Debug, PartialEq)]
pub enum Decoded {
    Message(ClientMessage),
    /// A push whose batch id is readable but whose content is not (e.g. an unknown entity kind):
    /// rejected permanently, the connection stays usable.
    InvalidPush {
        batch: BatchId,
        error: ApiError,
    },
    /// Not a message this server understands; the connection is closed.
    Invalid(ApiError),
}

pub fn decode_client_message(text: &str) -> Decoded {
    #[derive(serde::Deserialize)]
    struct PushHeader {
        #[serde(rename = "type")]
        kind: String,
        batch: BatchId,
    }
    let error = match serde_json::from_str::<ClientMessage>(text) {
        Ok(message) => return Decoded::Message(message),
        Err(error) => error,
    };
    match serde_json::from_str::<PushHeader>(text) {
        Ok(header) if header.kind == "push" => {
            Decoded::InvalidPush { batch: header.batch, error: invalid_request(format!("Push is malformed: {error}")) }
        }
        _ => Decoded::Invalid(invalid_request(format!("Message is not a valid hello or push: {error}"))),
    }
}
