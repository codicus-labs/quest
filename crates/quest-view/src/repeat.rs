//! The Repeat dialog's decisions, pure: which series a task's repeat belongs to, how a rule
//! reads ("Every 2 weeks"), the weekdays it shows, and what saving its fields means (`rule`:
//! the rule, or the message to show).

#[cfg(test)]
mod tests;

use chrono::{Datelike, NaiveDate};
use chrono_tz::Tz;
use quest_domain::{Frequency, MonthDay, RepeatPin, RepeatRule, Snapshot, Task, Weekdays};

use crate::log_work::parse_day;

/// The dialog's fields as typed or picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepeatForm<'a> {
    pub every: &'a str,
    pub frequency: Frequency,
    pub pin: RepeatPin,
    /// Empty: the start's weekday.
    pub weekdays: Weekdays,
    pub last_day: bool,
    pub month_day: &'a str,
    pub starts: &'a str,
    pub ends_on_date: bool,
    pub ends: &'a str,
    pub time_zone: &'a str,
}

impl RepeatForm<'_> {
    /// The weekdays the dialog shows as picked: the start's weekday until any is picked.
    pub fn shown_weekdays(&self, today: NaiveDate) -> Weekdays {
        if self.weekdays.is_empty() { Weekdays::on(self.start_or(today).weekday()) } else { self.weekdays }
    }

    /// The month day field's placeholder: the start's day of the month.
    pub fn default_month_day(&self, today: NaiveDate) -> String {
        self.start_or(today).day().to_string()
    }

    fn start_or(&self, today: NaiveDate) -> NaiveDate {
        parse_day(self.starts, today).unwrap_or(today)
    }

    /// The rule the fields describe, or the message for the first field that is wrong.
    pub fn rule(&self, today: NaiveDate) -> Result<RepeatRule, &'static str> {
        let every = parse_every(self.every).ok_or("Enter an interval from 1 to 999.")?;
        let starts = parse_day(self.starts, today).ok_or("Enter a start date.")?;
        let ends = if self.ends_on_date {
            let end = parse_day(self.ends, today).ok_or("Enter an end date.")?;
            if end < starts {
                return Err("The end date must be on or after the start.");
            }
            Some(end)
        } else {
            None
        };
        let time_zone =
            self.time_zone.trim().parse::<Tz>().map_err(|_| "Enter a timezone such as Europe/Berlin or UTC.")?;
        let month_day = if self.frequency != Frequency::Month {
            None
        } else if self.last_day {
            Some(MonthDay::Last)
        } else if self.month_day.trim().is_empty() {
            None
        } else {
            let day = self
                .month_day
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|day| (1..=31).contains(day))
                .ok_or("Enter a day of the month from 1 to 31.")?;
            Some(MonthDay::Day(day))
        };
        let weekdays = if self.frequency == Frequency::Week { self.weekdays } else { Weekdays::default() };
        Ok(RepeatRule { every, frequency: self.frequency, starts, ends, time_zone, pin: self.pin, weekdays, month_day })
    }
}

/// The interval field: a whole number from 1 to 999.
pub fn parse_every(text: &str) -> Option<u32> {
    text.trim().parse::<u32>().ok().filter(|n| (1..=999).contains(n))
}

/// The picked weekdays after a press on `day`'s chip, as the dialog shows them (`shown`). The
/// last shown day cannot be unpicked: then `picked` stays as it is (empty still means the start's
/// weekday).
pub fn toggle_weekday(picked: Weekdays, shown: Weekdays, day: chrono::Weekday) -> Weekdays {
    let days = shown.toggled(day);
    if days.is_empty() { picked } else { days }
}

/// What a repeat is set on: an occurrence edits its series (while that still repeats), any
/// other task itself.
pub fn series_of<'a>(snapshot: &'a Snapshot, task: &'a Task) -> &'a Task {
    task.series
        .filter(|_| task.repeat.is_none())
        .and_then(|id| snapshot.task(id))
        .filter(|series| series.repeat.is_some())
        .unwrap_or(task)
}

pub fn unit(frequency: Frequency) -> &'static str {
    match frequency {
        Frequency::Day => "day",
        Frequency::Week => "week",
        Frequency::Month => "month",
        Frequency::Year => "year",
    }
}

/// "Every day", "Every 2 weeks".
pub fn cadence(every: u32, frequency: Frequency) -> String {
    if every == 1 { format!("Every {}", unit(frequency)) } else { format!("Every {every} {}s", unit(frequency)) }
}

/// What a task's repeat row says: its series' cadence, or "Does not repeat".
pub fn summary(snapshot: &Snapshot, task: &Task) -> String {
    series_of(snapshot, task).repeat.map_or("Does not repeat".to_owned(), |s| cadence(s.rule.every, s.rule.frequency))
}
