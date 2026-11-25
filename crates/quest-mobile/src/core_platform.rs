//! `QuestCore` for the platform around the app: background sync, saving before the process
//! may be killed, and quick add (share target, shortcut, tile).

use std::time::Duration;

use quest_domain::EntityId;

use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::ids::parse_iso_day;
use crate::quick_add;
use crate::task_detail::day_value;
use crate::task_types::{DayValue, QuickAddForm};
use crate::ticker;
use crate::time::Now;
use crate::types::{Moment, SyncOutcome};

#[uniffi::export]
impl QuestCore {
    /// Blocks until sync is idle (outbox empty, catch-up done), cannot make progress (`Offline`:
    /// signed out, failed, or offline after a failed retry) or `timeout_ms` runs out. Never on
    /// the UI thread. Other calls on the core go on meanwhile.
    pub fn sync_until_idle(&self, timeout_ms: u64) -> SyncOutcome {
        match self.store().waiter().sync_until_idle(Duration::from_millis(timeout_ms)) {
            quest_store::SyncOutcome::Idle => SyncOutcome::Idle,
            quest_store::SyncOutcome::Offline => SyncOutcome::Offline,
            quest_store::SyncOutcome::TimedOut => SyncOutcome::TimedOut,
        }
    }

    /// Blocks until every local change is saved to disk (true), at most `timeout_ms` (false:
    /// time ran out or saving failed; the changes stay in memory and are retried). Call it when
    /// the app goes to the background, so a process kill loses nothing. Never on the UI thread.
    pub fn flush(&self, timeout_ms: u64) -> bool {
        self.store().waiter().flush(Duration::from_millis(timeout_ms))
    }

    /// When the local day turns (epoch ms, the next midnight in `now.time_zone` plus a few ms):
    /// for a view bound to today, such as the widget, that is not ticked in the background.
    pub fn next_day_ms(&self, now: Moment) -> Result<i64, MobileError> {
        guard(|| Ok(ticker::next_day(Now::parse(&now)?).timestamp_millis()))
    }

    /// The quick-add sheet, prefilled from `shared_text` (None: empty title).
    pub fn quick_add_form(&self, shared_text: Option<String>, now: Moment) -> Result<QuickAddForm, MobileError> {
        guard(|| Ok(quick_add::form(&self.store().snapshot(), shared_text.as_deref(), Now::parse(&now)?)))
    }

    /// Creates the task at the bottom of `day` ("2026-11-24"; None: unplanned) with `quest_id`
    /// (None: no quest) and `description`. Returns its id.
    pub fn quick_add(
        &self,
        title: String,
        description: String,
        quest_id: Option<String>,
        day: Option<String>,
        now: Moment,
    ) -> Result<String, MobileError> {
        guard(|| {
            // Only checked, like `plan_add_task`: the day comes in as `day`.
            Now::parse(&now)?;
            let id = EntityId::new();
            let snapshot = self.store().snapshot();
            let commands =
                quick_add::commands(&snapshot, id, &title, &description, quest_id.as_deref(), day.as_deref())?;
            for command in commands {
                self.dispatch(command)?;
            }
            Ok(id.to_string())
        })
    }
}

/// A day picked in the native date picker ("2026-11-26") with its label ("Nov 26, 2026"), as
/// in the task sheet. Pure, safe on the UI thread.
#[uniffi::export]
pub fn picked_day(iso: String) -> Result<DayValue, MobileError> {
    guard(|| Ok(day_value(parse_iso_day(&iso)?)))
}

#[cfg(test)]
#[path = "core_platform_tests.rs"]
mod tests;
