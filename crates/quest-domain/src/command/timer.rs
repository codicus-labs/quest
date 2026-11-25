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
    since: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<Vec<FieldWrite>, CommandError> {
    let running = snapshot.running_focus().filter(|r| r.entry.kind == EntryKind::Task);
    let Some(running) = running else { return Err(CommandError::NothingToRebook) };
    if since <= running.entry.started_at || since > now {
        return Err(CommandError::RebookOutsideEntry);
    }
    let mut writes = vec![write(EntityKind::TimeEntry, running.entry.id, fields::ENTRY_ENDED_AT, json!(since))];
    writes.extend(organizing(entry_id, running.entry.task_id, since, running.workspace));
    Ok(writes)
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
