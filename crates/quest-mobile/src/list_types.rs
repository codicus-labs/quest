//! Records of the redesigned lists (Today, Inbox, Logbook, a quest's list), the Quests tab, the
//! Tracked tab and the mini player. Like `types`: plain values, ready-made labels, ids as strings.

use crate::types::{QuestChip, UndoToast};

/// Which list to show.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ListKind {
    Today,
    Inbox,
    Logbook,
    Quest { quest_id: String },
}

/// How a row's date mark reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum MarkTone {
    /// "overdue · Mon 23": the error colour.
    Overdue,
    /// "due today": the accent.
    Accent,
    /// "since Tue 24", "due Fri": muted.
    Muted,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RowMark {
    pub text: String,
    pub tone: MarkTone,
}

/// One task row: check, title, date marks below, quest dot, Play/Stop.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ListRow {
    pub id: String,
    pub title: String,
    pub done: bool,
    /// The quest's dot; None without a quest or on the quest's own list.
    pub quest: Option<QuestChip>,
    /// Date marks in reading order; empty for done tasks.
    pub marks: Vec<RowMark>,
    /// This task's timer runs: Stop with `timer` instead of Play.
    pub timer_running: bool,
    /// The running time ("36:13"), counting live; None while not running.
    pub timer: Option<String>,
    /// Whether Play shows (open tasks).
    pub can_start_timer: bool,
}

/// A run of rows under an optional label ("2 done", "Wednesday 25 November").
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ListSection {
    pub label: Option<String>,
    pub rows: Vec<ListRow>,
}

/// One list as a screen shows it.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct TaskList {
    pub kind: ListKind,
    /// "Today", "Inbox", "Logbook" or the quest's name.
    pub title: String,
    /// "Wednesday 25 November · 3.9h tracked", "8.9h this week · 3 open"; empty when none.
    pub subtitle: String,
    /// A quest's list: its colour and this week's share of the weekly target (for the ring).
    pub quest: Option<QuestChip>,
    pub ring: f32,
    /// Open rows first (no label), then e.g. "2 done"; the logbook groups by day.
    pub sections: Vec<ListSection>,
    /// Whether nothing open is listed (the screen shows "Inbox zero" / "Nothing here.").
    pub empty: bool,
    pub toast: Option<UndoToast>,
}

/// One quest in the Quests tab.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct QuestNavRow {
    pub id: String,
    pub title: String,
    pub color_hex: String,
    /// Hours this week, "8.9h".
    pub week_hours: String,
    /// The ring: this week's hours against the weekly target, 0.0..=1.0.
    pub ring: f32,
    /// "Launch my idea, 8.9h this week", for TalkBack.
    pub a11y_label: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct QuestNav {
    pub active: Vec<QuestNavRow>,
    pub completed: Vec<QuestNavRow>,
}

/// One block in the Tracked tab's timeline.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct TrackedBlock {
    pub entry_id: String,
    /// The task's title, "Organizing", or "Other work".
    pub title: String,
    /// The quest's colour; None: neutral (organizing, no quest).
    pub color_hex: Option<String>,
    pub organizing: bool,
    pub running: bool,
    /// Minutes after local midnight, clamped to today.
    pub start_minute: u32,
    pub end_minute: u32,
    /// "09:00–10:30", "14:00–now".
    pub range: String,
}

/// One quest's line in "This week".
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct WeekQuestLine {
    pub title: String,
    pub color_hex: String,
    pub hours: String,
    /// Share of the weekly target, 0.0..=1.0.
    pub fraction: f32,
}

/// "This week": the global goal, per quest, Organizing, No quest, Postponed.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct WeekSummary {
    /// "12.8h / 20h".
    pub total: String,
    pub progress: f32,
    pub quests: Vec<WeekQuestLine>,
    pub organizing: String,
    /// "1.2h"; None when under a minute.
    pub no_quest: Option<String>,
    /// How many plan dates and deadlines moved later this week.
    pub postponed: u32,
}

/// The Tracked tab.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct TrackedData {
    /// Today's total, "3.9h".
    pub total: String,
    /// The hours the timeline spans (whole hours, e.g. 8 and 17).
    pub first_hour: u32,
    pub last_hour: u32,
    pub blocks: Vec<TrackedBlock>,
    /// Now, in minutes after local midnight.
    pub now_minute: u32,
    pub week: WeekSummary,
}

/// The mini player above the tabs.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum PlayerCard {
    /// A task's timer runs: quest, task, time, Stop; tapping opens Focus.
    Running { task_id: String, title: String, quest: Option<QuestChip>, timer: String },
    /// Organizing runs: time, Stop and "Back to …" when it interrupted a task.
    Organizing { timer: String, back_to_task_id: Option<String>, back_to_title: Option<String> },
    /// Nothing runs: Play resumes the task worked on last.
    Resume { task_id: String, title: String, quest: Option<QuestChip> },
    /// Nothing runs and nothing to resume: no player.
    Empty,
}
