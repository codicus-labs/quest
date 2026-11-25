use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use quest_domain::EntityId;

use super::*;
use crate::core::ChangeListener;
use crate::test_support::moment;
use crate::types::{AddPosition, HomeQuery, TaskSection};

#[derive(Default)]
struct Counting(AtomicUsize);

impl ChangeListener for Counting {
    fn on_change(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn open() -> (tempfile::TempDir, Arc<QuestCore>, Arc<Counting>) {
    let dir = tempfile::tempdir().unwrap();
    let changes = Arc::new(Counting::default());
    let core =
        QuestCore::open(dir.path().display().to_string(), crate::test_support::tokens(), changes.clone()).unwrap();
    (dir, core, changes)
}

/// Wednesday, 25 Nov 2026, 10:00 in Berlin.
fn now() -> Moment {
    moment("2026-11-25T10:00:00+01:00")
}

fn later(seconds: i64) -> Moment {
    Moment { epoch_ms: now().epoch_ms + seconds * 1000, ..now() }
}

fn add(core: &QuestCore, title: &str) -> String {
    core.add_task(title.into(), TaskSection::Other, AddPosition::Bottom, HomeQuery::default(), now()).unwrap()
}

fn titles(core: &QuestCore) -> Vec<String> {
    core.home(now(), HomeQuery::default()).unwrap().other.into_iter().map(|t| t.title).collect()
}

fn default_id() -> String {
    EntityId::DEFAULT_WORKSPACE.to_string()
}

fn form_error(reason: &str) -> MobileError {
    MobileError::InvalidForm { reason: reason.into() }
}

#[test]
fn creating_shows_the_new_empty_workspace_and_switching_keeps_data_apart() {
    let (_dir, core, changes) = open();
    add(&core, "Default task");
    let start = core.workspaces(now()).unwrap();
    assert_eq!((start.active.name.as_str(), start.live.len()), ("Default", 1));
    assert!(!start.active.can_delete, "the last workspace stays");

    let before = changes.0.load(Ordering::SeqCst);
    let work = core.create_workspace("  Work ".into()).unwrap();
    assert!(changes.0.load(Ordering::SeqCst) > before, "the app re-reads after a switch");
    let data = core.workspaces(now()).unwrap();
    assert_eq!((data.active.id.as_str(), data.active.name.as_str()), (work.as_str(), "Work"));
    assert_eq!(data.active.summary, "Current · Empty");
    assert!(data.live.iter().all(|w| w.can_delete));
    assert!(titles(&core).is_empty(), "a new workspace starts empty");

    add(&core, "Work task");
    core.switch_workspace(default_id()).unwrap();
    assert_eq!(titles(&core), ["Default task"]);
    core.switch_workspace(work.clone()).unwrap();
    assert_eq!(titles(&core), ["Work task"]);
    let default = core.workspaces(now()).unwrap().live.into_iter().find(|w| w.id == default_id()).unwrap();
    assert_eq!(default.summary, "0 quests · 1 task · 0h tracked");
}

#[test]
fn names_are_checked_like_on_the_desktop() {
    let (_dir, core, _) = open();
    assert_eq!(core.create_workspace("  ".into()), Err(form_error("Give the workspace a name.")));
    assert_eq!(core.create_workspace("x".repeat(61)), Err(form_error("Use at most 60 characters.")));
    assert_eq!(core.create_workspace("default".into()), Err(form_error("Another workspace already has this name.")));
    let work = core.create_workspace("Work".into()).unwrap();
    assert_eq!(
        core.rename_workspace(work.clone(), "DEFAULT".into()),
        Err(form_error("Another workspace already has this name."))
    );
    core.rename_workspace(work.clone(), "work".into()).unwrap();
    assert_eq!(core.workspaces(now()).unwrap().active.name, "work", "its own name in other case is fine");
}

#[test]
fn deleting_asks_with_the_content_falls_back_and_restores_unless_the_name_is_taken() {
    let (_dir, core, _) = open();
    assert!(matches!(core.delete_workspace_question(default_id()), Err(MobileError::Rejected { .. })));
    let work = core.create_workspace("Work".into()).unwrap();
    add(&core, "Work task");
    assert_eq!(
        core.delete_workspace_question(work.clone()).unwrap(),
        "Delete “Work”? It holds 0 quests, 1 task, 0h tracked. You can restore it under Deleted."
    );
    core.delete_workspace(work.clone(), now()).unwrap();
    let data = core.workspaces(now()).unwrap();
    assert_eq!(data.active.id, default_id(), "deleting the shown one shows a remaining one");
    assert_eq!(data.deleted.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(), ["Work"]);
    assert!(titles(&core).is_empty());

    let other = core.create_workspace("work".into()).unwrap();
    assert_eq!(
        core.restore_workspace(work.clone()),
        Err(form_error("Rename the other “Work” first. Another workspace already has this name."))
    );
    core.rename_workspace(other, "Side".into()).unwrap();
    core.restore_workspace(work.clone()).unwrap();
    core.switch_workspace(work).unwrap();
    assert_eq!(titles(&core), ["Work task"], "its data is back");
}

#[test]
fn the_default_workspace_can_be_deleted_while_another_remains() {
    let (_dir, core, _) = open();
    let work = core.create_workspace("Work".into()).unwrap();
    core.delete_workspace(default_id(), now()).unwrap();
    let data = core.workspaces(now()).unwrap();
    assert_eq!((data.active.id.as_str(), data.live.len()), (work.as_str(), 1));
    assert!(!data.active.can_delete);
}

#[test]
fn a_switch_drops_the_undo_of_the_old_workspace() {
    let (_dir, core, _) = open();
    let task = add(&core, "Default task");
    core.delete_task(task, now()).unwrap();
    assert!(core.home(now(), HomeQuery::default()).unwrap().toast.is_some());
    core.create_workspace("Work".into()).unwrap();
    assert_eq!(core.home(now(), HomeQuery::default()).unwrap().toast, None);
    core.undo(now()).unwrap();
    assert!(titles(&core).is_empty(), "undo brings nothing from the other workspace");
}

#[test]
fn the_timer_runs_on_in_its_workspace_and_starting_elsewhere_stops_it() {
    let (_dir, core, _) = open();
    let task = add(&core, "Write");
    core.start_timer(task, now()).unwrap();
    let work = core.create_workspace("Work".into()).unwrap();

    let note = core.workspaces(later(90)).unwrap().timer_elsewhere.expect("the timer stays visible");
    assert_eq!(
        (note.workspace_id.as_str(), note.workspace_name.as_str(), note.task_title.as_str(), note.time.as_str()),
        (default_id().as_str(), "Default", "Write", "01:30")
    );
    let session = core.running_session(later(90)).unwrap().expect("the notification stays");
    assert_eq!((session.title.as_str(), session.workspace_name.as_deref()), ("Write", Some("Default")));
    assert_eq!(session.chronometer_base_ms, now().epoch_ms);
    assert!(core.next_wake_ms(later(90)).unwrap() < later(92).epoch_ms, "ticks every second while it runs");

    let here = add(&core, "Work task");
    core.start_timer(here.clone(), later(100)).unwrap();
    assert_eq!(core.workspaces(later(101)).unwrap().timer_elsewhere, None);
    let session = core.running_session(later(101)).unwrap().unwrap();
    assert_eq!((session.task_id, session.workspace_name), (here, None));

    core.switch_workspace(default_id()).unwrap();
    assert_eq!(core.workspaces(later(102)).unwrap().timer_elsewhere.map(|t| t.workspace_id), Some(work));
    core.stop_timer(later(110)).unwrap();
    assert_eq!(core.running_session(later(111)).unwrap(), None, "stopping works from any workspace");
}

#[test]
fn the_notification_resumes_and_completes_a_task_of_another_workspace() {
    let (_dir, core, _) = open();
    let task = add(&core, "Write");
    core.start_timer(task.clone(), now()).unwrap();
    core.create_workspace("Work".into()).unwrap();

    core.stop_timer(later(10)).unwrap();
    assert!(core.is_task_open(task.clone()).unwrap(), "the paused notification stays");
    core.start_timer(task.clone(), later(20)).unwrap();
    let session = core.running_session(later(21)).unwrap().expect("resumed in Default");
    assert_eq!(session.workspace_name.as_deref(), Some("Default"));

    core.complete_task(task.clone(), later(30)).unwrap();
    assert_eq!(core.running_session(later(31)).unwrap(), None, "completing stops the timer");
    assert!(!core.is_task_open(task).unwrap());
    assert_eq!(core.home(later(32), HomeQuery::default()).unwrap().toast, None, "no undo for a task not shown");
}

#[test]
fn unknown_or_deleted_workspaces_are_refused() {
    let (_dir, core, _) = open();
    assert!(matches!(core.switch_workspace("nope".into()), Err(MobileError::InvalidId { .. })));
    assert!(matches!(core.switch_workspace(EntityId::new().to_string()), Err(MobileError::Rejected { .. })));
    let work = core.create_workspace("Work".into()).unwrap();
    core.delete_workspace(work.clone(), now()).unwrap();
    assert!(matches!(core.switch_workspace(work), Err(MobileError::Rejected { .. })));
}

#[test]
fn a_workspace_deleted_elsewhere_falls_back_and_drops_its_undo() {
    let (_dir, core, _) = open();
    let work = core.create_workspace("Work".into()).unwrap();
    let task = add(&core, "Work task");
    core.delete_task(task, now()).unwrap();
    core.workspaces(now()).unwrap();
    // As if another device's delete arrived: straight into the store, past the core's memory.
    let id = crate::ids::parse_id(&work).unwrap();
    core.store()
        .dispatch(quest_domain::Command::DeleteWorkspace {
            id,
            at: crate::test_support::at("2026-11-25T10:00:01+01:00"),
        })
        .unwrap();
    assert_eq!(core.workspaces(later(2)).unwrap().active.id, default_id());
    assert_eq!(core.home(later(2), HomeQuery::default()).unwrap().toast, None);
}
