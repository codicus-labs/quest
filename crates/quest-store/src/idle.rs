//! Waiting until sync has nothing left to do: the outbox is empty and catch-up is done. The
//! worker signals every status change, so waiters sleep instead of polling.

use std::sync::{Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::SyncStatus;
use crate::conn_state::ConnState;

/// How `Store::sync_until_idle` ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncOutcome {
    /// Live, caught up and every local change acknowledged by the server.
    Idle,
    /// Sync cannot make progress now: signed out, failed, or offline with at least one retry
    /// failed (a single dropped connection is retried first). `Store::status()` tells which;
    /// unsent changes stay in the outbox.
    Offline,
    /// Still connecting, catching up or pushing when the timeout ran out.
    TimedOut,
}

/// The outcome `status` settles, or `None` while sync is still making progress. A blip (one
/// dropped connection or failed first attempt, `attempt` 1) is not settled yet: the retry
/// comes within a second. Offline settles once a retry failed too.
pub(crate) fn settled_outcome(status: &SyncStatus, connection: &ConnState) -> Option<SyncOutcome> {
    match status {
        SyncStatus::Live { pending: 0 } => Some(SyncOutcome::Idle),
        SyncStatus::Live { .. } | SyncStatus::Connecting => None,
        SyncStatus::Offline { .. } if !has_failed_a_retry(connection) => None,
        SyncStatus::Offline { .. } | SyncStatus::SignedOut | SyncStatus::Failed { .. } => Some(SyncOutcome::Offline),
    }
}

fn has_failed_a_retry(connection: &ConnState) -> bool {
    match connection {
        ConnState::Connecting { attempt } | ConnState::Waiting { attempt, .. } => *attempt >= 2,
        _ => true,
    }
}

/// Wakes threads waiting for a status change. Counts changes so none is missed between
/// reading the status and starting to wait.
#[derive(Default)]
pub(crate) struct ChangeSignal {
    changes: Mutex<u64>,
    changed: Condvar,
}

impl ChangeSignal {
    pub fn notify(&self) {
        *self.changes.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        self.changed.notify_all();
    }

    /// Calls `settled` after every change until it returns an outcome or `timeout` runs out.
    /// A timeout too large to represent waits without a deadline.
    pub fn wait_until_settled(&self, timeout: Duration, settled: impl Fn() -> Option<SyncOutcome>) -> SyncOutcome {
        let deadline = Instant::now().checked_add(timeout);
        let mut changes = self.changes.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            let seen = *changes;
            if let Some(outcome) = settled() {
                return outcome;
            }
            changes = match deadline {
                None => {
                    self.changed.wait_while(changes, |current| *current == seen).unwrap_or_else(PoisonError::into_inner)
                }
                Some(deadline) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return SyncOutcome::TimedOut;
                    }
                    self.changed
                        .wait_timeout_while(changes, remaining, |current| *current == seen)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
            };
        }
    }
}

#[cfg(test)]
#[path = "idle_tests.rs"]
mod tests;
