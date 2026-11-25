//! The player card, pure: what runs now (a task, or Organizing with the task to go back to),
//! else the open task worked on last, to resume.

#[cfg(test)]
mod tests;

use chrono::{DateTime, Utc};
use quest_domain::{EntityId, EntryKind, Snapshot};

#[derive(Clone, Debug, PartialEq)]
pub enum Player {
    /// A session on a task, or without one (`task` none, `title` "No task").
    Running { task: Option<EntityId>, title: String, quest: Option<EntityId>, started_at: DateTime<Utc> },
    /// "Organizing", with "Back to <task>" when it came from a task that is still open.
    Organizing { started_at: DateTime<Utc>, back_to: Option<TaskRef> },
    /// Nothing runs: "Resume" the open task worked on last.
    Resume(TaskRef),
    /// Nothing runs and nothing to resume.
    Empty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRef {
    pub task: EntityId,
    pub title: String,
}

/// The player for the active workspace (a timer in another workspace shows elsewhere).
pub fn player(snapshot: &Snapshot) -> Player {
    let open =
        |id: EntityId| snapshot.task(id).filter(|t| !t.is_done()).map(|t| TaskRef { task: id, title: t.title.clone() });
    if let Some(entry) = snapshot.running_entry() {
        if entry.kind == EntryKind::Organizing {
            return Player::Organizing { started_at: entry.started_at, back_to: entry.origin_task_id.and_then(open) };
        }
        let title =
            entry.task_id.and_then(|id| snapshot.task(id)).map_or_else(|| "No task".to_owned(), |t| t.title.clone());
        return Player::Running {
            task: entry.task_id,
            title,
            quest: snapshot.entry_quest(entry),
            started_at: entry.started_at,
        };
    }
    snapshot
        .time_entries
        .iter()
        .filter(|e| e.kind == EntryKind::Task)
        .filter_map(|e| Some((e.ended_at?, open(e.task_id?)?)))
        .max_by_key(|(ended, _)| *ended)
        .map_or(Player::Empty, |(_, task)| Player::Resume(task))
}
