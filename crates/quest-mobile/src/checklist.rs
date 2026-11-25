//! A task's checklist, pure: its rows and the commands its edits send. `None` means there is
//! nothing to do (e.g. an unchanged text).

use quest_domain::{ChecklistItem, Command, EntityId, Position, Snapshot};
use quest_view::home::one_line;

use crate::error::MobileError;
use crate::ids::parse_id;
use crate::task_types::{ChecklistPosition, ChecklistRow};

pub(crate) fn rows(snapshot: &Snapshot, task: EntityId) -> Vec<ChecklistRow> {
    snapshot
        .checklist_of(task)
        .map(|item| ChecklistRow { id: item.id.to_string(), text: item.text.clone(), done: item.done })
        .collect()
}

/// A new item's text on one line, trimmed; `EmptyTitle` when blank.
pub(crate) fn add(
    task: EntityId,
    id: EntityId,
    text: &str,
    position: &ChecklistPosition,
) -> Result<Command, MobileError> {
    let text = one_line(text).trim().to_owned();
    if text.is_empty() {
        return Err(MobileError::EmptyTitle);
    }
    Ok(Command::AddChecklistItem { id, task_id: task, text, position: position_of(position)? })
}

/// Like a task's title: on one line, trimmed; empty or unchanged saves nothing.
pub(crate) fn rename(item: &ChecklistItem, text: &str) -> Option<Command> {
    let text = one_line(text).trim().to_owned();
    (!text.is_empty() && text != item.text).then_some(Command::RenameChecklistItem { id: item.id, text })
}

pub(crate) fn set_done(item: &ChecklistItem, done: bool) -> Option<Command> {
    (item.done != done).then_some(Command::SetChecklistItemDone { id: item.id, done })
}

pub(crate) fn position_of(position: &ChecklistPosition) -> Result<Position, MobileError> {
    Ok(match position {
        ChecklistPosition::Top => Position::Top,
        ChecklistPosition::Bottom => Position::Bottom,
        ChecklistPosition::Before { item_id } => Position::Before(parse_id(item_id)?),
    })
}

#[cfg(test)]
#[path = "checklist_tests.rs"]
mod tests;
