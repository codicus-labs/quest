//! Log work: time logged after the fact on an existing (even done) task or a new done task.

use chrono::{DateTime, TimeZone, Utc};

use super::*;
use crate::model::QuestColor;
use crate::review::WeeklyReview;
use crate::test_support::Replica;

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 23).unwrap()
}

fn at(h: u32, m: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 23, h, m, 0).unwrap()
}

fn with_quest() -> (Replica, EntityId) {
    let mut r = Replica::new();
    let quest = EntityId::new();
    r.run(Command::CreateQuest { id: quest, title: "Photo series".into(), color: QuestColor::Red });
    (r, quest)
}

fn quest_ms(r: &Replica) -> i64 {
    WeeklyReview::build(&r.snapshot, day(), &Utc, at(23, 0)).quest_ms
}

#[test]
fn logging_on_a_done_quest_task_counts_as_quest_hours() {
    let (mut r, quest) = with_quest();
    let task = EntityId::new();
    let create = Command::CreateTask {
        id: task,
        title: "Pick three motifs".into(),
        quest_id: Some(quest),
        planned_on: Some(day()),
        due_on: None,
        position: Position::Bottom,
    };
    r.run(create);
    r.run(Command::CompleteTask { id: task, at: at(15, 8) });
    let entry = EntityId::new();
    r.run(Command::LogWork {
        entry_id: entry,
        task: WorkTask::Existing(task),
        started_at: at(14, 50),
        ended_at: at(15, 5),
    });

    let logged = r.snapshot.time_entries.iter().find(|e| e.id == entry).expect("the session exists");
    assert_eq!((logged.task_id, logged.quest_id), (Some(task), Some(quest)), "it takes the task's quest");
    assert!(r.snapshot.task(task).unwrap().is_done(), "the task stays done");
    assert_eq!(quest_ms(&r), 15 * 60_000);
    assert_eq!(r.snapshot.work_between(at(0, 0), at(23, 0), at(23, 0)).quest_ms, 15 * 60_000);
}

#[test]
fn a_new_title_creates_a_done_task_with_the_chosen_quest() {
    let (mut r, quest) = with_quest();
    let (task, entry) = (EntityId::new(), EntityId::new());
    let new =
        WorkTask::New { id: task, title: " Sketch contact sheet ".into(), quest_id: Some(quest), planned_on: day() };
    r.run(Command::LogWork { entry_id: entry, task: new, started_at: at(9, 0), ended_at: at(9, 30) });

    let created = r.snapshot.task(task).expect("the task was created");
    assert_eq!(created.title, "Sketch contact sheet");
    assert_eq!((created.quest_id, created.planned_on), (Some(quest), Some(day())));
    assert_eq!(created.completed_at, Some(at(9, 30)), "done when the logged work ended");
    assert_eq!(quest_ms(&r), 30 * 60_000);
}

#[test]
fn a_new_task_without_a_quest_is_other_work() {
    let (mut r, _) = with_quest();
    let new = WorkTask::New { id: EntityId::new(), title: "Emails".into(), quest_id: None, planned_on: day() };
    r.run(Command::LogWork { entry_id: EntityId::new(), task: new, started_at: at(9, 0), ended_at: at(9, 30) });
    let work = r.snapshot.work_between(at(0, 0), at(23, 0), at(23, 0));
    assert_eq!((work.quest_ms, work.other_ms), (0, 30 * 60_000));
}

#[test]
fn a_refused_log_writes_nothing() {
    let (r, quest) = with_quest();
    let task = EntityId::new();
    let new = WorkTask::New { id: task, title: "Late".into(), quest_id: Some(quest), planned_on: day() };
    let backwards =
        Command::LogWork { entry_id: EntityId::new(), task: new.clone(), started_at: at(10, 0), ended_at: at(10, 0) };
    assert_eq!(plan(&backwards, &r.snapshot), Err(CommandError::InvalidTimeRange));
    let empty = WorkTask::New { id: task, title: "  ".into(), quest_id: Some(quest), planned_on: day() };
    let untitled =
        Command::LogWork { entry_id: EntityId::new(), task: empty, started_at: at(9, 0), ended_at: at(10, 0) };
    assert_eq!(plan(&untitled, &r.snapshot), Err(CommandError::EmptyTitle));
    let unknown = Command::LogWork {
        entry_id: EntityId::new(),
        task: WorkTask::Existing(EntityId::new()),
        started_at: at(9, 0),
        ended_at: at(10, 0),
    };
    assert!(matches!(plan(&unknown, &r.snapshot), Err(CommandError::UnknownTask(_))));
    assert!(r.snapshot.tasks.is_empty() && r.snapshot.time_entries.is_empty(), "nothing was created");
    assert_eq!(quest_ms(&r), 0);
}

#[test]
fn descriptions_are_plain_multiline_text() {
    let (mut r, _) = with_quest();
    let task = EntityId::new();
    r.run(Command::CreateTask {
        id: task,
        title: "Notes".into(),
        quest_id: None,
        planned_on: None,
        due_on: None,
        position: Position::Top,
    });
    assert_eq!(r.snapshot.task(task).unwrap().description, "", "no description until one is written");
    r.run(Command::SetTaskDescription { id: task, description: "Line one\nLine two".into() });
    assert_eq!(r.snapshot.task(task).unwrap().description, "Line one\nLine two");
    let unknown = Command::SetTaskDescription { id: EntityId::new(), description: "x".into() };
    assert!(matches!(plan(&unknown, &r.snapshot), Err(CommandError::UnknownTask(_))));
}
