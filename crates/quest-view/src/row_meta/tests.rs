use chrono::{NaiveDate, TimeZone, Utc};
use quest_domain::{EntityId, Task};

use super::*;

fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
}

fn task(plan: Option<u32>, due: Option<u32>) -> Task {
    Task {
        id: EntityId::new(),
        title: "T".into(),
        quest_id: None,
        planned_on: plan.map(d),
        due_on: due.map(d),
        sort_key: "a".into(),
        completed_at: None,
        description: String::new(),
        repeat: None,
        day_order: String::new(),
        series: None,
    }
}

fn texts(marks: Vec<DateMark>) -> Vec<(String, Tone)> {
    marks.into_iter().map(|m| (m.text, m.tone)).collect()
}

// Wednesday, November 25.
const TODAY: u32 = 25;

#[test]
fn overdue_reads_red_with_its_day() {
    let marks = texts(date_marks(&task(Some(23), Some(24)), d(TODAY), true));
    assert_eq!(marks, [("since Mon 23".into(), Tone::Muted), ("overdue · Tue 24".into(), Tone::Overdue)]);
}

#[test]
fn carried_over_and_due_today_in_the_accent() {
    let marks = texts(date_marks(&task(Some(24), Some(25)), d(TODAY), false));
    assert_eq!(marks, [("since Tue 24".into(), Tone::Muted), ("due today".into(), Tone::Accent)]);
}

#[test]
fn future_deadlines_name_the_weekday_then_the_date() {
    assert_eq!(texts(date_marks(&task(None, Some(27)), d(TODAY), false)), [("due Fri".into(), Tone::Muted)]);
    assert_eq!(texts(date_marks(&task(None, Some(26)), d(TODAY), false)), [("due tomorrow".into(), Tone::Muted)]);
    assert_eq!(texts(date_marks(&task(None, Some(30)), d(TODAY), false)), [("due Mon".into(), Tone::Muted)]);
    let far = NaiveDate::from_ymd_opt(2026, 12, 3).unwrap();
    assert_eq!(day_name(far, d(TODAY)), "3 Dec");
}

#[test]
fn a_plan_ahead_shows_only_where_asked() {
    assert_eq!(texts(date_marks(&task(Some(26), None), d(TODAY), true)), [("Tomorrow".into(), Tone::Muted)]);
    assert!(date_marks(&task(Some(26), None), d(TODAY), false).is_empty());
    assert!(date_marks(&task(Some(25), None), d(TODAY), true).is_empty(), "planned today needs no mark");
}

#[test]
fn done_tasks_show_no_dates() {
    let mut done = task(Some(23), Some(24));
    done.completed_at = Some(Utc.with_ymd_and_hms(2026, 11, 25, 9, 0, 0).unwrap());
    assert!(date_marks(&done, d(TODAY), true).is_empty());
}
