//! Quest's domain: entities, the field-level change model used for sync, the hybrid logical
//! clock that orders changes, commands that turn user intent into field writes, and the weekly
//! review. Everything here is pure; storage and networking live in other crates.

mod batch;
mod change;
mod command;
mod hlc;
mod hours;
mod model;
mod recurrence;
mod review;
mod snapshot;
mod sort_key;
#[cfg(test)]
mod test_support;
mod workspace;

pub use batch::{PlannedBatch, plan_all};
pub use change::{EntityId, EntityKind, FieldChange, FieldStore, FieldWrite};
pub use command::{Command, CommandError, Position, WorkTask, plan};
pub use hlc::{Hlc, HlcClock, HlcParseError, NodeId};
pub use hours::WorkTime;
pub use model::{
    Milestone, Quest, QuestColor, QuestStatus, Settings, Task, TimeBlock, TimeEntry, Workspace, WorkspaceContent,
    fields,
};
pub use recurrence::{Frequency, MonthDay, RepeatPin, RepeatRule, RepeatSchedule, Weekdays};
pub use review::{QuestHours, WeeklyReview, week_start};
pub use snapshot::{RunningFocus, Snapshot, Trashed};
pub use sort_key::sort_key_between;
