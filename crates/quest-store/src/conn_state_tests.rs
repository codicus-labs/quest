use std::time::Duration;

use super::*;

#[test]
fn backoff_doubles_and_is_capped() {
    assert_eq!(backoff(1, 1.0), Duration::from_millis(500));
    assert_eq!(backoff(2, 1.0), Duration::from_secs(1));
    assert_eq!(backoff(3, 1.0), Duration::from_secs(2));
    assert_eq!(backoff(10, 1.0), Duration::from_secs(30));
    assert_eq!(backoff(u32::MAX, 1.0), Duration::from_secs(30));
}

#[test]
fn jitter_scales_between_half_and_full_delay() {
    assert_eq!(backoff(3, 0.0), Duration::from_secs(1));
    assert_eq!(backoff(3, 0.5), Duration::from_millis(1500));
    assert_eq!(backoff(10, 0.0), Duration::from_secs(15));
}

#[test]
fn repeated_connect_failures_back_off_further() {
    let mut state = step(&ConnState::SignedOut, ConnEvent::SessionRestored, 1.0);
    assert_eq!(state, ConnState::Connecting { attempt: 0 });
    let mut delays = Vec::new();
    for _ in 0..4 {
        state = step(&state, ConnEvent::ConnectionLost, 1.0);
        let ConnState::Waiting { retry_in, .. } = state else { panic!("expected waiting, got {state:?}") };
        delays.push(retry_in);
        state = step(&state, ConnEvent::RetryElapsed, 1.0);
        assert!(matches!(state, ConnState::Connecting { .. }));
    }
    assert_eq!(delays, [500, 1000, 2000, 4000].map(Duration::from_millis));
}

#[test]
fn a_live_connection_resets_the_backoff() {
    let waiting = ConnState::Waiting { attempt: 5, retry_in: Duration::from_secs(8) };
    let connecting = step(&waiting, ConnEvent::RetryElapsed, 0.0);
    let live = step(&connecting, ConnEvent::Connected, 0.0);
    assert_eq!(live, ConnState::Live);
    assert_eq!(
        step(&live, ConnEvent::ConnectionLost, 1.0),
        ConnState::Waiting { attempt: 1, retry_in: Duration::from_millis(500) }
    );
}

#[test]
fn an_expired_session_stops_retrying_until_the_user_signs_in_again() {
    let failed = step(&ConnState::Live, ConnEvent::Fatal(StoreError::SessionExpired), 0.0);
    assert_eq!(failed, ConnState::Failed { error: StoreError::SessionExpired });
    assert_eq!(step(&failed, ConnEvent::RetryElapsed, 0.0), failed);
    assert_eq!(step(&failed, ConnEvent::SessionRestored, 0.0), ConnState::Connecting { attempt: 0 });
}

#[test]
fn sign_out_wins_from_any_state() {
    let states = [
        ConnState::Connecting { attempt: 2 },
        ConnState::Live,
        ConnState::Waiting { attempt: 1, retry_in: Duration::from_secs(1) },
        ConnState::Failed { error: StoreError::SessionExpired },
    ];
    for state in states {
        assert_eq!(step(&state, ConnEvent::SignOutRequested, 0.0), ConnState::SignedOut);
    }
}

#[test]
fn stale_events_do_not_change_state() {
    assert_eq!(step(&ConnState::SignedOut, ConnEvent::Connected, 0.0), ConnState::SignedOut);
    assert_eq!(step(&ConnState::SignedOut, ConnEvent::RetryElapsed, 0.0), ConnState::SignedOut);
    assert_eq!(step(&ConnState::Live, ConnEvent::RetryElapsed, 0.0), ConnState::Live);
}
