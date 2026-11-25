use super::{Command, CommandError, Position, WorkTask, entry, plan, task_quest, write};
use crate::{EntityId, EntityKind, FieldWrite, Snapshot, model::fields};
use chrono::{DateTime, NaiveDate, Utc};
use serde_json::json;

pub(super) fn completed_task(
    snapshot: &Snapshot,
    id: EntityId,
    title: &str,
    quest_id: Option<EntityId>,
    day: NaiveDate,
    completed_at: DateTime<Utc>,
) -> Result<Vec<FieldWrite>, CommandError> {
    let create =
        Command::CreateTask { id, title: title.to_owned(), quest_id, day: Some(day), position: Position::Bottom };
    let mut writes = plan(&create, snapshot)?;
    writes.push(write(EntityKind::Task, id, fields::TASK_COMPLETED_AT, json!(completed_at)));
    Ok(writes)
}

pub(super) fn plan_edit(
    snapshot: &Snapshot,
    id: EntityId,
    task: Option<&WorkTask>,
    quest_id: Option<EntityId>,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
) -> Result<Vec<FieldWrite>, CommandError> {
    let current = entry(snapshot, id)?;
    if let Some(quest) = quest_id.filter(|q| Some(*q) != current.quest_id && snapshot.quest(*q).is_none()) {
        return Err(CommandError::UnknownQuest(quest));
    }
    let mut writes = if started_at != current.started_at || ended_at != current.ended_at {
        plan(&Command::SetEntryTimes { id, started_at, ended_at }, snapshot)?
    } else {
        Vec::new()
    };
    let task_id = match task {
        Some(WorkTask::Existing(task)) => {
            if Some(*task) != current.task_id {
                task_quest(snapshot, Some(*task))?;
            }
            Some(*task)
        }
        Some(WorkTask::New { id, title, quest_id, day }) => {
            writes.extend(completed_task(snapshot, *id, title, *quest_id, *day, ended_at.unwrap_or(started_at))?);
            Some(*id)
        }
        None => None,
    };
    if task_id != current.task_id {
        writes.push(write(EntityKind::TimeEntry, id, fields::ENTRY_TASK_ID, json!(task_id)));
    }
    if quest_id != current.quest_id {
        writes.push(write(EntityKind::TimeEntry, id, fields::QUEST_ID, json!(quest_id)));
    }
    Ok(writes)
}
