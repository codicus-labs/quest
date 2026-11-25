//! Organizing: tracked time between tasks.

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use super::{CommandError, stop_running, write};
use crate::change::{EntityId, EntityKind, FieldWrite};
use crate::model::{EntryKind, fields};
use crate::snapshot::Snapshot;

pub(super) fn start_organizing(
    snapshot: &Snapshot,
    entry_id: EntityId,
    origin: Option<EntityId>,
    at: DateTime<Utc>,
) -> Result<Vec<FieldWrite>, CommandError> {
    if let Some(task) = origin {
        snapshot.task(task).ok_or(CommandError::UnknownTask(task))?;
    }
    let mut writes = stop_running(snapshot, at, |_| true);
    writes.extend(organizing(entry_id, origin, at, snapshot.workspace));
    Ok(writes)
}

/// Splits the running task session at `since`; the organizing part stays in its workspace.
pub(super) fn rebook(
    snapshot: &Snapshot,
    entry_id: EntityId,
    expected_task_entry: Option<EntityId>,
    since: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<Vec<FieldWrite>, CommandError> {
    let running = snapshot
        .running_focus()
        .filter(|r| r.entry.kind == EntryKind::Task && expected_task_entry.is_none_or(|id| r.entry.id == id));
    let Some(running) = running else { return Err(CommandError::NothingToRebook) };
    if since < running.entry.started_at || since > now {
        return Err(CommandError::RebookOutsideEntry);
    }
    let mut writes = vec![write(EntityKind::TimeEntry, running.entry.id, fields::ENTRY_ENDED_AT, json!(since))];
    writes.extend(organizing(entry_id, running.entry.task_id, since, running.workspace));
    writes.push(write(EntityKind::TimeEntry, entry_id, fields::ENTRY_REBOOKED_FROM, json!(running.entry.id)));
    Ok(writes)
}

pub(super) fn undo_rebook(
    snapshot: &Snapshot,
    entry_id: EntityId,
    at: DateTime<Utc>,
) -> Result<Vec<FieldWrite>, CommandError> {
    let booked = snapshot
        .time_entries
        .iter()
        .chain(&snapshot.discarded_rebooks)
        .find(|e| e.id == entry_id)
        .ok_or(CommandError::UnknownEntry(entry_id))?;
    let original = booked
        .rebooked_from
        .and_then(|id| snapshot.time_entries.iter().find(|e| e.id == id))
        .filter(|e| {
            e.kind == EntryKind::Task && e.task_id == booked.origin_task_id && e.ended_at == Some(booked.started_at)
        })
        .ok_or(CommandError::RebookChanged)?;
    if booked.is_running()
        && (snapshot.running_focus().is_none_or(|r| r.entry.id != entry_id)
            || original.task_id.is_some_and(|id| snapshot.task(id).is_none_or(|task| task.is_done())))
    {
        return Err(CommandError::RebookChanged);
    }
    Ok(vec![
        write(EntityKind::TimeEntry, entry_id, fields::DELETED_AT, json!(at)),
        write(EntityKind::TimeEntry, original.id, fields::ENTRY_ENDED_AT, json!(booked.ended_at)),
    ])
}

fn organizing(
    id: EntityId,
    origin: Option<EntityId>,
    started_at: DateTime<Utc>,
    workspace: EntityId,
) -> [FieldWrite; 5] {
    use EntityKind::TimeEntry;
    [
        write(TimeEntry, id, fields::ENTRY_KIND, json!(EntryKind::Organizing)),
        write(TimeEntry, id, fields::ENTRY_ORIGIN_TASK_ID, json!(origin)),
        write(TimeEntry, id, fields::ENTRY_STARTED_AT, json!(started_at)),
        write(TimeEntry, id, fields::ENTRY_ENDED_AT, Value::Null),
        write(TimeEntry, id, fields::WORKSPACE_ID, json!(workspace)),
    ]
}
