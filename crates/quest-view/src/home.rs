//! Home's decisions, pure: the quest a new quest-work task gets, and how a typed title is
//! saved (`rename`).

#[cfg(test)]
mod tests;

use quest_domain::{Command, EntityId, Snapshot, Task};

/// The quest "Add quest work…" gives a new task: the filtered one, the one picked on the
/// field's chip, else the quest of the newest quest task (ids grow with creation time), else
/// the first active quest. Only active quests count.
pub fn new_task_quest(snapshot: &Snapshot, filter: Option<EntityId>, pick: Option<EntityId>) -> Option<EntityId> {
    let usable = |id: &EntityId| snapshot.active_quests().any(|q| q.id == *id);
    filter
        .or(pick)
        .filter(usable)
        .or_else(|| {
            snapshot
                .tasks
                .iter()
                .filter_map(|t| Some((t.id, t.quest_id?)))
                .filter(|(_, q)| usable(q))
                .max()
                .map(|(_, q)| q)
        })
        .or_else(|| snapshot.active_quests().next().map(|q| q.id))
}

/// The rename a draft title asks for: none when it is empty or the title it already has.
pub fn rename(task: &Task, draft: &str) -> Option<Command> {
    let title = one_line(draft);
    let title = title.trim();
    if title.is_empty() || title == task.title {
        return None;
    }
    Some(Command::RenameTask { id: task.id, title: title.to_owned() })
}

/// `text` on one line: every line break becomes a space.
pub fn one_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\n', '\r'], " ")
}
