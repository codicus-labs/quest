use chrono::{NaiveDate, TimeZone, Utc};

use super::*;
use crate::change::EntityId;

fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
}

fn on(day: Option<u32>) -> Option<NaiveDate> {
    day.map(d)
}

fn dates(plan: Option<u32>, due: Option<u32>) -> TaskDates {
    TaskDates { planned_on: plan.map(d), due_on: due.map(d) }
}

fn task(plan: Option<u32>, due: Option<u32>) -> Task {
    Task {
        id: EntityId::new(),
        title: "t".into(),
        quest_id: None,
        planned_on: plan.map(d),
        due_on: due.map(d),
        sort_key: String::new(),
        completed_at: None,
        description: String::new(),
        repeat: None,
        day_order: String::new(),
        series: None,
    }
}

#[test]
fn planning_past_the_deadline_takes_the_deadline_along() {
    assert_eq!(set_planned(dates(Some(10), Some(12)), on(Some(14))), dates(Some(14), Some(14)));
    assert_eq!(set_planned(dates(Some(10), Some(12)), on(Some(11))), dates(Some(11), Some(12)));
    assert_eq!(set_planned(dates(None, Some(12)), on(Some(12))), dates(Some(12), Some(12)));
    assert_eq!(set_planned(dates(Some(10), Some(12)), None), dates(None, Some(12)));
}

#[test]
fn a_deadline_before_the_plan_date_takes_the_plan_date_along() {
    assert_eq!(set_due(dates(Some(14), Some(20)), on(Some(12))), dates(Some(12), Some(12)));
    assert_eq!(set_due(dates(Some(14), None), on(Some(16))), dates(Some(14), Some(16)));
    assert_eq!(set_due(dates(Some(14), Some(16)), None), dates(Some(14), None));
    assert_eq!(set_due(dates(None, None), on(Some(3))), dates(None, Some(3)));
}

#[test]
fn only_dates_moved_later_count_as_postponed() {
    // First time, earlier, removed: none.
    assert!(postponed_fields(dates(None, None), dates(Some(12), Some(14))).is_empty());
    assert!(postponed_fields(dates(Some(12), Some(14)), dates(Some(11), Some(13))).is_empty());
    assert!(postponed_fields(dates(Some(12), Some(14)), dates(None, None)).is_empty());
    // Later.
    assert_eq!(postponed_fields(dates(Some(12), None), dates(Some(13), None)), [DateField::Plan]);
    // Plan past the deadline: the deadline taken along counts too.
    let before = dates(Some(12), Some(14));
    assert_eq!(postponed_fields(before, set_planned(before, on(Some(16)))), [DateField::Plan, DateField::Due]);
    // Deadline moved earlier takes the plan date along: earlier, so none.
    assert!(postponed_fields(before, set_due(before, on(Some(11)))).is_empty());
    // An overdue deadline moved to a later day.
    assert_eq!(postponed_fields(dates(None, Some(2)), dates(None, Some(25))), [DateField::Due]);
    // A carried-over task planned for today.
    assert_eq!(postponed_fields(dates(Some(20), None), dates(Some(25), None)), [DateField::Plan]);
}

#[test]
fn today_holds_open_tasks_planned_or_due_until_today() {
    let today = d(25);
    let overdue_task = task(None, Some(24));
    let carried = task(Some(23), None);
    let due_today = task(None, Some(25));
    let planned_today = task(Some(25), Some(28));
    let later = task(Some(26), None);
    assert!([&overdue_task, &carried, &due_today, &planned_today].iter().all(|t| is_today(t, today)));
    assert!(!is_today(&later, today));
    assert!(overdue(&overdue_task, today) && !overdue(&due_today, today));
    assert!(carried_over(&carried, today) && !carried_over(&planned_today, today));
    let mut done = task(Some(20), Some(21));
    done.completed_at = Some(Utc.with_ymd_and_hms(2026, 11, 21, 9, 0, 0).unwrap());
    assert!(!is_today(&done, today) && !overdue(&done, today) && !carried_over(&done, today));
}

#[test]
fn the_inbox_holds_open_tasks_without_quest_and_plan_date() {
    assert!(is_inbox(&task(None, None)));
    assert!(is_inbox(&task(None, Some(30))));
    assert!(!is_inbox(&task(Some(30), None)));
    let mut filed = task(None, None);
    filed.quest_id = Some(EntityId::new());
    assert!(!is_inbox(&filed));
}
