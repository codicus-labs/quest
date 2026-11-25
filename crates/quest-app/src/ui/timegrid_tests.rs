//! A session over midnight: a span on each day it touches, with its ends only where they lie.

use chrono::{NaiveDate, NaiveTime};
use quest_domain::{EntityId, TimeEntry};

use super::{Ends, ends_on, entry_span};
use crate::clock;
use crate::timescale::Span;

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
}

fn at(d: u32, h: u32, m: u32) -> chrono::DateTime<chrono::Utc> {
    clock::at_local(day(d), NaiveTime::from_hms_opt(h, m, 0).unwrap())
}

fn session(from: (u32, u32, u32), to: Option<(u32, u32, u32)>) -> TimeEntry {
    TimeEntry {
        id: EntityId::new(),
        task_id: None,
        quest_id: None,
        started_at: at(from.0, from.1, from.2),
        ended_at: to.map(|(d, h, m)| at(d, h, m)),
    }
}

#[test]
fn a_session_over_midnight_has_a_span_on_both_days() {
    let night = session((14, 22, 30), Some((15, 7, 45)));
    let now = at(15, 14, 40);
    assert_eq!(entry_span(&night, day(14), now), Some(Span::new(22 * 60 + 30, 24 * 60)));
    assert_eq!(entry_span(&night, day(15), now), Some(Span::new(0, 7 * 60 + 45)));
    assert_eq!(entry_span(&night, day(13), now), None);
    assert_eq!(entry_span(&night, day(16), now), None);
}

#[test]
fn each_segment_has_only_the_ends_that_lie_on_its_day() {
    let night = session((14, 22, 30), Some((15, 7, 45)));
    assert_eq!(ends_on(&night, day(14)), Ends { start: true, end: false });
    assert_eq!(ends_on(&night, day(15)), Ends { start: false, end: true });
    let ends_at_midnight = session((14, 22, 0), Some((15, 0, 0)));
    assert_eq!(ends_on(&ends_at_midnight, day(14)), Ends { start: true, end: true });
    assert_eq!(entry_span(&ends_at_midnight, day(15), at(15, 14, 40)), None, "nothing on the next day");
    let running = session((15, 9, 0), None);
    assert_eq!(ends_on(&running, day(15)), Ends { start: true, end: true });
}
