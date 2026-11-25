use chrono::{DateTime, TimeZone, Utc};
use quest_domain::{EntityId, EntryKind, RunningFocus, Snapshot, Task, TimeEntry};

use super::*;

fn id(n: u128) -> EntityId {
    EntityId(uuid::Uuid::from_u128(n))
}

fn at(minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 23, 9, minute, 0).unwrap()
}

fn task(n: u128, done: bool) -> Task {
    Task {
        id: id(n),
        title: format!("T{n}"),
        quest_id: None,
        planned_on: None,
        due_on: None,
        sort_key: format!("{n}"),
        completed_at: done.then(|| at(0)),
        description: String::new(),
        repeat: None,
        day_order: String::new(),
        series: None,
    }
}

fn entry(n: u128, task: u128, minute: u32, running: bool) -> TimeEntry {
    TimeEntry {
        id: id(n),
        kind: EntryKind::Task,
        origin_task_id: None,
        rebooked_from: None,
        task_id: Some(id(task)),
        quest_id: None,
        started_at: at(minute),
        ended_at: (!running).then(|| at(minute + 5)),
    }
}

fn subject(target: &FocusTarget<'_>) -> Option<EntityId> {
    target.task().map(|t| t.id)
}

#[test]
fn the_running_task_wins_over_the_one_picked_last() {
    let running = entry(10, 1, 0, true);
    let snap = Snapshot {
        tasks: vec![task(1, false), task(2, false)],
        time_entries: vec![running.clone()],
        running: vec![RunningFocus { entry: running, workspace: EntityId::DEFAULT_WORKSPACE, task_title: None }],
        ..Snapshot::default()
    };
    let target = target(&snap, Some(id(2)));
    assert!(matches!(target, FocusTarget::Running { .. }));
    assert_eq!(subject(&target), Some(id(1)));
}

#[test]
fn paused_shows_the_picked_open_task_else_the_one_worked_on_last() {
    let snap = Snapshot {
        tasks: vec![task(1, false), task(2, false), task(3, true)],
        time_entries: vec![entry(10, 1, 0, false), entry(11, 2, 10, false)],
        ..Snapshot::default()
    };
    assert!(matches!(target(&snap, Some(id(1))), FocusTarget::Paused { .. }));
    assert_eq!(subject(&target(&snap, Some(id(1)))), Some(id(1)));
    assert_eq!(subject(&target(&snap, None)), Some(id(2)));
    assert_eq!(subject(&target(&snap, Some(id(3)))), Some(id(2)), "a done pick falls back");
    assert_eq!(subject(&target(&snap, Some(id(99)))), Some(id(2)), "an unknown pick falls back");
}

#[test]
fn nothing_without_an_open_task_worked_on() {
    let snap = Snapshot {
        tasks: vec![task(1, true), task(2, false)],
        time_entries: vec![entry(10, 1, 0, false)],
        ..Snapshot::default()
    };
    assert!(matches!(target(&snap, None), FocusTarget::Nothing));
}
