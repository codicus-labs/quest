//! Records of the task sheet, its checklist and the Repeat dialog. Like `types`: plain values,
//! ready-made labels, ids as strings.

use crate::types::QuestChip;

/// What the task sheet (the desktop's task dialog) shows.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct TaskDetail {
    pub id: String,
    pub title: String,
    /// Markdown, possibly several lines.
    pub description: String,
    pub done: bool,
    /// The task's quest; None: "No quest" is selected.
    pub quest: Option<QuestChip>,
    /// The quests it can get: the active ones. Kotlin lists "No quest" first.
    pub quest_options: Vec<QuestChip>,
    /// The day it is planned for; None: not planned.
    pub planned_on: Option<DayValue>,
    /// The deadline; None: none. Never before `planned_on`.
    pub due_on: Option<DayValue>,
    /// Open and the deadline has passed.
    pub overdue: bool,
    /// How often its dates were moved later.
    pub postponed: PostponedCount,
    /// In its order.
    pub checklist: Vec<ChecklistRow>,
    /// "Does not repeat", "Every day", "Every 2 weeks" (its series' rule).
    pub repeat_label: String,
    /// Whether it repeats: the Repeat dialog offers "Stop repeating".
    pub repeats: bool,
    /// "1h 30m total", "1m total" (minutes like the entries).
    pub logged_total: String,
    /// The task's newest sessions, at most 4 as on the desktop, newest first (`logged_total`
    /// counts all).
    pub logged: Vec<LoggedEntry>,
}

/// A day for the native date picker and as text.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DayValue {
    /// "2026-11-24".
    pub iso: String,
    /// "Nov 24, 2026".
    pub label: String,
}

/// How often a task's plan date and deadline were moved later.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct PostponedCount {
    pub plan: u32,
    pub due: u32,
    /// Both together.
    pub total: u32,
}

/// One item of a task's checklist.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ChecklistRow {
    pub id: String,
    pub text: String,
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ChecklistProgress {
    pub done: u32,
    pub total: u32,
}

/// Where a checklist item is added or moved to.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ChecklistPosition {
    Top,
    Bottom,
    /// Right before this item of the same checklist.
    Before {
        item_id: String,
    },
}

/// One session in the task sheet's logged time.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct LoggedEntry {
    pub id: String,
    /// "Nov 23 · 09:00–10:30"; a running one "Nov 23 · 09:00–now".
    pub when: String,
    /// "1h 30m", "45m", "1m" (the desktop entry editor's `duration_label`).
    pub duration: String,
    pub running: bool,
}

/// The Repeat dialog's fields, as on the desktop. Kotlin holds it as form state.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RepeatDraft {
    /// The interval, "1" to "999".
    pub every: String,
    pub frequency: RepeatFrequency,
    pub pin: RepeatPinChoice,
    /// Weekly only. Empty: the start's weekday.
    pub weekdays: Vec<RepeatWeekday>,
    /// Monthly only: "Last day" rather than "Day of the month".
    pub last_day: bool,
    /// Monthly, not the last day: "1" to "31"; empty: the start's day.
    pub month_day: String,
    /// The start, "2026-11-24" ("Nov 24, 2026" works too).
    pub starts: String,
    /// Ends: "On date" rather than "Never".
    pub ends_on_date: bool,
    /// The end date like `starts`; only read with `ends_on_date`.
    pub ends: String,
    /// IANA time zone, e.g. "Europe/Berlin".
    pub time_zone: String,
}

/// Labels: "day", "week", "month", "year".
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum RepeatFrequency {
    Day,
    Week,
    Month,
    Year,
}

/// Where each new occurrence lands in its day's list. Labels: "None", "Top", "Bottom".
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum RepeatPinChoice {
    None,
    Top,
    Bottom,
}

/// Chips "Mo" … "Su".
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum RepeatWeekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

/// The Repeat dialog's live state for a draft.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RepeatCheck {
    /// The weekday chips shown as picked.
    pub shown_weekdays: Vec<RepeatWeekday>,
    /// The month day field's placeholder: the start's day, e.g. "24".
    pub month_day_placeholder: String,
    /// "Every 2 weeks"; empty while the interval is not valid.
    pub summary: String,
    /// What saving would refuse, as the desktop says it; None when it would save.
    pub error: Option<String>,
    /// The start as read from the draft, for the date picker and the field's label; None when
    /// empty or unreadable.
    pub starts: Option<DayValue>,
    /// The end date the same way (also when "Never" is chosen, so switching keeps it).
    pub ends: Option<DayValue>,
}

/// The quick-add sheet (share target, app shortcut, quick settings tile).
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct QuickAddForm {
    /// The shared text's first non-empty line, on one line and trimmed; "" without text.
    pub title: String,
    /// The rest of the shared text, trimmed (e.g. the link under a page title); "" otherwise.
    pub description: String,
    /// Preselected: the quest new quest work gets on Home (the one used last, else the first
    /// active one); None without an active quest.
    pub quest: Option<QuestChip>,
    /// The quests it can get: the active ones. Kotlin lists "No quest" first.
    pub quest_options: Vec<QuestChip>,
    /// Preselected day: today.
    pub today: DayValue,
}
