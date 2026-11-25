use chrono::{DateTime, NaiveDate, Utc};
use quest_domain::{Command, EntityId, Position, Quest, QuestColor, QuestStatus, Snapshot, Task};

use super::*;
use crate::types::{AddPosition, Moment, TaskSection};

/// Wednesday, 25 Nov 2026, 00:30 in Berlin (still the 24th in UTC).
fn frozen() -> Now {
    let at = DateTime::parse_from_rfc3339("2026-11-24T23:30:00Z").unwrap().with_timezone(&Utc);
    Now::parse(&Moment { epoch_ms: at.timestamp_millis(), time_zone: "Europe/Berlin".into() }).unwrap()
}

fn id(n: u128) -> EntityId {
    EntityId(uuid::Uuid::from_u128(n))
}

fn snapshot_with(done: bool) -> Snapshot {
    let task = Task {
        id: id(1),
        title: "Draft".into(),
        quest_id: None,
        day: None,
        sort_key: "a".into(),
        block: None,
        completed_at: done.then(|| frozen().at),
        description: String::new(),
        repeat: None,
        series: None,
    };
    Snapshot { tasks: vec![task], ..Snapshot::default() }
}

fn task(done: bool) -> Task {
    snapshot_with(done).tasks.remove(0)
}

fn with_quest(snapshot: Snapshot) -> Snapshot {
    let quest = Quest {
        id: id(7),
        title: "Book".into(),
        color: QuestColor::Blue,
        status: QuestStatus::Active,
        sort_key: "a".into(),
        description: String::new(),
    };
    Snapshot { quests: vec![quest], ..snapshot }
}

#[test]
fn quest_work_lands_today_with_the_desktop_quest_at_the_chosen_end() {
    let snapshot = with_quest(snapshot_with(false));
    let command =
        add_task(&snapshot, id(1), " Draft ", TaskSection::QuestWork, AddPosition::Top, Choice::default(), frozen());
    assert_eq!(
        command.unwrap(),
        Command::CreateTask {
            id: id(1),
            title: "Draft".into(),
            quest_id: Some(id(7)),
            day: Some(NaiveDate::from_ymd_opt(2026, 11, 25).unwrap()),
            position: Position::Top,
        }
    );
}

#[test]
fn other_work_gets_no_quest_and_an_empty_title_is_refused() {
    let snapshot = with_quest(snapshot_with(false));
    let other =
        add_task(&snapshot, id(1), "Mail", TaskSection::Other, AddPosition::Bottom, Choice::default(), frozen());
    assert!(matches!(other.unwrap(), Command::CreateTask { quest_id: None, position: Position::Bottom, .. }));
    let empty = add_task(&snapshot, id(1), "  ", TaskSection::Other, AddPosition::Top, Choice::default(), frozen());
    assert_eq!(empty.unwrap_err(), MobileError::EmptyTitle);
    let pasted =
        add_task(&snapshot, id(1), "a\r\nb\nc", TaskSection::Other, AddPosition::Top, Choice::default(), frozen());
    assert!(
        matches!(pasted.unwrap(), Command::CreateTask { title, .. } if title == "a b c"),
        "one line, like the desktop field"
    );
}

#[test]
fn quest_work_ignores_a_filter_on_a_gone_quest() {
    let snapshot = with_quest(snapshot_with(false));
    let gone = Choice { filter: Some(id(99)), pick: None };
    let command = add_task(&snapshot, id(1), "Draft", TaskSection::QuestWork, AddPosition::Top, gone, frozen());
    assert!(matches!(command.unwrap(), Command::CreateTask { quest_id: Some(q), .. } if q == id(7)));
}

#[test]
fn toggling_completes_an_open_task_now_and_reopens_a_done_one() {
    let complete = Command::CompleteTaskFromPlan {
        id: id(1),
        at: frozen().at,
        today: NaiveDate::from_ymd_opt(2026, 11, 25).unwrap(),
    };
    assert_eq!(toggle_task(&task(false), frozen()), complete, "the local day: 00:30 in Berlin");
    assert_eq!(toggle_task(&task(true), frozen()), Command::ReopenTask { id: id(1) });
}

#[test]
fn a_done_task_cannot_start_its_timer() {
    let snapshot = snapshot_with(false);
    let start = start_timer(&snapshot, &task(false), id(9), frozen()).unwrap();
    assert_eq!(start, Some(Command::StartFocus { entry_id: id(9), task_id: id(1), at: frozen().at }));
    let done = start_timer(&snapshot, &task(true), id(9), frozen());
    assert_eq!(done, Err(MobileError::TaskIsDone { id: id(1).to_string() }));
}

#[test]
fn starting_the_timer_that_already_runs_does_nothing() {
    let running = quest_domain::TimeEntry {
        id: id(8),
        task_id: Some(id(1)),
        quest_id: None,
        started_at: frozen().at,
        ended_at: None,
    };
    let snapshot = crate::test_support::with_running(Snapshot { time_entries: vec![running], ..snapshot_with(false) });
    assert_eq!(start_timer(&snapshot, &task(false), id(9), frozen()), Ok(None));
}

#[test]
fn unchanged_values_save_nothing() {
    let t = task(false);
    assert_eq!(rename_task(&t, " Draft "), None);
    assert_eq!(rename_task(&t, ""), None);
    assert_eq!(set_description(&t, ""), None);
    assert_eq!(set_quest(&t, None), Ok(None));
    assert_eq!(set_day(&t, None), None);
    assert_eq!(
        set_description(&t, "a\nb"),
        Some(Command::SetTaskDescription { id: id(1), description: "a\nb".into() })
    );
}

#[test]
fn a_new_day_moves_to_the_bottom_and_a_bad_quest_id_is_refused() {
    let day = NaiveDate::from_ymd_opt(2026, 11, 26).unwrap();
    assert_eq!(
        set_day(&task(false), Some(day)),
        Some(Command::MoveTask { id: id(1), day: Some(day), position: Position::Bottom })
    );
    assert_eq!(set_quest(&task(false), Some("nope")), Err(MobileError::InvalidId { value: "nope".into() }));
    assert_eq!(
        set_quest(&task(false), Some(&id(7).to_string())),
        Ok(Some(Command::AssignTask { id: id(1), quest_id: Some(id(7)) }))
    );
}
