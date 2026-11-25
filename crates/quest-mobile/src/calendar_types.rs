//! Records of the Calendar, its drags, Log work and the entry editor. Like `types`: plain
//! values, ready-made labels, ids as strings, times as minutes after local midnight.

use crate::task_types::{DayGrid, DayValue};
use crate::types::{QuestChip, UndoToast};

/// The Calendar page to show.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct CalendarQuery {
    /// "2026-11-24"; None: the page with today.
    pub first_day: Option<String>,
    /// Days side by side: 1 (compact), 3 (medium), 7 (expanded, a week from Monday).
    pub day_count: u32,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct CalendarData {
    /// Its days, left to right.
    pub days: Vec<CalendarDay>,
    /// The first day of the page before and after (a swipe or the arrows).
    pub previous_first_day: String,
    pub next_first_day: String,
    pub shows_today: bool,
    /// "Nov 23 – 29": the week of the page's middle day, whose Quest Hours the header shows.
    pub week_label: String,
    /// "12.5h".
    pub week_hours: String,
    /// "20h".
    pub weekly_target: String,
    /// `week_hours` of `weekly_target`, 0 to 1.
    pub week_progress: f32,
    /// Where to scroll on open: the start of the current hour, in minutes after midnight.
    pub scroll_minute: u32,
    /// "November 2026", of the first day.
    pub month_label: String,
    pub toast: Option<UndoToast>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct CalendarDay {
    pub grid: DayGrid,
    /// "Tue".
    pub weekday: String,
    /// "24".
    pub day_number: String,
    pub is_today: bool,
    /// All work tracked on the day: "2.5h", "0h".
    pub total: String,
}

/// What a drag on the grid holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum GridDragKind {
    /// A long-pressed block, moved as a whole.
    Move,
    /// A block's top handle.
    StartEdge,
    /// A block's bottom handle.
    EndEdge,
    /// A long-press on empty space: time to log.
    NewSpan,
}

/// A drag on the calendar's grid.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct GridDrag {
    pub kind: GridDragKind,
    /// The dragged `GridBlock.id`; None for `NewSpan`.
    pub block_id: Option<String>,
    /// The day the drag started on.
    pub from_day: String,
    /// The unsnapped minute where the finger went down.
    pub grab_minute: f32,
}

/// Where a drop would land: the ghost drawn while dragging.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct GridPlacement {
    pub day: String,
    pub start_minute: u32,
    pub end_minute: u32,
    /// "09:15 – 10:00".
    pub range_label: String,
}

/// A task to pick in Log work or the entry editor.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct TaskOption {
    pub id: String,
    pub title: String,
    pub done: bool,
    /// "Open" or "Done".
    pub status: String,
    pub quest: Option<QuestChip>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct TaskChoices {
    pub options: Vec<TaskOption>,
    /// "Create “query”" when the query matched nothing (Log work offers it as a new completed task).
    pub create_label: Option<String>,
}

/// The Log work sheet's fields. Kotlin keeps them as form state.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct LogWorkDraft {
    /// The picked task; None: a new task named `new_title`.
    pub task_id: Option<String>,
    pub new_title: String,
    /// The new task's quest; ignored with a picked task (its own counts).
    pub quest_id: Option<String>,
    /// "2026-11-24".
    pub day: String,
    /// Minutes after midnight, 0 to 1439.
    pub start_minute: u32,
    pub end_minute: u32,
}

/// The field an error belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FormField {
    Task,
    /// The (start) date.
    Date,
    /// The entry editor's end date (over midnight).
    EndDate,
    Start,
    /// The end, also when it is not after the start.
    End,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FormError {
    pub field: FormField,
    /// The desktop's message, e.g. "The end must be after the start.".
    pub message: String,
}

/// The Log work sheet's live state for a draft.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct LogWorkCheck {
    /// "Nov 23, 2026" and the ISO day; None when `day` is unreadable.
    pub date: Option<DayValue>,
    /// "14:50".
    pub start_label: String,
    pub end_label: String,
    /// "1h 15m"; "–" while the end is not after the start.
    pub duration: String,
    /// The picked task's title, or the new task's; None: nothing chosen yet.
    pub task_title: Option<String>,
    /// Whether the title is a new task (it will be created done).
    pub is_new_task: bool,
    /// The quest the session counts towards: the picked task's own (read-only), a new task's
    /// chosen one; None without a task or title (the UI hides QUEST then) or without a quest.
    pub quest: Option<QuestChip>,
    /// Only a new task's quest can be chosen (a session's quest follows its task).
    pub can_choose_quest: bool,
    /// The active quests.
    pub quest_options: Vec<QuestChip>,
    /// What saving would refuse; None: "Log work" saves.
    pub error: Option<FormError>,
}

/// The entry editor's fields. Kotlin keeps them as form state.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct EntryDraft {
    pub entry_id: String,
    /// None: no task ("Other work").
    pub task_id: Option<String>,
    pub start_day: String,
    pub start_minute: u32,
    /// Only read with `ends_other_day`; else the end is on `start_day`.
    pub end_day: String,
    pub end_minute: u32,
    /// "Ends on a different day": over midnight.
    pub ends_other_day: bool,
}

/// The entry editor's live state for a draft.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct EntryCheck {
    /// The timer still runs: no end fields ("running").
    pub running: bool,
    /// None: "No task".
    pub task_title: Option<String>,
    /// The task's quest: the session always counts towards it.
    pub quest: Option<QuestChip>,
    /// "Open task" shows.
    pub can_open_task: bool,
    pub start_date: Option<DayValue>,
    pub start_label: String,
    pub end_date: Option<DayValue>,
    pub end_label: String,
    /// "1h 15m" (a running one up to now); "–" while a field is wrong.
    pub duration: String,
    /// What saving would refuse; None: Save is enabled.
    pub error: Option<FormError>,
}
