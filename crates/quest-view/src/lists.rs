//! The task lists, pure: Today, the Inbox and the Logbook, and how often a task was postponed.
//! The rules for what belongs where are the domain's (`quest_domain::is_today` and so on).

#[cfg(test)]
mod tests;

use chrono::{NaiveDate, TimeZone};
use quest_domain::{DateField, EntityId, Snapshot, Task, is_inbox, is_today, today_rank};

/// Open tasks planned or due today or earlier: overdue first, then carried over, then due today,
/// then the rest; by plan date within each group, then in list order.
pub fn today(snapshot: &Snapshot, today: NaiveDate) -> Vec<&Task> {
    let mut tasks: Vec<&Task> = snapshot.tasks.iter().filter(|t| is_today(t, today)).collect();
    tasks.sort_by_key(|t| (today_rank(t, today), t.planned_on));
    tasks
}

pub fn today_with_linger<'a>(
    snapshot: &'a Snapshot,
    today: NaiveDate,
    lingering: impl Fn(&Task) -> Option<&'a Task>,
) -> Vec<&'a Task> {
    let mut tasks: Vec<_> = snapshot
        .tasks
        .iter()
        .filter_map(|task| {
            let before = lingering(task).unwrap_or(task);
            is_today(before, today).then_some((task, (today_rank(before, today), before.planned_on)))
        })
        .collect();
    tasks.sort_by_key(|(_, order)| *order);
    tasks.into_iter().map(|(task, _)| task).collect()
}

pub fn inbox_with_linger<'a>(snapshot: &'a Snapshot, lingering: impl Fn(&Task) -> Option<&'a Task>) -> Vec<&'a Task> {
    snapshot.tasks.iter().filter(|task| is_inbox(lingering(task).unwrap_or(task))).collect()
}

/// Open tasks without a quest and a plan date, in list order.
pub fn inbox(snapshot: &Snapshot) -> Vec<&Task> {
    snapshot.tasks.iter().filter(|t| is_inbox(t)).collect()
}

/// Done tasks, the most recently completed first.
pub fn logbook(snapshot: &Snapshot) -> Vec<&Task> {
    let mut done: Vec<&Task> = snapshot.tasks.iter().filter(|t| t.is_done()).collect();
    done.sort_by_key(|t| std::cmp::Reverse(t.completed_at));
    done
}

/// Tasks completed on the local day `today` (the "n done" under Today), the most recent first.
pub fn done_today<'a, Tz: TimeZone>(snapshot: &'a Snapshot, today: NaiveDate, tz: &Tz) -> Vec<&'a Task> {
    logbook(snapshot).into_iter().filter(|t| completed_on(t, tz) == Some(today)).collect()
}

/// A quest's open tasks, in list order.
pub fn quest_open(snapshot: &Snapshot, quest: EntityId) -> Vec<&Task> {
    snapshot.tasks.iter().filter(|t| t.quest_id == Some(quest) && !t.is_done()).collect()
}

/// A quest's done tasks, the most recently completed first.
pub fn quest_done(snapshot: &Snapshot, quest: EntityId) -> Vec<&Task> {
    logbook(snapshot).into_iter().filter(|t| t.quest_id == Some(quest)).collect()
}

/// The Logbook grouped by the local day each task was completed, the newest day first.
pub fn logbook_days<'a, Tz: TimeZone>(snapshot: &'a Snapshot, tz: &Tz) -> Vec<(NaiveDate, Vec<&'a Task>)> {
    let mut days: Vec<(NaiveDate, Vec<&Task>)> = Vec::new();
    for task in logbook(snapshot) {
        let Some(day) = completed_on(task, tz) else { continue };
        match days.last_mut() {
            Some((last, tasks)) if *last == day => tasks.push(task),
            _ => days.push((day, vec![task])),
        }
    }
    days
}

/// Open tasks planned after today: the Plan count.
pub fn planned_ahead(snapshot: &Snapshot, today: NaiveDate) -> Vec<&Task> {
    snapshot.tasks.iter().filter(|t| !t.is_done() && t.planned_on.is_some_and(|d| d > today)).collect()
}

/// The sidebar's counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavCounts {
    pub inbox: usize,
    pub today: usize,
    pub plan: usize,
}

pub fn nav_counts(snapshot: &Snapshot, today: NaiveDate) -> NavCounts {
    NavCounts {
        inbox: inbox(snapshot).len(),
        today: self::today(snapshot, today).len(),
        plan: planned_ahead(snapshot, today).len(),
    }
}

fn completed_on<Tz: TimeZone>(task: &Task, tz: &Tz) -> Option<NaiveDate> {
    task.completed_at.map(|at| at.with_timezone(tz).date_naive())
}

/// How often a task's dates were postponed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Postponed {
    pub plan: usize,
    pub due: usize,
}

impl Postponed {
    pub fn total(self) -> usize {
        self.plan + self.due
    }
}

pub fn postponed(snapshot: &Snapshot, task: EntityId) -> Postponed {
    snapshot.postponements_of(task).fold(Postponed::default(), |mut count, p| {
        match p.field {
            DateField::Plan => count.plan += 1,
            DateField::Due => count.due += 1,
        }
        count
    })
}
