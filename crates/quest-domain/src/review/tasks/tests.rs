use chrono::{NaiveDate, TimeZone, Utc};

use super::*;
use crate::command::{Command, Position};
use crate::test_support::Replica;

fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
}

fn at(day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, day, hour, 0, 0).unwrap()
}

fn task(r: &mut Replica, planned_on: Option<NaiveDate>, due_on: Option<NaiveDate>) -> EntityId {
    let id = EntityId::new();
    r.run(Command::CreateTask {
        id,
        title: "t".into(),
        quest_id: None,
        planned_on,
        due_on,
        position: Position::Bottom,
    });
    id
}

#[test]
fn a_week_counts_postponements_lingering_done_and_new_tasks() {
    // The replica's clock starts at the epoch, so every task counts as created long ago.
    let mut r = Replica::new();
    let often = task(&mut r, Some(d(23)), Some(d(24)));
    let once = task(&mut r, Some(d(24)), None);
    let done = task(&mut r, Some(d(23)), None);
    let move_to = |id, day, at| Command::MoveTask { id, planned_on: Some(d(day)), position: Position::Bottom, at };
    r.run(move_to(often, 20, at(17, 9))); // the week before: earlier, nothing
    r.run(move_to(often, 25, at(24, 9))); // plan and deadline
    r.run(Command::SetTaskDue { id: often, due_on: Some(d(27)), today: d(25), at: at(25, 9) });
    r.run(move_to(once, 25, at(23, 9)));
    r.run(Command::CompleteTask { id: done, at: at(24, 12) });

    // Wednesday 25th: `often` and `once` are planned for today, nothing lingers yet.
    let week = WeekTasks::build(&r.snapshot, d(25), &Utc, at(25, 18));
    assert_eq!(week.week_start, d(23));
    assert_eq!(week.postponements, 4);
    let most: Vec<(EntityId, usize, usize)> = week.most_postponed.iter().map(|p| (p.task_id, p.plan, p.due)).collect();
    assert_eq!(most, [(often, 1, 2), (once, 1, 0)]);
    assert_eq!(week.most_postponed[0].last_at, at(25, 9));
    assert!(week.carried_over.is_empty() && week.overdue.is_empty());
    assert_eq!((week.completed, week.created), (1, 0));

    // Looking back from the next week: as of Sunday 29th.
    let later = WeekTasks::build(&r.snapshot, d(23), &Utc, at(30, 9));
    let carried: Vec<i64> = later.carried_over.iter().map(|l| l.days).collect();
    assert_eq!(carried, [4, 4]);
    assert_eq!(later.overdue, [Lingering { task_id: often, days: 2 }]);
}
