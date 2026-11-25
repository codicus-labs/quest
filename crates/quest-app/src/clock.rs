//! The app's clock and calendar: the only place that reads the system time or time zone.
//! The domain takes time as arguments.

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};

#[derive(Clone, Copy, Debug)]
pub enum Clock {
    System,
    /// Frozen at one instant (`--now`), so snapshots are reproducible.
    Fixed(DateTime<Utc>),
}

impl Clock {
    pub fn from_local(now: Option<NaiveDateTime>) -> Clock {
        match now {
            Some(local) => Clock::Fixed(to_utc(local)),
            None => Clock::System,
        }
    }

    pub fn now(&self) -> DateTime<Utc> {
        match self {
            Clock::System => Utc::now(),
            Clock::Fixed(at) => *at,
        }
    }
}

/// A local wall-clock time as UTC (the earlier one in a DST fold, the shifted one in a gap).
pub fn to_utc(local: NaiveDateTime) -> DateTime<Utc> {
    Local.from_local_datetime(&local).earliest().map_or_else(|| local.and_utc(), |t| t.with_timezone(&Utc))
}

pub fn local(at: DateTime<Utc>) -> NaiveDateTime {
    at.with_timezone(&Local).naive_local()
}

pub fn local_day(at: DateTime<Utc>) -> NaiveDate {
    local(at).date()
}

pub fn at_local(day: NaiveDate, time: NaiveTime) -> DateTime<Utc> {
    to_utc(day.and_time(time))
}

pub fn midnight(day: NaiveDate) -> DateTime<Utc> {
    at_local(day, NaiveTime::MIN)
}

/// How far past a deadline a wake asks to land: a poll that comes a hair early finds nothing
/// due and would ask again for almost nothing, frame after frame.
const WAKE_SLACK: std::time::Duration = std::time::Duration::from_millis(5);

/// How long to sleep from `now` until `at` has passed.
pub fn wake_for(now: DateTime<Utc>, at: DateTime<Utc>) -> std::time::Duration {
    (at - now).to_std().unwrap_or_default() + WAKE_SLACK
}

/// The next whole multiple of `seconds` after `now` (the next second, the next minute).
pub fn next_whole(now: DateTime<Utc>, seconds: i64) -> DateTime<Utc> {
    let next = (now.timestamp().div_euclid(seconds) + 1) * seconds;
    DateTime::from_timestamp(next, 0).unwrap_or(now)
}
