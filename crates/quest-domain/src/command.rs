//! User intents and the pure policy that turns them into field writes. The store stamps the
//! writes with its clock, persists them and syncs them; nothing here performs side effects.

mod checklist;
mod edit_entry;
mod task_dates;
mod timer;

use chrono::{DateTime, NaiveDate, Utc};
use serde_json::{Value, json};

use crate::change::{EntityId, EntityKind, FieldWrite};
use crate::model::{EntryKind, QuestColor, QuestStatus, fields};
use crate::recurrence::{self, RepeatRule};
use crate::snapshot::{RunningFocus, Snapshot};
use crate::sort_key::sort_key_between;

pub(crate) use task_dates::date_writes;

/// Where a new or moved task lands within its day, or an item within its checklist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
    Before(EntityId),
}

/// The task a `LogWork` session is for: one that exists (open or done), or a new one that is
/// created already done, planned on the day of the work.
#[derive(Clone, Debug, PartialEq)]
pub enum WorkTask {
    Existing(EntityId),
    New { id: EntityId, title: String, quest_id: Option<EntityId>, planned_on: NaiveDate },
}

/// Ids of created entities are chosen by the caller, so the UI can refer to them right away.
/// Commands that change a task's dates take the time `at`: moving a date later records a
/// `Postponement` then.
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
    /// Soft-deletes a quest. Its tasks and sessions stay and count as other work until the
    /// quest is restored.
    DeleteQuest {
        id: EntityId,
        at: DateTime<Utc>,
    },

    /// A new task. A deadline before the plan date takes the plan date along (see `set_due`).
    CreateTask {
        id: EntityId,
        title: String,
        quest_id: Option<EntityId>,
        planned_on: Option<NaiveDate>,
        due_on: Option<NaiveDate>,
        position: Position,
    },
    RenameTask {
        id: EntityId,
        title: String,
    },
    /// Markdown notes; empty clears them.
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
    /// Runs `edit` on a repeat's projected occurrence, creating the occurrence first.
    EditPlan {
        occurrence: Option<(EntityId, NaiveDate)>,
        now: DateTime<Utc>,
        edit: Box<Command>,
    },
    AssignTask {
        id: EntityId,
        quest_id: Option<EntityId>,
    },
    /// Plans a task for a day (or unplans it with `None`) at a position in that day's list;
    /// see `set_planned`. Also reorders within a day.
    MoveTask {
        id: EntityId,
        planned_on: Option<NaiveDate>,
        position: Position,
        at: DateTime<Utc>,
    },
    /// Sets or clears the deadline; see `set_due`.
    SetTaskDue {
        id: EntityId,
        due_on: Option<NaiveDate>,
        at: DateTime<Utc>,
    },
    CompleteTask {
        id: EntityId,
        at: DateTime<Utc>,
    },
    /// Completes a task from the plan: a repeat's occurrence planned for a later day moves to
    /// `today`, so the next one is not skipped.
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

    AddChecklistItem {
        id: EntityId,
        task_id: EntityId,
        text: String,
        position: Position,
    },
    RenameChecklistItem {
        id: EntityId,
        text: String,
    },
    /// Ticks an item off or opens it again.
    SetChecklistItemDone {
        id: EntityId,
        done: bool,
    },
    MoveChecklistItem {
        id: EntityId,
        position: Position,
    },
    DeleteChecklistItem {
        id: EntityId,
        at: DateTime<Utc>,
    },

    /// Takes a postponement back (undoing the move that recorded it).
    DeletePostponement {
        id: EntityId,
        at: DateTime<Utc>,
    },

    /// Starts tracking time on a task; stops whatever else is running.
    StartFocus {
        entry_id: EntityId,
        task_id: EntityId,
        at: DateTime<Utc>,
    },
    /// Starts tracking organizing; stops whatever else is running. `origin_task_id` is the task
    /// to go back to.
    StartOrganizing {
        entry_id: EntityId,
        origin_task_id: Option<EntityId>,
        at: DateTime<Utc>,
    },
    /// Books the running task session as organizing from `since` on: the task session ends at
    /// `since` and an organizing session (back to that task) runs from there.
    RebookAsOrganizing {
        entry_id: EntityId,
        since: DateTime<Utc>,
        now: DateTime<Utc>,
    },
    StopFocus {
        at: DateTime<Utc>,
    },
    /// Records a finished session after the fact; without a task it is other work.
    LogTime {
        entry_id: EntityId,
        task_id: Option<EntityId>,
        started_at: DateTime<Utc>,
        ended_at: DateTime<Utc>,
    },
    /// Logs finished work after the fact (the Log work dialog), all or nothing: the session,
    /// and for a new task that task, created done (at the session's end). The session counts
    /// towards the task's quest.
    LogWork {
        entry_id: EntityId,
        task: WorkTask,
        started_at: DateTime<Utc>,
        ended_at: DateTime<Utc>,
    },
    /// Edits a session. Giving an organizing session a task or quest makes it task work.
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

    /// Undoes a soft delete of a quest, task, checklist item or session.
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
    #[error("checklist item {0} does not exist")]
    UnknownChecklistItem(EntityId),
    #[error("postponement {0} does not exist")]
    UnknownPostponement(EntityId),
    #[error("session {0} does not exist")]
    UnknownEntry(EntityId),
    #[error("a session must end after it starts")]
    InvalidTimeRange,
    #[error("a finished session cannot be resumed by editing its times; start a new session instead")]
    CannotResumeEntry,
    #[error("no task session is running; start one before booking time as organizing")]
    NothingToRebook,
    #[error("organizing can only start within the running session, after its start and not in the future")]
    RebookOutsideEntry,
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
    use EntityKind::{Quest, Settings, Task, TimeEntry, Workspace};
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
        Command::DeleteQuest { id, at } => {
            quest_exists(*id)?;
            vec![write(Quest, *id, fields::DELETED_AT, json!(at))]
        }

        Command::CreateTask { id, title: raw, quest_id, planned_on, due_on, position } => {
            task_dates::create(snapshot, *id, raw, *quest_id, *planned_on, *due_on, *position)?
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
        Command::MoveTask { id, planned_on, position, at } => {
            task_dates::move_task(snapshot, *id, *planned_on, *position, *at)?
        }
        Command::SetTaskDue { id, due_on, at } => task_dates::set_task_due(snapshot, *id, *due_on, *at)?,
        Command::CompleteTask { id, at } | Command::CompleteTaskFromPlan { id, at, .. } => {
            let task = snapshot.task(*id).ok_or(CommandError::UnknownTask(*id))?;
            let mut writes = stop_running(snapshot, *at, |running| running.entry.task_id == Some(*id));
            writes.push(write(Task, *id, fields::TASK_COMPLETED_AT, json!(at)));
            if let Command::CompleteTaskFromPlan { today, .. } = command
                && task.planned_on.is_some_and(|day| day > *today)
                && (task.repeat.is_some()
                    || task.series.and_then(|id| snapshot.task(id)).is_some_and(|source| source.repeat.is_some()))
            {
                writes.extend(task_dates::move_task(snapshot, *id, Some(*today), Position::Bottom, *at)?);
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

        Command::AddChecklistItem { id, task_id, text, position } => {
            checklist::add(snapshot, *id, *task_id, text, *position)?
        }
        Command::RenameChecklistItem { id, text } => checklist::rename(snapshot, *id, text)?,
        Command::SetChecklistItemDone { id, done } => checklist::set_done(snapshot, *id, *done)?,
        Command::MoveChecklistItem { id, position } => checklist::move_item(snapshot, *id, *position)?,
        Command::DeleteChecklistItem { id, at } => checklist::delete(snapshot, *id, *at)?,

        Command::DeletePostponement { id, at } => {
            snapshot.postponements.iter().find(|p| p.id == *id).ok_or(CommandError::UnknownPostponement(*id))?;
            vec![write(EntityKind::Postponement, *id, fields::DELETED_AT, json!(at))]
        }

        Command::StartFocus { entry_id, task_id, at } => {
            let task = snapshot.task(*task_id).ok_or(CommandError::UnknownTask(*task_id))?;
            let mut writes = stop_running(snapshot, *at, |_| true);
            writes.extend(entry_writes(*entry_id, Some(*task_id), task.quest_id, *at, None));
            writes.push(in_workspace(TimeEntry, *entry_id, snapshot));
            writes
        }
        Command::StartOrganizing { entry_id, origin_task_id, at } => {
            timer::start_organizing(snapshot, *entry_id, *origin_task_id, *at)?
        }
        Command::RebookAsOrganizing { entry_id, since, now } => timer::rebook(snapshot, *entry_id, *since, *now)?,
        Command::StopFocus { at } => stop_running(snapshot, *at, |_| true),
        Command::LogTime { entry_id, task_id, started_at, ended_at } => {
            if ended_at <= started_at {
                return Err(CommandError::InvalidTimeRange);
            }
            let quest_id = task_quest(snapshot, *task_id)?;
            let mut writes = entry_writes(*entry_id, *task_id, quest_id, *started_at, Some(*ended_at)).to_vec();
            writes.push(in_workspace(TimeEntry, *entry_id, snapshot));
            writes
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
        Command::Restore { kind, id } => {
            snapshot
                .trash
                .iter()
                .find(|t| t.kind == *kind && t.id == *id)
                .ok_or(CommandError::NotInTrash { kind: *kind, id: *id })?;
            vec![write(*kind, *id, fields::DELETED_AT, Value::Null)]
        }
    };
    Ok(writes)
}

/// The fields of a task session (without its workspace).
fn entry_writes(
    id: EntityId,
    task_id: Option<EntityId>,
    quest_id: Option<EntityId>,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
) -> [FieldWrite; 5] {
    use EntityKind::TimeEntry;
    [
        write(TimeEntry, id, fields::ENTRY_KIND, json!(EntryKind::Task)),
        write(TimeEntry, id, fields::TASK_ID, json!(task_id)),
        write(TimeEntry, id, fields::QUEST_ID, json!(quest_id)),
        write(TimeEntry, id, fields::ENTRY_STARTED_AT, json!(started_at)),
        write(TimeEntry, id, fields::ENTRY_ENDED_AT, json!(ended_at)),
    ]
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
        WorkTask::New { id, title, quest_id, planned_on } => {
            let writes = edit_entry::completed_task(snapshot, *id, title, *quest_id, *planned_on, ended_at)?;
            (*id, *quest_id, writes)
        }
    };
    writes.extend(entry_writes(entry_id, Some(task_id), quest_id, started_at, Some(ended_at)));
    writes.push(in_workspace(EntityKind::TimeEntry, entry_id, snapshot));
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

/// A key at `position` among `siblings` (sorted keys, the moving record excluded); `anchor` is
/// the key of `Position::Before`'s record if it is among them.
fn key_at(siblings: &[&str], position: Position, anchor: Option<&str>) -> String {
    match position {
        Position::Top => sort_key_between(None, siblings.first().copied()),
        Position::Bottom => sort_key_between(siblings.last().copied(), None),
        Position::Before(_) => {
            let Some(anchor_key) = anchor else {
                return sort_key_between(siblings.last().copied(), None);
            };
            let previous = siblings.iter().copied().rfind(|key| *key < anchor_key);
            sort_key_between(previous, Some(anchor_key))
        }
    }
}

fn task_sort_key(snapshot: &Snapshot, moving: EntityId, day: Option<NaiveDate>, position: Position) -> String {
    let siblings: Vec<&str> =
        snapshot.tasks.iter().filter(|t| t.planned_on == day && t.id != moving).map(|t| t.sort_key.as_str()).collect();
    let anchor = match position {
        Position::Before(anchor) => {
            snapshot.task(anchor).filter(|t| t.planned_on == day && t.id != moving).map(|t| t.sort_key.as_str())
        }
        Position::Top | Position::Bottom => None,
    };
    key_at(&siblings, position, anchor)
}

#[cfg(test)]
mod checklist_tests;
#[cfg(test)]
mod dates_tests;
#[cfg(test)]
mod log_work_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod timer_tests;
