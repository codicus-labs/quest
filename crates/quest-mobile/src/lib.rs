//! Quest for native mobile UIs: a thin UniFFI layer over `quest-store` with screen-shaped
//! records. Policy stays in `quest-domain`; the caller passes time in (`Moment`).
//!
//! Threads: `QuestCore::open`, signing in and out and the network calls block and belong off the
//! UI thread (each says so). Everything else only touches memory. `ChangeListener::on_change` runs
//! on the store's worker thread.

mod account;
#[cfg(target_os = "android")]
mod android_tls;
mod archive;
mod calendar;
mod calendar_types;
mod capture;
mod capture_types;
mod commands;
mod connections;
mod core;
mod core_account;
mod core_calendar;
mod core_capture;
mod core_connections;
mod core_focus;
mod core_plan;
mod core_platform;
mod core_quests;
mod core_search;
mod core_tasks;
mod core_workspaces;
mod day_grid;
mod error;
mod focus;
mod grid_drag;
mod guard;
mod home;
mod ids;
mod plan;
mod plan_types;
mod quest_types;
mod quests;
mod quick_add;
mod repeat;
mod review;
mod review_types;
mod search;
mod search_types;
mod server_url;
mod settings;
mod status;
mod task_detail;
mod task_types;
#[cfg(test)]
mod test_support;
mod ticker;
mod time;
mod types;
mod undo;
mod vault;
mod work_forms;
mod workspaces;

pub use crate::calendar_types::{
    CalendarData, CalendarDay, CalendarQuery, EntryCheck, EntryDraft, FormError, FormField, GridDrag, GridDragKind,
    GridPlacement, LogWorkCheck, LogWorkDraft, TaskChoices, TaskOption,
};
pub use crate::capture_types::{
    CaptureApplied, CaptureDraft, CaptureView, DraftBlock, DraftMilestone, DraftQuest, DraftQuestOption, DraftQuestRef,
    DraftQuestView, DraftTask, DraftTaskView, DraftWork, DraftWorkTask, DraftWorkView,
};
pub use crate::connections::AiConnectionRow;
pub use crate::core::{ChangeListener, QuestCore};
pub use crate::core_plan::{plan_edge_speed, plan_window};
pub use crate::core_platform::picked_day;
pub use crate::core_tasks::{check_repeat, toggle_repeat_weekday};
pub use crate::error::MobileError;
pub use crate::plan_types::{MoveTarget, PlanData, PlanDay, PlanWindow};
pub use crate::quest_types::{
    ArchiveData, ArchiveGroup, ArchiveItem, HueOption, MilestoneRow, QuestCard, QuestDetail, QuestHue, QuestTaskGroup,
    QuestsData, TrashKind,
};
pub use crate::review_types::{ReviewData, ReviewDay, ReviewQuest, ReviewSegment, ReviewWeek};
pub use crate::search_types::{AppView, SearchKind, SearchMode, SearchOutcome, SearchResult, SearchToken};
pub use crate::task_types::{
    DayGrid, DayValue, GridBlock, GridBlockKind, HourMark, LoggedEntry, QuickAddForm, RepeatCheck, RepeatDraft,
    RepeatFrequency, RepeatPinChoice, RepeatWeekday, TaskDetail,
};
pub use crate::types::{
    Account, AddPosition, FocusPickTask, FocusSession, FocusView, HomeData, HomeQuery, HomeTask, Moment, QuestChip,
    RunningSession, SettingsData, SignInResult, SyncOutcome, SyncState, TaskSection, UndoToast, UrlPolicy,
};

pub use crate::vault::{SavedTokens, TokenStore, TokenStoreError};
pub use crate::workspaces::{TimerElsewhere, WorkspaceRow, WorkspacesData};

/// Checks a server URL before signing in: https always; http only for this device (localhost,
/// 127.0.0.0/8, ::1) and, under `UrlPolicy::Development`, the emulator's host (10.0.2.2).
/// `QuestCore::begin_sign_in` applies the same rule. Pure, safe on the UI thread.
#[uniffi::export]
pub fn check_server_url(url: String, policy: UrlPolicy) -> Result<(), MobileError> {
    guard::guard(|| server_url::parse_server_url(&url, policy).map(drop))
}

uniffi::setup_scaffolding!();
