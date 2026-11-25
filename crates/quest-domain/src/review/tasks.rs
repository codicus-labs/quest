//! What happened to tasks in a week, to learn from: what was postponed most, what is carried
//! over or overdue, how much got done against how much was added.

use std::collections::BTreeMap;

use chrono::{DateTime, Days, NaiveDate, TimeZone, Utc};

use super::{midnight, week_start};
use crate::change::EntityId;
use crate::dates::{DateField, carried_over, overdue};
use crate::snapshot::Snapshot;

/// A task postponed in the week.
#[derive(Clone, Debug, PartialEq)]
pub struct PostponedTask {
    pub task_id: EntityId,
    pub plan: usize,
    pub due: usize,
    pub last_at: DateTime<Utc>,
}

impl PostponedTask {
    pub fn total(&self) -> usize {
        self.plan + self.due
    }
}

/// An open task behind its plan date or deadline by `days`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lingering {
    pub task_id: EntityId,
    pub days: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeekTasks {
    pub week_start: NaiveDate,
    /// Tasks postponed in the week: most postponements first, then the latest.
    pub most_postponed: Vec<PostponedTask>,
    /// Postponements in the week.
    pub postponements: usize,
    /// Open tasks planned before the week's last day (or today, if earlier), longest first.
    /// Like the lists, this is today's state: a task done since no longer counts.
    pub carried_over: Vec<Lingering>,
    /// Open tasks past their deadline on that day, longest first.
    pub overdue: Vec<Lingering>,
    /// Tasks completed in the week.
    pub completed: usize,
    /// Tasks created in the week (that still exist).
    pub created: usize,
}

impl WeekTasks {
    pub fn build<Tz: TimeZone>(snapshot: &Snapshot, any_day: NaiveDate, tz: &Tz, now: DateTime<Utc>) -> Self {
        let start = week_start(any_day);
        let (from, to) = (midnight(start, tz), midnight(start + Days::new(7), tz));
        let in_week = |at: DateTime<Utc>| from <= at && at < to;
        let today = now.with_timezone(tz).date_naive();
        let as_of = today.min(start + Days::new(6));

        let mut by_task: BTreeMap<EntityId, PostponedTask> = BTreeMap::new();
        let in_week_postponements = snapshot.postponements.iter().filter(|p| in_week(p.at));
        for p in in_week_postponements.clone() {
            let task = by_task.entry(p.task_id).or_insert(PostponedTask {
                task_id: p.task_id,
                plan: 0,
                due: 0,
                last_at: p.at,
            });
            match p.field {
                DateField::Plan => task.plan += 1,
                DateField::Due => task.due += 1,
            }
            task.last_at = task.last_at.max(p.at);
        }
        let mut most_postponed: Vec<PostponedTask> = by_task.into_values().collect();
        most_postponed.sort_by(|a, b| b.total().cmp(&a.total()).then(b.last_at.cmp(&a.last_at)));

        let lingering = |behind: fn(&crate::Task, NaiveDate) -> bool, date: fn(&crate::Task) -> Option<NaiveDate>| {
            let mut tasks: Vec<Lingering> = snapshot
                .tasks
                .iter()
                .filter(|t| behind(t, as_of))
                .filter_map(|t| Some(Lingering { task_id: t.id, days: (as_of - date(t)?).num_days() }))
                .collect();
            tasks.sort_by_key(|l| std::cmp::Reverse(l.days));
            tasks
        };
        WeekTasks {
            week_start: start,
            postponements: in_week_postponements.count(),
            most_postponed,
            carried_over: lingering(carried_over, |t| t.planned_on),
            overdue: lingering(overdue, |t| t.due_on),
            completed: snapshot.tasks.iter().filter_map(|t| t.completed_at).filter(|at| in_week(*at)).count(),
            created: snapshot.task_created.values().filter(|at| in_week(**at)).count(),
        }
    }
}

#[cfg(test)]
mod tests;
