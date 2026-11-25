use chrono::{FixedOffset, TimeZone};
use quest_domain::{EntryKind, Postponement, Quest, QuestColor, QuestStatus, Task, TimeEntry};

use super::*;

fn local() -> FixedOffset {
    FixedOffset::east_opt(3600).unwrap()
}

fn at(day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
    local().with_ymd_and_hms(2026, 11, day, hour, minute, 0).unwrap().to_utc()
}

fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
}

fn task(plan: Option<NaiveDate>, due: Option<NaiveDate>, done: Option<DateTime<Utc>>) -> Task {
    Task {
        id: EntityId::new(),
        title: "Task".into(),
        quest_id: None,
        planned_on: plan,
        due_on: due,
        day_order: String::new(),
        sort_key: String::new(),
        completed_at: done,
        description: String::new(),
        repeat: None,
        series: None,
    }
}

#[test]
fn week_counts_live_tasks_and_lists_only_open_dates_from_the_selected_week() {
    let behind = task(Some(d(23)), Some(d(24)), None);
    let behind_id = behind.id;
    let first = task(Some(d(23)), Some(d(23)), Some(at(23, 0, 0)));
    let next = task(None, None, Some(at(30, 0, 0)));
    let old = task(Some(d(21)), Some(d(21)), Some(at(22, 23, 59)));
    let snapshot = Snapshot {
        task_created: [(first.id, at(23, 0, 0)), (next.id, at(30, 0, 0)), (old.id, at(22, 23, 59))].into(),
        tasks: vec![
            behind,
            first,
            next,
            old,
            task(Some(d(22)), Some(d(22)), None),
            task(Some(d(27)), Some(d(27)), None),
        ],
        ..Snapshot::default()
    };
    let review = Review::build(&snapshot, d(27), &local(), at(27, 14, 0));
    assert_eq!((review.tasks.completed, review.tasks.created), (1, 1));
    assert_eq!((review.previous_tasks.completed, review.previous_tasks.created), (1, 1));
    assert_eq!(review.carried_over, [Since { task_id: behind_id, day: d(23) }]);
    assert_eq!(review.overdue, [Since { task_id: behind_id, day: d(24) }]);
    let previous = Review::build(&snapshot, offset_week(d(27), -1).unwrap(), &local(), at(27, 14, 0));
    assert_eq!(previous.tasks, review.previous_tasks);
    assert!(previous.carried_over.is_empty(), "Sunday is not past at the previous week's cutoff");
    assert_eq!(offset_week(d(27), 0), Some(d(23)));
    assert_eq!(offset_week(d(27), 1), Some(d(30)));
    assert_eq!(offset_week(d(27), i32::MAX), None);
}

#[test]
fn history_only_joins_adjacent_changes_of_the_same_field_and_ranks_top_three() {
    let tasks: Vec<_> = (0..4).map(|_| task(None, None, None)).collect();
    let ids: Vec<_> = tasks.iter().map(|t| t.id).collect();
    let p = |index: usize, field, from, to, at| Postponement {
        id: EntityId::new(),
        task_id: ids[index],
        field,
        from: d(from),
        to: d(to),
        at,
    };
    let snapshot = Snapshot {
        tasks,
        postponements: vec![
            p(0, DateField::Plan, 18, 19, at(22, 23, 59)),
            p(0, DateField::Plan, 19, 23, at(23, 0, 0)),
            p(1, DateField::Plan, 19, 23, at(23, 0, 0)),
            p(2, DateField::Plan, 19, 23, at(24, 0, 0)),
            p(3, DateField::Plan, 19, 23, at(25, 0, 0)),
            p(0, DateField::Due, 23, 25, at(25, 1, 0)),
            p(0, DateField::Plan, 23, 24, at(25, 2, 0)),
            p(0, DateField::Plan, 26, 27, at(27, 0, 0)),
            p(0, DateField::Plan, 27, 30, at(30, 0, 0)),
        ],
        ..Snapshot::default()
    };
    let review = Review::build(&snapshot, d(27), &local(), at(27, 14, 0));
    assert_eq!(review.tasks.postponements, 7);
    assert_eq!(review.previous_tasks.postponements, 1);
    assert_eq!(review.most_postponed.iter().map(|item| item.task_id).collect::<Vec<_>>(), [ids[0], ids[3], ids[2]]);
    assert_eq!(
        review.most_postponed[0],
        Postponed {
            task_id: ids[0],
            count: 4,
            plan: vec![vec![d(19), d(23), d(24)], vec![d(26), d(27)]],
            deadline: vec![vec![d(23), d(25)]],
        }
    );
}

#[test]
fn daily_time_splits_at_local_midnight_and_shares_use_all_tracked_time() {
    let task = task(None, None, None);
    let quest_id = EntityId::new();
    let entry = |kind, quest_id, started_at, ended_at| TimeEntry {
        id: EntityId::new(),
        kind,
        task_id: Some(task.id),
        quest_id,
        origin_task_id: None,
        rebooked_from: None,
        started_at,
        ended_at,
    };
    let snapshot = Snapshot {
        time_entries: vec![
            entry(EntryKind::Task, Some(quest_id), at(22, 23, 0), Some(at(23, 1, 0))),
            entry(EntryKind::Organizing, None, at(23, 23, 30), Some(at(24, 0, 30))),
            entry(EntryKind::Task, None, at(24, 1, 0), None),
            entry(EntryKind::Task, None, at(30, 0, 0), Some(at(30, 1, 0))),
        ],
        tasks: vec![task],
        quests: vec![Quest {
            id: quest_id,
            title: "Quest".into(),
            color: QuestColor::Red,
            status: QuestStatus::Active,
            sort_key: String::new(),
        }],
        ..Snapshot::default()
    };
    let review = Review::build(&snapshot, d(24), &local(), at(24, 2, 0));
    let hour = 3_600_000;
    assert_eq!(review.time.total_ms_per_day, [hour + hour / 2, hour + hour / 2, 0, 0, 0, 0, 0]);
    assert_eq!(review.time.quest_ms, hour);
    assert_eq!(review.previous_quest_ms, hour);
    assert_eq!(review.time.target_ms, i64::from(snapshot.settings.weekly_target_minutes) * 60_000);
    assert_eq!(
        review.shares,
        [Bucket::Quest(quest_id), Bucket::Organizing, Bucket::NoQuest].map(|bucket| Share {
            bucket,
            ms: hour,
            fraction: 1.0 / 3.0
        })
    );
}
