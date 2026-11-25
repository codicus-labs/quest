#[cfg(test)]
mod tests;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Timelike, Utc};
use quest_domain::TimeEntry;

use crate::log_work::{parse_clock, parse_day};
/// What the form holds when Save is pressed. A running entry has no END fields.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft<'a> {
    pub start_date: &'a str,
    pub start: &'a str,
    pub end: Option<EndFields<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EndFields<'a> {
    pub date: &'a str,
    pub time: &'a str,
}

/// The field a refused save points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldError {
    StartDate,
    Start,
    EndDate,
    End,
    /// The end is not after the start.
    EndNotAfterStart,
}

impl FieldError {
    pub fn message(self) -> &'static str {
        match self {
            FieldError::StartDate | FieldError::EndDate => "Enter a date like Nov 23, 2026.",
            FieldError::Start => "Enter a start time like 14:50.",
            FieldError::End => "Enter an end time like 15:05.",
            FieldError::EndNotAfterStart => "The end must be after the start.",
        }
    }
}

/// When the entry runs; no end while it is still running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Times {
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

/// The times the form names. `to_utc` reads a local wall-clock time.
pub fn times(
    draft: &Draft,
    entry: &TimeEntry,
    today: NaiveDate,
    to_utc: impl Fn(NaiveDateTime) -> DateTime<Utc>,
) -> Result<Times, FieldError> {
    let start_day = parse_day(draft.start_date, today).ok_or(FieldError::StartDate)?;
    let start = parse_clock(draft.start).ok_or(FieldError::Start)?;
    let keep = |shown: DateTime<Utc>, stored: DateTime<Utc>| if to_minute(stored) == shown { stored } else { shown };
    let started_at = keep(to_utc(start_day.and_time(start)), entry.started_at);
    let Some(end) = draft.end else { return Ok(Times { started_at, ended_at: None }) };
    let end_day = parse_day(end.date, today).ok_or(FieldError::EndDate)?;
    let end_time = parse_clock(end.time).ok_or(FieldError::End)?;
    let shown = to_utc(end_day.and_time(end_time));
    let ended_at = entry.ended_at.map_or(shown, |stored| keep(shown, stored));
    if ended_at <= started_at {
        return Err(FieldError::EndNotAfterStart);
    }
    Ok(Times { started_at, ended_at: Some(ended_at) })
}

fn to_minute(at: DateTime<Utc>) -> DateTime<Utc> {
    at.with_second(0).and_then(|t| t.with_nanosecond(0)).unwrap_or(at)
}

/// "1h 15m", "45m", "2h": how long the entry runs.
pub fn duration_label(ms: i64) -> String {
    let minutes = ms.max(0) / 60_000;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}
