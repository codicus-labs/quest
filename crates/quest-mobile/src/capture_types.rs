//! Records of AI capture: the proposal as an editable draft, and how it reads on screen.

use crate::task_types::{DayValue, RepeatDraft};
use crate::types::QuestChip;

/// A quest an item is assigned to: an existing one or a new one from the same draft.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum DraftQuestRef {
    Existing { id: String },
    New { key: String },
}

/// The AI's proposal, as the user reviews and edits it. Keys are unique across all lists.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct CaptureDraft {
    pub quests: Vec<DraftQuest>,
    pub tasks: Vec<DraftTask>,
    pub work: Vec<DraftWork>,
    /// What the AI proposed but was left out, as sentences: "“Pay” was left out: the date …".
    pub dropped: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DraftQuest {
    pub key: String,
    pub selected: bool,
    pub title: String,
    pub description: String,
    pub milestones: Vec<DraftMilestone>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DraftMilestone {
    pub title: String,
    pub day: DayValue,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DraftTask {
    pub key: String,
    pub selected: bool,
    pub title: String,
    pub description: String,
    pub quest: Option<DraftQuestRef>,
    /// The AI guessed the quest: show it marked.
    pub quest_uncertain: bool,
    /// None: Someday (the backlog).
    pub day: Option<DayValue>,
    /// Only kept with a day.
    pub block: Option<DraftBlock>,
    /// Only kept with a day; the series starts on it (the draft's `starts` and `pin` are not used).
    pub repeat: Option<RepeatDraft>,
}

/// A planned block as the time pickers give it: minutes since midnight, 0 to 1439. An end of 0
/// is midnight at the end of the day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DraftBlock {
    pub start_minutes: u32,
    pub end_minutes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DraftWork {
    pub key: String,
    pub selected: bool,
    pub task: DraftWorkTask,
    /// The AI guessed the task or its quest: show it marked.
    pub uncertain: bool,
    pub day: DayValue,
    /// Minutes since midnight, 0 to 1439, as the time pickers give them. An end of 0 is midnight
    /// at the end of the day.
    pub start_minutes: u32,
    pub end_minutes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum DraftWorkTask {
    /// An existing task (open or done); `title` is for display only.
    Existing { id: String, title: String },
    /// A new task, created done on the session's day.
    New { title: String, quest: Option<DraftQuestRef> },
}

/// How the draft reads on screen, item by item in the draft's order.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct CaptureView {
    pub quests: Vec<DraftQuestView>,
    pub tasks: Vec<DraftTaskView>,
    pub work: Vec<DraftWorkView>,
    /// For the quest picker: the active quests, then the selected new quests of the draft. Kotlin
    /// lists "No quest" first.
    pub quest_options: Vec<DraftQuestOption>,
    pub selected_count: u32,
    /// Something is selected and every selected item can be created.
    pub can_apply: bool,
    /// "Create (3)".
    pub apply_label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DraftQuestView {
    pub key: String,
    /// The colour the quest will get.
    pub color_hex: String,
    /// "2 milestones", or "".
    pub detail: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DraftTaskView {
    pub key: String,
    /// The quest it will get (None also when its new quest is deselected).
    pub quest: Option<QuestChip>,
    /// "Tomorrow · 09:00–10:00", "Fri, Nov 27", "Someday".
    pub when: String,
    /// "09:00" and "24:00" for the block's time pickers.
    pub block_start_label: Option<String>,
    pub block_end_label: Option<String>,
    /// "Every week on Mon, Wed"-style summary of the repeat.
    pub repeat_summary: Option<String>,
    /// What to fix before it can be created.
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DraftWorkView {
    pub key: String,
    pub task_title: String,
    /// The quest the session counts towards.
    pub quest: Option<QuestChip>,
    /// "Today · 09:00–11:00".
    pub when: String,
    pub start_label: String,
    /// "24:00" for midnight.
    pub end_label: String,
    /// "2:00".
    pub duration: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DraftQuestOption {
    pub quest: DraftQuestRef,
    /// For a new quest, `id` is its key.
    pub chip: QuestChip,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct CaptureApplied {
    /// "Created 1 quest, 2 tasks and 1 session".
    pub message: String,
    /// "Undo": the snackbar's action, which calls `undo_capture`.
    pub undo_label: String,
    /// How long the snackbar shows, as long as the desktop's toast.
    pub show_for_ms: u64,
}
