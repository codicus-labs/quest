//! AI capture: free text (typed or dictated) structured into a proposal the user confirms.
//!
//! `POST /v1/ai/capture` with `Authorization: Bearer <token>` takes a [`CaptureRequest`] and returns a
//! [`CaptureResponse`]. The server forwards the text and the compact context to the AI provider it is
//! configured for; the key never leaves the server. Failures return an [`crate::ApiError`] with one of
//! the `Ai*`/`RateLimited` codes (see [`crate::ErrorCode`]).
//!
//! The proposal names existing quests and tasks by id. Everything new carries a temporary key
//! ([`TempKey`]) that is only valid within the proposal. Nothing is created until the client applies
//! the (possibly edited) proposal with ordinary domain commands.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc, Weekday};
use quest_domain::{EntityId, Frequency, MonthDay};
use serde::{Deserialize, Serialize};

pub const AI_CAPTURE_PATH: &str = "/v1/ai/capture";

/// Bumped on incompatible changes to the capture types; the server rejects other versions.
pub const CAPTURE_VERSION: u32 = 2;

/// Longest text the server accepts, in characters (about five minutes of dictation).
pub const MAX_CAPTURE_TEXT_CHARS: usize = 4000;
/// Context limits, enforced by the server and respected by the clients' context builder.
pub const MAX_CONTEXT_QUESTS: usize = 50;
pub const MAX_CONTEXT_TASKS: usize = 150;
/// Longest title in the context, in characters; clients shorten longer ones.
pub const MAX_CONTEXT_TITLE_CHARS: usize = 120;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureRequest {
    /// [`CAPTURE_VERSION`].
    pub version: u32,
    pub text: String,
    /// The client's current time; with `time_zone` it fixes "today", "tomorrow" and clock times.
    pub now: DateTime<Utc>,
    /// IANA name, e.g. `Europe/Berlin`.
    pub time_zone: String,
    /// BCP 47 tag of the text's language, e.g. `de-DE`.
    pub language: String,
    pub context: CaptureContext,
}

/// Just enough of the user's data to assign the text to existing quests and tasks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CaptureContext {
    /// Active quests.
    pub quests: Vec<ContextQuest>,
    /// Tasks around today (and some backlog), for logging work on them.
    pub tasks: Vec<ContextTask>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextQuest {
    pub id: EntityId,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextTask {
    pub id: EntityId,
    pub title: String,
    pub quest_id: Option<EntityId>,
    pub planned_on: Option<NaiveDate>,
    pub due_on: Option<NaiveDate>,
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureResponse {
    /// [`CAPTURE_VERSION`].
    pub version: u32,
    pub proposal: CaptureProposal,
}

/// Names a new quest or task within one proposal, e.g. `q1`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TempKey(pub String);

/// What the AI recognised, already validated. Every list may be empty.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CaptureProposal {
    pub quests: Vec<ProposedQuest>,
    pub tasks: Vec<ProposedTask>,
    pub work: Vec<ProposedWork>,
    /// Items the validation threw away, so the user knows something was not taken over.
    pub dropped: Vec<Dropped>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProposedQuest {
    pub key: TempKey,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QuestRef {
    Existing { id: EntityId },
    New { key: TempKey },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProposedTask {
    pub key: TempKey,
    pub title: String,
    pub description: String,
    pub quest: Option<QuestRef>,
    /// The quest was guessed (or the AI named one that does not exist): the UI marks it.
    pub quest_uncertain: bool,
    /// When to work on it; `None` leaves it unplanned.
    pub planned_on: Option<NaiveDate>,
    /// The deadline; never before `planned_on` (validation applies the domain's rule).
    pub due_on: Option<NaiveDate>,
    /// Only with a `planned_on`; the series starts on it.
    pub repeat: Option<ProposedRepeat>,
}

/// A repeat rule without its start (the task's plan date) and time zone (the client's).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedRepeat {
    pub every: u32,
    pub frequency: Frequency,
    /// For weekly rules; empty means the start's weekday.
    pub weekdays: Vec<Weekday>,
    /// For monthly rules; `None` means the start's day of month.
    pub month_day: Option<MonthDay>,
    pub ends: Option<NaiveDate>,
}

/// Work already done, logged like the Log work dialog does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProposedWork {
    pub key: TempKey,
    pub task: WorkTarget,
    /// The task was guessed: the UI marks it.
    pub uncertain: bool,
    pub day: NaiveDate,
    pub start: NaiveTime,
    /// After `start`, or `00:00` for midnight at the end of `day`.
    pub end: NaiveTime,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkTarget {
    /// An existing task, open or done; the session counts towards its quest.
    Existing { id: EntityId },
    /// A new task, created done on the session's day.
    New { title: String, quest: Option<QuestRef> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dropped {
    /// The item's title as far as known, shortened.
    pub title: String,
    pub reason: DropReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropReason {
    /// A required field is missing, empty or malformed.
    Incomplete,
    /// A date or time that does not exist or lies too far away.
    InvalidDate,
    /// The end of a session is not after its start.
    InvalidTimeRange,
    /// A title or description is too long, or the proposal has too many items.
    TooLong,
    /// Names a task that is not in the context.
    UnknownTask,
    /// Names a new quest that the proposal does not contain.
    UnknownQuest,
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
