use chrono::{DateTime, Utc};
use quest_domain::{EntityId, EntryKind, Quest, QuestColor, QuestStatus, Settings, Snapshot, Task, TimeEntry};

use super::*;
use crate::types::{HomeQuery, Moment};

const BERLIN: &str = "Europe/Berlin";

fn at(rfc3339: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(rfc3339).unwrap().with_timezone(&Utc)
}

/// Wednesday, 25 Nov 2026, 10:00 in Berlin.
fn frozen() -> Now {
    now_at("2026-11-25T10:00:00+01:00")
}

fn now_at(rfc3339: &str) -> Now {
    Now::parse(&Moment { epoch_ms: at(rfc3339).timestamp_millis(), time_zone: BERLIN.into() }).unwrap()
}

fn id(n: u128) -> EntityId {
    EntityId(uuid::Uuid::from_u128(n))
}

fn quest(n: u128, title: &str, color: QuestColor, status: QuestStatus) -> Quest {
    Quest { id: id(n), title: title.into(), color, status, sort_key: format!("{n}") }
}

fn task(n: u128, title: &str, quest: Option<u128>, day: &str, done: bool) -> Task {
    Task {
        id: id(n),
        title: title.into(),
        quest_id: quest.map(id),
        planned_on: Some(day.parse().unwrap()),
        due_on: None,
        sort_key: format!("{n}"),
        completed_at: done.then(|| at("2026-11-25T09:00:00+01:00")),
        description: String::new(),
        repeat: None,
        series: None,
    }
}

fn entry(n: u128, quest: Option<u128>, from: &str, to: Option<&str>) -> TimeEntry {
    TimeEntry {
        id: id(n),
        kind: EntryKind::Task,
        task_id: None,
        quest_id: quest.map(id),
        origin_task_id: None,
        started_at: at(from),
        ended_at: to.map(at),
    }
}

fn snapshot() -> Snapshot {
    Snapshot {
        quests: vec![
            quest(1, "Write book", QuestColor::Blue, QuestStatus::Active),
            quest(2, "Old quest", QuestColor::Red, QuestStatus::Archived),
        ],
        tasks: vec![
            task(10, "Draft chapter", Some(1), "2026-11-25", true),
            task(11, "Groceries", None, "2026-11-25", false),
            task(12, "Orphan", Some(99), "2026-11-25", false),
            task(13, "Tomorrow", Some(1), "2026-11-26", false),
        ],
        time_entries: vec![
            // Sunday before this week: not counted.
            entry(20, Some(1), "2026-11-22T10:00:00+01:00", Some("2026-11-22T12:00:00+01:00")),
            // Monday: 2h for the week.
            entry(21, Some(1), "2026-11-23T08:00:00+01:00", Some("2026-11-23T10:00:00+01:00")),
            // Today: 1.5h quest work, 1h other work, and a running quest session (0.5h so far).
            entry(22, Some(1), "2026-11-25T07:00:00+01:00", Some("2026-11-25T08:30:00+01:00")),
            entry(23, None, "2026-11-25T08:30:00+01:00", Some("2026-11-25T09:30:00+01:00")),
            entry(24, Some(1), "2026-11-25T09:30:00+01:00", None),
        ],
        settings: Settings { weekly_target_minutes: 600 },
        ..Snapshot::default()
    }
}

fn home_of(snapshot: &Snapshot, now: Now) -> HomeData {
    home(snapshot, now, Choice::default(), None)
}

fn titles(tasks: &[HomeTask]) -> Vec<&str> {
    tasks.iter().map(|t| t.title.as_str()).collect()
}

#[test]
fn splits_today_into_quest_work_and_other_with_open_tasks_first() {
    let data = home_of(&snapshot(), frozen());
    assert_eq!(titles(&data.quest_work), ["Draft chapter"]);
    assert_eq!(titles(&data.other), ["Groceries", "Orphan"], "a task whose quest is gone is other work");
    let chip = data.quest_work[0].quest.clone().unwrap();
    assert_eq!(chip, QuestChip { id: id(1).to_string(), name: "Write book".into(), color_hex: "#5c8fd9".into() });
    assert!(data.quest_work[0].done && !data.quest_work[0].can_start_timer);

    let mut more = snapshot();
    more.tasks.insert(0, task(14, "Outline", Some(1), "2026-11-25", false));
    more.tasks.push(task(15, "Edit", Some(1), "2026-11-25", false));
    assert_eq!(titles(&home_of(&more, frozen()).quest_work), ["Outline", "Edit", "Draft chapter"]);
}

#[test]
fn the_filter_narrows_quest_work_only_and_hides_the_field_chip() {
    let mut snap = snapshot();
    snap.quests.push(quest(3, "Garden", QuestColor::Green, QuestStatus::Active));
    snap.tasks.push(task(16, "Weed", Some(3), "2026-11-25", false));
    let unfiltered = home_of(&snap, frozen());
    assert_eq!(titles(&unfiltered.quest_work), ["Weed", "Draft chapter"]);
    assert_eq!(unfiltered.add_quest.map(|q| q.name), Some("Garden".into()), "the newest quest task's quest");

    let filtered = home(&snap, frozen(), Choice { filter: Some(id(1)), pick: None }, None);
    assert_eq!(titles(&filtered.quest_work), ["Draft chapter"]);
    assert_eq!(titles(&filtered.other), ["Groceries", "Orphan"]);
    assert_eq!(filtered.quest_filter.map(|q| q.name), Some("Write book".into()));
    assert_eq!(filtered.add_quest, None);

    let gone = home(&snap, frozen(), Choice { filter: Some(id(77)), pick: None }, None);
    assert_eq!(titles(&gone.quest_work), ["Weed", "Draft chapter"], "a filter on a gone quest is no filter");
    assert_eq!((gone.quest_filter, gone.add_quest.map(|q| q.name)), (None, Some("Garden".into())));
    let archived = home(&snap, frozen(), Choice { filter: Some(id(2)), pick: None }, None);
    assert!(archived.quest_work.is_empty(), "an archived quest still filters, as on the desktop");
    assert_eq!(archived.quest_filter.map(|q| q.name), Some("Old quest".into()));

    let picked = home(&snap, frozen(), Choice { filter: None, pick: Some(id(1)) }, None);
    assert_eq!(picked.add_quest.map(|q| q.name), Some("Write book".into()));
}

#[test]
fn cards_show_tracked_time_and_the_running_timer() {
    let mut snap = snapshot();
    snap.time_entries.push(TimeEntry {
        id: id(30),
        kind: EntryKind::Task,
        task_id: Some(id(11)),
        quest_id: None,
        origin_task_id: None,
        started_at: at("2026-11-25T09:40:00+01:00"),
        ended_at: None,
    });
    let snap = crate::test_support::with_running(snap);
    let data = home_of(&snap, frozen());
    let groceries = &data.other[0];
    assert_eq!((groceries.tracked.as_deref(), groceries.timer_running), (Some("20:00"), true));
    assert_eq!(data.other[1].tracked, None, "nothing tracked, nothing running");
}

#[test]
fn counts_quest_hours_today_and_this_week_including_the_running_session() {
    let data = home_of(&snapshot(), frozen());

    assert_eq!(data.day_label, "Wednesday, November 25");
    assert_eq!(data.day_iso, "2026-11-25");
    assert_eq!(data.today_hours, "2h");
    assert_eq!(data.week_hours, "4h");
    assert_eq!(data.weekly_target, "10h");
    assert!((data.week_progress - 0.4).abs() < 1e-6);
}

#[test]
fn offers_only_active_quests_for_new_tasks() {
    let names: Vec<String> = home_of(&snapshot(), frozen()).quests.into_iter().map(|q| q.name).collect();
    assert_eq!(names, ["Write book"]);
}

#[test]
fn today_follows_the_callers_time_zone() {
    // 23:30 UTC on the 24th is already the 25th in Berlin.
    let data = home_of(&snapshot(), now_at("2026-11-24T23:30:00Z"));
    assert_eq!(data.day_label, "Wednesday, November 25");
    assert_eq!(data.quest_work.len() + data.other.len(), 3);
}

#[test]
fn a_malformed_filter_id_is_refused() {
    let query = HomeQuery { quest_filter: Some("nope".into()), quest_pick: None };
    assert_eq!(Choice::parse(&query), Err(crate::error::MobileError::InvalidId { value: "nope".into() }));
}

#[test]
fn an_unknown_time_zone_is_refused() {
    let moment = Moment { epoch_ms: 0, time_zone: "Mars/Olympus".into() };
    assert!(matches!(Now::parse(&moment), Err(crate::error::MobileError::InvalidTimeZone { .. })));
}

#[test]
fn an_out_of_range_time_is_refused() {
    let moment = Moment { epoch_ms: i64::MAX, time_zone: BERLIN.into() };
    assert_eq!(Now::parse(&moment).unwrap_err(), crate::error::MobileError::InvalidMoment { epoch_ms: i64::MAX });
}

#[test]
fn quests_to_choose_from_put_the_most_recently_used_first() {
    let snap = Snapshot {
        quests: vec![
            quest(1, "Write book", QuestColor::Blue, QuestStatus::Active),
            quest(2, "Garden", QuestColor::Green, QuestStatus::Active),
            quest(3, "Run", QuestColor::Red, QuestStatus::Active),
        ],
        time_entries: vec![
            entry(20, Some(2), "2026-11-25T08:00:00+01:00", Some("2026-11-25T09:00:00+01:00")),
            entry(21, Some(1), "2026-11-24T08:00:00+01:00", Some("2026-11-24T09:00:00+01:00")),
        ],
        ..Snapshot::default()
    };
    let names: Vec<String> =
        home(&snap, frozen(), Choice::default(), None).quests.into_iter().map(|q| q.name).collect();
    assert_eq!(names, ["Garden", "Write book", "Run"]);
}
