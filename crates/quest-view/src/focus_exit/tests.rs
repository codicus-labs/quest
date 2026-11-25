use chrono::TimeZone;
use quest_domain::RunningFocus;

use super::*;

fn at(minute: u32, second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 23, 9, minute, second).unwrap()
}

fn running(kind: EntryKind) -> Snapshot {
    let task = EntityId::new();
    let entry = TimeEntry {
        id: EntityId::new(),
        kind,
        task_id: (kind == EntryKind::Task).then_some(task),
        origin_task_id: (kind == EntryKind::Organizing).then_some(task),
        rebooked_from: None,
        quest_id: None,
        started_at: at(0, 0),
        ended_at: None,
    };
    Snapshot {
        time_entries: vec![entry.clone()],
        running: vec![RunningFocus { entry, workspace: EntityId::DEFAULT_WORKSPACE, task_title: None }],
        ..Snapshot::default()
    }
}

#[test]
fn hint_at_two_minutes_keep_until_reopened_and_left_again() {
    let snap = running(EntryKind::Task);
    let mut exit = FocusExit::default();
    assert_eq!(exit.hint(&snap, at(30, 0)), None);
    exit.leave(&snap, at(10, 0));
    assert_eq!(exit.hint(&snap, at(11, 59)), None);
    assert_eq!(exit.hint(&snap, at(12, 0)), Some(at(10, 0)));
    exit.keep(&snap, at(12, 0));
    assert!(exit.book(&snap, EntityId::new(), at(30, 0)).is_none());
    assert_eq!(exit.departure(&snap).unwrap().1, at(10, 0));
    exit.open();
    assert!(exit.departure(&snap).is_none());
    exit.leave(&snap, at(31, 0));
    let command = exit.book(&snap, EntityId::new(), at(33, 0));
    assert!(matches!(command, Some(Command::RebookAsOrganizing { since, expected_task_entry: Some(id), .. })
        if since == at(31, 0) && id == snap.running_entry().unwrap().id));
}

#[test]
fn departure_is_local_to_the_entry_and_never_applies_to_organizing() {
    let mut exit = FocusExit::default();
    exit.leave(&running(EntryKind::Task), at(10, 0));
    assert!(exit.hint(&running(EntryKind::Task), at(12, 0)).is_none());
    let organizing = running(EntryKind::Organizing);
    exit.leave(&organizing, at(10, 0));
    assert!(exit.departure(&organizing).is_none());
    assert!(exit.hint(&organizing, at(12, 0)).is_none());
}

#[test]
fn timeline_marks_only_the_part_after_departure() {
    let snap = running(EntryKind::Task);
    let mut exit = FocusExit::default();
    exit.leave(&snap, at(10, 0));
    let now = at(20, 0);
    let day = crate::tracked::day_timeline_with_focus(&snap, now.date_naive(), &Utc, now, &exit);
    let block = &day.blocks[0];
    assert_eq!((block.start_minute, block.outside_since_minute, block.end_minute), (540, Some(550), 560));
    assert_eq!(day.tracked_ms, 20 * 60_000);
}
