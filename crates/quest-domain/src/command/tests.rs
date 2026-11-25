use chrono::{TimeZone, Utc};

use super::*;
use crate::review::WeeklyReview;
use crate::test_support::Replica;

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
}

#[test]
fn tasks_insert_at_top_bottom_and_before() {
    let mut r = Replica::new();
    let [a, b, c, d] = [EntityId::new(), EntityId::new(), EntityId::new(), EntityId::new()];
    let create = |id, title: &str, position| Command::CreateTask {
        id,
        title: title.into(),
        quest_id: None,
        planned_on: Some(day(15)),
        due_on: None,
        position,
    };
    r.run(create(a, "a", Position::Bottom));
    r.run(create(b, "b", Position::Bottom));
    r.run(create(c, "c", Position::Top));
    r.run(create(d, "d", Position::Before(b)));
    let order: Vec<&str> = r.snapshot.tasks_on(day(15)).map(|t| t.title.as_str()).collect();
    assert_eq!(order, ["c", "a", "d", "b"]);
}

#[test]
fn starting_focus_stops_the_running_session_and_completing_stops_its_own() {
    let mut r = Replica::new();
    let (quest, t1, t2) = (EntityId::new(), EntityId::new(), EntityId::new());
    r.run(Command::CreateQuest { id: quest, title: "Launch".into(), color: QuestColor::Red });
    for t in [t1, t2] {
        r.run(Command::CreateTask {
            id: t,
            title: "t".into(),
            quest_id: Some(quest),
            planned_on: None,
            due_on: None,
            position: Position::Bottom,
        });
    }
    let at = |min| Utc.with_ymd_and_hms(2026, 10, 15, 9, min, 0).unwrap();
    r.run(Command::StartFocus { entry_id: EntityId::new(), task_id: t1, at: at(0) });
    r.run(Command::StartFocus { entry_id: EntityId::new(), task_id: t2, at: at(10) });
    assert_eq!(r.snapshot.running_entry().unwrap().task_id, Some(t2));
    assert_eq!(r.snapshot.task_tracked_ms(t1, at(30)), 10 * 60_000);

    r.run(Command::CompleteTask { id: t2, at: at(25) });
    assert!(r.snapshot.running_entry().is_none());
    assert!(r.snapshot.task(t2).unwrap().is_done());
    assert_eq!(r.snapshot.time_entries[1].quest_id, Some(quest));
}

#[test]
fn weekly_review_splits_quest_and_other_time_per_day() {
    let mut r = Replica::new();
    let (quest, quest_task, chore) = (EntityId::new(), EntityId::new(), EntityId::new());
    r.run(Command::CreateQuest { id: quest, title: "Run".into(), color: QuestColor::Lime });
    r.run(Command::CreateTask {
        id: quest_task,
        title: "5k".into(),
        quest_id: Some(quest),
        planned_on: None,
        due_on: None,
        position: Position::Bottom,
    });
    r.run(Command::CreateTask {
        id: chore,
        title: "mail".into(),
        quest_id: None,
        planned_on: None,
        due_on: None,
        position: Position::Bottom,
    });
    // Thursday 2026-10-15, 23:00–01:00 spans into Friday.
    let thu_23 = Utc.with_ymd_and_hms(2026, 10, 15, 23, 0, 0).unwrap();
    r.run(Command::StartFocus { entry_id: EntityId::new(), task_id: quest_task, at: thu_23 });
    r.run(Command::StartFocus { entry_id: EntityId::new(), task_id: chore, at: thu_23 + chrono::Duration::hours(2) });
    r.run(Command::StopFocus { at: thu_23 + chrono::Duration::hours(3) });

    let review = WeeklyReview::build(&r.snapshot, day(15), &Utc, thu_23 + chrono::Duration::hours(5));
    assert_eq!(review.week_start, day(12));
    assert_eq!(review.quest_ms_per_day[3], 3_600_000);
    assert_eq!(review.quest_ms_per_day[4], 3_600_000);
    assert_eq!(review.other_ms, 3_600_000);
    assert_eq!(review.by_quest, vec![crate::review::QuestHours { quest_id: quest, ms: 7_200_000 }]);
    assert_eq!(review.quest_share_percent(), 66);
    assert_eq!(review.by_quest_per_day[3], vec![crate::review::QuestHours { quest_id: quest, ms: 3_600_000 }]);
    assert!(review.by_quest_per_day[0].is_empty());
}

#[test]
fn a_days_quest_work_is_split_by_quest_in_the_weeks_order() {
    let mut r = Replica::new();
    let (big, small) = (EntityId::new(), EntityId::new());
    let (big_task, small_task) = (EntityId::new(), EntityId::new());
    for (quest, task) in [(big, big_task), (small, small_task)] {
        r.run(Command::CreateQuest { id: quest, title: "q".into(), color: QuestColor::Lime });
        r.run(Command::CreateTask {
            id: task,
            title: "t".into(),
            quest_id: Some(quest),
            planned_on: None,
            due_on: None,
            position: Position::Bottom,
        });
    }
    let at = |d, h| Utc.with_ymd_and_hms(2026, 10, d, h, 0, 0).unwrap();
    // Monday: small 1 h, then big 2 h; Tuesday: big 1 h.
    r.run(Command::StartFocus { entry_id: EntityId::new(), task_id: small_task, at: at(12, 8) });
    r.run(Command::StartFocus { entry_id: EntityId::new(), task_id: big_task, at: at(12, 9) });
    r.run(Command::StopFocus { at: at(12, 11) });
    r.run(Command::StartFocus { entry_id: EntityId::new(), task_id: big_task, at: at(13, 8) });
    r.run(Command::StopFocus { at: at(13, 9) });

    let review = WeeklyReview::build(&r.snapshot, day(14), &Utc, at(14, 0));
    let monday: Vec<(EntityId, i64)> = review.by_quest_per_day[0].iter().map(|q| (q.quest_id, q.ms)).collect();
    assert_eq!(monday, [(big, 7_200_000), (small, 3_600_000)]);
    assert_eq!(review.by_quest_per_day[1].len(), 1);
}

#[test]
fn deleted_and_incomplete_records_are_not_materialised() {
    let mut r = Replica::new();
    let task = EntityId::new();
    r.run(Command::CreateTask {
        id: task,
        title: "x".into(),
        quest_id: None,
        planned_on: None,
        due_on: None,
        position: Position::Bottom,
    });
    r.run(Command::DeleteTask { id: task, at: Utc::now() });
    assert!(r.snapshot.tasks.is_empty());

    let orphan = FieldWrite { kind: EntityKind::Task, id: EntityId::new(), field: fields::SORT_KEY, value: json!("V") };
    r.store.apply(&orphan.stamp(r.clock.now(99)));
    assert!(Snapshot::from_store(&r.store, EntityId::DEFAULT_WORKSPACE).tasks.is_empty());
}

#[test]
fn invalid_commands_are_rejected() {
    let r = Replica::new();
    let missing = EntityId::new();
    assert_eq!(
        plan(&Command::DeleteTask { id: missing, at: Utc::now() }, &r.snapshot),
        Err(CommandError::UnknownTask(missing))
    );
    let blank = Command::CreateQuest { id: EntityId::new(), title: "  ".into(), color: QuestColor::Blue };
    assert_eq!(plan(&blank, &r.snapshot), Err(CommandError::EmptyTitle));
}

#[test]
fn sessions_can_be_logged_moved_reassigned_and_deleted() {
    let mut r = Replica::new();
    let (quest, task, entry) = (EntityId::new(), EntityId::new(), EntityId::new());
    r.run(Command::CreateQuest { id: quest, title: "Write".into(), color: QuestColor::Orange });
    r.run(Command::CreateTask {
        id: task,
        title: "Essay".into(),
        quest_id: Some(quest),
        planned_on: None,
        due_on: None,
        position: Position::Bottom,
    });
    let at = |h| Utc.with_ymd_and_hms(2026, 11, 23, h, 0, 0).unwrap();

    r.run(Command::LogTime { entry_id: entry, task_id: None, started_at: at(9), ended_at: at(10) });
    assert_eq!(r.snapshot.time_entries[0].quest_id, None);
    let edit = |task| Command::EditEntry {
        id: entry,
        task: Some(WorkTask::Existing(task)),
        quest_id: Some(quest),
        started_at: at(9),
        ended_at: Some(at(10)),
    };
    r.run(edit(task));
    assert_eq!(r.snapshot.time_entries[0].quest_id, Some(quest));
    r.run(Command::SetEntryTimes { id: entry, started_at: at(11), ended_at: Some(at(13)) });
    assert_eq!(r.snapshot.task_tracked_ms(task, at(14)), 2 * 3_600_000);
    r.run(Command::DeleteEntry { id: entry, at: at(15) });
    assert!(r.snapshot.time_entries.is_empty());
}

#[test]
fn session_edits_are_validated() {
    let mut r = Replica::new();
    let (task, entry) = (EntityId::new(), EntityId::new());
    r.run(Command::CreateTask {
        id: task,
        title: "t".into(),
        quest_id: None,
        planned_on: None,
        due_on: None,
        position: Position::Bottom,
    });
    let at = |h| Utc.with_ymd_and_hms(2026, 11, 23, h, 0, 0).unwrap();
    let backwards = Command::LogTime { entry_id: entry, task_id: None, started_at: at(10), ended_at: at(9) };
    assert_eq!(plan(&backwards, &r.snapshot), Err(CommandError::InvalidTimeRange));

    r.run(Command::LogTime { entry_id: entry, task_id: Some(task), started_at: at(9), ended_at: at(10) });
    let resume = Command::SetEntryTimes { id: entry, started_at: at(9), ended_at: None };
    assert_eq!(plan(&resume, &r.snapshot), Err(CommandError::CannotResumeEntry));

    let running = EntityId::new();
    r.run(Command::StartFocus { entry_id: running, task_id: task, at: at(11) });
    r.run(Command::SetEntryTimes { id: running, started_at: at(10), ended_at: None });
    assert_eq!(r.snapshot.running_entry().unwrap().started_at, at(10));
}

#[test]
fn moving_onto_another_day_puts_the_task_at_the_bottom_of_that_day() {
    let mut r = Replica::new();
    let (existing, moved) = (EntityId::new(), EntityId::new());
    r.run(Command::CreateTask {
        id: existing,
        title: "a".into(),
        quest_id: None,
        planned_on: Some(day(16)),
        due_on: None,
        position: Position::Bottom,
    });
    r.run(Command::CreateTask {
        id: moved,
        title: "b".into(),
        quest_id: None,
        planned_on: Some(day(15)),
        due_on: None,
        position: Position::Bottom,
    });
    let at = Utc.with_ymd_and_hms(2026, 10, 15, 9, 0, 0).unwrap();
    r.run(Command::MoveTask { id: moved, planned_on: Some(day(16)), position: Position::Bottom, at });
    let titles: Vec<&str> = r.snapshot.tasks_on(day(16)).map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["a", "b"]);
}

#[test]
fn deleting_a_quest_keeps_its_work_and_restore_requires_a_trashed_record() {
    let mut r = Replica::new();
    let (quest, other, task) = (EntityId::new(), EntityId::new(), EntityId::new());
    r.run(Command::CreateQuest { id: quest, title: "Launch".into(), color: QuestColor::Red });
    r.run(Command::CreateQuest { id: other, title: "Run".into(), color: QuestColor::Lime });
    r.run(Command::CreateTask {
        id: task,
        title: "t".into(),
        quest_id: Some(quest),
        planned_on: None,
        due_on: None,
        position: Position::Bottom,
    });
    let at = |h| Utc.with_ymd_and_hms(2026, 11, 23, h, 0, 0).unwrap();
    let live = Command::Restore { kind: EntityKind::Quest, id: quest };
    assert_eq!(plan(&live, &r.snapshot), Err(CommandError::NotInTrash { kind: EntityKind::Quest, id: quest }));
    r.run(Command::DeleteQuest { id: other, at: at(8) });
    r.run(Command::DeleteQuest { id: quest, at: at(9) });

    assert!(r.snapshot.quest(quest).is_none());
    assert_eq!(r.snapshot.task(task).unwrap().quest_id, Some(quest), "work stays");
    assert_eq!(r.snapshot.trash.len(), 2);
    assert_eq!(r.snapshot.trash[0].id, quest, "most recent first");

    r.run(Command::Restore { kind: EntityKind::Quest, id: quest });
    assert!(r.snapshot.quest(quest).is_some());
}

#[test]
fn deleting_a_running_task_stops_its_session() {
    let mut r = Replica::new();
    let task = EntityId::new();
    r.run(Command::CreateTask {
        id: task,
        title: "t".into(),
        quest_id: None,
        planned_on: None,
        due_on: None,
        position: Position::Bottom,
    });
    let at = |h| Utc.with_ymd_and_hms(2026, 11, 23, h, 0, 0).unwrap();
    r.run(Command::StartFocus { entry_id: EntityId::new(), task_id: task, at: at(9) });
    r.run(Command::DeleteTask { id: task, at: at(10) });
    assert!(r.snapshot.running_entry().is_none());
    assert_eq!(r.snapshot.trash[0].title.as_deref(), Some("t"));
}

#[test]
fn quests_archived_before_completed_existed_stay_archived_and_never_count_as_completed() {
    let mut r = Replica::new();
    let (legacy, reached) = (EntityId::new(), EntityId::new());
    r.run(Command::CreateQuest { id: legacy, title: "Old archive".into(), color: QuestColor::Red });
    r.run(Command::CreateQuest { id: reached, title: "Run".into(), color: QuestColor::Lime });
    // The value older apps stored when they archived (or "completed") a quest.
    let old_value = crate::change::FieldChange {
        kind: EntityKind::Quest,
        id: legacy,
        field: fields::QUEST_STATUS.to_owned(),
        value: serde_json::json!("archived"),
        hlc: r.clock.now(100),
    };
    r.store.apply(&old_value);
    r.run(Command::SetQuestStatus { id: reached, status: QuestStatus::Completed });

    assert_eq!(r.snapshot.quest(legacy).unwrap().status, QuestStatus::Archived);
    let completed: Vec<EntityId> = r.snapshot.completed_quests().map(|q| q.id).collect();
    assert_eq!(completed, [reached], "only a quest completed as such counts");
    assert_eq!(r.snapshot.active_quests().count(), 0);
    assert_eq!(serde_json::to_value(QuestStatus::Completed).unwrap(), serde_json::json!("completed"));
}

#[test]
fn completing_and_reopening_a_quest_keeps_its_tracked_time() {
    let mut r = Replica::new();
    let (quest, task, entry) = (EntityId::new(), EntityId::new(), EntityId::new());
    r.run(Command::CreateQuest { id: quest, title: "Run".into(), color: QuestColor::Lime });
    r.run(Command::CreateTask {
        id: task,
        title: "Long run".into(),
        quest_id: Some(quest),
        planned_on: None,
        due_on: None,
        position: Position::Bottom,
    });
    let start = Utc.with_ymd_and_hms(2026, 10, 15, 9, 0, 0).unwrap();
    r.run(Command::LogWork {
        entry_id: entry,
        task: WorkTask::Existing(task),
        started_at: start,
        ended_at: start + chrono::Duration::hours(2),
    });
    let tracked = |r: &Replica| r.snapshot.time_entries.iter().filter(|e| e.quest_id == Some(quest)).count();
    r.run(Command::SetQuestStatus { id: quest, status: QuestStatus::Completed });
    assert_eq!((r.snapshot.quest(quest).unwrap().status, tracked(&r)), (QuestStatus::Completed, 1));
    r.run(Command::SetQuestStatus { id: quest, status: QuestStatus::Active });
    assert_eq!((r.snapshot.quest(quest).unwrap().status, tracked(&r)), (QuestStatus::Active, 1));
}
