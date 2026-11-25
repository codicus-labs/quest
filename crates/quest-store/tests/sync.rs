//! End-to-end behaviour of the store: persistence across restarts and sync against a real,
//! in-process server.

#[allow(dead_code)] // shared harness; each test file uses part of it
mod common;

use std::time::Duration;

use common::{TestServer, TestStore};
use quest_domain::Command;
use quest_store::{StoreError, SyncOutcome, SyncStatus};

const PATIENCE: Duration = Duration::from_secs(20);

#[test]
fn offline_commands_survive_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (quest, task) = {
        let a = TestStore::open(dir.path());
        assert_eq!(a.store().status(), SyncStatus::SignedOut);
        let quest = a.create_quest("Ship Quest");
        let task = a.create_task("Write tests");
        a.store().dispatch(Command::RenameTask { id: task, title: "Write more tests".into() }).unwrap();
        (quest, task)
    };
    let reopened = TestStore::open(dir.path());
    let snapshot = reopened.store().snapshot();
    assert_eq!(snapshot.quest(quest).map(|q| q.title.as_str()), Some("Ship Quest"));
    assert_eq!(reopened.task_title(task).as_deref(), Some("Write more tests"));
    assert_eq!(reopened.store().account(), None);
}

#[test]
fn a_flushed_change_survives_a_kill() {
    let dir = tempfile::tempdir().unwrap();
    let a = TestStore::open(dir.path());
    let task = a.create_task("Saved before the kill");
    assert!(a.store().waiter().flush(PATIENCE));
    // A killed process never drops its store, so nothing gets a chance to save on the way out.
    std::mem::forget(a);
    let reopened = TestStore::open(dir.path());
    assert_eq!(reopened.task_title(task).as_deref(), Some("Saved before the kill"));
}

#[test]
fn flushing_after_the_store_is_gone_fails_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let a = TestStore::open(dir.path());
    let waiter = a.store().waiter();
    drop(a);
    let started = std::time::Instant::now();
    assert!(!waiter.flush(PATIENCE));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn flushing_with_nothing_to_save_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let a = TestStore::open(dir.path());
    assert!(a.store().flush(PATIENCE));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_change_on_one_device_arrives_live_on_another() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    let mut b = TestStore::open(&dir.path().join("b"));
    a.sign_in(&server, "anna").await.unwrap();
    a.wait_live_and_drained();
    b.sign_in(&server, "anna").await.unwrap();
    b.wait_live_and_drained();
    let account = b.store().account().unwrap();
    assert_eq!(account.info.email.as_deref(), Some("anna@example.test"));
    assert_eq!(account.server_url, server.url());

    let task = a.create_task("Plan the week");
    b.wait_for("the task from A", |s| s.snapshot().task(task).is_some());
    a.wait_live_and_drained();
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_edits_on_two_devices_converge_to_the_later_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    let mut b = TestStore::open(&dir.path().join("b"));
    a.sign_in(&server, "anna").await.unwrap();
    a.wait_live_and_drained();
    b.sign_in(&server, "anna").await.unwrap();
    let task = a.create_task("Original");
    b.wait_for("the task from A", |s| s.snapshot().task(task).is_some());

    server.stop().await;
    a.wait_offline();
    b.wait_offline();
    a.store().dispatch(Command::RenameTask { id: task, title: "Edited on A".into() }).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    b.store().dispatch(Command::RenameTask { id: task, title: "Edited on B".into() }).unwrap();
    assert!(matches!(a.store().status(), SyncStatus::Offline { pending: 1, .. }));

    server.start().await;
    for store in [&a, &b] {
        store.wait_for("the later edit to win", |s| s.snapshot().task(task).is_some_and(|t| t.title == "Edited on B"));
        store.wait_live_and_drained();
    }
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_store_opened_while_the_server_is_down_drains_its_outbox_once_it_is_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let a_dir = dir.path().join("a");
    let vault = {
        let mut a = TestStore::open(&a_dir);
        a.sign_in(&server, "anna").await.unwrap();
        a.wait_live_and_drained();
        a.vault.clone()
    };
    server.stop().await;

    let a = TestStore::open_with_vault(&a_dir, vault);
    assert_eq!(a.store().account().map(|a| a.info.subject.is_empty()), Some(false), "still signed in");
    a.wait_offline();
    let task = a.create_task("Written offline");
    a.wait_for("the offline change to be pending", |s| matches!(s.status(), SyncStatus::Offline { pending: 5, .. }));

    server.start().await;
    a.wait_live_and_drained();
    let mut b = TestStore::open(&dir.path().join("b"));
    b.sign_in(&server, "anna").await.unwrap();
    b.wait_for("the offline change on another device", |s| s.snapshot().task(task).is_some());
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_oversized_value_is_set_aside_while_everything_else_syncs() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    let mut b = TestStore::open(&dir.path().join("b"));
    a.sign_in(&server, "anna").await.unwrap();
    a.wait_live_and_drained();
    b.sign_in(&server, "anna").await.unwrap();
    let task = a.create_task("Small");
    let huge = "x".repeat(quest_protocol::MAX_VALUE_BYTES + 1);
    a.store().dispatch(Command::RenameTask { id: task, title: huge }).unwrap();
    let next = a.create_task("After the huge one");
    b.wait_for("the later task", |s| s.snapshot().task(next).is_some());
    a.wait_live_and_drained();
    assert_eq!(a.store().rejected_count(), 1);
    assert_eq!(b.task_title(task).as_deref(), Some("Small"));
    server.stop().await;
}

#[test]
fn edits_survive_a_temporarily_locked_database() {
    let dir = tempfile::tempdir().unwrap();
    let a = TestStore::open(dir.path());
    let blocker = rusqlite::Connection::open(dir.path().join("guest").join("quest.sqlite")).unwrap();
    blocker.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let task = a.create_task("Written while locked");
    a.wait_for("a storage error", |s| matches!(s.status(), SyncStatus::Failed { error: StoreError::Storage { .. } }));
    assert!(a.task_title(task).is_some(), "the edit stays visible");

    blocker.execute_batch("COMMIT").unwrap();
    a.wait_for("saving to recover", |s| s.status() == SyncStatus::SignedOut);
    drop(a);
    let reopened = TestStore::open(dir.path());
    assert!(reopened.task_title(task).is_some(), "the edit was saved after the lock went away");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_follows_push_ack_and_catch_up() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    a.sign_in(&server, "anna").await.unwrap();
    a.wait_live_and_drained();
    let b_dir = dir.path().join("b");
    let b_vault = {
        let mut b = TestStore::open(&b_dir);
        b.sign_in(&server, "anna").await.unwrap();
        b.wait_live_and_drained();
        b.vault.clone()
    };

    let task = a.create_task("Pushed and acked");
    assert_eq!(a.store().sync_until_idle(PATIENCE), SyncOutcome::Idle);
    assert_eq!(a.store().status(), SyncStatus::Live { pending: 0 });

    let b = TestStore::open_with_vault(&b_dir, b_vault);
    assert_eq!(b.store().sync_until_idle(PATIENCE), SyncOutcome::Idle);
    assert!(b.store().snapshot().task(task).is_some(), "catch-up is done when idle");
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_a_server_it_reports_offline_and_keeps_the_change() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    a.sign_in(&server, "anna").await.unwrap();
    a.wait_live_and_drained();
    server.stop().await;

    a.create_task("Written offline");
    assert_eq!(a.store().sync_until_idle(PATIENCE), SyncOutcome::Offline);
    assert!(matches!(a.store().status(), SyncStatus::Offline { pending, .. } if pending > 0));
}

#[test]
fn the_guest_is_offline_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let a = TestStore::open(dir.path());
    assert_eq!(a.store().sync_until_idle(PATIENCE), SyncOutcome::Offline);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconnect_now_skips_the_rest_of_the_backoff() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    a.sign_in(&server, "anna").await.unwrap();
    a.wait_live_and_drained();
    server.stop().await;
    a.wait_for(
        "a long backoff",
        |s| matches!(s.status(), SyncStatus::Offline { retry_in, .. } if retry_in > Duration::from_secs(4)),
    );

    server.start().await;
    let started = std::time::Instant::now();
    a.store().reconnect_now();
    a.wait_live_and_drained();
    assert!(started.elapsed() < Duration::from_secs(3), "reconnected at once, not after the backoff");
    server.stop().await;
}
