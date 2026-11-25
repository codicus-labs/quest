//! The task lists, pure: Today, the Inbox and the Logbook, and how often a task was postponed.
//! The rules for what belongs where are the domain's (`quest_domain::is_today` and so on).

#[cfg(test)]
mod tests;

use chrono::NaiveDate;
use quest_domain::{DateField, EntityId, Snapshot, Task, is_inbox, is_today, today_rank};

/// Open tasks planned or due today or earlier: overdue first, then carried over, then due today,
/// then the rest; by plan date within each group, then in list order.
pub fn today(snapshot: &Snapshot, today: NaiveDate) -> Vec<&Task> {
    let mut tasks: Vec<&Task> = snapshot.tasks.iter().filter(|t| is_today(t, today)).collect();
    tasks.sort_by_key(|t| (today_rank(t, today), t.planned_on));
    tasks
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
