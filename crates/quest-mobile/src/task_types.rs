//! Records of the task sheet, the Repeat dialog and the day grid. Like `types`: plain values,
//! ready-made labels, ids as strings.

use crate::types::QuestChip;

/// What the task sheet (the desktop's task dialog) shows.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct TaskDetail {
    pub id: String,
    pub title: String,
    /// Plain text, possibly several lines.
    pub description: String,
    pub done: bool,
    /// The task's quest; None: "No quest" is selected.
    pub quest: Option<QuestChip>,
    /// The quests it can get: the active ones. Kotlin lists "No quest" first.
    pub quest_options: Vec<QuestChip>,
    /// Its day; None: the backlog ("Move to backlog" only shows with a day).
    pub date: Option<DayValue>,
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

/// A day's time grid, read-only: 24 hours, positions in minutes after local midnight.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct DayGrid {
    /// "2026-11-23".
    pub day: String,
    /// "Monday, November 23".
    pub day_label: String,
    /// The hour lines 01:00 to 23:00.
    pub hours: Vec<HourMark>,
    /// Sessions, then planned blocks, in drawing order.
    pub blocks: Vec<GridBlock>,
    /// The now-line in minutes (with the fraction of the current minute), only on today.
    pub now_minute: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct HourMark {
    /// Minutes after midnight, e.g. 60.
    pub minute: u32,
    /// "01:00".
    pub label: String,
}

/// Something drawn on the grid.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct GridBlock {
    /// The session's or task's id; a session's later segment past midnight adds "-<day>".
    pub id: String,
    pub kind: GridBlockKind,
    /// Minutes after midnight, 0..1440.
    pub start_minute: u32,
    /// After `start_minute` (at least a minute, as on the desktop), at most 1440.
    pub end_minute: u32,
    /// The task's title; a session without a task: "Other work".
    pub title: String,
    /// "09:00 – 10:30".
    pub range_label: String,
    /// The quest's colour ("#rrggbb"); None: the theme's faint colour.
    pub color_hex: Option<String>,
    /// A planned block of a done task (its title dims).
    pub done: bool,
    /// The lane it takes among overlapping items, and how many lanes they share: it is
    /// `1 / lane_count` wide.
    pub lane: u32,
    pub lane_count: u32,
    /// What actions take: the session's id (without the "-<day>" suffix) or the planned task's.
    pub target_id: String,
    /// "Session Write docs, 09:00 – 10:30" / "Planned Write docs, 09:00 – 10:30".
    pub a11y_label: String,
    /// Long-press moves the whole block: any planned block, a finished session wholly on this day.
    pub can_move: bool,
    /// The top handle changes the start (a session's start lies on this day).
    pub start_handle: bool,
    /// The bottom handle changes the end (not while running; a session's end lies on this day).
    pub end_handle: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum GridBlockKind {
    /// Tracked time: filled and hatched, a colour bar on the left.
    Session,
    /// The running timer: dashed, growing to now.
    Running,
    /// A task's planned block: an outline.
    Planned,
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
