//! The dates a task row shows on its right, pure: "overdue · Mon 23", "since Tue 24", "due today",
//! "due Fri", and a plan date ahead ("Tomorrow", "Fri", "3 Dec"). Done tasks show none.

#[cfg(test)]
mod tests;

use chrono::{Datelike, NaiveDate};
use quest_domain::Task;

/// How a date mark reads: muted, in the accent (due today) or in red (overdue).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Muted,
    Accent,
    Overdue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DateMark {
    pub text: String,
    pub tone: Tone,
}

/// The row's date marks, left to right. `show_plan_ahead` adds a future plan date (not in lists
/// where every task is planned for today or earlier anyway).
pub fn date_marks(task: &Task, today: NaiveDate, show_plan_ahead: bool) -> Vec<DateMark> {
    if task.is_done() {
        return Vec::new();
    }
    let mut marks = Vec::new();
    match task.planned_on {
        Some(plan) if plan < today => {
            marks.push(DateMark { text: format!("since {}", past_day(plan)), tone: Tone::Muted })
        }
        Some(plan) if plan > today && show_plan_ahead => {
            marks.push(DateMark { text: day_name(plan, today), tone: Tone::Muted })
        }
        _ => {}
    }
    if let Some(due) = task.due_on {
        marks.push(due_mark(due, today));
    }
    marks
}

fn due_mark(due: NaiveDate, today: NaiveDate) -> DateMark {
    if due < today {
        return DateMark { text: format!("overdue · {}", past_day(due)), tone: Tone::Overdue };
    }
    if due == today {
        return DateMark { text: "due today".to_owned(), tone: Tone::Accent };
    }
    let name = if (due - today).num_days() == 1 { "tomorrow".to_owned() } else { day_name(due, today) };
    DateMark { text: format!("due {name}"), tone: Tone::Muted }
}

/// "Mon 23": a past day, by weekday and day of month.
pub fn past_day(day: NaiveDate) -> String {
    format!("{} {}", day.format("%a"), day.day())
}

/// "Today", "Tomorrow", "Yesterday", the weekday within a week ("Fri"), else "3 Dec".
pub fn day_name(day: NaiveDate, today: NaiveDate) -> String {
    match (day - today).num_days() {
        0 => "Today".to_owned(),
        1 => "Tomorrow".to_owned(),
        -1 => "Yesterday".to_owned(),
        n if n.abs() < 7 => day.format("%a").to_string(),
        _ => format!("{} {}", day.day(), day.format("%b")),
    }
}
