use std::sync::Arc;

use super::*;
use crate::core::ChangeListener;
use crate::test_support::moment;
use crate::types::{AddPosition, HomeQuery, TaskSection};

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

fn later(seconds: i64) -> Moment {
    Moment { epoch_ms: now().epoch_ms + seconds * 1000, ..now() }
}

fn add(core: &QuestCore, title: &str) -> String {
    core.add_task(title.into(), TaskSection::Other, AddPosition::Bottom, HomeQuery::default(), now()).unwrap()
}

fn shown_task(view: FocusView) -> Option<(String, bool)> {
    match view {
        FocusView::Session { session } => Some((session.task_id, session.running)),
        FocusView::Pick { .. } => None,
    }
}

#[test]
fn focus_follows_the_timer_and_keeps_the_task_once_paused() {
    let (_dir, core) = open();
    let id = add(&core, "Write");
    assert_eq!(shown_task(core.focus(now(), None).unwrap()), None);
    core.start_timer(id.clone(), now()).unwrap();
    assert_eq!(shown_task(core.focus(later(5), None).unwrap()), Some((id.clone(), true)));
    assert_eq!(core.running_session(later(5)).unwrap().map(|s| s.chronometer_base_ms), Some(now().epoch_ms));
    core.stop_timer(later(10)).unwrap();
    assert_eq!(core.running_session(later(10)).unwrap(), None);
    assert_eq!(shown_task(core.focus(later(10), None).unwrap()), Some((id, false)), "worked on last");
}

#[test]
fn complete_ends_the_session_offers_undo_and_leaves_a_done_task_alone() {
    let (_dir, core) = open();
    let id = add(&core, "Write");
    core.start_timer(id.clone(), now()).unwrap();
    core.complete_task(id.clone(), later(60)).unwrap();
    assert_eq!(core.running_session(later(61)).unwrap(), None);
    let home = core.home(later(61), HomeQuery::default()).unwrap();
    assert!(home.other[0].done);
    assert_eq!(home.toast.map(|t| t.text), Some("Completed “Write”".into()));
    core.complete_task(id, later(62)).unwrap();
    assert!(core.home(later(63), HomeQuery::default()).unwrap().other[0].done, "not reopened");
    assert_eq!(shown_task(core.focus(later(63), None).unwrap()), None);
}

#[test]
fn an_unknown_focus_task_is_ignored() {
    let (_dir, core) = open();
    assert_eq!(shown_task(core.focus(now(), Some("not-an-id".into())).unwrap()), None);
}

#[test]
fn complete_on_a_paused_task_and_undo_reopens_without_restarting_the_timer() {
    let (_dir, core) = open();
    let id = add(&core, "Write");
    core.start_timer(id.clone(), now()).unwrap();
    core.stop_timer(later(30)).unwrap();
    core.complete_task(id.clone(), later(40)).unwrap();
    assert!(core.home(later(41), HomeQuery::default()).unwrap().other[0].done);
    core.undo(later(42)).unwrap();
    let home = core.home(later(43), HomeQuery::default()).unwrap();
    assert!(!home.other[0].done);
    assert!(!home.other[0].timer_running);
    assert_eq!(core.running_session(later(43)).unwrap(), None);
}

#[test]
fn complete_on_an_unknown_or_deleted_task_does_nothing() {
    let (_dir, core) = open();
    let id = add(&core, "Write");
    core.delete_task(id.clone(), now()).unwrap();
    core.complete_task(id, later(1)).unwrap();
    core.complete_task("not-an-id".into(), later(2)).unwrap();
}
