//! What the native UIs see: plain records shaped for one screen each. Domain and store types
//! never cross the FFI boundary; ids travel as strings.

use crate::task_types::{DayValue, PostponedCount};
use crate::work_types::SessionKind;

/// The caller's notion of "now": the instant and the IANA time zone (e.g. "Europe/Berlin").
/// Mobile platforms own the clock and the zone, so they pass both in.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct Moment {
    pub epoch_ms: i64,
    pub time_zone: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct QuestChip {
    pub id: String,
    pub name: String,
    /// "#rrggbb".
    pub color_hex: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct HomeTask {
    pub id: String,
    pub title: String,
    /// The check; a done task's title is struck through and its card dimmed.
    pub done: bool,
    /// The quest this task works towards, if it has one that exists. None: "No quest".
    pub quest: Option<QuestChip>,
    /// Tracked time as on the desktop card ("36:00", "1:30:00"), shown while the timer runs or
    /// once time was tracked; None shows nothing. Counts live while the timer runs.
    pub tracked: Option<String>,
    /// This task's timer runs: a dot before the time, Pause instead of Play, an accent edge.
    pub timer_running: bool,
    /// Whether the card offers Play/Pause (not on done tasks).
    pub can_start_timer: bool,
    /// The day it is planned for; None: not planned.
    pub planned_on: Option<DayValue>,
    /// The deadline; None: none.
    pub due_on: Option<DayValue>,
    /// Open and the deadline has passed.
    pub overdue: bool,
    /// How often its dates were moved later.
    pub postponed: PostponedCount,
}

/// UI state of the Home screen the core needs to pick sections and quests. Kotlin keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq, uniffi::Record)]
pub struct HomeQuery {
    /// The quest filter's id: QUEST WORK shows only this quest's tasks.
    pub quest_filter: Option<String>,
    /// The quest picked on the "Add quest work…" field's chip.
    pub quest_pick: Option<String>,
}

/// Home's two lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum TaskSection {
    /// QUEST WORK: tasks with a quest.
    QuestWork,
    /// OTHER: tasks without one.
    Other,
}

/// Where a new task lands in its list (the desktop's Top/Bottom switch).
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AddPosition {
    Top,
    Bottom,
}

/// The undo snackbar.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UndoToast {
    /// "Deleted “Write tests”", "Completed “Write tests”", "Reopened “Write tests”".
    pub text: String,
    /// "Undo".
    pub action_label: String,
    /// When it goes by itself (epoch ms); `tick` removes it then.
    pub expires_at_ms: i64,
}

/// Everything the Home screen shows for one day.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct HomeData {
    /// "Monday, November 23".
    pub day_label: String,
    /// The same local day, "2026-11-23".
    pub day_iso: String,
    /// Quest Hours today, e.g. "3.6h".
    pub today_hours: String,
    /// Quest Hours this week (Monday to Sunday), e.g. "12h".
    pub week_hours: String,
    /// The weekly target, e.g. "20h".
    pub weekly_target: String,
    /// Week progress towards the weekly target, 0.0..=1.0.
    pub week_progress: f32,
    /// QUEST WORK: today's tasks with a quest (only the filtered quest's), open ones first.
    pub quest_work: Vec<HomeTask>,
    /// OTHER: today's tasks without a quest, open ones first. Hiding them is Kotlin's.
    pub other: Vec<HomeTask>,
    /// The active quest filter as a chip; None without a filter or when its quest is gone.
    pub quest_filter: Option<QuestChip>,
    /// The chip on the "Add quest work…" field: the quest a new task there gets. None while a
    /// filter is set (the task gets the filter's quest) or when there is no active quest.
    pub add_quest: Option<QuestChip>,
    /// Active quests, for the filter and the field's chip.
    pub quests: Vec<QuestChip>,
    /// The undo snackbar, while one shows.
    pub toast: Option<UndoToast>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SyncState {
    SignedOut,
    Connecting,
    Live {
        pending: u64,
    },
    Offline {
        pending: u64,
        retry_in_secs: u64,
    },
    Failed {
        message: String,
    },
    /// The account's sign-in expired (or was revoked): offer "Sign in again". Local data stays.
    SessionExpired,
}

/// How `QuestCore::sync_until_idle` ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SyncOutcome {
    /// Live, caught up, and every local change acknowledged by the server.
    Idle,
    /// Sync cannot make progress now: signed out, failed, or offline after a failed retry.
    /// Unsent changes stay in the outbox.
    Offline,
    /// Still connecting, catching up or pushing when the timeout ran out.
    TimedOut,
}

/// The signed-in account (the guest has none).
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct Account {
    /// How to name it: the email, else the name, else "Signed in".
    pub display_name: String,
    /// From the identity provider; display only.
    pub email: Option<String>,
    pub name: Option<String>,
    /// The Quest server's origin, e.g. "https://quest.example.com".
    pub server_url: String,
    /// The account's opt-in to AI features (off by default; applies to all devices).
    pub ai_enabled: bool,
    /// The server has an AI provider.
    pub ai_available: bool,
    /// AI entries (dictate → structure, search action, shortcut, share target) show: both of the above.
    pub ai_features: bool,
}

/// How a sign-in ended (`QuestCore::finish_sign_in`).
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SignInResult {
    /// Switched from the guest to the account. If `offer_guest_import`, ask once "Add guest data
    /// to this account?" (`import_guest_data`); No leaves it in the guest store.
    SignedIn { offer_guest_import: bool, account: Account },
    /// The account's expired session was renewed; syncing goes on.
    Renewed { account: Account },
    /// The browser came back without a sign-in answer (e.g. after signing out): nothing to do.
    NotASignIn,
}

/// Which plain-http servers the build may use (see `check_server_url`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum UrlPolicy {
    /// Release builds: plain http only to this device (loopback, e.g. through `adb reverse`).
    Release,
    /// Debug builds: also the emulator's host 10.0.2.2. On a real device that is an ordinary LAN
    /// address, so a release build must not send sign-ins and data there unencrypted.
    Development,
}

/// What the Settings screen shows.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct SettingsData {
    /// Weekly target for Quest Hours, in minutes.
    pub weekly_target_minutes: u32,
    /// The same target as shown elsewhere, e.g. "20h".
    pub weekly_target_label: String,
}

/// What the Focus screen shows (the desktop's Focus view).
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FocusView {
    /// A task to focus on: the running one, else `focus_task` if open, else the open task worked
    /// on last.
    Session { session: FocusSession },
    /// Nothing to focus on: today's open tasks to start one ("What are you working on?").
    Pick { tasks: Vec<FocusPickTask> },
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FocusSession {
    pub task_id: String,
    /// Editable in place; save with `rename_task`.
    pub title: String,
    /// Plain text, editable in place; save with `set_task_description`.
    pub description: String,
    /// The quest chip; None shows "No quest".
    pub quest: Option<QuestChip>,
    /// The top bar's caps label: "FOCUS · WORK", or "FOCUS" without a quest.
    pub header: String,
    /// The task's tracked time over all its sessions, big: "36:00", "1:30:00". Counts live
    /// while it runs.
    pub timer: String,
    /// Pause instead of Resume; the header's dot is lit.
    pub running: bool,
    /// "RUNNING" / "PAUSED".
    pub state_label: String,
}

/// One row of Focus's "What are you working on?" list; tapping it starts the timer.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FocusPickTask {
    pub id: String,
    pub title: String,
    /// For the colour edge; None: no quest.
    pub quest: Option<QuestChip>,
}

/// The running timer, for the ongoing notification.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RunningSession {
    pub kind: SessionKind,
    /// The task's id; None while organizing.
    pub task_id: Option<String>,
    /// The task's title, or "Organizing".
    pub title: String,
    /// The quest's name; None without a quest.
    pub quest_name: Option<String>,
    /// "#rrggbb"; None without a quest.
    pub quest_color_hex: Option<String>,
    /// Chronometer base, epoch ms: now minus the task's tracked time, so a chronometer counting
    /// from here shows the Focus timer's value; organizing: the session's start.
    pub chronometer_base_ms: i64,
    /// Organizing only: the open task it interrupted, for "Back to …" (`start_timer` on it).
    pub origin_task_id: Option<String>,
    /// "Back to" this title.
    pub origin_title: Option<String>,
    /// The timer's workspace when it is not the shown one ("in Default"); None when it is.
    pub workspace_name: Option<String>,
}
