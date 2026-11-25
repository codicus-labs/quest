use chrono::{TimeZone, Utc};

use super::*;
use crate::test_support::Replica;

fn at(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 25, hour, minute, 0).unwrap()
}

fn setup() -> (Replica, EntityId, EntityId) {
    let mut r = Replica::new();
    let (quest, task) = (EntityId::new(), EntityId::new());
    r.run(Command::CreateQuest { id: quest, title: "Launch".into(), color: QuestColor::Red });
    r.run(Command::CreateTask {
        id: task,
        title: "Write".into(),
        quest_id: Some(quest),
        planned_on: None,
        due_on: None,
        position: Position::Bottom,
    });
    (r, quest, task)
}

#[test]
fn organizing_stops_the_running_session_and_remembers_where_to_go_back() {
    let (mut r, _, task) = setup();
    r.run(Command::StartFocus { entry_id: EntityId::new(), task_id: task, at: at(9, 0) });
    let organizing = EntityId::new();
    r.run(Command::StartOrganizing { entry_id: organizing, origin_task_id: Some(task), at: at(9, 30) });
    let running = r.snapshot.running_entry().unwrap();
    assert_eq!((running.id, running.kind, running.origin_task_id), (organizing, EntryKind::Organizing, Some(task)));
    assert_eq!((running.task_id, running.quest_id), (None, None));
    assert_eq!(r.snapshot.task_tracked_ms(task, at(10, 0)), 30 * 60_000);
    let work = r.snapshot.work_between(at(0, 0), at(23, 0), at(10, 0));
    assert_eq!((work.quest_ms, work.organizing_ms, work.other_ms), (30 * 60_000, 30 * 60_000, 0));

    let missing = EntityId::new();
    let unknown = Command::StartOrganizing { entry_id: EntityId::new(), origin_task_id: Some(missing), at: at(10, 0) };
    assert_eq!(plan(&unknown, &r.snapshot), Err(CommandError::UnknownTask(missing)));
}

#[test]
fn timer_stops_discard_only_organizing_under_one_minute() {
    use chrono::TimeDelta;
    let start = at(9, 0);
    for (elapsed, replace, discarded) in [(0, false, true), (59_999, true, true), (60_000, false, false)] {
        let (mut r, _, task) = setup();
        let organizing = EntityId::new();
        let end = start + TimeDelta::milliseconds(elapsed);
        r.run(Command::StartOrganizing { entry_id: organizing, origin_task_id: Some(task), at: start });
        if replace {
            r.run(Command::StartFocus { entry_id: EntityId::new(), task_id: task, at: end });
            assert_eq!(r.snapshot.running_entry().unwrap().task_id, Some(task));
        } else {
            r.run(Command::StopFocus { at: end });
            assert!(r.snapshot.running_entry().is_none());
        }
        assert_eq!(r.snapshot.time_entries.iter().any(|e| e.id == organizing), !discarded);
        assert_eq!(r.snapshot.trash.iter().any(|e| e.id == organizing), discarded);
    }
    let (mut r, _, task) = setup();
    let id = EntityId::new();
    r.run(Command::StartFocus { entry_id: id, task_id: task, at: start });
    r.run(Command::StopFocus { at: start + TimeDelta::seconds(1) });
    assert_eq!(r.snapshot.time_entries[0].id, id);
}

#[test]
fn undo_rebook_also_joins_a_discarded_snippet() {
    let (mut r, _, task) = setup();
    let (focus, organizing) = (EntityId::new(), EntityId::new());
    let end = at(9, 40) + chrono::TimeDelta::seconds(30);
    r.run(Command::StartFocus { entry_id: focus, task_id: task, at: at(9, 0) });
    r.run(Command::RebookAsOrganizing {
        expected_task_entry: Some(focus),
        entry_id: organizing,
        since: at(9, 40),
        now: end,
    });
    r.run(Command::StopFocus { at: end });
    assert_eq!(r.snapshot.time_entries.len(), 1);
    r.run(Command::UndoOrganizingRebook { entry_id: organizing, at: at(9, 41) });
    assert_eq!(r.snapshot.time_entries[0].ended_at, Some(end));
    assert!(r.snapshot.running_entry().is_none());
}

#[test]
fn rebooking_splits_the_running_task_session_at_since() {
    let (mut r, quest, task) = setup();
    let focus = EntityId::new();
    r.run(Command::StartFocus { entry_id: focus, task_id: task, at: at(9, 0) });
    let organizing = EntityId::new();
    for since in [at(8, 0), at(10, 1)] {
        let rebook =
            Command::RebookAsOrganizing { expected_task_entry: None, entry_id: organizing, since, now: at(10, 0) };
        assert_eq!(plan(&rebook, &r.snapshot), Err(CommandError::RebookOutsideEntry));
    }
    r.run(Command::RebookAsOrganizing {
        expected_task_entry: None,
        entry_id: organizing,
        since: at(9, 40),
        now: at(9, 42),
    });

    let ended = r.snapshot.time_entries.iter().find(|e| e.id == focus).unwrap();
    assert_eq!((ended.ended_at, ended.quest_id), (Some(at(9, 40)), Some(quest)));
    let running = r.snapshot.running_entry().unwrap();
    assert_eq!((running.id, running.started_at, running.origin_task_id), (organizing, at(9, 40), Some(task)));

    let again = Command::RebookAsOrganizing {
        expected_task_entry: None,
        entry_id: EntityId::new(),
        since: at(9, 41),
        now: at(9, 42),
    };
    assert_eq!(plan(&again, &r.snapshot), Err(CommandError::NothingToRebook));
    r.run(Command::StopFocus { at: at(9, 50) });
    assert_eq!(plan(&again, &r.snapshot), Err(CommandError::NothingToRebook));
}

#[test]
fn rebook_cannot_split_a_replacement_timer() {
    let (mut r, _, task) = setup();
    r.run(Command::StartFocus { entry_id: EntityId::new(), task_id: task, at: at(9, 0) });
    let stale = Command::RebookAsOrganizing {
        entry_id: EntityId::new(),
        expected_task_entry: Some(EntityId::new()),
        since: at(9, 10),
        now: at(9, 12),
    };
    assert_eq!(plan(&stale, &r.snapshot), Err(CommandError::NothingToRebook));
}

#[test]
fn giving_an_organizing_session_a_task_makes_it_task_work() {
    let (mut r, quest, task) = setup();
    let entry = EntityId::new();
    r.run(Command::StartOrganizing { entry_id: entry, origin_task_id: None, at: at(9, 0) });
    r.run(Command::EditEntry {
        id: entry,
        task: Some(WorkTask::Existing(task)),
        quest_id: Some(quest),
        started_at: at(9, 0),
        ended_at: Some(at(9, 30)),
    });
    let edited = &r.snapshot.time_entries[0];
    assert_eq!((edited.kind, edited.task_id, edited.quest_id), (EntryKind::Task, Some(task), Some(quest)));
}

#[test]
fn booking_a_task_session_as_organizing_keeps_its_times_and_remembers_the_task() {
    let (mut r, _, task) = setup();
    let entry = EntityId::new();
    r.run(Command::LogTime { entry_id: entry, task_id: Some(task), started_at: at(9, 0), ended_at: at(9, 30) });
    r.run(Command::BookEntryAsOrganizing { id: entry });
    let booked = &r.snapshot.time_entries[0];
    assert_eq!((booked.kind, booked.task_id, booked.quest_id), (EntryKind::Organizing, None, None));
    assert_eq!((booked.origin_task_id, booked.started_at, booked.ended_at), (Some(task), at(9, 0), Some(at(9, 30))));
    assert_eq!(plan(&Command::BookEntryAsOrganizing { id: entry }, &r.snapshot), Ok(Vec::new()));
}
