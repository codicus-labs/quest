//! The task sheet, mapped from the snapshot as the desktop's task dialog shows it. Pure.

use std::cmp::Reverse;

use chrono::{DateTime, Utc};
use quest_domain::{Snapshot, Task, TimeEntry};
use quest_view::entry_editor::duration_label;
use quest_view::log_work::date_label;
use quest_view::repeat;

use crate::home::{chip, quest_options};
use crate::ids::iso_day;
use crate::task_types::{DayValue, LoggedEntry, TaskDetail};
use crate::time::Now;

/// How many sessions the sheet lists, as the desktop's task dialog.
const LOGGED_SHOWN: usize = 4;

pub(crate) fn task_detail(snapshot: &Snapshot, task: &Task, now: Now) -> TaskDetail {
    let mut entries: Vec<&TimeEntry> = snapshot.time_entries.iter().filter(|e| e.task_id == Some(task.id)).collect();
    entries.sort_by_key(|e| Reverse(e.started_at));
    TaskDetail {
        id: task.id.to_string(),
        title: task.title.clone(),
        description: task.description.clone(),
        done: task.is_done(),
        quest: task.quest_id.and_then(|id| snapshot.quest(id)).map(chip),
        quest_options: quest_options(snapshot, now.at),
        date: task.day.map(day_value),
        repeat_label: repeat::summary(snapshot, task),
        repeats: repeat::series_of(snapshot, task).repeat.is_some(),
        logged_total: format!("{} total", duration_label(snapshot.task_tracked_ms(task.id, now.at))),
        logged: entries.into_iter().take(LOGGED_SHOWN).map(|e| logged_entry(e, now)).collect(),
    }
}

pub(crate) fn day_value(day: chrono::NaiveDate) -> DayValue {
    DayValue { iso: iso_day(day), label: date_label(day) }
}

/// "Nov 23 · 09:00–10:30" and its duration, in the caller's time zone.
fn logged_entry(entry: &TimeEntry, now: Now) -> LoggedEntry {
    let local = |at: DateTime<Utc>| at.with_timezone(&now.tz);
    let start = local(entry.started_at);
    let end = entry.ended_at.map_or("now".to_owned(), |end| local(end).format("%H:%M").to_string());
    LoggedEntry {
        id: entry.id.to_string(),
        when: format!("{} · {}–{end}", start.format("%b %-d"), start.format("%H:%M")),
        duration: duration_label(entry.tracked_ms(now.at)),
        running: entry.is_running(),
    }
}

#[cfg(test)]
#[path = "task_detail_tests.rs"]
mod tests;
