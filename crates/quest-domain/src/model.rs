//! Typed entities materialised from merged fields. A record missing a required field (for
//! example, half of it has not synced yet) is not materialised at all.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::change::{EntityId, FieldValues};
use crate::recurrence::{Occurrence, RepeatSchedule};

/// Field names on the wire and in storage. Every entity also has `deleted_at`: deleting is a
/// timestamped tombstone (soft delete), so nothing is lost and a delete can be undone.
pub mod fields {
    pub const DELETED_AT: &str = "deleted_at";
    pub const TITLE: &str = "title";
    pub const SORT_KEY: &str = "sort_key";
    pub const QUEST_ID: &str = "quest_id";

    pub const QUEST_COLOR: &str = "color";
    pub const QUEST_STATUS: &str = "status";
    pub const QUEST_DESCRIPTION: &str = "description";

    pub const TASK_DAY: &str = "day";
    pub const TASK_BLOCK: &str = "block";
    pub const TASK_COMPLETED_AT: &str = "completed_at";
    pub const TASK_DESCRIPTION: &str = "description";
    pub const TASK_REPEAT: &str = "repeat";
    pub const TASK_OCCURRENCE: &str = "occurrence";

    pub const MILESTONE_ACHIEVED_ON: &str = "achieved_on";

    pub const ENTRY_TASK_ID: &str = "task_id";
    pub const ENTRY_STARTED_AT: &str = "started_at";
    pub const ENTRY_ENDED_AT: &str = "ended_at";

    pub const SETTINGS_WEEKLY_TARGET_MINUTES: &str = "weekly_target_minutes";

    /// The workspace a quest, task, milestone or session belongs to. Absent on records from before
    /// workspaces and from older clients: those follow their parent (see `workspace.rs`).
    pub const WORKSPACE_ID: &str = "workspace_id";
    pub const WORKSPACE_NAME: &str = "name";
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestColor {
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

impl QuestColor {
    pub const ALL: [QuestColor; 9] = [
        QuestColor::Red,
        QuestColor::Orange,
        QuestColor::Yellow,
        QuestColor::Lime,
        QuestColor::Green,
        QuestColor::Teal,
        QuestColor::Blue,
        QuestColor::Violet,
        QuestColor::Pink,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestStatus {
    Active,
    /// Put away without being reached: the archive lists it. Quests completed before
    /// `Completed` existed stay here, so they never count as reached goals.
    Archived,
    /// Reached: Quests lists it under Completed and counts it. Its tracked time stays.
    Completed,
}

/// A long-term goal that tasks, tracked time and milestones belong to.
#[derive(Clone, Debug, PartialEq)]
pub struct Quest {
    pub id: EntityId,
    pub title: String,
    pub color: QuestColor,
    pub status: QuestStatus,
    pub sort_key: String,
    /// Plain text notes, possibly several lines; empty when there are none.
    pub description: String,
}

/// A planned slot on a day's calendar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeBlock {
    pub start: NaiveTime,
    pub minutes: u32,
}

/// A concrete piece of work. Without a `day` it sits in the backlog ("someday").
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub id: EntityId,
    pub title: String,
    pub quest_id: Option<EntityId>,
    pub day: Option<NaiveDate>,
    pub sort_key: String,
    pub block: Option<TimeBlock>,
    pub completed_at: Option<DateTime<Utc>>,
    /// Plain text notes, possibly several lines; empty when there are none.
    pub description: String,
    pub repeat: Option<RepeatSchedule>,
    pub series: Option<EntityId>,
}

impl Task {
    pub fn is_done(&self) -> bool {
        self.completed_at.is_some()
    }
}

/// A notable achievement on the way to a quest.
#[derive(Clone, Debug, PartialEq)]
pub struct Milestone {
    pub id: EntityId,
    pub quest_id: EntityId,
    pub title: String,
    pub achieved_on: NaiveDate,
}

/// Tracked time. Running while `ended_at` is `None`; at most one entry runs at a time (commands
/// enforce it, and the snapshot treats the latest-started one as the running one if two devices
/// started offline).
#[derive(Clone, Debug, PartialEq)]
pub struct TimeEntry {
    pub id: EntityId,
    pub task_id: Option<EntityId>,
    pub quest_id: Option<EntityId>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

impl TimeEntry {
    pub fn is_running(&self) -> bool {
        self.ended_at.is_none()
    }

    pub fn tracked_ms(&self, now: DateTime<Utc>) -> i64 {
        let end = self.ended_at.unwrap_or(now);
        (end - self.started_at).num_milliseconds().max(0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub weekly_target_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { weekly_target_minutes: 20 * 60 }
    }
}

/// A separate set of quests, tasks, sessions and settings within one account or guest profile.
/// Deleting one hides everything in it; restoring it brings everything back.
#[derive(Clone, Debug, PartialEq)]
pub struct Workspace {
    pub id: EntityId,
    pub name: String,
    pub deleted_at: Option<DateTime<Utc>>,
    /// What deleting it would hide.
    pub content: WorkspaceContent,
}

impl Workspace {
    /// The default workspace's name until the user renames it.
    pub const DEFAULT_NAME: &str = "Default";

    pub fn is_default(&self) -> bool {
        self.id == EntityId::DEFAULT_WORKSPACE
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// The default workspace exists without a record: older clients and integrations that know
    /// no workspaces write into it. Like any other it can be renamed, deleted and restored.
    pub(crate) fn from_fields(id: EntityId, values: Option<&FieldValues>) -> Option<Self> {
        let name: Option<String> = values.and_then(|values| get(values, fields::WORKSPACE_NAME));
        let is_default = id == EntityId::DEFAULT_WORKSPACE;
        let name = if is_default { name.unwrap_or_else(|| Workspace::DEFAULT_NAME.to_owned()) } else { name? };
        let deleted_at = values.and_then(deleted_at);
        Some(Workspace { id, name, deleted_at, content: WorkspaceContent::default() })
    }
}

/// Live records in a workspace. Tracked time counts finished sessions only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceContent {
    pub quests: usize,
    pub tasks: usize,
    pub tracked_ms: i64,
}

pub(crate) fn get<T: DeserializeOwned>(values: &FieldValues, field: &str) -> Option<T> {
    values.get(field).and_then(|(value, _)| serde_json::from_value(value.clone()).ok())
}

/// A nullable field: absent or `null` both mean `None`.
pub(crate) fn get_opt<T: DeserializeOwned>(values: &FieldValues, field: &str) -> Option<T> {
    get::<Option<T>>(values, field).flatten()
}

pub(crate) fn workspace_id(values: &FieldValues) -> Option<EntityId> {
    get(values, fields::WORKSPACE_ID)
}

pub(crate) fn deleted_at(values: &FieldValues) -> Option<DateTime<Utc>> {
    get_opt(values, fields::DELETED_AT)
}

impl Quest {
    pub(crate) fn from_fields(id: EntityId, values: &FieldValues) -> Option<Self> {
        Some(Quest {
            id,
            title: get(values, fields::TITLE)?,
            color: get(values, fields::QUEST_COLOR)?,
            status: get(values, fields::QUEST_STATUS).unwrap_or(QuestStatus::Active),
            sort_key: get(values, fields::SORT_KEY).unwrap_or_default(),
            description: get(values, fields::QUEST_DESCRIPTION).unwrap_or_default(),
        })
    }
}

impl Task {
    pub(crate) fn from_fields(id: EntityId, values: &FieldValues) -> Option<Self> {
        let initial: Option<Occurrence> = get(values, fields::TASK_OCCURRENCE);
        Some(Task {
            id,
            title: get(values, fields::TITLE).or_else(|| initial.as_ref().map(|o| o.title.clone()))?,
            quest_id: get(values, fields::QUEST_ID).unwrap_or_else(|| initial.as_ref().and_then(|o| o.quest_id)),
            day: get(values, fields::TASK_DAY).unwrap_or_else(|| initial.as_ref().map(|o| o.day)),
            sort_key: get(values, fields::SORT_KEY)
                .or_else(|| initial.as_ref().map(|o| o.sort_key.clone()))
                .unwrap_or_default(),
            block: get(values, fields::TASK_BLOCK).unwrap_or_else(|| initial.as_ref().and_then(|o| o.block)),
            completed_at: get_opt(values, fields::TASK_COMPLETED_AT),
            description: get(values, fields::TASK_DESCRIPTION)
                .or_else(|| initial.as_ref().map(|o| o.description.clone()))
                .unwrap_or_default(),
            repeat: get_opt(values, fields::TASK_REPEAT),
            series: initial.map(|o| o.series),
        })
    }
}

impl Milestone {
    pub(crate) fn from_fields(id: EntityId, values: &FieldValues) -> Option<Self> {
        Some(Milestone {
            id,
            quest_id: get(values, fields::QUEST_ID)?,
            title: get(values, fields::TITLE)?,
            achieved_on: get(values, fields::MILESTONE_ACHIEVED_ON)?,
        })
    }
}

impl TimeEntry {
    pub(crate) fn from_fields(id: EntityId, values: &FieldValues) -> Option<Self> {
        Some(TimeEntry {
            id,
            task_id: get_opt(values, fields::ENTRY_TASK_ID),
            quest_id: get_opt(values, fields::QUEST_ID),
            started_at: get(values, fields::ENTRY_STARTED_AT)?,
            ended_at: get_opt(values, fields::ENTRY_ENDED_AT),
        })
    }
}

impl Settings {
    pub(crate) fn from_fields(values: Option<&FieldValues>) -> Self {
        let defaults = Settings::default();
        let Some(values) = values else { return defaults };
        Settings {
            weekly_target_minutes: get(values, fields::SETTINGS_WEEKLY_TARGET_MINUTES)
                .unwrap_or(defaults.weekly_target_minutes),
        }
    }
}
