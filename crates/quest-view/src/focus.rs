//! What Focus is about, pure: the running task, else the one started or picked last, else the
//! open task worked on last.

#[cfg(test)]
mod tests;

use quest_domain::{EntityId, Snapshot, Task};

/// What the Focus view is about. Pausing stops the time entry but keeps the task in view.
pub enum FocusTarget<'a> {
    Running { task: &'a Task },
    Paused { task: &'a Task },
    Nothing,
}

impl<'a> FocusTarget<'a> {
    pub fn task(&self) -> Option<&'a Task> {
        match self {
            FocusTarget::Running { task } | FocusTarget::Paused { task } => Some(task),
            FocusTarget::Nothing => None,
        }
    }
}

/// The running task; without one, `focus_task` (the task started or selected last) if it is
/// open, else the open task worked on last. Nothing when there is no such open task (Focus then
/// offers a choice).
pub fn target(snapshot: &Snapshot, focus_task: Option<EntityId>) -> FocusTarget<'_> {
    if let Some(entry) = snapshot.running_entry()
        && let Some(task) = entry.task_id.and_then(|id| snapshot.task(id))
    {
        return FocusTarget::Running { task };
    }
    let worked_on_last =
        || snapshot.time_entries.iter().rev().filter_map(|e| snapshot.task(e.task_id?)).find(|t| !t.is_done());
    let open = |id: EntityId| snapshot.task(id).filter(|t| !t.is_done());
    match focus_task.and_then(open).or_else(worked_on_last) {
        Some(task) => FocusTarget::Paused { task },
        None => FocusTarget::Nothing,
    }
}
