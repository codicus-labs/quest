use std::sync::Arc;

use super::*;
use crate::core::ChangeListener;
use crate::test_support::moment;
use crate::types::{AddPosition, HomeQuery, TaskSection};
use crate::work_types::SessionKind;

struct Quiet;

impl ChangeListener for Quiet {
    fn on_change(&self) {}
}

fn open() -> (tempfile::TempDir, Arc<QuestCore>) {
    let dir = tempfile::tempdir().unwrap();
    let core =
        QuestCore::open(dir.path().display().to_string(), crate::test_support::tokens(), Arc::new(Quiet)).unwrap();
    (dir, core)
}

/// Wednesday, 25 Nov 2026, 10:00 in Berlin.
fn now() -> Moment {
    moment("2026-11-25T10:00:00+01:00")
}

fn draft(new_title: &str) -> LogWorkDraft {
    LogWorkDraft {
        task_id: None,
        new_title: new_title.into(),
        quest_id: None,
        day: "2026-11-25".into(),
        start_minute: 480,
        end_minute: 540,
    }
}

/// The new task "Draft" logged 08:00–09:00: its id and the entry's.
fn logged(core: &QuestCore) -> (String, String) {
    let entry = core.log_work(draft("Draft"), now()).unwrap();
    let task = core.task_choices("Draft".into()).unwrap().options.remove(0);
    (task.id, entry)
}

#[test]
fn log_work_needs_a_task_and_creates_a_new_one_done() {
    let (_dir, core) = open();
    let error = core.log_work(draft(""), now()).unwrap_err();
    assert_eq!(error, MobileError::InvalidForm { reason: "Choose a task or type a title for a new one.".into() });

    let (task, entry) = logged(&core);
    let detail = core.task_detail(task, now()).unwrap();
    assert!(detail.done, "a new task is created done");
    assert_eq!(
        (detail.logged[0].id.as_str(), detail.logged[0].when.as_str()),
        (entry.as_str(), "Nov 25 · 08:00–09:00")
    );
}

#[test]
fn the_entry_editor_saves_changes_and_deletes_with_undo() {
    let (_dir, core) = open();
    let (task, entry) = logged(&core);
    let mut draft = core.entry_draft(entry.clone(), now()).unwrap();
    draft.start_minute = 450;
    let check = core.check_entry(draft.clone(), now()).unwrap();
    assert_eq!((check.duration.as_str(), check.kind), ("1h 30m", SessionKind::Task));
    core.save_entry(draft, now()).unwrap();
    assert_eq!(core.task_detail(task.clone(), now()).unwrap().logged[0].when, "Nov 25 · 07:30–09:00");

    core.delete_entry(entry.clone(), now()).unwrap();
    assert!(core.task_detail(task.clone(), now()).unwrap().logged.is_empty());
    core.undo(now()).unwrap();
    assert_eq!(core.task_detail(task, now()).unwrap().logged.len(), 1);
    core.delete_entry(entry.clone(), now()).unwrap();
    let gone = core.entry_draft(entry.clone(), now()).unwrap_err();
    assert_eq!(gone, MobileError::UnknownEntry { id: entry });
}

#[test]
fn organizing_runs_without_a_task_and_leads_back_to_its_origin() {
    let (_dir, core) = open();
    let task =
        core.add_task("Write".into(), TaskSection::Other, AddPosition::Bottom, HomeQuery::default(), now()).unwrap();
    core.start_timer(task.clone(), moment("2026-11-25T09:00:00+01:00")).unwrap();
    let organizing = core.start_organizing(Some(task.clone()), now()).unwrap();

    let running = core.running_session(now()).unwrap().unwrap();
    assert_eq!((running.kind, running.task_id, running.title.as_str()), (SessionKind::Organizing, None, "Organizing"));
    assert_eq!((running.origin_task_id, running.origin_title), (Some(task.clone()), Some("Write".into())));
    assert_eq!(running.chronometer_base_ms, now().epoch_ms);
    assert_eq!(
        core.task_detail(task.clone(), now()).unwrap().logged[0].when,
        "Nov 25 · 09:00–10:00",
        "the task stopped"
    );

    let check = core.check_entry(core.entry_draft(organizing.clone(), now()).unwrap(), now()).unwrap();
    assert_eq!((check.kind, check.running, check.task_title), (SessionKind::Organizing, true, None));
    core.stop_timer(moment("2026-11-25T10:10:00+01:00")).unwrap();
    assert_eq!(core.running_session(now()).unwrap(), None);
}

#[test]
fn rebooking_splits_the_running_task_session() {
    let (_dir, core) = open();
    let task =
        core.add_task("Write".into(), TaskSection::Other, AddPosition::Bottom, HomeQuery::default(), now()).unwrap();
    core.start_timer(task.clone(), moment("2026-11-25T09:00:00+01:00")).unwrap();
    let outside = moment("2026-11-25T08:00:00+01:00").epoch_ms;
    assert!(matches!(core.rebook_as_organizing(outside, now()), Err(MobileError::Rejected { .. })));

    let since = moment("2026-11-25T09:40:00+01:00").epoch_ms;
    core.rebook_as_organizing(since, now()).unwrap();
    let running = core.running_session(now()).unwrap().unwrap();
    assert_eq!((running.kind, running.origin_task_id), (SessionKind::Organizing, Some(task.clone())));
    assert_eq!(core.task_detail(task, now()).unwrap().logged[0].when, "Nov 25 · 09:00–09:40");
}

#[test]
fn an_entry_booked_as_organizing_keeps_its_times_and_goes_back_to_its_task() {
    let (_dir, core) = open();
    let (_task, entry) = logged(&core);
    core.book_entry_as_organizing(entry.clone()).unwrap();
    let draft = core.entry_draft(entry.clone(), now()).unwrap();
    let check = core.check_entry(draft.clone(), now()).unwrap();
    assert_eq!((check.kind, draft.task_id, draft.start_minute), (SessionKind::Organizing, None, 480));
    assert!(matches!(core.book_entry_as_organizing("nope".into()), Err(MobileError::InvalidId { .. })));
}
