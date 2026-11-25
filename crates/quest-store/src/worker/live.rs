//! A live sync connection. Reading and writing are decoupled: a writer task sends queued
//! frames (each with a timeout), so a slow peer never blocks reading. Pings keep the
//! connection observable; silence or a missing ack ends it.

use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use quest_domain::FieldChange;
use quest_protocol::{BatchId, ClientMessage, Cursor, ErrorCode, PROTOCOL_VERSION, ServerMessage};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::{self, Message};
use uuid::Uuid;

use super::{Interrupt, Worker};
use crate::batch::{Limits, plan_batch};
use crate::conn_state::ConnEvent;
use crate::error::StoreError;
use crate::remote::Socket;

/// During catch-up, publish a snapshot at most every this many frames.
const PUBLISH_EVERY_FRAMES: u32 = 20;

type Sink = SplitSink<Socket, Message>;
type Stream = SplitStream<Socket>;

/// The push the server has not answered yet.
struct InFlight {
    batch: BatchId,
    seqs: Vec<i64>,
    sent_at: Instant,
}

struct Connection {
    outgoing: mpsc::UnboundedSender<Message>,
    welcomed: bool,
    caught_up: bool,
    in_flight: Option<InFlight>,
    last_frame: Instant,
    frames_since_publish: u32,
    unpublished: bool,
}

impl Connection {
    fn send(&self, message: &ClientMessage) {
        // Serialising our own message types cannot fail; a closed writer ends the connection
        // through its own branch in `serve`.
        if let Ok(json) = serde_json::to_string(message) {
            let _ = self.outgoing.send(Message::text(json));
        }
    }
}

fn lost() -> Option<Interrupt> {
    Some(Interrupt::Event(ConnEvent::ConnectionLost))
}

/// When the connection counts as dead: no frame within `limit`, or no answer to a push.
pub(super) fn liveness_deadline(last_frame: Instant, push_sent_at: Option<Instant>, limit: Duration) -> Instant {
    let silence = last_frame + limit;
    push_sent_at.map_or(silence, |sent| silence.min(sent + limit))
}

impl Worker {
    /// Says hello, then pushes the outbox one batch at a time and applies incoming changes
    /// until the connection drops or a request interrupts.
    pub(super) async fn live(&mut self) -> Interrupt {
        let Some(socket) = self.socket.take() else {
            return Interrupt::Event(ConnEvent::ConnectionLost);
        };
        let (sink, mut stream) = socket.split();
        let (outgoing, queue) = mpsc::unbounded_channel();
        let mut writer = tokio::spawn(write_frames(sink, queue, self.timing.send_timeout));
        let outcome = self.serve(&mut stream, outgoing, &mut writer).await;
        writer.abort();
        outcome
    }

    async fn serve(
        &mut self,
        stream: &mut Stream,
        outgoing: mpsc::UnboundedSender<Message>,
        writer: &mut JoinHandle<()>,
    ) -> Interrupt {
        let (cursor, epoch) = match self.db.sync_position() {
            Ok(position) => position,
            Err(error) => return self.storage_failed(error),
        };
        let mut conn = Connection {
            outgoing,
            welcomed: false,
            caught_up: false,
            in_flight: None,
            last_frame: Instant::now(),
            frames_since_publish: 0,
            unpublished: false,
        };
        conn.send(&ClientMessage::Hello { protocol: PROTOCOL_VERSION, node: self.node, cursor, epoch });
        let ping_every = self.timing.ping_every;
        let mut ping = tokio::time::interval_at(Instant::now() + ping_every, ping_every);
        loop {
            if conn.welcomed
                && conn.in_flight.is_none()
                && let Err(interrupt) = self.push_next(&mut conn)
            {
                return interrupt;
            }
            let push_sent_at = conn.in_flight.as_ref().map(|pushed| pushed.sent_at);
            let deadline = liveness_deadline(conn.last_frame, push_sent_at, self.timing.silence_limit);
            let outcome = tokio::select! {
                frame = stream.next() => {
                    conn.last_frame = Instant::now();
                    self.on_frame(frame, &mut conn)
                }
                input = self.next_input() => self.handle(input),
                _ = ping.tick() => {
                    let _ = conn.outgoing.send(Message::Ping(Default::default()));
                    None
                }
                () = tokio::time::sleep_until(deadline) => lost(),
                _ = &mut *writer => lost(),
            };
            if let Some(interrupt) = outcome {
                return interrupt;
            }
        }
    }

    /// Sends the next batch that fits the protocol limits; oversized changes are set aside.
    fn push_next(&mut self, conn: &mut Connection) -> Result<(), Interrupt> {
        loop {
            let rows = self.db.outbox_batch(Limits::PROTOCOL.changes).map_err(|e| self.storage_failed(e))?;
            if rows.is_empty() {
                return Ok(());
            }
            let plan = plan_batch(rows, Limits::PROTOCOL);
            if !plan.reject.is_empty() {
                let reason = StoreError::Server { message: "value is larger than the server accepts".into() };
                self.reject(&plan.reject, &reason.to_string())?;
            }
            if plan.push.is_empty() {
                continue;
            }
            let (seqs, changes): (Vec<i64>, Vec<FieldChange>) = plan.push.into_iter().unzip();
            let batch = BatchId(Uuid::now_v7());
            conn.send(&ClientMessage::Push { batch, changes });
            conn.in_flight = Some(InFlight { batch, seqs, sent_at: Instant::now() });
            return Ok(());
        }
    }

    fn reject(&mut self, seqs: &[i64], reason: &str) -> Result<(), Interrupt> {
        self.db.reject_outbox(seqs, reason).map_err(|e| self.storage_failed(e))?;
        self.shared.remove_pending(seqs.len());
        self.shared.add_rejected(seqs.len());
        self.shared.notify();
        Ok(())
    }

    fn on_frame(
        &mut self,
        frame: Option<Result<Message, tungstenite::Error>>,
        conn: &mut Connection,
    ) -> Option<Interrupt> {
        let text = match frame {
            None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return lost(),
            Some(Ok(Message::Text(text))) => text,
            Some(Ok(_)) => return None,
        };
        let message = parse_server_message(&text)?;
        let result = match message {
            ServerMessage::Welcome { epoch, cursor_reset } => self.on_welcome(epoch, cursor_reset, conn),
            ServerMessage::Changes { changes, cursor, caught_up } => self.on_changes(&changes, cursor, caught_up, conn),
            ServerMessage::Ack { batch } => self.on_ack(batch, conn),
            ServerMessage::Rejected { batch, error } => match conn.in_flight.take_if(|pushed| pushed.batch == batch) {
                Some(rejected) => self.reject(&rejected.seqs, &error.to_string()),
                None => Ok(()),
            },
            ServerMessage::Error { error } => Err(Interrupt::Event(match error.code {
                // The token ran out while connected: reconnect with a refreshed one.
                ErrorCode::Unauthorized => {
                    self.token_rejected = true;
                    ConnEvent::ConnectionLost
                }
                ErrorCode::UnsupportedProtocol => ConnEvent::Fatal(StoreError::UnsupportedProtocol),
                _ => ConnEvent::ConnectionLost,
            })),
        };
        result.err()
    }

    fn on_welcome(
        &mut self,
        epoch: quest_protocol::Epoch,
        cursor_reset: bool,
        conn: &mut Connection,
    ) -> Result<(), Interrupt> {
        let queued = self.db.welcome(epoch, cursor_reset, self.node).map_err(|e| self.storage_failed(e))?;
        if queued > 0 {
            self.shared.add_pending(queued);
            self.shared.notify();
        }
        conn.welcomed = true;
        Ok(())
    }

    fn on_changes(
        &mut self,
        changes: &[FieldChange],
        cursor: Cursor,
        caught_up: bool,
        conn: &mut Connection,
    ) -> Result<(), Interrupt> {
        self.db.apply_remote(changes, cursor).map_err(|e| self.storage_failed(e))?;
        self.report_storage(Ok(()));
        conn.unpublished |= self.shared.apply_remote(changes);
        conn.frames_since_publish += 1;
        let first_catch_up = caught_up && !conn.caught_up;
        if conn.unpublished && (caught_up || conn.frames_since_publish >= PUBLISH_EVERY_FRAMES) {
            self.shared.publish_latest();
            self.remember_fallback();
            conn.unpublished = false;
            conn.frames_since_publish = 0;
            if !first_catch_up {
                self.shared.notify();
            }
        }
        if first_catch_up {
            conn.caught_up = true;
            self.shared.set_caught_up();
            self.shared.notify();
        }
        Ok(())
    }

    fn on_ack(&mut self, batch: BatchId, conn: &mut Connection) -> Result<(), Interrupt> {
        // An ack for a batch that was retried after a reconnect needs no action.
        let Some(acked) = conn.in_flight.take_if(|pushed| pushed.batch == batch) else {
            return Ok(());
        };
        self.db.remove_outbox(&acked.seqs).map_err(|e| self.storage_failed(e))?;
        self.shared.remove_pending(acked.seqs.len());
        self.shared.notify();
        Ok(())
    }
}

/// Changes frame read field by field, so one change of an unknown kind (from a newer client)
/// does not hide the rest.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum LenientFrame {
    Changes { changes: Vec<serde_json::Value>, cursor: Cursor, caught_up: bool },
}

/// Parses a server frame. Unknown frames are logged and skipped rather than ending the
/// connection, which would only reconnect into the same frame.
fn parse_server_message(text: &str) -> Option<ServerMessage> {
    if let Ok(message) = serde_json::from_str(text) {
        return Some(message);
    }
    let Ok(LenientFrame::Changes { changes, cursor, caught_up }) = serde_json::from_str(text) else {
        tracing::warn!(frame = %text.chars().take(200).collect::<String>(), "skipping unknown sync frame");
        return None;
    };
    let total = changes.len();
    let changes: Vec<FieldChange> =
        changes.into_iter().filter_map(|change| serde_json::from_value(change).ok()).collect();
    tracing::warn!(skipped = total - changes.len(), "skipping changes this client does not understand");
    Some(ServerMessage::Changes { changes, cursor, caught_up })
}

async fn write_frames(mut sink: Sink, mut queue: mpsc::UnboundedReceiver<Message>, timeout: Duration) {
    while let Some(message) = queue.recv().await {
        match tokio::time::timeout(timeout, sink.send(message)).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) | Err(_) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_ends_the_connection() {
        let start = Instant::now();
        let limit = Duration::from_secs(45);
        assert_eq!(liveness_deadline(start, None, limit), start + limit);
    }

    #[test]
    fn an_unanswered_push_ends_it_even_while_frames_arrive() {
        let pushed = Instant::now();
        let recent_frame = pushed + Duration::from_secs(30);
        let limit = Duration::from_secs(45);
        assert_eq!(liveness_deadline(recent_frame, Some(pushed), limit), pushed + limit);
    }

    #[test]
    fn unknown_frames_are_skipped() {
        assert_eq!(parse_server_message(r#"{"type":"from_the_future"}"#), None);
        assert_eq!(parse_server_message("not json"), None);
    }

    #[test]
    fn changes_of_unknown_kinds_are_dropped_but_the_cursor_advances() {
        let frame = r#"{"type":"changes","cursor":9,"caught_up":true,"changes":[
            {"kind":"habit","id":"00000000-0000-0000-0000-000000000001","field":"x","value":1,"hlc":"0000000000000001-00000000-0000000000000001"},
            {"kind":"task","id":"00000000-0000-0000-0000-000000000002","field":"title","value":"ok","hlc":"0000000000000001-00000000-0000000000000001"}
        ]}"#;
        let Some(ServerMessage::Changes { changes, cursor, caught_up }) = parse_server_message(frame) else {
            panic!("expected changes");
        };
        assert_eq!((changes.len(), cursor, caught_up), (1, Cursor(9), true));
        assert_eq!(changes[0].value, "ok");
    }
}
