//! Accounts and the guest end to end: switching accounts, importing guest data, refresh, expiry,
//! sign-out, and account settings, against a real server with the OIDC fake.

#[allow(dead_code)] // shared harness; each test file uses part of it
mod common;

use common::{TestServer, TestStore};
use quest_protocol::{OAuthErrorBody, TOKEN_PATH};
use quest_store::{AccountKey, StoreError, StoredTokens, SyncStatus, TokenVault};

fn key(a: &TestStore) -> AccountKey {
    let account = a.store().account().expect("signed in");
    AccountKey::new(&account.server_url, &account.info.issuer, &account.info.subject)
}

fn tokens(a: &TestStore) -> StoredTokens {
    a.vault.load(&key(a)).unwrap().expect("tokens")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_account_sees_only_its_own_data() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut device = TestStore::open(&dir.path().join("device"));
    device.sign_in(&server, "anna").await.unwrap();
    device.wait_live_and_drained();
    let annas = device.create_task("Anna's task");
    device.wait_live_and_drained();

    device.sign_out().await.unwrap();
    assert!(device.store().snapshot().tasks.is_empty(), "back to the (empty) guest");
    device.sign_in(&server, "ben").await.unwrap();
    device.wait_live_and_drained();
    assert!(device.store().snapshot().tasks.is_empty(), "Ben does not see Anna's data");
    let bens = device.create_task("Ben's task");
    device.wait_live_and_drained();

    // Signing in as Anna while Ben is active is refused.
    let refused = device.sign_in(&server, "anna").await;
    assert!(matches!(refused, Err(StoreError::AnotherAccountActive { .. })), "{refused:?}");

    device.sign_out().await.unwrap();
    device.sign_in(&server, "anna").await.unwrap();
    device.wait_for("Anna's data from the server", |s| s.snapshot().task(annas).is_some());
    device.wait_live_and_drained();
    assert!(device.store().snapshot().task(bens).is_none());
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guest_data_moves_into_the_account_when_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    let quest = a.create_quest("Guest quest");
    let task = a.create_task("Guest task");

    let offered = a.sign_in(&server, "anna").await.unwrap();
    assert!(offered);
    assert!(a.store().snapshot().task(task).is_none());
    assert_eq!(a.import_guest().await.unwrap(), 2);
    assert_eq!(a.task_title(task).as_deref(), Some("Guest task"));
    a.wait_live_and_drained();

    let mut b = TestStore::open(&dir.path().join("b"));
    assert!(!b.sign_in(&server, "anna").await.unwrap(), "nothing to offer without guest data");
    b.wait_for("the imported data on another device", |s| {
        s.snapshot().task(task).is_some() && s.snapshot().quest(quest).is_some()
    });

    a.sign_out().await.unwrap();
    assert!(a.store().snapshot().tasks.is_empty(), "the guest store was emptied by the import");
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn declined_guest_data_stays_with_the_guest() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    let task = a.create_task("Stays local");
    assert!(a.sign_in(&server, "anna").await.unwrap());
    a.wait_live_and_drained();
    assert!(a.store().snapshot().task(task).is_none());

    a.sign_out().await.unwrap();
    assert_eq!(a.task_title(task).as_deref(), Some("Stays local"));
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_expired_or_rejected_access_token_is_refreshed_with_rotation() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    a.sign_in(&server, "anna").await.unwrap();
    a.wait_live_and_drained();
    let first = tokens(&a);

    // Expired by the clock: refreshed before connecting.
    a.vault.save(&key(&a), &StoredTokens { access_expires_at_ms: 0, ..first.clone() }).unwrap();
    server.stop().await;
    a.wait_offline();
    server.start().await;
    a.store().reconnect_now();
    a.wait_live_and_drained();
    let second = tokens(&a);
    assert_ne!(second.access_token, first.access_token);
    assert_ne!(second.refresh_token, first.refresh_token, "the refresh token rotates");

    // Rejected by the server (401): refreshed and retried once.
    a.vault.save(&key(&a), &StoredTokens { access_token: "not-a-token".into(), ..second.clone() }).unwrap();
    server.stop().await;
    a.wait_offline();
    server.start().await;
    a.store().reconnect_now();
    a.wait_live_and_drained();
    assert_ne!(tokens(&a).access_token, "not-a-token");
    assert_ne!(tokens(&a).refresh_token, second.refresh_token);

    // REST calls refresh too.
    let current = tokens(&a);
    a.vault.save(&key(&a), &StoredTokens { access_token: "stale".into(), ..current }).unwrap();
    let connections = a.store().connections();
    let listed = tokio::task::spawn_blocking(move || connections.list()).await.unwrap();
    assert_eq!(listed, Ok(vec![]));
    assert_ne!(tokens(&a).access_token, "stale");
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_refresh_expires_the_session_and_signing_in_again_resumes() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    a.sign_in(&server, "anna").await.unwrap();
    a.wait_live_and_drained();
    let task = a.create_task("Kept through the expiry");
    a.wait_live_and_drained();

    let broken = StoredTokens {
        access_token: "not-a-token".into(),
        refresh_token: Some("not-a-refresh-token".into()),
        access_expires_at_ms: u64::MAX,
    };
    a.vault.save(&key(&a), &broken).unwrap();
    server.stop().await;
    a.wait_offline();
    server.start().await;
    a.store().reconnect_now();
    a.wait_for("the expired session", |s| s.status() == SyncStatus::Failed { error: StoreError::SessionExpired });
    assert!(a.task_title(task).is_some(), "the data stays");

    assert!(!a.sign_in(&server, "anna").await.unwrap(), "renewed, not switched");
    a.wait_live_and_drained();
    assert!(a.task_title(task).is_some());
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn signing_out_revokes_the_refresh_token_and_deletes_the_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    a.sign_in(&server, "anna").await.unwrap();
    a.wait_live_and_drained();
    let account_key = key(&a);
    let refresh_token = tokens(&a).refresh_token.expect("a refresh token");

    a.sign_out().await.unwrap();

    assert_eq!(a.vault.load(&account_key).unwrap(), None);
    assert_eq!(a.store().status(), SyncStatus::SignedOut);
    let refused = reqwest::Client::new()
        .post(server.url().join(TOKEN_PATH).unwrap())
        .form(&[("grant_type", "refresh_token"), ("refresh_token", &refresh_token), ("client_id", "quest-desktop")])
        .send()
        .await
        .unwrap();
    assert_eq!(refused.json::<OAuthErrorBody>().await.unwrap().error, "invalid_grant");
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_ai_opt_in_is_stored_on_the_server_for_every_device() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    let mut b = TestStore::open(&dir.path().join("b"));
    a.sign_in(&server, "anna").await.unwrap();
    b.sign_in(&server, "anna").await.unwrap();
    assert!(!a.store().account().unwrap().info.ai_enabled, "off by default");

    let info = a.blocking(|_, store| (None, store.set_ai_enabled(true))).await.unwrap();
    assert!(info.ai_enabled);
    assert!(a.store().account().unwrap().info.ai_enabled);

    // Another device learns it when it connects next.
    server.stop().await;
    b.wait_offline();
    server.start().await;
    b.store().reconnect_now();
    b.wait_for("the opt-in on the other device", |s| s.account().is_some_and(|a| a.info.ai_enabled));
    server.stop().await;
}
