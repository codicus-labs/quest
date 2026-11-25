mod common;

use std::time::Duration;

use common::{NODE, Setup, TestServer, change};
use quest_domain::{EntityId, NodeId};
use quest_protocol::{
    BatchId, ClientMessage, Cursor, DESKTOP_CLIENT_ID, Epoch, ErrorCode, MAX_CLOCK_DRIFT_MS, MAX_FRAME_BYTES,
    MAX_PUSH_CHANGES, MAX_VALUE_BYTES, PROTOCOL_VERSION, ServerMessage,
};
use quest_server::SyncTimeouts;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn pushed_changes_are_caught_up_by_new_and_broadcast_to_open_connections() {
    let server = TestServer::start().await;
    let session = server.sign_in("ada").await;
    let (mut watcher, initial) = server.connect(&session, None).await;
    assert!(initial.changes.is_empty());
    assert_eq!(initial.cursor, Cursor(0));

    let (mut writer, _) = server.connect(&session, None).await;
    let task = EntityId::new();
    let pushed = vec![change(task, "title", json!("Write tests"), 10), change(task, "done", json!(false), 10)];
    writer.push(pushed.clone()).await;

    assert_eq!(watcher.next_changes().await, (pushed.clone(), Cursor(2)));
    assert_eq!(writer.next_changes().await, (pushed.clone(), Cursor(2)), "sender gets its own winners too");

    let (_late, caught_up) = server.connect(&session, None).await;
    assert_eq!((caught_up.changes, caught_up.cursor), (pushed, Cursor(2)));
}

#[tokio::test]
async fn older_hlc_loses_and_is_not_broadcast() {
    let server = TestServer::start().await;
    let session = server.sign_in("ada").await;
    let (mut watcher, _) = server.connect(&session, None).await;
    let (mut writer, _) = server.connect(&session, None).await;
    let task = EntityId::new();
    let newer = change(task, "title", json!("newer"), 20);

    writer.push(vec![newer.clone()]).await;
    assert_eq!(watcher.next_changes().await.0, vec![newer.clone()]);
    writer.next_changes().await;

    writer.push(vec![change(task, "title", json!("older"), 10), newer.clone()]).await;
    watcher.expect_silence().await;
    writer.expect_silence().await;

    let (_, caught_up) = server.connect(&session, None).await;
    assert_eq!((caught_up.changes, caught_up.cursor), (vec![newer], Cursor(1)));
}

#[tokio::test]
async fn equal_hlc_is_resolved_by_value_regardless_of_order() {
    let server = TestServer::start().await;
    let session = server.sign_in("ada").await;
    let (mut client, _) = server.connect(&session, None).await;
    let (first, second) = (EntityId::new(), EntityId::new());

    client.push(vec![change(first, "title", json!("a"), 10)]).await;
    client.push(vec![change(first, "title", json!("b"), 10)]).await;
    client.push(vec![change(second, "title", json!("b"), 10)]).await;
    client.push(vec![change(second, "title", json!("a"), 10)]).await;

    let (_, caught_up) = server.connect(&session, None).await;
    let winners: Vec<_> = caught_up.changes.iter().map(|c| (c.id, c.value.clone())).collect();
    assert_eq!(winners, vec![(first, json!("b")), (second, json!("b"))]);
}

#[tokio::test]
async fn users_only_see_their_own_changes() {
    let server = TestServer::start().await;
    let ada = server.sign_in("ada").await;
    let bob = server.sign_in("bob").await;
    let (mut bob_client, _) = server.connect(&bob, None).await;
    let (mut ada_client, _) = server.connect(&ada, None).await;

    let task = EntityId::new();
    ada_client.push(vec![change(task, "title", json!("Ada's"), 10)]).await;
    bob_client.expect_silence().await;

    bob_client.push(vec![change(task, "title", json!("Bob's"), 5)]).await;
    assert_eq!(bob_client.next_changes().await.1, Cursor(1), "Bob's sequence is independent of Ada's");

    let (_, bob_view) = server.connect(&bob, None).await;
    assert_eq!(bob_view.changes, vec![change(task, "title", json!("Bob's"), 5)]);
}

#[tokio::test]
async fn reconnect_with_cursor_only_gets_newer_changes() {
    let server = TestServer::start().await;
    let session = server.sign_in("ada").await;
    let (mut client, first_sync) = server.connect(&session, None).await;
    let task = EntityId::new();
    client.push(vec![change(task, "title", json!("first"), 10)]).await;
    let (_, cursor) = client.next_changes().await;
    drop(client);

    let (mut other, _) = server.connect(&session, None).await;
    let second = change(task, "done", json!(true), 11);
    other.push(vec![second.clone()]).await;

    let (_, resumed) = server.connect(&session, Some((first_sync.epoch, cursor))).await;
    assert!(!resumed.cursor_reset);
    assert_eq!((resumed.changes, resumed.cursor), (vec![second], Cursor(2)));
}

#[tokio::test]
async fn unknown_epoch_or_cursor_beyond_history_resets_to_full_catch_up() {
    let server = TestServer::start().await;
    let session = server.sign_in("ada").await;
    let (mut client, first) = server.connect(&session, None).await;
    assert!(first.cursor_reset, "a first sync has no epoch to resume");
    let pushed = change(EntityId::new(), "title", json!("kept"), 10);
    client.push(vec![pushed.clone()]).await;

    let (_, resumed) = server.connect(&session, Some((first.epoch, Cursor(1)))).await;
    assert!(!resumed.cursor_reset);
    assert!(resumed.changes.is_empty());

    for from in [(first.epoch, Cursor(99)), (Epoch(Uuid::now_v7()), Cursor(1))] {
        let (_, reset) = server.connect(&session, Some(from)).await;
        assert!(reset.cursor_reset, "{from:?} must reset");
        assert_eq!((reset.epoch, reset.changes, reset.cursor), (first.epoch, vec![pushed.clone()], Cursor(1)));
    }
}

#[tokio::test]
async fn catch_up_is_split_into_frames() {
    let server = TestServer::start().await;
    let session = server.sign_in("ada").await;
    let (mut client, first) = server.connect(&session, None).await;
    let changes: Vec<_> = (0..1000).map(|n| change(EntityId::new(), "title", json!(n), 10)).collect();
    client.push(changes.clone()).await;

    let (mut reader, _) = server.connect(&session, Some((first.epoch, Cursor(1000)))).await;
    reader.expect_silence().await;
    let (_, caught_up) = server.connect(&session, None).await;
    assert_eq!((caught_up.changes, caught_up.cursor), (changes, Cursor(1000)));
}

#[tokio::test]
async fn invalid_batches_are_rejected_and_the_connection_stays_usable() {
    let server = TestServer::start().await;
    let session = server.sign_in("ada").await;
    let (mut client, _) = server.connect(&session, None).await;
    let id = EntityId::new();
    let mut foreign_node = change(id, "title", json!(1), 10);
    foreign_node.hlc.node = NodeId(NODE.0 + 1);
    let invalid_batches = [
        (
            "too many changes",
            (0..=MAX_PUSH_CHANGES as u64).map(|n| change(EntityId::new(), "t", json!(n), 10)).collect(),
        ),
        ("oversized value", vec![change(id, "title", json!("x".repeat(MAX_VALUE_BYTES)), 10)]),
        ("far-future clock", vec![change(id, "title", json!(1), now_ms() + MAX_CLOCK_DRIFT_MS + 60_000)]),
        ("foreign node", vec![foreign_node]),
        ("empty field name", vec![change(id, "", json!(1), 10)]),
    ];
    for (reason, changes) in invalid_batches {
        let reply = client.push_for_reply(changes).await;
        let ServerMessage::Rejected { error, .. } = reply else { panic!("{reason}: expected rejected, got {reply:?}") };
        assert_eq!(error.code, ErrorCode::InvalidRequest, "{reason}");
    }

    let batch = BatchId(Uuid::now_v7());
    let mut unknown_kind = serde_json::to_value(change(id, "title", json!(1), 10)).unwrap();
    unknown_kind["kind"] = json!("spaceship");
    client.send_text(&json!({"type": "push", "batch": batch, "changes": [unknown_kind]}).to_string()).await;
    assert!(matches!(client.reply_to(batch).await, ServerMessage::Rejected { .. }), "unknown kind");

    client.expect_silence().await;
    client.push(vec![change(id, "title", json!("valid"), 10)]).await;
}

#[tokio::test]
async fn protocol_violations_are_answered_with_an_error_and_close() {
    let server = TestServer::start().await;
    let session = server.sign_in("ada").await;

    let mut client = server.connect_raw(Some(&session.token)).await.unwrap();
    client
        .send(&ClientMessage::Hello { protocol: PROTOCOL_VERSION + 1, node: NODE, cursor: Cursor(0), epoch: None })
        .await;
    assert_eq!(client.closing_error().await.code, ErrorCode::UnsupportedProtocol);

    let mut client = server.connect_raw(Some(&session.token)).await.unwrap();
    client.send(&ClientMessage::Push { batch: BatchId(Uuid::nil()), changes: vec![] }).await;
    assert_eq!(client.closing_error().await.code, ErrorCode::InvalidRequest, "push before hello");

    let (mut client, _) = server.connect(&session, None).await;
    client.send(&ClientMessage::Hello { protocol: PROTOCOL_VERSION, node: NODE, cursor: Cursor(0), epoch: None }).await;
    assert_eq!(client.closing_error().await.code, ErrorCode::InvalidRequest, "second hello");

    let (mut client, _) = server.connect(&session, None).await;
    client.send_text("not json").await;
    assert_eq!(client.closing_error().await.code, ErrorCode::InvalidRequest);
}

#[tokio::test]
async fn messages_over_the_frame_limit_close_the_connection() {
    let server = TestServer::start().await;
    let session = server.sign_in("ada").await;
    let (mut client, _) = server.connect(&session, None).await;
    // The server may close while the frame is still being written.
    let sent = client.try_send_text(&"x".repeat(MAX_FRAME_BYTES + 1)).await;
    assert!(sent.is_err() || client.is_closed().await);
}

fn short_timeouts() -> SyncTimeouts {
    SyncTimeouts {
        hello: Duration::from_millis(300),
        idle: Duration::from_millis(600),
        ping: Duration::from_millis(150),
        writer_drain: Duration::from_millis(200),
    }
}

#[tokio::test]
async fn connections_without_hello_are_closed() {
    let server = TestServer::start_with(short_timeouts()).await;
    let session = server.sign_in("ada").await;
    let mut client = server.connect_raw(Some(&session.token)).await.unwrap();
    assert!(client.closes_within(Duration::from_secs(2)).await);
}

#[tokio::test]
async fn responsive_clients_stay_connected_and_silent_ones_are_dropped() {
    let server = TestServer::start_with(short_timeouts()).await;
    let session = server.sign_in("ada").await;
    let (mut client, _) = server.connect(&session, None).await;

    // Reading answers the server's pings, which counts as activity.
    assert!(!client.closes_within(Duration::from_millis(1500)).await, "a client answering pings stays connected");

    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(client.is_closed().await, "a client that answers nothing for the idle timeout is dropped");
}

#[tokio::test]
async fn pushing_while_a_large_catch_up_is_pending_does_not_deadlock() {
    let server = TestServer::start().await;
    let session = server.sign_in("ada").await;
    let big_value = json!("x".repeat(60_000));
    let big_batch =
        |wall_ms| (0..60).map(|_| change(EntityId::new(), "notes", big_value.clone(), wall_ms)).collect::<Vec<_>>();
    let (mut seeder, _) = server.connect(&session, None).await;
    for wall_ms in 1..=4 {
        seeder.push(big_batch(wall_ms)).await;
    }
    drop(seeder);

    let result = tokio::time::timeout(Duration::from_secs(60), async {
        let mut client = server.connect_raw(Some(&session.token)).await.unwrap();
        client
            .send(&ClientMessage::Hello { protocol: PROTOCOL_VERSION, node: NODE, cursor: Cursor(0), epoch: None })
            .await;
        // Push without reading while the server writes ~14 MB of catch-up.
        let batches: Vec<BatchId> = (0..4).map(|_| BatchId(Uuid::now_v7())).collect();
        for (batch, wall_ms) in batches.iter().zip(5..) {
            client.send(&ClientMessage::Push { batch: *batch, changes: big_batch(wall_ms) }).await;
        }
        for batch in batches {
            assert!(matches!(client.reply_to(batch).await, ServerMessage::Ack { .. }));
        }
    })
    .await;
    assert!(result.is_ok(), "client and server deadlocked");
}

#[tokio::test]
async fn shutdown_closes_connections_and_restart_keeps_data() {
    let dir = tempfile::tempdir().unwrap();
    let database_url = common::database_url(&dir);
    let setup = Setup { database_url: Some(database_url.clone()), ..Setup::default() };
    let first = TestServer::start_custom(setup).await;
    let fake = first.fake.clone();
    let session = first.sign_in("ada").await;
    let (mut client, synced) = first.connect(&session, None).await;
    let pushed = change(EntityId::new(), "title", json!("kept"), 10);
    client.push(vec![pushed.clone()]).await;

    let addr = first.server.addr();
    first.server.shutdown().await;
    assert!(client.is_closed().await, "open sync connections are closed on shutdown");

    let setup = Setup { database_url: Some(database_url), bind: Some(addr), fake: Some(fake), ..Setup::default() };
    let second = TestServer::start_custom(setup).await;
    let (_, resumed) = second.connect(&session, Some((synced.epoch, Cursor(0)))).await;
    assert!(!resumed.cursor_reset, "the epoch survives a restart");
    assert_eq!(resumed.changes, vec![pushed]);
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
}

async fn post_form(server: &TestServer, path: &str, form: &[(&str, &str)]) -> reqwest::Response {
    common::oauth::http().post(server.url(path)).form(form).send().await.unwrap()
}

#[tokio::test]
async fn revoking_the_grant_closes_its_open_connections_at_once() {
    let server = TestServer::start().await;
    let session = server.sign_in("ada").await;
    let other = server.sign_in("ada").await;
    let (mut revoked, _) = server.connect(&session, None).await;
    let (mut untouched, _) = server.connect(&other, None).await;

    let form = [("token", session.refresh.as_str()), ("client_id", DESKTOP_CLIENT_ID)];
    assert_eq!(post_form(&server, "/revoke", &form).await.status().as_u16(), 200);
    let error = revoked.closing_error().await;
    assert_eq!(error.code, ErrorCode::Unauthorized);
    assert!(error.message.contains("revoked"), "{}", error.message);
    // Another sign-in (another grant) of the same account stays connected.
    untouched.push(vec![change(EntityId::new(), "title", json!("still here"), 1)]).await;
}

#[tokio::test]
async fn a_reused_refresh_token_closes_the_connections_of_its_grant() {
    let server = TestServer::start().await;
    let session = server.sign_in("ada").await;
    let (mut client, _) = server.connect(&session, None).await;
    let refresh = [
        ("grant_type", "refresh_token"),
        ("refresh_token", session.refresh.as_str()),
        ("client_id", DESKTOP_CLIENT_ID),
    ];
    assert_eq!(post_form(&server, "/token", &refresh).await.status().as_u16(), 200);
    client.expect_silence().await;
    // The old refresh token again: reuse revokes the grant.
    assert_eq!(post_form(&server, "/token", &refresh).await.status().as_u16(), 400);
    assert_eq!(client.closing_error().await.code, ErrorCode::Unauthorized);
}

#[tokio::test]
async fn connections_close_when_their_access_token_expires() {
    let ttl = Duration::from_millis(1500);
    let setup = Setup { auth: Some(Box::new(move |auth| auth.access_token_ttl = ttl)), ..Setup::default() };
    let server = TestServer::start_custom(setup).await;
    let session = server.sign_in("ada").await;
    let opened = std::time::Instant::now();
    let (mut client, _) = server.connect(&session, None).await;
    let error = client.closing_error().await;
    assert_eq!(error.code, ErrorCode::Unauthorized);
    assert!(error.message.contains("expired"), "{}", error.message);
    assert!(opened.elapsed() < ttl + Duration::from_secs(1), "closed by the token's expiry");
}
