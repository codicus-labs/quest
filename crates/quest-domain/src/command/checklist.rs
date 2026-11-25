//! Commands on a task's checklist.

use chrono::{DateTime, Utc};
use serde_json::json;

use super::{CommandError, Position, in_workspace, key_at, title, write};
use crate::change::{EntityId, EntityKind, FieldWrite};
use crate::model::{ChecklistItem, fields};
use crate::snapshot::Snapshot;

use EntityKind::ChecklistItem as Item;

fn item(snapshot: &Snapshot, id: EntityId) -> Result<&ChecklistItem, CommandError> {
    snapshot.checklist_item(id).ok_or(CommandError::UnknownChecklistItem(id))
}

fn sort_key(snapshot: &Snapshot, task: EntityId, moving: EntityId, position: Position) -> String {
    let siblings: Vec<&ChecklistItem> = snapshot.checklist_of(task).filter(|i| i.id != moving).collect();
    let keys: Vec<&str> = siblings.iter().map(|i| i.sort_key.as_str()).collect();
    let anchor = match position {
        Position::Before(anchor) => siblings.iter().find(|i| i.id == anchor).map(|i| i.sort_key.as_str()),
        Position::Top | Position::Bottom => None,
    };
    key_at(&keys, position, anchor)
}

pub(super) fn add(
    snapshot: &Snapshot,
    id: EntityId,
    task: EntityId,
    text: &str,
    position: Position,
) -> Result<Vec<FieldWrite>, CommandError> {
    snapshot.task(task).ok_or(CommandError::UnknownTask(task))?;
    Ok(vec![
        write(Item, id, fields::TASK_ID, json!(task)),
        write(Item, id, fields::CHECKLIST_TEXT, json!(if text.is_empty() { String::new() } else { title(text)? })),
        write(Item, id, fields::CHECKLIST_DONE, json!(false)),
        write(Item, id, fields::SORT_KEY, json!(sort_key(snapshot, task, id, position))),
        in_workspace(Item, id, snapshot),
    ])
}

pub(super) fn rename(snapshot: &Snapshot, id: EntityId, text: &str) -> Result<Vec<FieldWrite>, CommandError> {
    item(snapshot, id)?;
    Ok(vec![write(Item, id, fields::CHECKLIST_TEXT, json!(text))])
}

pub(super) fn set_done(snapshot: &Snapshot, id: EntityId, done: bool) -> Result<Vec<FieldWrite>, CommandError> {
    item(snapshot, id)?;
    Ok(vec![write(Item, id, fields::CHECKLIST_DONE, json!(done))])
}

pub(super) fn move_item(
    snapshot: &Snapshot,
    id: EntityId,
    position: Position,
) -> Result<Vec<FieldWrite>, CommandError> {
    let task = item(snapshot, id)?.task_id;
    Ok(vec![write(Item, id, fields::SORT_KEY, json!(sort_key(snapshot, task, id, position)))])
}

pub(super) fn delete(snapshot: &Snapshot, id: EntityId, at: DateTime<Utc>) -> Result<Vec<FieldWrite>, CommandError> {
    item(snapshot, id)?;
    Ok(vec![write(Item, id, fields::DELETED_AT, json!(at))])
}
