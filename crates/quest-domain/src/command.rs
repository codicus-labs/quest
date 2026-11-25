//! User intents and the pure policy that turns them into field writes. The store stamps the
//! writes with its clock, persists them and syncs them; nothing here performs side effects.

mod edit_entry;

use chrono::{DateTime, NaiveDate, Utc};
use serde_json::{Value, json};

use crate::change::{EntityId, EntityKind, FieldWrite};
use crate::model::{QuestColor, QuestStatus, TimeBlock, fields};
use crate::recurrence::{self, RepeatRule};
use crate::snapshot::{RunningFocus, Snapshot};
use crate::sort_key::sort_key_between;

/// Where a new or moved task lands within its day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
    Before(EntityId),
}

/// The task a `LogWork` session is for: one that exists (open or done), or a new one that is
/// created already done.
#[derive(Clone, Debug, PartialEq)]
pub enum WorkTask {
    Existing(EntityId),
    New { id: EntityId, title: String, quest_id: Option<EntityId>, day: NaiveDate },
}

/// Ids of created entities are chosen by the caller, so the UI can refer to them right away.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    CreateQuest {
        id: EntityId,
        title: String,
        color: QuestColor,
    },
    RenameQuest {
        id: EntityId,
        title: String,
    },
    SetQuestColor {
        id: EntityId,
        color: QuestColor,
    },
    SetQuestStatus {
        id: EntityId,
        status: QuestStatus,
    },
    /// Plain text notes; empty clears them.
    SetQuestDescription {
        id: EntityId,
        description: String,
    },
    /// Soft-deletes a quest and its milestones. Its tasks and sessions stay and count as other
    /// work until the quest is restored.
    DeleteQuest {
        id: EntityId,
        at: DateTime<Utc>,
    },

    CreateTask {
        id: EntityId,
        title: String,
        quest_id: Option<EntityId>,
        day: Option<NaiveDate>,
        position: Position,
    },
    RenameTask {
        id: EntityId,
        title: String,
    },
    /// Plain text notes; empty clears them.
    SetTaskDescription {
        id: EntityId,
        description: String,
    },
    SetTaskRepeat {
        id: EntityId,
        rule: Option<RepeatRule>,
        now: DateTime<Utc>,
    },
    GenerateDueTasks {
        now: DateTime<Utc>,
    },
    EditPlan {
        occurrence: Option<(EntityId, NaiveDate)>,
        now: DateTime<Utc>,
        edit: Box<Command>,
    },
    AssignTask {
        id: EntityId,
        quest_id: Option<EntityId>,
    },
    /// Moves a task to a day (or the backlog with `None`) at a position in that day's list.
    MoveTask {
        id: EntityId,
        day: Option<NaiveDate>,
        position: Position,
    },
    SetTaskBlock {
        id: EntityId,
        block: Option<TimeBlock>,
    },
    /// Plans a task into a day's calendar (e.g. dropped onto a time grid). If the day changes,
    /// the task goes to the bottom of that day's list.
    ScheduleTask {
        id: EntityId,
        day: NaiveDate,
        block: TimeBlock,
    },
    CompleteTask {
        id: EntityId,
        at: DateTime<Utc>,
    },
    CompleteTaskFromPlan {
        id: EntityId,
        at: DateTime<Utc>,
        today: NaiveDate,
    },
    ReopenTask {
        id: EntityId,
    },
    DeleteTask {
        id: EntityId,
        at: DateTime<Utc>,
    },

    AddMilestone {
        id: EntityId,
        quest_id: EntityId,
        title: String,
        achieved_on: NaiveDate,
    },
    RenameMilestone {
        id: EntityId,
        title: String,
    },
    SetMilestoneDate {
        id: EntityId,
        achieved_on: NaiveDate,
    },
    DeleteMilestone {
        id: EntityId,
        at: DateTime<Utc>,
    },

    /// Starts tracking time on a task; stops whatever else is running.
    StartFocus {
        entry_id: EntityId,
        task_id: EntityId,
        at: DateTime<Utc>,
    },
    StopFocus {
        at: DateTime<Utc>,
    },
    /// Records a finished session after the fact (e.g. dragged out on the calendar).
    LogTime {
        entry_id: EntityId,
        task_id: Option<EntityId>,
        started_at: DateTime<Utc>,
        ended_at: DateTime<Utc>,
    },
    /// Logs finished work after the fact (the Log work dialog), all or nothing: the session,
    /// and for a new task that task, created done (at the session's end) on `day`. The session
    /// counts towards the task's quest.
    LogWork {
        entry_id: EntityId,
        task: WorkTask,
        started_at: DateTime<Utc>,
        ended_at: DateTime<Utc>,
    },
    EditEntry {
        id: EntityId,
        task: Option<WorkTask>,
        quest_id: Option<EntityId>,
        started_at: DateTime<Utc>,
        ended_at: Option<DateTime<Utc>>,
    },
    /// Moves or resizes a session. A running session keeps running unless `ended_at` is given;
    /// a finished session cannot be made running again.
    SetEntryTimes {
        id: EntityId,
        started_at: DateTime<Utc>,
        ended_at: Option<DateTime<Utc>>,
    },
    /// Links a session to a task (and its quest), or to nothing (other work).
    AssignEntry {
        id: EntityId,
        task_id: Option<EntityId>,
    },
    DeleteEntry {
        id: EntityId,
        at: DateTime<Utc>,
    },

    /// The active workspace's weekly target.
    SetWeeklyTarget {
        minutes: u32,
    },

    /// A new, empty workspace. Switching to it is up to the client.
    CreateWorkspace {
        id: EntityId,
        name: String,
    },
    RenameWorkspace {
        id: EntityId,
        name: String,
    },
    /// Soft-deletes a workspace with everything in it; see `Snapshot::check_deletable`.
    DeleteWorkspace {
        id: EntityId,
        at: DateTime<Utc>,
    },
    RestoreWorkspace {
        id: EntityId,
    },

    /// Undoes a soft delete. Restoring a quest also restores the milestones deleted with it.
    Restore {
        kind: EntityKind,
        id: EntityId,
    },
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum CommandError {
    #[error("quest {0} does not exist")]
    UnknownQuest(EntityId),
    #[error("task {0} does not exist")]
    UnknownTask(EntityId),
    #[error("milestone {0} does not exist")]
    UnknownMilestone(EntityId),
    #[error("session {0} does not exist")]
    UnknownEntry(EntityId),
    #[error("a session must end after it starts")]
    InvalidTimeRange,
    #[error("a finished session cannot be resumed by editing its times; start a new session instead")]
    CannotResumeEntry,
    #[error("{kind:?} {id} is not in the trash")]
    NotInTrash { kind: EntityKind, id: EntityId },
    #[error("a title cannot be empty")]
    EmptyTitle,
    #[error("enter a repeat interval from 1 to 999 and an end date on or after the start")]
    InvalidRepeatRule,
    #[error("workspace {0} does not exist")]
    UnknownWorkspace(EntityId),
    #[error("workspace {0} already exists")]
    WorkspaceExists(EntityId),
    #[error("workspace {0} is not deleted")]
    WorkspaceNotDeleted(EntityId),
    #[error("the last workspace cannot be deleted; create another one first")]
    CannotDeleteLastWorkspace,
}

fn write(kind: EntityKind, id: EntityId, field: &'static str, value: Value) -> FieldWrite {
    FieldWrite { kind, id, field, value }
}

/// Puts a new record into the snapshot's workspace.
fn in_workspace(kind: EntityKind, id: EntityId, snapshot: &Snapshot) -> FieldWrite {
    write(kind, id, fields::WORKSPACE_ID, json!(snapshot.workspace))
}

fn title(raw: &str) -> Result<String, CommandError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(CommandError::EmptyTitle);
    }
    Ok(trimmed.to_owned())
}

/// Decides the field writes a command needs, validated against the current state.
pub fn plan(command: &Command, snapshot: &Snapshot) -> Result<Vec<FieldWrite>, CommandError> {
    use EntityKind::{Milestone, Quest, Settings, Task, TimeEntry, Workspace};
    let quest_exists = |id: EntityId| snapshot.quest(id).map(|_| ()).ok_or(CommandError::UnknownQuest(id));
    let task_exists = |id: EntityId| snapshot.task(id).map(|_| ()).ok_or(CommandError::UnknownTask(id));

    let writes = match command {
        Command::CreateQuest { id, title: raw, color } => {
            let last = snapshot.quests.last().map(|q| q.sort_key.as_str());
            vec![
                write(Quest, *id, fields::TITLE, json!(title(raw)?)),
                write(Quest, *id, fields::QUEST_COLOR, json!(color)),
                write(Quest, *id, fields::QUEST_STATUS, json!(QuestStatus::Active)),
                write(Quest, *id, fields::SORT_KEY, json!(sort_key_between(last, None))),
                in_workspace(Quest, *id, snapshot),
            ]
        }
        Command::RenameQuest { id, title: raw } => {
            quest_exists(*id)?;
            vec![write(Quest, *id, fields::TITLE, json!(title(raw)?))]
        }
        Command::SetQuestColor { id, color } => {
            quest_exists(*id)?;
            vec![write(Quest, *id, fields::QUEST_COLOR, json!(color))]
        }
        Command::SetQuestStatus { id, status } => {
            quest_exists(*id)?;
            vec![write(Quest, *id, fields::QUEST_STATUS, json!(status))]
        }
        Command::SetQuestDescription { id, description } => {
            quest_exists(*id)?;
            vec![write(Quest, *id, fields::QUEST_DESCRIPTION, json!(description))]
        }
        Command::DeleteQuest { id, at } => {
            quest_exists(*id)?;
            let milestones = snapshot.milestones_of(*id).map(|m| write(Milestone, m.id, fields::DELETED_AT, json!(at)));
            std::iter::once(write(Quest, *id, fields::DELETED_AT, json!(at))).chain(milestones).collect()
        }

        Command::CreateTask { id, title: raw, quest_id, day, position } => {
            if let Some(quest) = quest_id {
                quest_exists(*quest)?;
            }
            vec![
                write(Task, *id, fields::TITLE, json!(title(raw)?)),
                write(Task, *id, fields::QUEST_ID, json!(quest_id)),
                write(Task, *id, fields::TASK_DAY, json!(day)),
                write(Task, *id, fields::SORT_KEY, json!(task_sort_key(snapshot, *id, *day, *position))),
                in_workspace(Task, *id, snapshot),
            ]
        }
        Command::RenameTask { id, title: raw } => {
            task_exists(*id)?;
            vec![write(Task, *id, fields::TITLE, json!(title(raw)?))]
        }
        Command::SetTaskDescription { id, description } => {
            task_exists(*id)?;
            vec![write(Task, *id, fields::TASK_DESCRIPTION, json!(description))]
        }
        Command::SetTaskRepeat { id, rule, now } => recurrence::set_rule(snapshot, *id, *rule, *now)?,
        Command::GenerateDueTasks { now } => recurrence::generate_due(snapshot, *now),
        Command::EditPlan { occurrence, now, edit } => recurrence::edit_plan(snapshot, *occurrence, *now, edit)?,
        Command::AssignTask { id, quest_id } => {
            task_exists(*id)?;
            if let Some(quest) = quest_id {
                quest_exists(*quest)?;
            }
            vec![write(Task, *id, fields::QUEST_ID, json!(quest_id))]
        }
        Command::MoveTask { id, day, position } => {
            task_exists(*id)?;
            vec![
                write(Task, *id, fields::TASK_DAY, json!(day)),
                write(Task, *id, fields::SORT_KEY, json!(task_sort_key(snapshot, *id, *day, *position))),
            ]
        }
        Command::SetTaskBlock { id, block } => {
            task_exists(*id)?;
            vec![write(Task, *id, fields::TASK_BLOCK, json!(block))]
        }
        Command::ScheduleTask { id, day, block } => {
            let task = snapshot.task(*id).ok_or(CommandError::UnknownTask(*id))?;
            let mut writes = vec![write(Task, *id, fields::TASK_BLOCK, json!(block))];
            if task.day != Some(*day) {
                writes.push(write(Task, *id, fields::TASK_DAY, json!(day)));
                let key = task_sort_key(snapshot, *id, Some(*day), Position::Bottom);
                writes.push(write(Task, *id, fields::SORT_KEY, json!(key)));
            }
            writes
        }
        Command::CompleteTask { id, at } | Command::CompleteTaskFromPlan { id, at, .. } => {
            let task = snapshot.task(*id).ok_or(CommandError::UnknownTask(*id))?;
            let mut writes = stop_running(snapshot, *at, |running| running.entry.task_id == Some(*id));
            writes.push(write(Task, *id, fields::TASK_COMPLETED_AT, json!(at)));
            if let Command::CompleteTaskFromPlan { today, .. } = command
                && task.day.is_some_and(|day| day > *today)
                && (task.repeat.is_some()
                    || task.series.and_then(|id| snapshot.task(id)).is_some_and(|source| source.repeat.is_some()))
            {
                writes.push(write(Task, *id, fields::TASK_DAY, json!(today)));
                let key = task_sort_key(snapshot, *id, Some(*today), Position::Bottom);
                writes.push(write(Task, *id, fields::SORT_KEY, json!(key)));
            }
            writes
        }
        Command::ReopenTask { id } => {
            task_exists(*id)?;
            vec![write(Task, *id, fields::TASK_COMPLETED_AT, Value::Null)]
        }
        Command::DeleteTask { id, at } => {
            task_exists(*id)?;
            let mut writes = stop_running(snapshot, *at, |running| running.entry.task_id == Some(*id));
            writes.push(write(Task, *id, fields::DELETED_AT, json!(at)));
            writes
        }

        Command::AddMilestone { id, quest_id, title: raw, achieved_on } => {
            quest_exists(*quest_id)?;
            vec![
                write(Milestone, *id, fields::QUEST_ID, json!(quest_id)),
                write(Milestone, *id, fields::TITLE, json!(title(raw)?)),
                write(Milestone, *id, fields::MILESTONE_ACHIEVED_ON, json!(achieved_on)),
                in_workspace(Milestone, *id, snapshot),
            ]
        }
        Command::RenameMilestone { id, title: raw } => {
            milestone_exists(snapshot, *id)?;
            vec![write(Milestone, *id, fields::TITLE, json!(title(raw)?))]
        }
        Command::SetMilestoneDate { id, achieved_on } => {
            milestone_exists(snapshot, *id)?;
            vec![write(Milestone, *id, fields::MILESTONE_ACHIEVED_ON, json!(achieved_on))]
        }
        Command::DeleteMilestone { id, at } => {
            milestone_exists(snapshot, *id)?;
            vec![write(Milestone, *id, fields::DELETED_AT, json!(at))]
        }

        Command::StartFocus { entry_id, task_id, at } => {
            let task = snapshot.task(*task_id).ok_or(CommandError::UnknownTask(*task_id))?;
            let mut writes = stop_running(snapshot, *at, |_| true);
            writes.extend([
                write(TimeEntry, *entry_id, fields::ENTRY_TASK_ID, json!(task_id)),
                write(TimeEntry, *entry_id, fields::QUEST_ID, json!(task.quest_id)),
                write(TimeEntry, *entry_id, fields::ENTRY_STARTED_AT, json!(at)),
                write(TimeEntry, *entry_id, fields::ENTRY_ENDED_AT, Value::Null),
                in_workspace(TimeEntry, *entry_id, snapshot),
            ]);
            writes
        }
        Command::StopFocus { at } => stop_running(snapshot, *at, |_| true),
        Command::LogTime { entry_id, task_id, started_at, ended_at } => {
            if ended_at <= started_at {
                return Err(CommandError::InvalidTimeRange);
            }
            let quest_id = task_quest(snapshot, *task_id)?;
            vec![
                write(TimeEntry, *entry_id, fields::ENTRY_TASK_ID, json!(task_id)),
                write(TimeEntry, *entry_id, fields::QUEST_ID, json!(quest_id)),
                write(TimeEntry, *entry_id, fields::ENTRY_STARTED_AT, json!(started_at)),
                write(TimeEntry, *entry_id, fields::ENTRY_ENDED_AT, json!(ended_at)),
                in_workspace(TimeEntry, *entry_id, snapshot),
            ]
        }
        Command::LogWork { entry_id, task, started_at, ended_at } => {
            log_work(snapshot, *entry_id, task, *started_at, *ended_at)?
        }
        Command::EditEntry { id, task, quest_id, started_at, ended_at } => {
            edit_entry::plan_edit(snapshot, *id, task.as_ref(), *quest_id, *started_at, *ended_at)?
        }
        Command::SetEntryTimes { id, started_at, ended_at } => {
            let entry = entry(snapshot, *id)?;
            if !entry.is_running() && ended_at.is_none() {
                return Err(CommandError::CannotResumeEntry);
            }
            if ended_at.is_some_and(|end| end <= *started_at) {
                return Err(CommandError::InvalidTimeRange);
            }
            let mut writes = vec![write(TimeEntry, *id, fields::ENTRY_STARTED_AT, json!(started_at))];
            if let Some(end) = ended_at {
                writes.push(write(TimeEntry, *id, fields::ENTRY_ENDED_AT, json!(end)));
            }
            writes
        }
        Command::AssignEntry { id, task_id } => {
            entry(snapshot, *id)?;
            let quest_id = task_quest(snapshot, *task_id)?;
            vec![
                write(TimeEntry, *id, fields::ENTRY_TASK_ID, json!(task_id)),
                write(TimeEntry, *id, fields::QUEST_ID, json!(quest_id)),
            ]
        }
        Command::DeleteEntry { id, at } => {
            entry(snapshot, *id)?;
            vec![write(TimeEntry, *id, fields::DELETED_AT, json!(at))]
        }

        Command::SetWeeklyTarget { minutes } => {
            vec![write(Settings, snapshot.workspace, fields::SETTINGS_WEEKLY_TARGET_MINUTES, json!(minutes))]
        }

        Command::CreateWorkspace { id, name } => {
            if snapshot.workspace(*id).is_some() {
                return Err(CommandError::WorkspaceExists(*id));
            }
            let mut writes = keep_default_revived(snapshot);
            writes.push(write(Workspace, *id, fields::WORKSPACE_NAME, json!(title(name)?)));
            writes
        }
        Command::RenameWorkspace { id, name } => {
            snapshot.live_workspace(*id).ok_or(CommandError::UnknownWorkspace(*id))?;
            vec![write(Workspace, *id, fields::WORKSPACE_NAME, json!(title(name)?))]
        }
        Command::DeleteWorkspace { id, at } => {
            snapshot.check_deletable(*id)?;
            let mut writes = stop_running(snapshot, *at, |running| running.workspace == *id);
            writes.push(write(Workspace, *id, fields::DELETED_AT, json!(at)));
            writes
        }
        Command::RestoreWorkspace { id } => {
            let workspace = snapshot.workspace(*id).ok_or(CommandError::UnknownWorkspace(*id))?;
            if !workspace.is_deleted() {
                return Err(CommandError::WorkspaceNotDeleted(*id));
            }
            let mut writes = keep_default_revived(snapshot);
            writes.push(write(Workspace, *id, fields::DELETED_AT, Value::Null));
            writes
        }
        Command::Restore { kind, id } => restore(snapshot, *kind, *id)?,
    };
    Ok(writes)
}

fn log_work(
    snapshot: &Snapshot,
    entry_id: EntityId,
    task: &WorkTask,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
) -> Result<Vec<FieldWrite>, CommandError> {
    if ended_at <= started_at {
        return Err(CommandError::InvalidTimeRange);
    }
    let (task_id, quest_id, mut writes) = match task {
        WorkTask::Existing(id) => (*id, task_quest(snapshot, Some(*id))?, Vec::new()),
        WorkTask::New { id, title, quest_id, day } => {
            let writes = edit_entry::completed_task(snapshot, *id, title, *quest_id, *day, ended_at)?;
            (*id, *quest_id, writes)
        }
    };
    writes.extend([
        write(EntityKind::TimeEntry, entry_id, fields::ENTRY_TASK_ID, json!(task_id)),
        write(EntityKind::TimeEntry, entry_id, fields::QUEST_ID, json!(quest_id)),
        write(EntityKind::TimeEntry, entry_id, fields::ENTRY_STARTED_AT, json!(started_at)),
        write(EntityKind::TimeEntry, entry_id, fields::ENTRY_ENDED_AT, json!(ended_at)),
        in_workspace(EntityKind::TimeEntry, entry_id, snapshot),
    ]);
    Ok(writes)
}

fn milestone_exists(snapshot: &Snapshot, id: EntityId) -> Result<(), CommandError> {
    snapshot.milestones.iter().any(|m| m.id == id).then_some(()).ok_or(CommandError::UnknownMilestone(id))
}

fn restore(snapshot: &Snapshot, kind: EntityKind, id: EntityId) -> Result<Vec<FieldWrite>, CommandError> {
    let trashed =
        snapshot.trash.iter().find(|t| t.kind == kind && t.id == id).ok_or(CommandError::NotInTrash { kind, id })?;
    let mut writes = vec![write(kind, id, fields::DELETED_AT, Value::Null)];
    if kind == EntityKind::Quest {
        // Milestones are not materialised while deleted, so find them through the trash: the
        // ones deleted together with the quest share its timestamp.
        let deleted_with_quest = snapshot.trash.iter().filter(|t| {
            t.kind == EntityKind::Milestone && t.quest_id == Some(id) && t.deleted_at == trashed.deleted_at
        });
        writes.extend(deleted_with_quest.map(|m| write(EntityKind::Milestone, m.id, fields::DELETED_AT, Value::Null)));
    }
    Ok(writes)
}

fn entry(snapshot: &Snapshot, id: EntityId) -> Result<&crate::model::TimeEntry, CommandError> {
    snapshot.time_entries.iter().find(|e| e.id == id).ok_or(CommandError::UnknownEntry(id))
}

/// The quest a session on `task` counts towards.
fn task_quest(snapshot: &Snapshot, task: Option<EntityId>) -> Result<Option<EntityId>, CommandError> {
    let Some(task) = task else { return Ok(None) };
    Ok(snapshot.task(task).ok_or(CommandError::UnknownTask(task))?.quest_id)
}

/// Restores the default workspace in storage when it is only shown because every workspace was
/// deleted: otherwise it would count as deleted again once another one is live.
fn keep_default_revived(snapshot: &Snapshot) -> Vec<FieldWrite> {
    if !snapshot.default_revived {
        return Vec::new();
    }
    vec![write(EntityKind::Workspace, EntityId::DEFAULT_WORKSPACE, fields::DELETED_AT, Value::Null)]
}

/// Ends every running entry matching `which` (normally at most one), in any workspace: one
/// person focuses on one thing at a time.
fn stop_running(snapshot: &Snapshot, at: DateTime<Utc>, which: impl Fn(&RunningFocus) -> bool) -> Vec<FieldWrite> {
    snapshot
        .running
        .iter()
        .filter(|running| which(running))
        .map(|RunningFocus { entry, .. }| {
            write(EntityKind::TimeEntry, entry.id, fields::ENTRY_ENDED_AT, json!(at.max(entry.started_at)))
        })
        .collect()
}

fn task_sort_key(snapshot: &Snapshot, moving: EntityId, day: Option<NaiveDate>, position: Position) -> String {
    let siblings: Vec<&str> =
        snapshot.tasks.iter().filter(|t| t.day == day && t.id != moving).map(|t| t.sort_key.as_str()).collect();
    match position {
        Position::Top => sort_key_between(None, siblings.first().copied()),
        Position::Bottom => sort_key_between(siblings.last().copied(), None),
        Position::Before(anchor) => {
            let anchor_key = snapshot.task(anchor).filter(|t| t.day == day).map(|t| t.sort_key.as_str());
            let Some(anchor_key) = anchor_key else {
                return sort_key_between(siblings.last().copied(), None);
            };
            let previous = siblings.iter().copied().rfind(|key| *key < anchor_key);
            sort_key_between(previous, Some(anchor_key))
        }
    }
}

#[cfg(test)]
mod log_work_tests;
#[cfg(test)]
mod tests;
