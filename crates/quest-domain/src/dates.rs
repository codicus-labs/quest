//! A task's two dates and the lists they put it in. Pure: today is passed in.
//!
//! - The plan date (`planned_on`) is when the user wants to work on a task, the deadline
//!   (`due_on`) when it has to be done. The plan date is never after the deadline.
//! - Only the user moves dates (through commands). The system never does: an unfinished task
//!   stays carried over, a missed deadline stays overdue.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::model::Task;

/// Which of a task's dates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateField {
    Plan,
    Due,
}

/// A task's plan date and deadline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TaskDates {
    pub planned_on: Option<NaiveDate>,
    pub due_on: Option<NaiveDate>,
}

impl TaskDates {
    pub fn get(self, field: DateField) -> Option<NaiveDate> {
        match field {
            DateField::Plan => self.planned_on,
            DateField::Due => self.due_on,
        }
    }
}

/// Sets the plan date. Planning past the deadline takes the deadline along.
pub fn set_planned(dates: TaskDates, planned_on: Option<NaiveDate>) -> TaskDates {
    let due_on = match (planned_on, dates.due_on) {
        (Some(plan), Some(due)) if due < plan => Some(plan),
        _ => dates.due_on,
    };
    TaskDates { planned_on, due_on }
}

/// Sets the deadline. A deadline before the plan date takes the plan date along.
pub fn set_due(dates: TaskDates, due_on: Option<NaiveDate>) -> TaskDates {
    let planned_on = match (due_on, dates.planned_on) {
        (Some(due), Some(plan)) if plan > due => Some(due),
        _ => dates.planned_on,
    };
    TaskDates { planned_on, due_on }
}

/// The dates that moved later: set before and after, and later after. A date taken along
/// counts, and so does planning a carried-over task for today. Setting a date for the first
/// time, moving it earlier and removing it do not.
pub fn postponed_fields(before: TaskDates, after: TaskDates) -> Vec<DateField> {
    [DateField::Plan, DateField::Due]
        .into_iter()
        .filter(|field| matches!((before.get(*field), after.get(*field)), (Some(from), Some(to)) if to > from))
        .collect()
}

/// Planned for an earlier day and not done.
pub fn carried_over(task: &Task, today: NaiveDate) -> bool {
    !task.is_done() && task.planned_on.is_some_and(|plan| plan < today)
}

/// Its deadline has passed and it is not done.
pub fn overdue(task: &Task, today: NaiveDate) -> bool {
    !task.is_done() && task.due_on.is_some_and(|due| due < today)
}

/// Open and planned or due today or earlier.
pub fn is_today(task: &Task, today: NaiveDate) -> bool {
    !task.is_done() && (task.planned_on.is_some_and(|d| d <= today) || task.due_on.is_some_and(|d| d <= today))
}

/// Where a task stands in Today: overdue first, then carried over, then due today, then the
/// rest. Sorting by it keeps the order within each group.
pub fn today_rank(task: &Task, today: NaiveDate) -> u8 {
    if overdue(task, today) {
        0
    } else if carried_over(task, today) {
        1
    } else if task.due_on == Some(today) {
        2
    } else {
        3
    }
}

/// Open, without a quest and without a plan date. A deadline alone keeps it in the inbox.
pub fn is_inbox(task: &Task) -> bool {
    !task.is_done() && task.quest_id.is_none() && task.planned_on.is_none()
}

#[cfg(test)]
mod tests;
