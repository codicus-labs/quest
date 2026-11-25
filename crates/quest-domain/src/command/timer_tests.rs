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
fn rebooking_splits_the_running_task_session_at_since() {
    let (mut r, quest, task) = setup();
    let focus = EntityId::new();
    r.run(Command::StartFocus { entry_id: focus, task_id: task, at: at(9, 0) });
    let organizing = EntityId::new();
    for since in [at(9, 0), at(8, 0), at(10, 1)] {
        let rebook = Command::RebookAsOrganizing { entry_id: organizing, since, now: at(10, 0) };
        assert_eq!(plan(&rebook, &r.snapshot), Err(CommandError::RebookOutsideEntry));
    }
    r.run(Command::RebookAsOrganizing { entry_id: organizing, since: at(9, 40), now: at(9, 42) });

    let ended = r.snapshot.time_entries.iter().find(|e| e.id == focus).unwrap();
    assert_eq!((ended.ended_at, ended.quest_id), (Some(at(9, 40)), Some(quest)));
    let running = r.snapshot.running_entry().unwrap();
    assert_eq!((running.id, running.started_at, running.origin_task_id), (organizing, at(9, 40), Some(task)));

    let again = Command::RebookAsOrganizing { entry_id: EntityId::new(), since: at(9, 41), now: at(9, 42) };
    assert_eq!(plan(&again, &r.snapshot), Err(CommandError::NothingToRebook));
    r.run(Command::StopFocus { at: at(9, 50) });
    assert_eq!(plan(&again, &r.snapshot), Err(CommandError::NothingToRebook));
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
