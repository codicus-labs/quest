use chrono::{DateTime, NaiveDate, TimeZone, Utc};

use crate::change::EntityId;
use crate::command::{Command, Position};
use crate::model::QuestColor;
use crate::test_support::Replica;

fn at(h: u32, m: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 15, h, m, 0).unwrap()
}

/// A quest task and an other task on Oct 15.
fn setup() -> (Replica, EntityId, EntityId, EntityId) {
    let mut r = Replica::new();
    let (quest, quest_task, other_task) = (EntityId::new(), EntityId::new(), EntityId::new());
    r.run(Command::CreateQuest { id: quest, title: "Launch".into(), color: QuestColor::Red });
    let day = Some(NaiveDate::from_ymd_opt(2026, 10, 15).unwrap());
    for (id, quest_id) in [(quest_task, Some(quest)), (other_task, None)] {
        r.run(Command::CreateTask {
            id,
            title: "t".into(),
            quest_id,
            planned_on: day,
            due_on: None,
            position: Position::Bottom,
        });
    }
    (r, quest, quest_task, other_task)
}

fn log(r: &mut Replica, task: EntityId, from: DateTime<Utc>, to: DateTime<Utc>) {
    r.run(Command::LogTime { entry_id: EntityId::new(), task_id: Some(task), started_at: from, ended_at: to });
}

#[test]
fn quest_hours_count_only_time_on_quest_tasks() {
    let (mut r, _, quest_task, other_task) = setup();
    log(&mut r, quest_task, at(9, 0), at(10, 0));
    log(&mut r, other_task, at(11, 0), at(11, 30));
    let work = r.snapshot.work_between(at(0, 0), at(23, 59), at(23, 59));
    assert_eq!(work.quest_ms, 3_600_000);
    assert_eq!(work.other_ms, 1_800_000);
    assert_eq!(work.total_ms(), 5_400_000);
}

#[test]
fn the_window_clips_sessions_and_a_running_one_counts_to_now() {
    let (mut r, _, quest_task, _) = setup();
    log(&mut r, quest_task, at(9, 0), at(10, 0));
    r.run(Command::StartFocus { entry_id: EntityId::new(), task_id: quest_task, at: at(14, 0) });
    let work = r.snapshot.work_between(at(9, 30), at(23, 0), at(14, 15));
    assert_eq!(work.quest_ms, 45 * 60_000, "30 min of the morning plus 15 running");
}

#[test]
fn a_deleted_quest_counts_as_other_work() {
    let (mut r, quest, quest_task, _) = setup();
    log(&mut r, quest_task, at(9, 0), at(10, 0));
    r.run(Command::DeleteQuest { id: quest, at: at(12, 0) });
    let work = r.snapshot.work_between(at(0, 0), at(23, 0), at(23, 0));
    assert_eq!((work.quest_ms, work.other_ms), (0, 3_600_000));
    let task = r.snapshot.task(quest_task).unwrap();
    assert!(!r.snapshot.is_quest_work(task), "its task shows under Other until the quest is restored");
}
