//! The sync connection lifecycle as a pure state machine. The worker performs what each state
//! asks for (connect, wait) and feeds the outcome back as an event.

use std::time::Duration;

use crate::error::StoreError;

const BACKOFF_BASE: Duration = Duration::from_millis(500);
const BACKOFF_CAP: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ConnState {
    /// No account (guest) or signed out: nothing to sync.
    SignedOut,
    /// Opening the WebSocket. `attempt` counts failures since the last live connection.
    Connecting { attempt: u32 },
    /// Connected: pushing the outbox and receiving changes.
    Live,
    /// Waiting before the next connection attempt.
    Waiting { attempt: u32, retry_in: Duration },
    /// Stopped until the user acts (signs in again, updates the app).
    Failed { error: StoreError },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ConnEvent {
    /// The store opened with a session, or the user signed in again after it expired.
    SessionRestored,
    SignOutRequested,
    Connected,
    /// Connecting failed or a live connection dropped; worth retrying.
    ConnectionLost,
    /// The server or the local database refused in a way retrying cannot fix.
    Fatal(StoreError),
    RetryElapsed,
}

/// The next state. `jitter` in `[0, 1)` spreads reconnects of many clients apart.
pub(crate) fn step(state: &ConnState, event: ConnEvent, jitter: f64) -> ConnState {
    use ConnEvent as E;
    use ConnState as S;
    match (state, event) {
        (_, E::SignOutRequested) => S::SignedOut,
        (_, E::Fatal(error)) => S::Failed { error },
        (_, E::SessionRestored) => S::Connecting { attempt: 0 },
        (S::Connecting { .. }, E::Connected) => S::Live,
        (S::Connecting { attempt }, E::ConnectionLost) => {
            S::Waiting { attempt: attempt + 1, retry_in: backoff(attempt + 1, jitter) }
        }
        (S::Live, E::ConnectionLost) => S::Waiting { attempt: 1, retry_in: backoff(1, jitter) },
        (S::Waiting { attempt, .. }, E::RetryElapsed) => S::Connecting { attempt: *attempt },
        (state, _) => state.clone(),
    }
}

/// Exponential backoff for the `attempt`-th retry (1-based), capped, scaled by 50–100 %.
pub(crate) fn backoff(attempt: u32, jitter: f64) -> Duration {
    let exponent = attempt.saturating_sub(1).min(16);
    let full = BACKOFF_BASE.saturating_mul(1 << exponent).min(BACKOFF_CAP);
    full.mul_f64(0.5 + 0.5 * jitter.clamp(0.0, 1.0))
}

#[cfg(test)]
#[path = "conn_state_tests.rs"]
mod tests;
