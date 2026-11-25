use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::*;

fn offline() -> SyncStatus {
    SyncStatus::Offline { pending: 1, retry_in: Duration::ZERO }
}

#[test]
fn a_blip_is_not_settled_yet() {
    let waiting = ConnState::Waiting { attempt: 1, retry_in: Duration::from_millis(300) };
    assert_eq!(settled_outcome(&offline(), &waiting), None);
    assert_eq!(settled_outcome(&offline(), &ConnState::Connecting { attempt: 1 }), None);
}

#[test]
fn offline_settles_once_a_retry_failed() {
    let waiting = ConnState::Waiting { attempt: 2, retry_in: Duration::from_secs(1) };
    assert_eq!(settled_outcome(&offline(), &waiting), Some(SyncOutcome::Offline));
    assert_eq!(settled_outcome(&offline(), &ConnState::Connecting { attempt: 2 }), Some(SyncOutcome::Offline));
}

#[test]
fn signed_out_settles_at_once() {
    assert_eq!(settled_outcome(&SyncStatus::SignedOut, &ConnState::SignedOut), Some(SyncOutcome::Offline));
}

#[test]
fn an_unrepresentable_timeout_waits_without_a_deadline() {
    let signal = Arc::new(ChangeSignal::default());
    let done = Arc::new(AtomicBool::new(false));
    let notifier = {
        let (signal, done) = (signal.clone(), done.clone());
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            done.store(true, Ordering::SeqCst);
            signal.notify();
        })
    };
    let outcome = signal.wait_until_settled(Duration::MAX, || done.load(Ordering::SeqCst).then_some(SyncOutcome::Idle));
    assert_eq!(outcome, SyncOutcome::Idle);
    notifier.join().unwrap();
}
