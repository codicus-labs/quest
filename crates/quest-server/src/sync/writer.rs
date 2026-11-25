//! The sending half of a sync connection.

use axum::extract::ws::{Message, WebSocket};
use futures_util::SinkExt;
use futures_util::stream::SplitSink;
use quest_protocol::{ApiError, Cursor, ErrorCode, ServerMessage, UserId};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{Instant, interval_at};

use super::ConnectionError;
use crate::AppState;
use crate::hub::LiveBatch;
use crate::policy::{CATCH_UP_FRAME_CHANGES, frame_len, live_frames};

pub enum Outbound {
    Frame(ServerMessage),
    /// Subscribe to live changes, then catch up from `from`.
    StartSync {
        from: Cursor,
    },
}

struct Writer {
    state: AppState,
    user: UserId,
    sink: SplitSink<WebSocket, Message>,
    live: Option<broadcast::Receiver<LiveBatch>>,
    /// Cursor after the last change sent on this connection.
    sent: Cursor,
}

/// Runs until the reader closes `commands` (then sends a close frame) or sending fails.
pub async fn run(
    state: AppState,
    user: UserId,
    sink: SplitSink<WebSocket, Message>,
    mut commands: mpsc::Receiver<Outbound>,
) -> Result<(), ConnectionError> {
    let ping_every = state.timeouts.ping;
    let mut ping = interval_at(Instant::now() + ping_every, ping_every);
    let mut writer = Writer { state, user, sink, live: None, sent: Cursor(0) };
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                None => {
                    let _ = writer.send_raw(Message::Close(None)).await;
                    return Ok(());
                }
                Some(Outbound::Frame(message)) => writer.send(&message).await?,
                Some(Outbound::StartSync { from }) => {
                    // Subscribe before catching up so nothing committed in between is missed;
                    // `live_frames` drops what catch-up already sent.
                    writer.live = Some(writer.state.hub.subscribe(user));
                    writer.catch_up(from).await?;
                }
            },
            batch = next_live(&mut writer.live) => match batch {
                Ok(batch) => writer.forward(&batch).await?,
                Err(RecvError::Lagged(skipped)) => {
                    tracing::debug!(user = %user.0, skipped, "live receiver lagged; catching up from storage");
                    writer.catch_up(writer.sent).await?;
                }
                Err(RecvError::Closed) => return Ok(()),
            },
            _ = ping.tick() => writer.send_raw(Message::Ping(Default::default())).await?,
        }
    }
}

async fn next_live(live: &mut Option<broadcast::Receiver<LiveBatch>>) -> Result<LiveBatch, RecvError> {
    match live {
        Some(live) => live.recv().await,
        None => std::future::pending().await,
    }
}

impl Writer {
    /// Sends every stored change after `cursor`, in frames bounded by count and size; the last
    /// frame has `caught_up: true`.
    async fn catch_up(&mut self, mut cursor: Cursor) -> Result<(), ConnectionError> {
        loop {
            let page = match self.state.repo.changes_after(self.user, cursor, CATCH_UP_FRAME_CHANGES + 1).await {
                Ok(page) => page,
                Err(error) => return Err(self.fail_internal(error).await),
            };
            let frame = &page[..frame_len(&page, CATCH_UP_FRAME_CHANGES)];
            let caught_up = frame.len() == page.len();
            cursor = frame.last().map_or(cursor, |last| last.seq);
            let changes = frame.iter().map(|sequenced| sequenced.change.clone()).collect();
            self.send(&ServerMessage::Changes { changes, cursor, caught_up }).await?;
            self.sent = self.sent.max(cursor);
            if caught_up {
                return Ok(());
            }
        }
    }

    async fn forward(&mut self, batch: &LiveBatch) -> Result<(), ConnectionError> {
        for frame in live_frames(batch, self.sent) {
            self.send(&frame).await?;
        }
        if let Some(last) = batch.last() {
            self.sent = self.sent.max(last.seq);
        }
        Ok(())
    }

    async fn fail_internal(&mut self, error: sqlx::Error) -> ConnectionError {
        tracing::error!(user = %self.user.0, %error, "loading changes failed");
        let message = "The server could not load your changes; reconnect and retry later";
        let _ = self
            .send(&ServerMessage::Error { error: ApiError { code: ErrorCode::Internal, message: message.into() } })
            .await;
        ConnectionError::Database(error)
    }

    async fn send(&mut self, message: &ServerMessage) -> Result<(), ConnectionError> {
        let json = serde_json::to_string(message).expect("server messages always serialize");
        self.send_raw(Message::Text(json.into())).await
    }

    /// Gives up if the client does not take the frame within the idle timeout.
    async fn send_raw(&mut self, message: Message) -> Result<(), ConnectionError> {
        let sent = tokio::time::timeout(self.state.timeouts.idle, self.sink.send(message)).await;
        sent.map_err(|_| ConnectionError::TimedOut("client stopped reading"))??;
        Ok(())
    }
}
