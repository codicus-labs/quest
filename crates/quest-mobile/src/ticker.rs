//! When the core wants to be woken and whether a tick should generate due recurring tasks.
//! Same rule as the desktop's `State::refresh`. Pure: time and state come in as arguments.

use std::sync::Arc;

use chrono::{DateTime, Days, NaiveDate, TimeDelta, Utc};
use quest_domain::Snapshot;

use crate::time::Now;

/// What the last tick saw, so the next one knows whether anything turned.
#[derive(Default)]
pub(crate) struct TickMemory {
    /// The (UTC minute, local day) in which `GenerateDueTasks` last ran. `None` forces a run.
    generated_in: Option<(i64, NaiveDate)>,
    /// The snapshot after that run; a different one (sync, user edits) forces a run.
    seen: Option<Arc<Snapshot>>,
}

impl TickMemory {
    /// Due tasks are generated once per minute, at local midnight, and whenever the data changed.
    pub fn should_generate(&self, snapshot: &Arc<Snapshot>, now: Now) -> bool {
        let snapshot_changed = self.seen.as_ref().is_none_or(|seen| !Arc::ptr_eq(seen, snapshot));
        snapshot_changed || self.generated_in != Some(minute_key(now))
    }

    pub fn generated(&mut self, snapshot: Arc<Snapshot>, now: Now) {
        self.generated_in = Some(minute_key(now));
        self.seen = Some(snapshot);
    }

    /// Generating must wait (e.g. while connecting, so a fresh device does not create
    /// occurrences the server already has); try again on the next tick.
    pub fn postpone(&mut self) {
        self.generated_in = None;
    }
}

/// How far past a boundary a wake lands, as on the desktop (`quest-app` `clock::WAKE_SLACK`): a
/// wake that comes a hair early finds nothing turned and would ask again for almost nothing.
pub(crate) const WAKE_SLACK: TimeDelta = TimeDelta::milliseconds(5);

/// The next instant the core wants a tick: the next whole second while a timer runs, else the
/// next whole minute, never later than the next local midnight nor than the moment the undo
/// snackbar goes (`toast_until`, as the desktop wakes for it); plus `WAKE_SLACK`.
pub(crate) fn next_wake(now: Now, timer_running: bool, toast_until: Option<DateTime<Utc>>) -> DateTime<Utc> {
    let step = if timer_running { 1 } else { 60 };
    let turn = next_whole(now.at, step).min(now.midnight(now.today() + Days::new(1)));
    toast_until.filter(|until| *until > now.at).map_or(turn, |until| turn.min(until)) + WAKE_SLACK
}

/// The start of the next local day (its midnight, or the jump on a day that skips it), plus
/// `WAKE_SLACK`: when a day-bound view such as the widget turns over.
pub(crate) fn next_day(now: Now) -> DateTime<Utc> {
    now.midnight(now.today() + Days::new(1)) + WAKE_SLACK
}

fn minute_key(now: Now) -> (i64, NaiveDate) {
    (now.at.timestamp().div_euclid(60), now.today())
}

/// The next whole multiple of `seconds` after `at` (the next second, the next minute).
fn next_whole(at: DateTime<Utc>, seconds: i64) -> DateTime<Utc> {
    let next = (at.timestamp().div_euclid(seconds) + 1) * seconds;
    DateTime::from_timestamp(next, 0).unwrap_or(at)
}

#[cfg(test)]
#[path = "ticker_tests.rs"]
mod tests;
