//! Sync behaviour against a scripted fake server, for situations a real server does not
//! produce on demand: silence, rejected batches, epoch changes, unknown frames.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use quest_domain::{EntityId, EntityKind, FieldChange, Hlc, NodeId};
use quest_protocol::{ApiError, ClientMessage, Cursor, Epoch, ErrorCode, ServerMessage};
use serde_json::json;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use std::sync::Arc;

use crate::db::Db;
use crate::layout::AccountKey;
use crate::session::AccountSession;
use crate::vault::{MemoryVault, StoredTokens, TokenVault};
use crate::worker::Timing;
use crate::{Store, StoreError, SyncStatus};

type ServerSocket = WebSocketStream<TcpStream>;

const PATIENT: Timing = Timing {
    ping_every: Duration::from_secs(15),
    silence_limit: Duration::from_secs(10),
    send_timeout: Duration::from_secs(5),
};

/// Accepts WebSocket connections and hands them to the test.
async fn fake_server() -> (SocketAddr, mpsc::UnboundedReceiver<ServerSocket>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            if let Ok(socket) = tokio_tungstenite::accept_async(stream).await {
                let _ = tx.send(socket);
            }
        }
    });
    (addr, rx)
}

/// A signed-in store pointed at `addr`; `seed` prepares the database before opening.
fn open_store(dir: &Path, addr: SocketAddr, timing: Timing, seed: impl FnOnce(&mut Db)) -> (Store, Receiver<()>) {
    let tokens = StoredTokens { access_token: "t".into(), refresh_token: None, access_expires_at_ms: u64::MAX };
    open_store_with(dir, addr, timing, Some(tokens), seed)
}

fn open_store_with(
    dir: &Path,
    addr: SocketAddr,
    timing: Timing,
    tokens: Option<StoredTokens>,
    seed: impl FnOnce(&mut Db),
) -> (Store, Receiver<()>) {
    let mut db = Db::open(dir).unwrap();
    let server_url: url::Url = format!("http://{addr}/").parse().unwrap();
    let key = AccountKey::new(&server_url, "https://id.example", "anna");
    let vault = Arc::new(MemoryVault::default());
    if let Some(tokens) = tokens {
        vault.save(&key, &tokens).unwrap();
    }
    let session = Arc::new(AccountSession::new(server_url, "quest-desktop".into(), key, vault));
    seed(&mut db);
    drop(db);
    let (tx, rx) = channel();
    let tx = std::sync::Mutex::new(tx);
    let on_change = Box::new(move || {
        let _ = tx.lock().unwrap().send(());
    });
    let store = Store::open_with(dir, Some(session), timing, on_change).unwrap();
    (store, rx)
}

fn wait_for(store: &Store, changes: &Receiver<()>, what: &str, condition: impl Fn(&Store) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition(store) {
        assert!(Instant::now() < deadline, "timed out waiting for {what}; status is {:?}", store.status());
        let _ = changes.recv_timeout(Duration::from_millis(50));
    }
}

async fn next_client_message(socket: &mut ServerSocket) -> ClientMessage {
    loop {
        match socket.next().await.expect("client stays connected").expect("valid frame") {
            Message::Text(text) => return serde_json::from_str(&text).expect("client message"),
            _ => continue,
        }
    }
}

async fn send(socket: &mut ServerSocket, message: &ServerMessage) {
    socket.send(Message::text(serde_json::to_string(message).unwrap())).await.unwrap();
}

/// Answers the client's hello and finishes catch-up with nothing to report.
async fn greet(socket: &mut ServerSocket) {
    assert!(matches!(next_client_message(socket).await, ClientMessage::Hello { .. }));
    send(socket, &ServerMessage::Welcome { epoch: Epoch(Uuid::from_u128(1)), cursor_reset: false }).await;
    send(socket, &ServerMessage::Changes { changes: vec![], cursor: Cursor(0), caught_up: true }).await;
}

fn title_change(node: u64) -> FieldChange {
    FieldChange {
        kind: EntityKind::Settings,
        id: EntityId::SETTINGS,
        field: "weekly_target_minutes".into(),
        value: json!(90),
        hlc: Hlc { wall_ms: 1, counter: 0, node: NodeId(node) },
    }
}

fn is_live(pending: usize) -> impl Fn(&Store) -> bool {
    move |store| store.status() == SyncStatus::Live { pending }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_silent_server_is_dropped_and_reconnected() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, mut sockets) = fake_server().await;
    let timing = Timing { ping_every: Duration::from_millis(50), silence_limit: Duration::from_millis(300), ..PATIENT };
    let (store, changes) = open_store(dir.path(), addr, timing, |_| {});
    let mut first = sockets.recv().await.unwrap();
    greet(&mut first).await;
    wait_for(&store, &changes, "live", is_live(0));

    // The server stops reading and writing but keeps the socket open.
    wait_for(&store, &changes, "the silent connection to be dropped", |s| {
        matches!(s.status(), SyncStatus::Offline { .. })
    });
    let reconnect = tokio::time::timeout(Duration::from_secs(5), sockets.recv()).await;
    assert!(reconnect.expect("the store reconnects").is_some());
    drop(first);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rejected_batch_is_set_aside_and_the_next_one_flows() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, mut sockets) = fake_server().await;
    let (store, changes) =
        open_store(dir.path(), addr, PATIENT, |db| db.persist_local(&[title_change(1)], true).unwrap());
    let mut socket = sockets.recv().await.unwrap();
    greet(&mut socket).await;
    let ClientMessage::Push { batch, .. } = next_client_message(&mut socket).await else { panic!("expected a push") };
    let error = ApiError { code: ErrorCode::InvalidRequest, message: "nope".into() };
    send(&mut socket, &ServerMessage::Rejected { batch, error }).await;
    wait_for(&store, &changes, "the batch to be set aside", |s| s.rejected_count() == 1 && is_live(0)(s));

    store.dispatch(quest_domain::Command::SetWeeklyTarget { minutes: 30 }).unwrap();
    let ClientMessage::Push { batch, changes: pushed } = next_client_message(&mut socket).await else {
        panic!("expected the next push")
    };
    assert_eq!(pushed.len(), 1);
    send(&mut socket, &ServerMessage::Ack { batch }).await;
    wait_for(&store, &changes, "the next batch to be acked", is_live(0));
    assert_eq!(store.rejected_count(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_server_epoch_resets_the_cursor_and_requeues_local_state() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, mut sockets) = fake_server().await;
    let old_epoch = Epoch(Uuid::from_u128(1));
    let new_epoch = Epoch(Uuid::from_u128(2));
    let node = std::cell::Cell::new(NodeId(0));
    let (store, changes) = open_store(dir.path(), addr, PATIENT, |db| {
        node.set(db.load().unwrap().node);
        db.apply_remote(&[title_change(node.get().0)], Cursor(5)).unwrap();
        db.welcome(old_epoch, false, node.get()).unwrap();
    });
    let mut socket = sockets.recv().await.unwrap();
    let ClientMessage::Hello { cursor, epoch, .. } = next_client_message(&mut socket).await else { panic!("hello") };
    assert_eq!((cursor, epoch), (Cursor(5), Some(old_epoch)));
    send(&mut socket, &ServerMessage::Welcome { epoch: new_epoch, cursor_reset: true }).await;
    send(&mut socket, &ServerMessage::Changes { changes: vec![], cursor: Cursor(0), caught_up: true }).await;
    let ClientMessage::Push { batch, changes: pushed } = next_client_message(&mut socket).await else { panic!("push") };
    assert_eq!(pushed, [title_change(node.get().0)]);
    send(&mut socket, &ServerMessage::Ack { batch }).await;
    wait_for(&store, &changes, "the re-push to be acked", is_live(0));
    drop(store);
    let db = Db::open(dir.path()).unwrap();
    assert_eq!(db.sync_position().unwrap(), (Cursor(0), Some(new_epoch)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_is_reported_after_catch_up_and_unknown_frames_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, mut sockets) = fake_server().await;
    let (store, changes) = open_store(dir.path(), addr, PATIENT, |_| {});
    let mut socket = sockets.recv().await.unwrap();
    let (finish, finished) = oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        assert!(matches!(next_client_message(&mut socket).await, ClientMessage::Hello { .. }));
        send(&mut socket, &ServerMessage::Welcome { epoch: Epoch(Uuid::from_u128(1)), cursor_reset: false }).await;
        send(
            &mut socket,
            &ServerMessage::Changes { changes: vec![title_change(7)], cursor: Cursor(1), caught_up: false },
        )
        .await;
        socket.send(Message::text("garbage")).await.unwrap();
        socket.send(Message::text(r#"{"type":"from_the_future"}"#)).await.unwrap();
        finished.await.unwrap();
        send(&mut socket, &ServerMessage::Changes { changes: vec![], cursor: Cursor(1), caught_up: true }).await;
        socket
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(store.status(), SyncStatus::Connecting);
    finish.send(()).unwrap();
    wait_for(&store, &changes, "live after catch-up", is_live(0));
    assert_eq!(store.snapshot().settings.weekly_target_minutes, 90);
    let _socket = server.await.unwrap();
    assert!(sockets.try_recv().is_err(), "unknown frames must not cause a reconnect");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn waiting_for_idle_times_out_while_catch_up_is_unfinished() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, mut sockets) = fake_server().await;
    let (store, _changes) = open_store(dir.path(), addr, PATIENT, |_| {});
    let mut socket = sockets.recv().await.unwrap();
    assert!(matches!(next_client_message(&mut socket).await, ClientMessage::Hello { .. }));
    send(&mut socket, &ServerMessage::Welcome { epoch: Epoch(Uuid::from_u128(1)), cursor_reset: false }).await;

    let started = Instant::now();
    let outcome = tokio::task::spawn_blocking(move || store.sync_until_idle(Duration::from_millis(300))).await;
    assert_eq!(outcome.unwrap(), crate::SyncOutcome::TimedOut);
    assert!(started.elapsed() >= Duration::from_millis(300));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn waiting_for_idle_rides_out_a_dropped_connection() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, mut sockets) = fake_server().await;
    let (store, changes) = open_store(dir.path(), addr, PATIENT, |_| {});
    let mut first = sockets.recv().await.unwrap();
    greet(&mut first).await;
    wait_for(&store, &changes, "live", is_live(0));

    drop(first);
    wait_for(&store, &changes, "the dropped connection", |s| matches!(s.status(), SyncStatus::Offline { .. }));
    let server = tokio::spawn(async move {
        let mut second = sockets.recv().await.unwrap();
        greet(&mut second).await;
        second
    });
    let outcome = tokio::task::spawn_blocking(move || store.sync_until_idle(Duration::from_secs(10))).await;
    assert_eq!(outcome.unwrap(), crate::SyncOutcome::Idle);
    let _second = server.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_tokens_the_session_has_expired_and_nothing_connects() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, mut sockets) = fake_server().await;
    let (store, changes) = open_store_with(dir.path(), addr, PATIENT, None, |_| {});
    wait_for(&store, &changes, "the expired session", |s| {
        s.status() == SyncStatus::Failed { error: StoreError::SessionExpired }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(sockets.try_recv().is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn changes_from_an_older_client_land_in_the_default_workspace_not_the_active_one() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, mut sockets) = fake_server().await;
    let (store, changes) = open_store(dir.path(), addr, PATIENT, |_| {});
    let mut socket = sockets.recv().await.unwrap();
    greet(&mut socket).await;
    wait_for(&store, &changes, "live", is_live(0));
    let side = EntityId::new();
    store.dispatch(quest_domain::Command::CreateWorkspace { id: side, name: "Side".into() }).unwrap();
    store.switch_workspace(side).unwrap();

    // A task as a client without workspaces sends it: no workspace field.
    let task = EntityId::new();
    let legacy = FieldChange {
        kind: EntityKind::Task,
        id: task,
        field: "title".into(),
        value: json!("From the old app"),
        hlc: Hlc { wall_ms: 1, counter: 0, node: NodeId(7) },
    };
    send(&mut socket, &ServerMessage::Changes { changes: vec![legacy], cursor: Cursor(1), caught_up: true }).await;
    wait_for(&store, &changes, "the default workspace to hold the task", |s| {
        s.switch_workspace(EntityId::DEFAULT_WORKSPACE).is_ok() && s.snapshot().task(task).is_some()
    });
    store.switch_workspace(side).unwrap();
    assert!(store.snapshot().task(task).is_none());
}
