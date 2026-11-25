//! How numbers and dates read on screen. Pure.

use chrono::{Datelike, NaiveDate};

/// Hours with one decimal, dropping a trailing ".0": 3.6h, 18h, 0h.
pub fn hours(ms: i64) -> String {
    let tenths = (ms.max(0) as f64 / 360_000.0).round() as i64;
    if tenths % 10 == 0 { format!("{}h", tenths / 10) } else { format!("{}.{}h", tenths / 10, tenths % 10) }
}

/// A timer: 36:00 under an hour, 1:30:00 from an hour on.
pub fn timer(ms: i64) -> String {
    let secs = ms.max(0) / 1000;
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

/// "Thursday, October 15".
pub fn long_day(day: NaiveDate) -> String {
    day.format("%A, %B %-d").to_string()
}

/// "Oct 12 – 18", or "Sep 28 – Oct 4" across months.
pub fn week_range(monday: NaiveDate) -> String {
    let sunday = monday + chrono::Days::new(6);
    if monday.month() == sunday.month() {
        format!("{} – {}", monday.format("%b %-d"), sunday.day())
    } else {
        format!("{} – {}", monday.format("%b %-d"), sunday.format("%b %-d"))
    }
}

pub fn hh_mm(minutes_of_day: u32) -> String {
    format!("{:02}:{:02}", minutes_of_day / 60, minutes_of_day % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hours_round_to_tenths() {
        assert_eq!(hours(0), "0h");
        assert_eq!(hours(18 * 3_600_000), "18h");
        assert_eq!(hours(3 * 3_600_000 + 36 * 60_000), "3.6h");
    }

    #[test]
    fn timers_grow_an_hour_field() {
        assert_eq!(timer(36 * 60_000), "36:00");
        assert_eq!(timer(90 * 60_000), "1:30:00");
        assert_eq!(timer(-5), "00:00");
    }

    #[test]
    fn dates_read_like_the_reference() {
        let thursday = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        assert_eq!(long_day(thursday), "Thursday, October 15");
        assert_eq!(week_range(NaiveDate::from_ymd_opt(2026, 10, 12).unwrap()), "Oct 12 – 18");
        assert_eq!(week_range(NaiveDate::from_ymd_opt(2026, 9, 28).unwrap()), "Sep 28 – Oct 4");
        assert_eq!(hh_mm(9 * 60 + 5), "09:05");
    }
}
