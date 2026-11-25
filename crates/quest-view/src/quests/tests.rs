use chrono::{DateTime, NaiveDate, Utc};
use quest_domain::{EntityId, EntryKind, Quest, QuestColor, QuestStatus, Snapshot, Task, TimeEntry};

use super::*;

fn id(n: u128) -> EntityId {
    EntityId(uuid::Uuid::from_u128(n))
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, d).unwrap()
}

fn quest(n: u128, color: QuestColor) -> Quest {
    Quest { id: id(n), title: format!("Q{n}"), color, status: QuestStatus::Active, sort_key: format!("{n}") }
}

fn task(n: u128, on: Option<NaiveDate>, done: Option<&str>) -> Task {
    Task {
        id: id(n),
        title: format!("T{n}"),
        quest_id: Some(id(1)),
        planned_on: on,
        due_on: None,
        sort_key: format!("{n}"),
        completed_at: done.map(at),
        description: String::new(),
        repeat: None,
        series: None,
    }
}

#[test]
fn a_new_quest_takes_the_first_free_colour_then_blue() {
    let mut snap =
        Snapshot { quests: vec![quest(1, QuestColor::Red), quest(2, QuestColor::Yellow)], ..Snapshot::default() };
    assert_eq!(new_quest_color(&snap), QuestColor::Orange);
    snap.quests = QuestColor::ALL.iter().enumerate().map(|(i, c)| quest(i as u128, *c)).collect();
    assert_eq!(new_quest_color(&snap), QuestColor::Blue);
}

#[test]
fn quest_time_is_clipped_to_the_window_and_runs_until_now() {
    let entry = |n, start: &str, end: Option<&str>| TimeEntry {
        id: id(n),
        kind: EntryKind::Task,
        origin_task_id: None,
        task_id: None,
        quest_id: Some(id(1)),
        started_at: at(start),
        ended_at: end.map(at),
    };
    let snap = Snapshot {
        time_entries: vec![
            entry(20, "2026-11-22T23:00:00Z", Some("2026-11-23T01:00:00Z")),
            entry(21, "2026-11-24T10:00:00Z", None),
        ],
        ..Snapshot::default()
    };
    let now = at("2026-11-24T10:30:00Z");
    let week = quest_ms(&snap, id(1), at("2026-11-23T00:00:00Z"), at("2026-11-30T00:00:00Z"), now);
    assert_eq!(week, 90 * 60_000);
    assert_eq!(total_ms(&snap, id(1), now), 150 * 60_000);
    assert_eq!(total_ms(&snap, id(2), now), 0);
}

#[test]
fn tasks_group_by_day_then_backlog_then_done_latest_first() {
    let snap = Snapshot {
        tasks: vec![
            task(10, Some(day(25)), None),
            task(11, Some(day(24)), None),
            task(12, None, None),
            task(13, Some(day(20)), Some("2026-11-20T10:00:00Z")),
            task(14, None, Some("2026-11-22T10:00:00Z")),
            task(15, Some(day(30)), None),
        ],
        ..Snapshot::default()
    };
    let groups = task_groups(&snap, id(1), day(24));
    let labels: Vec<&str> = groups.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(labels, ["Today", "Tomorrow", "Mon, Nov 30", "Not planned", "Done"]);
    let done: Vec<EntityId> = groups[4].1.iter().map(|t| t.id).collect();
    assert_eq!(done, [id(14), id(13)]);
    assert_eq!(day_label(day(23), day(24)), "Yesterday");
}

#[test]
fn the_overview_counts_active_and_completed_but_not_archived_quests() {
    let mut done = quest(2, QuestColor::Green);
    done.status = QuestStatus::Completed;
    let mut put_away = quest(3, QuestColor::Blue);
    put_away.status = QuestStatus::Archived;
    let snapshot = Snapshot { quests: vec![quest(1, QuestColor::Red), done, put_away], ..Snapshot::default() };
    let now = at("2026-11-25T10:00:00Z");
    assert_eq!(overview(&snapshot, now), Overview { all_time_ms: 0, in_progress: 1, completed: 1 });
    let ids = |tab| listed(&snapshot, tab).map(|q| q.id).collect::<Vec<_>>();
    assert_eq!((ids(QuestsTab::Active), ids(QuestsTab::Completed)), (vec![id(1)], vec![id(2)]));
    assert_eq!((count_label(0), count_label(7), count_label(12)), ("00".to_owned(), "07".to_owned(), "12".to_owned()));
}
