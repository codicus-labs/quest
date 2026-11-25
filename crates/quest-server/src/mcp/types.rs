//! Inputs and outputs of the tools, with JSON Schemas for the models. Days are ISO dates
//! (`2026-11-24`) in the call's time zone; instants are RFC 3339 with that zone's offset.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What every tool takes: the user's IANA time zone and the workspace it works in.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct CallContext {
    /// The user's IANA time zone, e.g. "Europe/Berlin". Defaults to the server's.
    #[serde(default)]
    pub time_zone: Option<String>,
    /// The workspace to read and write: its id or its exact name (case-insensitive), from
    /// list_workspaces. Defaults to the default workspace.
    #[serde(default)]
    pub workspace: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DoneFilter {
    Open,
    Done,
    #[default]
    All,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct ListTasksInput {
    /// One plan date (ISO date). Without day, from/to and unplanned: today.
    #[serde(default)]
    pub day: Option<String>,
    /// First day of a range (ISO date, inclusive).
    #[serde(default)]
    pub from: Option<String>,
    /// Last day of a range (ISO date, inclusive); at most 62 days after `from`.
    #[serde(default)]
    pub to: Option<String>,
    /// true: tasks without a plan date instead of days.
    #[serde(default)]
    pub unplanned: bool,
    /// Only tasks of this quest.
    #[serde(default)]
    pub quest_id: Option<String>,
    /// open, done or all (default).
    #[serde(default)]
    pub status: DoneFilter,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct TaskIdInput {
    pub task_id: String,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuestFilter {
    #[default]
    Active,
    Completed,
    All,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct ListQuestsInput {
    /// active (default), completed or all.
    #[serde(default)]
    pub status: QuestFilter,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct QuestIdInput {
    pub quest_id: String,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct SearchInput {
    /// Words to find in titles and descriptions (case-insensitive).
    pub query: String,
    /// Also search the archive (deleted items that `restore` can bring back).
    #[serde(default)]
    pub include_archived: bool,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct WeekInput {
    /// Any day of the week (ISO date); default this week.
    #[serde(default)]
    pub week_of: Option<String>,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct ContextInput {
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct ListEntriesInput {
    /// First day (ISO date, inclusive); default today.
    #[serde(default)]
    pub from: Option<String>,
    /// Last day (ISO date, inclusive); default `from`. At most 62 days after `from`.
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub quest_id: Option<String>,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrequencyInput {
    Day,
    Week,
    Month,
    Year,
}

/// When a task repeats. The series starts on the task's plan date (or `starts`).
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct RepeatInput {
    /// Every n days/weeks/months/years (1 to 999); default 1.
    #[serde(default)]
    pub every: Option<u32>,
    pub frequency: FrequencyInput,
    /// Weekly only: weekdays like ["mon", "thu"]; default the start's weekday.
    #[serde(default)]
    pub weekdays: Vec<String>,
    /// Monthly only: day of month 1 to 31, or "last"; default the start's day.
    #[serde(default)]
    pub month_day: Option<String>,
    /// First day (ISO date); default the task's plan date, else today.
    #[serde(default)]
    pub starts: Option<String>,
    /// Last day (ISO date); default never.
    #[serde(default)]
    pub ends: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct CreateTaskInput {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Plan date (ISO date); default none.
    #[serde(default)]
    pub planned_on: Option<String>,
    /// Deadline (ISO date); default none.
    #[serde(default)]
    pub due_on: Option<String>,
    #[serde(default)]
    pub quest_id: Option<String>,
    #[serde(default)]
    pub repeat: Option<RepeatInput>,
    #[serde(flatten)]
    pub context: CallContext,
}

/// Only the given fields change.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct UpdateTaskInput {
    pub task_id: String,
    #[serde(default)]
    pub title: Option<String>,
    /// Replaces the description; "" clears it.
    #[serde(default)]
    pub description: Option<String>,
    /// Plan date (ISO date), or "none" to unplan. Moves the task to the end of that day.
    #[serde(default)]
    pub planned_on: Option<String>,
    /// Deadline (ISO date), or "none" to clear it.
    #[serde(default)]
    pub due_on: Option<String>,
    /// A quest id, or "none" to take the task out of its quest.
    #[serde(default)]
    pub quest_id: Option<String>,
    /// Sets the repeat of the task's series.
    #[serde(default)]
    pub repeat: Option<RepeatInput>,
    /// true: stop repeating.
    #[serde(default)]
    pub clear_repeat: bool,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PositionInput {
    Top,
    #[default]
    Bottom,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct MoveTaskInput {
    pub task_id: String,
    /// Plan date (ISO date), or "none" to unplan.
    pub planned_on: String,
    /// top or bottom (default) of that day's list.
    #[serde(default)]
    pub position: PositionInput,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Task,
    Quest,
    ChecklistItem,
    TimeEntry,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct RestoreInput {
    pub kind: ItemKind,
    pub id: String,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColorInput {
    Red,
    Orange,
    Yellow,
    Lime,
    Green,
    Teal,
    Blue,
    Violet,
    Pink,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct CreateQuestInput {
    pub title: String,
    /// Default: the next unused color.
    #[serde(default)]
    pub color: Option<ColorInput>,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuestStatusInput {
    Active,
    Completed,
}

/// Only the given fields change.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct UpdateQuestInput {
    pub quest_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub color: Option<ColorInput>,
    /// completed = done (moves to Completed); active reopens it.
    #[serde(default)]
    pub status: Option<QuestStatusInput>,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct AddChecklistItemInput {
    pub task_id: String,
    pub text: String,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct CheckChecklistItemInput {
    pub item_id: String,
    /// true (default): done; false: open again.
    #[serde(default = "yes")]
    pub done: bool,
    #[serde(flatten)]
    pub context: CallContext,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct RenameChecklistItemInput {
    pub item_id: String,
    pub text: String,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct ChecklistItemIdInput {
    pub item_id: String,
    #[serde(flatten)]
    pub context: CallContext,
}

/// Tracks a task, or organizing (sorting out lists between tasks).
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
pub struct StartTimerInput {
    /// The task to track; with organizing, the task to go back to (optional).
    #[serde(default)]
    pub task_id: Option<String>,
    /// true: track organizing instead of the task.
    #[serde(default)]
    pub organizing: bool,
    #[serde(flatten)]
    pub context: CallContext,
}

/// Finished work: an existing task (`task_id`) or a new one (`new_task_title`, created done).
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct LogWorkInput {
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub new_task_title: Option<String>,
    /// Quest of a new task.
    #[serde(default)]
    pub quest_id: Option<String>,
    /// ISO date; default today.
    #[serde(default)]
    pub day: Option<String>,
    /// Local "HH:MM".
    pub start: String,
    /// Local "HH:MM", after the start on the same day.
    pub end: String,
    #[serde(flatten)]
    pub context: CallContext,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct CaptureInput {
    /// Free text describing quests, tasks and finished work, in any language.
    pub text: String,
    /// false (default): only return the proposal. true: create everything in it (all or nothing).
    #[serde(default)]
    pub apply: bool,
    /// BCP 47 language of the text, e.g. "de-DE"; default "en".
    #[serde(default)]
    pub language: Option<String>,
    #[serde(flatten)]
    pub context: CallContext,
}

// Outputs.

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RefOut {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct TaskOut {
    pub id: String,
    pub title: String,
    pub description: String,
    pub quest: Option<RefOut>,
    /// Plan date (ISO date); null: not planned.
    pub planned_on: Option<String>,
    /// Deadline (ISO date); null: none.
    pub due_on: Option<String>,
    pub done: bool,
    /// RFC 3339.
    pub completed_at: Option<String>,
    /// e.g. "Every 2 weeks"; null if it does not repeat.
    pub repeat: Option<String>,
    /// A planned repeat not saved yet; changing it saves it.
    pub planned_repeat: bool,
    pub tracked_minutes: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct TasksOut {
    pub time_zone: String,
    pub tasks: Vec<TaskOut>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct TaskResult {
    pub time_zone: String,
    pub task: TaskOut,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct QuestOut {
    pub id: String,
    pub title: String,
    pub color: String,
    /// active, completed, or archived (put away by an older app version; not counted as completed).
    pub status: String,
    pub tracked_minutes: i64,
    pub open_tasks: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct QuestsOut {
    pub quests: Vec<QuestOut>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct QuestDetailOut {
    pub time_zone: String,
    pub quest: QuestOut,
    /// Open tasks first (by plan date), then the 20 most recently done.
    pub tasks: Vec<TaskOut>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct QuestResult {
    pub quest: QuestOut,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ChecklistItemOut {
    pub id: String,
    pub text: String,
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ChecklistOut {
    pub task: RefOut,
    /// In display order.
    pub items: Vec<ChecklistItemOut>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ArchivedOut {
    pub kind: ItemKind,
    pub id: String,
    pub title: Option<String>,
    /// RFC 3339.
    pub archived_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SearchOut {
    pub time_zone: String,
    pub quests: Vec<QuestOut>,
    pub tasks: Vec<TaskOut>,
    /// Only with include_archived.
    pub archived: Vec<ArchivedOut>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct QuestMinutesOut {
    pub quest: RefOut,
    pub minutes: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct DayMinutesOut {
    /// ISO date.
    pub day: String,
    pub quest_minutes: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct WeekReviewOut {
    pub time_zone: String,
    /// Monday, ISO date.
    pub week_start: String,
    pub quest_minutes: i64,
    /// Sorting out lists between tasks.
    pub organizing_minutes: i64,
    /// Other tracked time not linked to a quest.
    pub other_minutes: i64,
    pub target_minutes: i64,
    pub quest_share_percent: u8,
    pub days: Vec<DayMinutesOut>,
    /// Most time first.
    pub quests: Vec<QuestMinutesOut>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntryKindOut {
    Task,
    Organizing,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct EntryOut {
    pub id: String,
    /// task: work (on a task or other work); organizing: sorting out lists.
    pub kind: EntryKindOut,
    pub task: Option<RefOut>,
    /// Organizing only: the task to go back to.
    pub origin_task: Option<RefOut>,
    pub quest: Option<RefOut>,
    /// RFC 3339.
    pub started_at: String,
    /// RFC 3339; null while running.
    pub ended_at: Option<String>,
    pub minutes: i64,
    pub running: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct EntriesOut {
    pub time_zone: String,
    pub entries: Vec<EntryOut>,
    pub total_minutes: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct TimerOut {
    pub time_zone: String,
    /// null: no timer runs.
    pub running: Option<EntryOut>,
    /// The workspace the session is in (one timer per account); null when none runs.
    pub workspace: Option<RefOut>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct StopOut {
    pub time_zone: String,
    /// The session that was stopped; null if discarded or no timer was running.
    pub stopped: Option<EntryOut>,
    pub discarded: bool,
    /// The workspace the stopped session is in; null if no timer was running.
    pub workspace: Option<RefOut>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct EntryResult {
    pub time_zone: String,
    pub entry: EntryOut,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ArchiveResult {
    pub archived: ArchivedOut,
    /// How to undo it.
    pub hint: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RestoreResult {
    pub restored: ItemKind,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct CaptureOut {
    pub applied: bool,
    /// What was recognised: quests, tasks and work, each with a temporary key.
    pub proposal: serde_json::Value,
    /// Items the AI answer had that were left out, as sentences.
    pub dropped: Vec<String>,
    /// e.g. "Created 1 quest, 2 tasks and 1 session"; empty unless applied.
    pub message: String,
    /// Ids of what was created (unless applied: empty).
    pub created: Vec<CreatedOut>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct CreatedOut {
    pub kind: ItemKind,
    pub id: String,
}

/// No arguments. The empty `properties` is there for clients that expect one on every input.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[schemars(extend("properties" = {}))]
pub struct ListWorkspacesInput {}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct WorkspaceOut {
    pub id: String,
    pub name: String,
    /// The workspace tools use without a workspace argument.
    pub is_default: bool,
    pub quests: usize,
    pub tasks: usize,
    /// All tracked time in the workspace, rounded to a tenth of an hour.
    pub tracked_hours: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct WorkspacesOut {
    /// The default workspace first, then by creation.
    pub workspaces: Vec<WorkspaceOut>,
}
