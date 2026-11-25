use quest_domain::Snapshot;

use super::*;
use crate::test_support::{at, berlin, entry, id, quest, task, with_running};

/// Monday, 23 Nov 2026, 10:00:30 in Berlin.
fn now() -> Now {
    berlin("2026-11-23T10:00:30+01:00")
}

fn snapshot() -> Snapshot {
    let mut done = task(3, "Done", None, Some("2026-11-23"));
    done.completed_at = Some(at("2026-11-23T08:00:00+01:00"));
    let mut write = task(1, "Write", Some(7), Some("2026-11-23"));
    write.description = "Draft the intro".into();
    Snapshot {
        quests: vec![quest(7, "Work")],
        tasks: vec![write, task(2, "Read", None, Some("2026-11-23")), done, task(4, "Later", None, Some("2026-11-24"))],
        ..Snapshot::default()
    }
}

fn session(view: FocusView) -> FocusSession {
    match view {
        FocusView::Session { session } => *session,
        FocusView::Pick { .. } => panic!("expected a session"),
    }
}

#[test]
fn a_running_timer_shows_its_task_with_all_its_tracked_time() {
    let mut snap = snapshot();
    snap.time_entries = vec![
        entry(20, Some(1), Some(7), "2026-11-23T08:00:00+01:00", Some("2026-11-23T09:00:00+01:00")),
        entry(21, Some(1), Some(7), "2026-11-23T10:00:00+01:00", None),
    ];
    let snap = with_running(snap);
    let shown = session(focus(&snap, Some(id(2)), now()));
    assert_eq!(shown.task_id, id(1).to_string(), "the running task wins over the pick");
    assert_eq!(shown.title, "Write");
    assert_eq!(shown.description, "Draft the intro");
    assert_eq!(shown.quest.map(|q| q.name), Some("Work".into()));
    assert_eq!(shown.header, "FOCUS · WORK");
    assert_eq!(shown.timer, "1:00:30");
    assert!(shown.running);
    assert_eq!(shown.state_label, "RUNNING");
}

#[test]
fn paused_shows_the_pick_without_a_quest() {
    let shown = session(focus(&snapshot(), Some(id(2)), now()));
    assert_eq!(shown.task_id, id(2).to_string());
    assert_eq!(shown.header, "FOCUS");
    assert_eq!(shown.quest, None);
    assert_eq!(shown.timer, "00:00");
    assert!(!shown.running);
    assert_eq!(shown.state_label, "PAUSED");
}

#[test]
fn nothing_to_focus_on_lists_todays_open_tasks() {
    let FocusView::Pick { tasks } = focus(&snapshot(), None, now()) else { panic!("expected the pick list") };
    let titles: Vec<&str> = tasks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["Write", "Read"]);
    assert_eq!(tasks[0].quest.as_ref().map(|q| q.color_hex.as_str()), Some("#5c8fd9"));
}

#[test]
fn the_running_session_counts_the_chronometer_from_all_tracked_time() {
    let mut snap = snapshot();
    assert_eq!(running_session(&snap, now(), None), None);
    snap.time_entries = vec![
        entry(20, Some(1), Some(7), "2026-11-23T08:00:00+01:00", Some("2026-11-23T08:30:00+01:00")),
        entry(21, Some(1), Some(7), "2026-11-23T10:00:00+01:00", None),
    ];
    let snap = with_running(snap);
    let running = running_session(&snap, now(), None).unwrap();
    assert_eq!(running.title, "Write");
    assert_eq!(running.quest_name.as_deref(), Some("Work"));
    assert_eq!(running.quest_color_hex.as_deref(), Some("#5c8fd9"));
    assert_eq!(running.chronometer_base_ms, at("2026-11-23T09:30:00+01:00").timestamp_millis());
}

#[test]
fn a_timer_without_a_task_or_on_a_deleted_one_shows_no_session() {
    let mut snap = snapshot();
    snap.time_entries = vec![entry(20, None, Some(7), "2026-11-23T10:00:00+01:00", None)];
    snap = with_running(snap);
    assert_eq!(running_session(&snap, now(), None), None);
    snap.time_entries = vec![entry(21, Some(99), None, "2026-11-23T10:00:00+01:00", None)];
    snap = with_running(snap);
    assert_eq!(running_session(&snap, now(), None), None);
}

#[test]
fn an_empty_day_offers_an_empty_pick_list() {
    let snap = Snapshot { tasks: Vec::new(), ..snapshot() };
    assert_eq!(focus(&snap, None, now()), FocusView::Pick { tasks: Vec::new() });
}
