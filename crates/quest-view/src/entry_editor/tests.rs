use chrono::NaiveDate;
use quest_domain::{EntityId, EntryKind};

use super::*;

fn at(day: u32, h: u32, m: u32, s: u32) -> DateTime<Utc> {
    NaiveDate::from_ymd_opt(2026, 11, day).unwrap().and_hms_opt(h, m, s).unwrap().and_utc()
}

fn entry() -> TimeEntry {
    TimeEntry {
        id: EntityId::new(),
        kind: EntryKind::Task,
        origin_task_id: None,
        rebooked_from: None,
        task_id: None,
        quest_id: None,
        started_at: at(23, 14, 50, 20),
        ended_at: Some(at(23, 15, 5, 40)),
    }
}

fn parse(e: &TimeEntry, start: &str, end_date: &str, end: &str) -> Result<Times, FieldError> {
    let draft = Draft { start_date: "Nov 23, 2026", start, end: Some(EndFields { date: end_date, time: end }) };
    times(&draft, e, e.started_at.date_naive(), |time| time.and_utc())
}

#[test]
fn editable_minutes_keep_unmodified_seconds() {
    let e = entry();
    let new = parse(&e, "14:00", "Nov 23, 2026", "15:05").unwrap();
    assert_eq!(new.started_at, at(23, 14, 0, 0));
    assert_eq!(new.ended_at, e.ended_at);
}

#[test]
fn an_unmodified_entry_shorter_than_one_minute_stays_editable() {
    let mut e = entry();
    e.ended_at = Some(at(23, 14, 50, 40));
    let parsed = parse(&e, "14:50", "Nov 23, 2026", "14:50").unwrap();
    assert_eq!((parsed.started_at, parsed.ended_at), (e.started_at, e.ended_at));
}

#[test]
fn an_end_before_the_start_or_an_unreadable_field_is_refused() {
    let e = entry();
    assert_eq!(parse(&e, "16:00", "Nov 23, 2026", "15:05"), Err(FieldError::EndNotAfterStart));
    assert_eq!(parse(&e, "25:00", "Nov 23, 2026", "15:05"), Err(FieldError::Start));
    assert_eq!(parse(&e, "14:00", "someday", "15:05"), Err(FieldError::EndDate));
    assert_eq!(parse(&e, "14:00", "Nov 23, 2026", "x"), Err(FieldError::End));
}

#[test]
fn an_end_on_the_next_day_crosses_midnight() {
    let t = parse(&entry(), "22:30", "Nov 24, 2026", "01:15").unwrap();
    assert_eq!(duration_label((t.ended_at.unwrap() - t.started_at).num_milliseconds()), "2h 45m");
}

#[test]
fn durations_read_in_hours_and_minutes() {
    assert_eq!(duration_label(75 * 60_000), "1h 15m");
    assert_eq!(duration_label(45 * 60_000), "45m");
    assert_eq!(duration_label(120 * 60_000), "2h");
    assert_eq!(duration_label(-5), "0m");
}
