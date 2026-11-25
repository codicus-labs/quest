//! Workspaces end to end: the remembered choice per profile, one sync for all workspaces of an
//! account, devices that upgrade offline, and guest data moving into an account.

#[allow(dead_code)] // shared harness; each test file uses part of it
mod common;

use common::{TestServer, TestStore};
use quest_domain::{Command, EntityId, Workspace};
use quest_store::Store;

const DEFAULT: EntityId = EntityId::DEFAULT_WORKSPACE;

fn create_workspace(device: &TestStore, name: &str) -> EntityId {
    let id = EntityId::new();
    device.store().dispatch(Command::CreateWorkspace { id, name: name.into() }).unwrap();
    id
}

/// Titles of the tasks `store` shows in `workspace`; switches there.
fn tasks_in(store: &Store, workspace: EntityId) -> Vec<String> {
    store.switch_workspace(workspace).unwrap();
    store.snapshot().tasks.iter().map(|t| t.title.clone()).collect()
}

fn workspace_names(store: &Store) -> Vec<(EntityId, String)> {
    store.snapshot().live_workspaces().map(|w| (w.id, w.name.clone())).collect()
}

#[test]
fn the_chosen_workspace_and_its_data_come_back_after_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (side, task) = {
        let device = TestStore::open(dir.path());
        let side = create_workspace(&device, "Side");
        device.store().switch_workspace(side).unwrap();
        (side, device.create_task("Side task"))
    };
    let reopened = TestStore::open(dir.path());
    let snapshot = reopened.store().snapshot();
    assert_eq!(snapshot.workspace, side);
    assert_eq!(reopened.task_title(task).as_deref(), Some("Side task"));
    assert!(tasks_in(reopened.store(), DEFAULT).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_devices_upgraded_offline_sync_into_one_default_and_keep_workspaces_apart() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut laptop = TestStore::open(&dir.path().join("laptop"));
    let mut phone = TestStore::open(&dir.path().join("phone"));
    laptop.sign_in(&server, "anna").await.unwrap();
    laptop.wait_live_and_drained();
    phone.sign_in(&server, "anna").await.unwrap();
    phone.wait_live_and_drained();

    server.stop().await;
    laptop.wait_offline();
    phone.wait_offline();
    laptop.store().dispatch(Command::RenameWorkspace { id: DEFAULT, name: "Home".into() }).unwrap();
    let work = create_workspace(&laptop, "Work");
    laptop.store().switch_workspace(work).unwrap();
    laptop.create_task("Work task");
    let hobby = create_workspace(&phone, "Hobby");
    phone.create_task("Default task from the phone");
    phone.store().switch_workspace(hobby).unwrap();
    phone.create_task("Hobby task");

    server.start().await;
    for device in [&laptop, &phone] {
        device.wait_for("both devices' workspaces", |s| s.snapshot().workspaces.len() == 3);
        device.wait_live_and_drained();
    }
    for device in [&laptop, &phone] {
        let store = device.store();
        let expected = [(DEFAULT, "Home".to_owned()), (work, "Work".to_owned()), (hobby, "Hobby".to_owned())];
        let mut names = workspace_names(store);
        names[1..].sort_by_key(|(id, _)| *id);
        let mut expected = expected.to_vec();
        expected[1..].sort_by_key(|(id, _)| *id);
        assert_eq!(names, expected, "exactly one default workspace");
        assert_eq!(tasks_in(store, DEFAULT), ["Default task from the phone"]);
        assert_eq!(tasks_in(store, work), ["Work task"]);
        assert_eq!(tasks_in(store, hobby), ["Hobby task"]);
    }
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guest_workspaces_move_into_the_account_and_the_guest_default_into_the_account_default() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut device = TestStore::open(&dir.path().join("device"));
    device.create_task("Guest default task");
    let side = create_workspace(&device, "Guest side");
    device.store().switch_workspace(side).unwrap();
    device.create_task("Guest side task");

    assert!(device.sign_in(&server, "anna").await.unwrap());
    assert_eq!(device.store().snapshot().workspace, DEFAULT, "each profile remembers its own choice");
    device.import_guest().await.unwrap();
    device.wait_live_and_drained();
    let store = device.store();
    assert_eq!(
        workspace_names(store),
        [(DEFAULT, Workspace::DEFAULT_NAME.to_owned()), (side, "Guest side".to_owned())]
    );
    assert_eq!(tasks_in(store, DEFAULT), ["Guest default task"]);
    assert_eq!(tasks_in(store, side), ["Guest side task"]);
    server.stop().await;
}

#[test]
fn a_command_can_act_on_another_workspace_than_the_shown_one() {
    let dir = tempfile::tempdir().unwrap();
    let device = TestStore::open(dir.path());
    let task = device.create_task("Write");
    let work = create_workspace(&device, "Work");
    let store = device.store();
    store.switch_workspace(work).unwrap();

    let complete = Command::CompleteTask { id: task, at: chrono::Utc::now() };
    assert!(store.dispatch(complete.clone()).is_err(), "Default's task is not in Work");
    store.dispatch_in(DEFAULT, complete).unwrap();
    assert_eq!(store.snapshot().workspace, work, "the shown workspace stays");
    let default = store.snapshot_in(DEFAULT).expect("Default is live");
    assert!(default.task(task).unwrap().is_done());
    assert!(std::sync::Arc::ptr_eq(&default, &store.snapshot_in(DEFAULT).unwrap()), "built once per change");
    assert!(store.snapshot_in(EntityId::new()).is_none());
    let unknown = EntityId::new();
    assert!(store.dispatch_in(unknown, Command::CompleteTask { id: task, at: chrono::Utc::now() }).is_err());
}
