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
    let mut writes = vec![
        write(Task, id, fields::TITLE, json!(title(raw_title)?)),
        write(Task, id, fields::QUEST_ID, json!(quest_id)),
        write(Task, id, fields::TASK_PLANNED_ON, json!(dates.planned_on)),
        write(Task, id, fields::TASK_DUE_ON, json!(dates.due_on)),
        write(Task, id, fields::SORT_KEY, json!(task_sort_key(snapshot, id, dates.planned_on, position))),
        in_workspace(Task, id, snapshot),
    ];
    if let Some(day) = dates.planned_on.filter(|_| position != Position::Bottom) {
        writes.extend(super::task_order::order_writes(snapshot, id, super::TaskList::Day(day), position, day));
    } else {
        writes.push(super::task_order::append_day(snapshot, id));
    }
    Ok(writes)
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
    let after = set_planned(before, planned_on);
    let mut writes = date_writes(snapshot, id, before, after, at);
    if position != Position::Bottom || before.planned_on == planned_on {
        let list = match planned_on {
            Some(day) => super::TaskList::Day(day),
            None => task.quest_id.map_or(super::TaskList::Inbox, super::TaskList::Quest),
        };
        if planned_on.is_some() {
            writes.retain(|write| write.field != fields::TASK_DAY_ORDER);
        }
        writes.extend(super::task_order::order_writes(snapshot, id, list, position, at.date_naive()));
    }
    Ok(writes)
}

pub(super) fn set_task_due(
    snapshot: &Snapshot,
    id: EntityId,
    due_on: Option<NaiveDate>,
    today: NaiveDate,
    at: DateTime<Utc>,
) -> Result<Vec<FieldWrite>, CommandError> {
    let task = snapshot.task(id).ok_or(CommandError::UnknownTask(id))?;
    let before = task.dates();
    let after = set_due(before, due_on);
    let mut writes = date_writes(snapshot, id, before, after, at);
    if !crate::is_today(task, today)
        && !task.is_done()
        && after.planned_on == before.planned_on
        && after.due_on.is_some_and(|due| due <= today)
    {
        writes.push(super::task_order::append_day(snapshot, id));
    }
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
    if before.planned_on != after.planned_on {
        writes.push(super::task_order::append_day(snapshot, task));
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
