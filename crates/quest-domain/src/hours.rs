//! Quest Hours: tracked time split into quest work and other work. The headers' figures (today,
//! this week, a day's total) come from here, so they count the way the weekly review does.

use chrono::{DateTime, Utc};

use crate::change::EntityId;
use crate::model::{EntryKind, Task, TimeEntry};
use crate::snapshot::{Snapshot, overlap_ms};

/// Tracked time inside a window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkTime {
    /// On tasks of a quest that exists: Quest Hours.
    pub quest_ms: i64,
    /// Organizing between tasks.
    pub organizing_ms: i64,
    /// Everything else.
    pub other_ms: i64,
}

impl WorkTime {
    pub fn total_ms(&self) -> i64 {
        self.quest_ms + self.organizing_ms + self.other_ms
    }
}

impl Snapshot {
    /// The quest a session counts towards: its quest, while that quest exists (a deleted
    /// quest's sessions count as other work until it is restored).
    pub fn entry_quest(&self, entry: &TimeEntry) -> Option<EntityId> {
        entry.quest_id.filter(|id| self.quest(*id).is_some())
    }

    /// Whether a task is quest work (it has a quest that exists) rather than other work.
    pub fn is_quest_work(&self, task: &Task) -> bool {
        task.quest_id.is_some_and(|id| self.quest(id).is_some())
    }

    /// Tracked time within `[from, to)`; a running session counts up to `now`.
    pub fn work_between(&self, from: DateTime<Utc>, to: DateTime<Utc>, now: DateTime<Utc>) -> WorkTime {
        let mut work = WorkTime::default();
        for entry in &self.time_entries {
            let ms = overlap_ms(entry, from, to, now);
            match (entry.kind, self.entry_quest(entry)) {
                (EntryKind::Organizing, _) => work.organizing_ms += ms,
                (EntryKind::Task, Some(_)) => work.quest_ms += ms,
                (EntryKind::Task, None) => work.other_ms += ms,
            }
        }
        work
    }
}

#[cfg(test)]
mod tests;
