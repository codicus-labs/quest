//! The Log work dialog's decisions, pure: which tasks the TASK field offers (done ones too), the
//! quest the session counts towards, and what saving does (`decide`): one `LogWork` command, or
//! the field that is wrong.

#[cfg(test)]
mod tests;

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use quest_domain::{Command, EntityId, Snapshot, Task, WorkTask};

use crate::palette::{parse_date, parse_time};

/// How many tasks the TASK field suggests.
const MAX_SUGGESTIONS: usize = 5;

/// What the form holds when Log work is pressed.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft<'a> {
    /// The existing task picked from the suggestions, if any.
    pub task: Option<EntityId>,
    /// The TASK field: the picked task's title, or a new task's.
    pub title: &'a str,
    pub date: &'a str,
    pub start: &'a str,
    pub end: &'a str,
    /// The quest for a new task (a picked task brings its own).
    pub quest: Option<EntityId>,
}

/// The field a refused save points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldError {
    /// No task picked and no title typed.
    Task,
    Date,
    Start,
    End,
    /// The end is not after the start.
    EndNotAfterStart,
}

impl FieldError {
    pub fn message(self) -> &'static str {
        match self {
            FieldError::Task => "Choose a task or type a title for a new one.",
            FieldError::Date => "Enter a date like Nov 23, 2026.",
            FieldError::Start => "Enter a start time like 14:50.",
            FieldError::End => "Enter an end time like 15:05.",
            FieldError::EndNotAfterStart => "The end must be after the start.",
        }
    }
}

/// "Nov 23, 2026": how the DATE field shows a day.
pub fn date_label(day: NaiveDate) -> String {
    day.format("%b %-d, %Y").to_string()
}

/// "14:50".
pub fn time_label(time: NaiveTime) -> String {
    time.format("%H:%M").to_string()
}

pub fn parse_day(text: &str, today: NaiveDate) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text.trim(), "%b %d, %Y").ok().or_else(|| parse_date(text, today))
}

pub fn parse_clock(text: &str) -> Option<NaiveTime> {
    let minutes = parse_time(text.trim())?;
    NaiveTime::from_hms_opt((minutes / 60) as u32, (minutes % 60) as u32, 0)
}

/// Tasks whose title contains `query`, open ones first, then done ones (most recently done
/// first). Nothing for an empty query.
pub fn suggestions<'a>(snapshot: &'a Snapshot, query: &str) -> Vec<&'a Task> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<&Task> = snapshot.tasks.iter().filter(|t| t.title.to_lowercase().contains(&query)).collect();
    hits.sort_by_key(|t| (t.is_done(), std::cmp::Reverse(t.completed_at)));
    hits.truncate(MAX_SUGGESTIONS);
    hits
}

/// The quest the session will count towards: a picked task's own, else the one chosen.
pub fn quest(snapshot: &Snapshot, draft_task: Option<EntityId>, chosen: Option<EntityId>) -> Option<EntityId> {
    match draft_task.and_then(|id| snapshot.task(id)) {
        Some(task) => task.quest_id.filter(|q| snapshot.quest(*q).is_some()),
        None => chosen,
    }
}

/// The command that logs the draft, with ids for the new session (and task); `to_utc` reads a
/// local wall-clock time.
pub fn decide(
    draft: &Draft,
    today: NaiveDate,
    to_utc: impl Fn(NaiveDateTime) -> DateTime<Utc>,
    entry_id: EntityId,
    new_task_id: EntityId,
) -> Result<Command, FieldError> {
    let title = draft.title.trim();
    if draft.task.is_none() && title.is_empty() {
        return Err(FieldError::Task);
    }
    let day = parse_day(draft.date, today).ok_or(FieldError::Date)?;
    let start = parse_clock(draft.start).ok_or(FieldError::Start)?;
    let end = parse_clock(draft.end).ok_or(FieldError::End)?;
    if end <= start {
        return Err(FieldError::EndNotAfterStart);
    }
    let task = match draft.task {
        Some(id) => WorkTask::Existing(id),
        None => WorkTask::New { id: new_task_id, title: title.to_owned(), quest_id: draft.quest, planned_on: day },
    };
    Ok(Command::LogWork {
        entry_id,
        task,
        started_at: to_utc(day.and_time(start)),
        ended_at: to_utc(day.and_time(end)),
    })
}
