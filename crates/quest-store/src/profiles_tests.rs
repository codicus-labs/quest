//! Profiles without a server: the account stores point at a closed port and stay offline.

use std::path::Path;

use quest_domain::{Command, EntityId, Position};
use quest_protocol::UserId;
use uuid::Uuid;

use super::*;
use crate::SyncStatus;
use crate::layout::ActiveProfile;
use crate::vault::MemoryVault;

/// Nothing listens here, so connecting fails at once.
const CLOSED: &str = "http://127.0.0.1:9";

fn open(dir: &Path, vault: &Arc<MemoryVault>) -> (Profiles, Store) {
    let config = ProfilesConfig { data_dir: dir.to_path_buf(), vault: vault.clone() };
    Profiles::open(config, || {}).unwrap()
}

fn signed_in(subject: &str) -> SignedIn {
    SignedIn {
        server_url: CLOSED.parse().unwrap(),
        client_id: "quest-desktop".into(),
        tokens: StoredTokens {
            access_token: format!("access-{subject}"),
            refresh_token: Some(format!("refresh-{subject}")),
            access_expires_at_ms: u64::MAX,
        },
        account: AccountInfo {
            user_id: UserId(Uuid::now_v7()),
            issuer: "https://id.example".into(),
            subject: subject.into(),
            email: Some(format!("{subject}@example.com")),
            name: None,
            ai_enabled: false,
            ai_available: false,
        },
    }
}

fn key(subject: &str) -> AccountKey {
    AccountKey::new(&CLOSED.parse().unwrap(), "https://id.example", subject)
}

fn create_task(store: &Store, title: &str) -> EntityId {
    let id = EntityId::new();
    store
        .dispatch(Command::CreateTask {
            id,
            title: title.into(),
            quest_id: None,
            planned_on: None,
            due_on: None,
            position: Position::Bottom,
        })
        .unwrap();
    id
}

fn switched(outcome: SignInOutcome) -> (Store, bool) {
    match outcome {
        SignInOutcome::Switched { store, offer_guest_import } => (store, offer_guest_import),
        SignInOutcome::Renewed => panic!("expected a switch"),
    }
}

#[test]
fn a_fresh_device_starts_as_guest_and_keeps_its_data() {
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(MemoryVault::default());
    let (_, guest) = open(dir.path(), &vault);
    assert_eq!(guest.status(), SyncStatus::SignedOut);
    assert_eq!(guest.account(), None);
    let task = create_task(&guest, "Offline first");
    assert_eq!(guest.pending(), 0, "the guest has nothing to sync");
    drop(guest);

    let (_, reopened) = open(dir.path(), &vault);
    assert!(reopened.snapshot().task(task).is_some());
}

#[test]
fn signing_in_switches_to_an_empty_account_store_and_offers_the_guest_data_once() {
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(MemoryVault::default());
    let (profiles, guest) = open(dir.path(), &vault);
    let guest_task = create_task(&guest, "Guest work");

    let (anna, offer) = switched(profiles.sign_in(&guest, signed_in("anna")).unwrap());

    assert!(offer, "first sign-in with guest data");
    assert!(anna.snapshot().task(guest_task).is_none(), "only the account's data is visible");
    let account = anna.account().unwrap();
    assert_eq!((account.server_url.as_str(), account.info.subject.as_str()), ("http://127.0.0.1:9/", "anna"));
    assert_ne!(anna.status(), SyncStatus::SignedOut);
    assert!(vault.load(&key("anna")).unwrap().is_some());
    drop((guest, anna));

    // The account stays active across restarts.
    let (profiles, anna) = open(dir.path(), &vault);
    assert_eq!(anna.account().unwrap().info.subject, "anna");
    assert!(matches!(profiles.layout.read_active().unwrap(), ActiveProfile::Account { .. }));
}

#[test]
fn no_offer_without_guest_data() {
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(MemoryVault::default());
    let (profiles, guest) = open(dir.path(), &vault);
    let (_, offer) = switched(profiles.sign_in(&guest, signed_in("anna")).unwrap());
    assert!(!offer);
}

#[test]
fn signing_in_again_renews_the_active_account_and_another_one_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(MemoryVault::default());
    let (profiles, guest) = open(dir.path(), &vault);
    let (anna, _) = switched(profiles.sign_in(&guest, signed_in("anna")).unwrap());

    let mut again = signed_in("anna");
    again.tokens.access_token = "renewed".into();
    again.account.ai_enabled = true;
    assert!(matches!(profiles.sign_in(&anna, again).unwrap(), SignInOutcome::Renewed));
    assert_eq!(vault.load(&key("anna")).unwrap().unwrap().access_token, "renewed");
    assert!(anna.account().unwrap().info.ai_enabled);

    let refused = profiles.sign_in(&anna, signed_in("ben"));
    assert!(matches!(
        refused,
        Err(StoreError::AnotherAccountActive { ref active, ref signing_in })
            if active == "anna@example.com" && signing_in == "ben@example.com"
    ));
    assert!(vault.load(&key("ben")).unwrap().is_none());
}

#[test]
fn importing_moves_guest_data_into_the_account_with_fresh_timestamps() {
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(MemoryVault::default());
    let (profiles, guest) = open(dir.path(), &vault);
    let task = create_task(&guest, "Guest work");
    assert!(guest.flush(Duration::from_secs(5)));
    let guest_clock = guest.shared.fields().highest_hlc().unwrap();
    let (anna, _) = switched(profiles.sign_in(&guest, signed_in("anna")).unwrap());
    drop(guest);

    let imported = profiles.import_guest(&anna).unwrap();

    assert_eq!(imported, 1);
    assert_eq!(anna.snapshot().task(task).unwrap().title, "Guest work");
    assert!(anna.pending() > 0, "the import waits in the outbox for the server");
    let stamped = anna.shared.fields().highest_hlc().unwrap();
    assert_ne!(stamped.node, guest_clock.node, "stamped by the account store's clock");
    drop(anna);
    let reopened_guest = profiles.open_guest().unwrap();
    assert!(reopened_guest.snapshot().task(task).is_none(), "the guest store is empty afterwards");
}

#[test]
fn signing_out_deletes_the_account_data_and_tokens_and_returns_to_the_guest() {
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(MemoryVault::default());
    let (profiles, guest) = open(dir.path(), &vault);
    let guest_task = create_task(&guest, "Stays a guest task");
    let (anna, _) = switched(profiles.sign_in(&guest, signed_in("anna")).unwrap());
    drop(guest);
    create_task(&anna, "Unsynced");
    assert!(anna.pending() > 0, "the app warns before signing out");

    let guest = profiles.sign_out(&anna).unwrap();

    assert_eq!(anna.status(), SyncStatus::SignedOut);
    assert!(guest.snapshot().task(guest_task).is_some());
    assert_eq!(guest.account(), None);
    assert!(!profiles.layout.account_dir(&key("anna")).exists());
    assert!(vault.load(&key("anna")).unwrap().is_none());
    assert_eq!(profiles.layout.read_active().unwrap(), ActiveProfile::Guest);
    assert!(matches!(profiles.sign_out(&guest), Err(StoreError::NotSignedIn)));

    // Signing in again starts from what the server has, and asks about the guest data again.
    let (anna, offer) = switched(profiles.sign_in(&guest, signed_in("anna")).unwrap());
    assert!(offer);
    assert!(anna.snapshot().tasks.is_empty());
}
