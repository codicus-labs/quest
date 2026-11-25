//! Records of the Quests page, a quest's page and the archive. Like `types`: plain values,
//! ready-made labels, ids as strings, colours as hex.

use crate::types::{HomeTask, UndoToast};

/// A quest's colour (the desktop's nine).
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum QuestHue {
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

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct HueOption {
    pub hue: QuestHue,
    /// "Red", for TalkBack.
    pub name: String,
    pub color_hex: String,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MilestoneRow {
    pub id: String,
    pub title: String,
    /// "Nov 24, 2026".
    pub date_label: String,
    /// "Show, Nov 24, 2026", for TalkBack.
    pub a11y_label: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct QuestCard {
    pub id: String,
    pub title: String,
    pub color_hex: String,
    /// Quest Hours this week: "0.3h".
    pub week_hours: String,
    /// "0.3h total".
    pub total_hours: String,
    /// "2 open tasks" / "1 open task".
    pub open_tasks: String,
    /// "Photo series, 0.3h this week, 0.3h total, 2 open tasks".
    pub a11y_label: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct QuestsData {
    /// Quest Hours ever, of the quests that exist: "0.3h".
    pub all_time_hours: String,
    /// Active quests, two digits at least: "02".
    pub in_progress: String,
    /// Completed quests: "01". Archived ones never count.
    pub completed_count: String,
    /// Active quests in their order.
    pub active: Vec<QuestCard>,
    /// Completed quests in their order (not the archived ones: they are in the archive).
    pub completed: Vec<QuestCard>,
    /// "Active · 1".
    pub active_label: String,
    /// "Completed · 0".
    pub completed_label: String,
    /// The nine colours for a new quest, the one it gets unless another is picked selected.
    pub new_quest_hues: Vec<HueOption>,
    pub toast: Option<UndoToast>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct QuestTaskGroup {
    /// "Today", "Tomorrow", "Yesterday", "Mon, Nov 30", "Backlog" or "Done".
    pub label: String,
    pub tasks: Vec<HomeTask>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct QuestDetail {
    pub id: String,
    pub title: String,
    /// Plain text, possibly several lines; empty when there is none.
    pub description: String,
    pub color_hex: String,
    /// All nine colours, the quest's selected.
    pub hues: Vec<HueOption>,
    pub completed: bool,
    /// "Complete quest", "Reopen quest", or "Unarchive" for a quest an older version archived.
    pub status_action: String,
    /// "Completed" or "Archived" next to the name; none while active.
    pub badge: Option<String>,
    /// The editor's line: "0.3h this week · 1.2h total · 2 open tasks".
    pub summary: String,
    /// "0.3h" this week.
    pub week_hours: String,
    /// "1.2h" ever.
    pub total_hours: String,
    pub open_count: u32,
    pub done_count: u32,
    /// Open tasks by day, then the backlog, then what's done.
    pub task_groups: Vec<QuestTaskGroup>,
    /// Newest first, to read only; empty: the page has no milestones section.
    pub milestones: Vec<MilestoneRow>,
    pub toast: Option<UndoToast>,
}

/// What kind of deleted thing an archive row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum TrashKind {
    Quest,
    Task,
    Milestone,
    Session,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ArchiveItem {
    pub kind: TrashKind,
    pub id: String,
    /// A session without a task is "Other work".
    pub title: String,
    /// "Photo series · Deleted Nov 24, 14:05".
    pub meta: String,
    pub color_hex: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ArchiveGroup {
    /// "Deleted quests", "Deleted tasks", "Deleted milestones", "Deleted sessions".
    pub title: String,
    /// Newest first.
    pub items: Vec<ArchiveItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ArchiveData {
    /// Quests an older version archived ("Archived quests"), to unarchive or open.
    pub archived_quests: Vec<ArchiveItem>,
    /// Only groups with items; none and no archived quests: "Nothing archived or deleted."
    pub groups: Vec<ArchiveGroup>,
}
