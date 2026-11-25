use chrono::{TimeZone, Utc};

use super::*;
use crate::dates::DateField;
use crate::test_support::Replica;

fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
}

fn at(hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 25, hour, 0, 0).unwrap()
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

fn dates(r: &Replica, id: EntityId) -> (Option<NaiveDate>, Option<NaiveDate>) {
    let task = r.snapshot.task(id).unwrap();
    (task.planned_on, task.due_on)
}

fn postponed(r: &Replica, id: EntityId) -> Vec<(DateField, NaiveDate, NaiveDate)> {
    r.snapshot.postponements_of(id).map(|p| (p.field, p.from, p.to)).collect()
}

#[test]
fn creating_keeps_the_plan_date_on_or_before_the_deadline() {
    let mut r = Replica::new();
    let id = task(&mut r, Some(d(28)), Some(d(26)));
    assert_eq!(dates(&r, id), (Some(d(26)), Some(d(26))));
    assert!(r.snapshot.postponements.is_empty());
}

#[test]
fn moving_past_the_deadline_takes_it_along_and_records_both_postponements() {
    let mut r = Replica::new();
    let id = task(&mut r, Some(d(25)), Some(d(26)));
    r.run(Command::MoveTask { id, planned_on: Some(d(28)), position: Position::Bottom, at: at(9) });
    assert_eq!(dates(&r, id), (Some(d(28)), Some(d(28))));
    assert_eq!(postponed(&r, id), [(DateField::Plan, d(25), d(28)), (DateField::Due, d(26), d(28))]);
    assert!(r.snapshot.postponements.iter().all(|p| p.at == at(9)));

    // Earlier, removed or first set: nothing more.
    r.run(Command::MoveTask { id, planned_on: Some(d(27)), position: Position::Bottom, at: at(10) });
    r.run(Command::MoveTask { id, planned_on: None, position: Position::Bottom, at: at(11) });
    r.run(Command::MoveTask { id, planned_on: Some(d(30)), position: Position::Bottom, at: at(12) });
    assert_eq!(postponed(&r, id).len(), 3, "the deadline taken along to the 30th");
    assert_eq!(dates(&r, id), (Some(d(30)), Some(d(30))));
}

#[test]
fn a_deadline_before_the_plan_date_takes_it_along_to_that_days_bottom() {
    let mut r = Replica::new();
    let other = task(&mut r, Some(d(26)), None);
    let id = task(&mut r, Some(d(28)), None);
    r.run(Command::SetTaskDue { id, due_on: Some(d(26)), at: at(9) });
    assert_eq!(dates(&r, id), (Some(d(26)), Some(d(26))));
    assert!(r.snapshot.postponements.is_empty(), "set for the first time, plan date moved earlier");
    let order: Vec<EntityId> = r.snapshot.tasks_on(d(26)).map(|t| t.id).collect();
    assert_eq!(order, [other, id]);

    r.run(Command::SetTaskDue { id, due_on: Some(d(27)), at: at(10) });
    assert_eq!(postponed(&r, id), [(DateField::Due, d(26), d(27))]);
    r.run(Command::SetTaskDue { id, due_on: None, at: at(11) });
    assert_eq!(dates(&r, id), (Some(d(26)), None));
}

#[test]
fn planning_a_carried_over_task_for_today_counts_and_undo_deletes_the_postponement() {
    let mut r = Replica::new();
    let id = task(&mut r, Some(d(20)), None);
    r.run(Command::MoveTask { id, planned_on: Some(d(25)), position: Position::Top, at: at(9) });
    let postponement = r.snapshot.postponements[0].id;
    r.run(Command::MoveTask { id, planned_on: Some(d(20)), position: Position::Top, at: at(9) });
    r.run(Command::DeletePostponement { id: postponement, at: at(9) });
    assert!(r.snapshot.postponements.is_empty());
    assert_eq!(
        plan(&Command::DeletePostponement { id: postponement, at: at(10) }, &r.snapshot),
        Err(CommandError::UnknownPostponement(postponement))
    );
}

#[test]
fn postponing_offline_on_two_devices_keeps_both_postponements() {
    let mut a = Replica::on_node(1);
    let id = task(&mut a, Some(d(25)), None);
    let mut b = Replica::on_node(2);
    b.merge(&a);
    a.run(Command::MoveTask { id, planned_on: Some(d(26)), position: Position::Bottom, at: at(9) });
    b.run(Command::MoveTask { id, planned_on: Some(d(27)), position: Position::Bottom, at: at(10) });
    a.merge(&b);
    b.merge(&a);
    assert_eq!(a.snapshot.postponements, b.snapshot.postponements);
    assert_eq!(a.snapshot.postponements.len(), 2, "a counter would have lost one");
}
