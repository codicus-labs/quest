//! Commands that set a task's dates. All of them go through `set_planned`/`set_due` and record
//! a `Postponement` for every date they move later.

use chrono::{DateTime, NaiveDate, Utc};
use serde_json::json;
use uuid::Uuid;

use super::{CommandError, Position, in_workspace, task_sort_key, title, write};
use crate::change::{EntityId, EntityKind, FieldWrite};
use crate::dates::{DateField, TaskDates, postponed_fields, set_due, set_planned};
use crate::model::fields;
use crate::snapshot::Snapshot;

pub(super) fn create(
    snapshot: &Snapshot,
    id: EntityId,
    raw_title: &str,
    quest_id: Option<EntityId>,
    planned_on: Option<NaiveDate>,
    due_on: Option<NaiveDate>,
    position: Position,
) -> Result<Vec<FieldWrite>, CommandError> {
    if let Some(quest) = quest_id {
        snapshot.quest(quest).ok_or(CommandError::UnknownQuest(quest))?;
    }
    let dates = set_due(set_planned(TaskDates::default(), planned_on), due_on);
    use EntityKind::Task;
    Ok(vec![
        write(Task, id, fields::TITLE, json!(title(raw_title)?)),
        write(Task, id, fields::QUEST_ID, json!(quest_id)),
        write(Task, id, fields::TASK_PLANNED_ON, json!(dates.planned_on)),
        write(Task, id, fields::TASK_DUE_ON, json!(dates.due_on)),
        write(Task, id, fields::SORT_KEY, json!(task_sort_key(snapshot, id, dates.planned_on, position))),
        in_workspace(Task, id, snapshot),
    ])
}

pub(super) fn move_task(
    snapshot: &Snapshot,
    id: EntityId,
    planned_on: Option<NaiveDate>,
    position: Position,
    at: DateTime<Utc>,
) -> Result<Vec<FieldWrite>, CommandError> {
    let task = snapshot.task(id).ok_or(CommandError::UnknownTask(id))?;
    let before = task.dates();
    let mut writes = date_writes(snapshot, id, before, set_planned(before, planned_on), at);
    writes.push(write(
        EntityKind::Task,
        id,
        fields::SORT_KEY,
        json!(task_sort_key(snapshot, id, planned_on, position)),
    ));
    Ok(writes)
}

pub(super) fn set_task_due(
    snapshot: &Snapshot,
    id: EntityId,
    due_on: Option<NaiveDate>,
    at: DateTime<Utc>,
) -> Result<Vec<FieldWrite>, CommandError> {
    let task = snapshot.task(id).ok_or(CommandError::UnknownTask(id))?;
    let before = task.dates();
    let after = set_due(before, due_on);
    if after.planned_on == before.planned_on {
        return Ok(date_writes(snapshot, id, before, after, at));
    }
    // The plan date taken along moves the task to the bottom of that day.
    let mut writes = date_writes(snapshot, id, before, after, at);
    let key = task_sort_key(snapshot, id, after.planned_on, Position::Bottom);
    writes.push(write(EntityKind::Task, id, fields::SORT_KEY, json!(key)));
    Ok(writes)
}

/// Writes the dates that differ between `before` and `after`, and a postponement for each one
/// moved later. The caller has applied the rules between the two dates.
pub(crate) fn date_writes(
    snapshot: &Snapshot,
    task: EntityId,
    before: TaskDates,
    after: TaskDates,
    at: DateTime<Utc>,
) -> Vec<FieldWrite> {
    let mut writes = Vec::new();
    if after.planned_on != before.planned_on {
        writes.push(write(EntityKind::Task, task, fields::TASK_PLANNED_ON, json!(after.planned_on)));
    }
    if after.due_on != before.due_on {
        writes.push(write(EntityKind::Task, task, fields::TASK_DUE_ON, json!(after.due_on)));
    }
    for field in postponed_fields(before, after) {
        let (Some(from), Some(to)) = (before.get(field), after.get(field)) else { continue };
        writes.extend(postponement(snapshot, task, field, from, to, at));
    }
    writes
}

/// A postponement's fields. Its id derives from what it records, so planning the same command
/// again yields the same record.
fn postponement(
    snapshot: &Snapshot,
    task: EntityId,
    field: DateField,
    from: NaiveDate,
    to: NaiveDate,
    at: DateTime<Utc>,
) -> [FieldWrite; 6] {
    let name = format!("{field:?}:{from}:{to}:{}", at.timestamp_millis());
    let id = EntityId(Uuid::new_v5(&task.0, name.as_bytes()));
    use EntityKind::Postponement;
    [
        write(Postponement, id, fields::TASK_ID, json!(task)),
        write(Postponement, id, fields::POSTPONEMENT_FIELD, json!(field)),
        write(Postponement, id, fields::POSTPONEMENT_FROM, json!(from)),
        write(Postponement, id, fields::POSTPONEMENT_TO, json!(to)),
        write(Postponement, id, fields::POSTPONEMENT_AT, json!(at)),
        in_workspace(Postponement, id, snapshot),
    ]
}
