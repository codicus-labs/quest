//! The `/v1/sync` WebSocket. A reader loop handles hello and pushes; a writer task owns the
//! sending half (catch-up, live changes, replies, pings), so neither side's backlog can stop the
//! other from making progress.

mod writer;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use futures_util::StreamExt;
use futures_util::stream::SplitStream;
use quest_domain::{FieldChange, NodeId};
use quest_protocol::{ApiError, BatchId, ClientMessage, ErrorCode, MAX_FRAME_BYTES, ServerMessage, UserId};
use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::Instant;
use uuid::Uuid;

use crate::auth::AppToken;
use crate::policy::{Decoded, check_protocol, decode_client_message, invalid_request, resume, validate_push};
use crate::repo::Repository;
use crate::{AppState, unix_ms};
use writer::Outbound;

/// Replies the reader may queue ahead of the writer before it waits.
const OUTBOX_CAPACITY: usize = 64;

pub async fn upgrade(State(state): State<AppState>, token: AppToken, ws: WebSocketUpgrade) -> Response {
    // Subscribed before the upgrade, so no revocation after the token check is missed.
    let revoked = state.revoked_grants();
    let access = Access { grant: token.grant, expires_at: token.expires_at, revoked };
    let user = token.user;
    ws.max_message_size(MAX_FRAME_BYTES).max_frame_size(MAX_FRAME_BYTES).on_upgrade(move |socket| async move {
        tracing::debug!(user = %user.0, "sync connection opened");
        match run(state, user, access, socket).await {
            Ok(()) => tracing::debug!(user = %user.0, "sync connection closed"),
            Err(error) => tracing::debug!(user = %user.0, %error, "sync connection ended"),
        }
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("websocket: {0}")]
    Socket(#[from] axum::Error),
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    TimedOut(&'static str),
    #[error("closed after error: {0}")]
    Refused(ApiError),
    #[error("writer stopped")]
    WriterGone,
}

/// The access token a connection was opened with: it ends when the token expires or its grant is
/// revoked.
struct Access {
    grant: Uuid,
    /// Unix ms.
    expires_at: i64,
    revoked: broadcast::Receiver<Uuid>,
}

enum Ended {
    Read(Result<(), ConnectionError>),
    AccessEnded(&'static str),
}

async fn run(state: AppState, user: UserId, mut access: Access, socket: WebSocket) -> Result<(), ConnectionError> {
    let (sink, stream) = socket.split();
    let (outbox, commands) = mpsc::channel(OUTBOX_CAPACITY);
    let mut writer = tokio::spawn(writer::run(state.clone(), user, sink, commands));
    let mut shutdown = state.shutdown_signal();
    let repo = state.repo.clone();
    let mut reader = Reader { state, user, stream, outbox };
    let ended = tokio::select! {
        result = reader.serve() => Ended::Read(result),
        _ = wait_for_shutdown(&mut shutdown) => Ended::Read(Ok(())),
        reason = access_ends(&mut access, &repo) => Ended::AccessEnded(reason),
        written = &mut writer => return written.unwrap_or(Err(ConnectionError::WriterGone)),
    };
    let read = match ended {
        Ended::Read(result) => result,
        Ended::AccessEnded(reason) => {
            tracing::debug!(user = %user.0, grant = %access.grant, reason, "closing sync connection");
            Err(reader.refuse(ApiError { code: ErrorCode::Unauthorized, message: reason.into() }).await)
        }
    };
    let drain = reader.state.timeouts.writer_drain;
    // Closing the outbox lets the writer flush queued replies, then close the socket.
    drop(reader);
    match tokio::time::timeout(drain, &mut writer).await {
        Ok(written) => written.unwrap_or(Err(ConnectionError::WriterGone))?,
        Err(_) => writer.abort(),
    }
    read
}

/// Waits until the connection's access token expires or its grant is revoked; returns what the
/// client is told.
async fn access_ends(access: &mut Access, repo: &Repository) -> &'static str {
    const EXPIRED: &str = "The access token expired; refresh it and reconnect";
    const REVOKED: &str = "This sign-in was revoked; sign in again";
    let remaining = u64::try_from(access.expires_at.saturating_sub(unix_ms())).unwrap_or(0);
    let expiry = tokio::time::sleep(Duration::from_millis(remaining));
    tokio::pin!(expiry);
    // A revocation between the token check and the subscription.
    if !repo.grant_active(access.grant).await.unwrap_or(true) {
        return REVOKED;
    }
    loop {
        tokio::select! {
            () = &mut expiry => return EXPIRED,
            revoked = access.revoked.recv() => match revoked {
                Ok(grant) if grant == access.grant => return REVOKED,
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    if !repo.grant_active(access.grant).await.unwrap_or(true) {
                        return REVOKED;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    expiry.as_mut().await;
                    return EXPIRED;
                }
            },
        }
    }
}

async fn wait_for_shutdown(signal: &mut watch::Receiver<bool>) {
    let _ = signal.wait_for(|stopping| *stopping).await;
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
}

/// What the reader received, after decoding.
enum Incoming {
    Decoded(Decoded),
    /// Pings and pongs: only count as activity.
    Activity,
    Closed,
}

struct Reader {
    state: AppState,
    user: UserId,
    stream: SplitStream<WebSocket>,
    outbox: mpsc::Sender<Outbound>,
}

impl Reader {
    async fn serve(&mut self) -> Result<(), ConnectionError> {
        let Some(node) = self.handshake().await? else { return Ok(()) };
        loop {
            let deadline = Instant::now() + self.state.timeouts.idle;
            match self.receive(deadline, "client was silent too long").await? {
                Incoming::Closed => return Ok(()),
                Incoming::Activity => {}
                Incoming::Decoded(Decoded::Message(ClientMessage::Push { batch, changes })) => {
                    self.push(node, batch, changes).await?;
                }
                Incoming::Decoded(Decoded::InvalidPush { batch, error }) => {
                    self.queue(ServerMessage::Rejected { batch, error }).await?;
                }
                Incoming::Decoded(Decoded::Message(ClientMessage::Hello { .. })) => {
                    return Err(self.refuse(invalid_request("Hello was already received on this connection")).await);
                }
                Incoming::Decoded(Decoded::Invalid(error)) => return Err(self.refuse(error).await),
            }
        }
    }

    /// Waits for `Hello`, answers `Welcome` and starts catch-up. Returns the client's node, or
    /// `None` if it went away first.
    async fn handshake(&mut self) -> Result<Option<NodeId>, ConnectionError> {
        let deadline = Instant::now() + self.state.timeouts.hello;
        let (protocol, node, cursor, epoch) = loop {
            match self.receive(deadline, "client did not say hello in time").await? {
                Incoming::Closed => return Ok(None),
                Incoming::Activity => {}
                Incoming::Decoded(Decoded::Message(ClientMessage::Hello { protocol, node, cursor, epoch })) => {
                    break (protocol, node, cursor, epoch);
                }
                Incoming::Decoded(Decoded::Invalid(error)) => return Err(self.refuse(error).await),
                Incoming::Decoded(_) => {
                    return Err(self.refuse(invalid_request("Send hello with your cursor before pushing")).await);
                }
            }
        };
        if let Err(error) = check_protocol(protocol) {
            return Err(self.refuse(error).await);
        }
        let sequence = match self.state.repo.sequence_state(self.user).await {
            Ok(sequence) => sequence,
            Err(error) => return Err(self.fail_internal("loading sync state failed", error).await),
        };
        let resumed = resume(epoch, cursor, &sequence);
        self.queue(ServerMessage::Welcome { epoch: sequence.epoch, cursor_reset: resumed.cursor_reset }).await?;
        self.outbox.send(Outbound::StartSync { from: resumed.from }).await.map_err(|_| ConnectionError::WriterGone)?;
        Ok(Some(node))
    }

    async fn push(&mut self, node: NodeId, batch: BatchId, changes: Vec<FieldChange>) -> Result<(), ConnectionError> {
        if let Err(error) = validate_push(&changes, node, now_ms()) {
            return self.queue(ServerMessage::Rejected { batch, error }).await;
        }
        let channel = self.state.hub.channel(self.user);
        let committed = {
            let _write = channel.write_lock.lock().await;
            let winners = self.state.repo.apply_changes(self.user, &changes).await;
            if let Ok(winners) = &winners
                && !winners.is_empty()
            {
                // No receivers is fine: then nobody is listening live.
                let _ = channel.sender.send(winners.clone().into());
            }
            winners
        };
        match committed {
            Ok(_) => self.queue(ServerMessage::Ack { batch }).await,
            Err(error) => Err(self.fail_internal("applying push failed", error).await),
        }
    }

    async fn receive(&mut self, deadline: Instant, timeout: &'static str) -> Result<Incoming, ConnectionError> {
        let next = tokio::time::timeout_at(deadline, self.stream.next()).await;
        let Some(message) = next.map_err(|_| ConnectionError::TimedOut(timeout))? else { return Ok(Incoming::Closed) };
        Ok(match message? {
            Message::Text(text) => Incoming::Decoded(decode_client_message(text.as_str())),
            Message::Binary(_) => {
                Incoming::Decoded(Decoded::Invalid(invalid_request("Send JSON text frames, not binary")))
            }
            Message::Close(_) => Incoming::Closed,
            Message::Ping(_) | Message::Pong(_) => Incoming::Activity,
        })
    }

    async fn queue(&self, message: ServerMessage) -> Result<(), ConnectionError> {
        self.outbox.send(Outbound::Frame(message)).await.map_err(|_| ConnectionError::WriterGone)
    }

    /// Sends `error` as the connection's last frame.
    async fn refuse(&self, error: ApiError) -> ConnectionError {
        let _ = self.queue(ServerMessage::Error { error: error.clone() }).await;
        ConnectionError::Refused(error)
    }

    async fn fail_internal(&self, context: &str, error: sqlx::Error) -> ConnectionError {
        tracing::error!(user = %self.user.0, %error, "{context}");
        let message = "The server could not process this; reconnect and retry later";
        let _ = self.refuse(ApiError { code: ErrorCode::Internal, message: message.into() }).await;
        ConnectionError::Database(error)
    }
}
