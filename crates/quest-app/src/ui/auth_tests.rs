//! The account dialog's flows without a real server or browser: signing in starts the loopback
//! wait and can be cancelled, signing out warns about unsent changes and returns to the guest
//! data, and the guest import dialog. The browser is the no-op program `true`.

use std::time::{Duration, Instant};

use rune::{Key, Mods, Runtime};

use super::log_work_tests::text_of;
use super::tests::{click, command, demo_app, press, signed_in_demo_app};
use crate::browser::Browser;
use crate::state::{AuthStep, Overlay, State};

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key))
}

/// Steps frames until `done` holds (results come from threads).
fn frames_until(rt: &mut Runtime<State>, done: impl Fn(&State) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !done(&rt.state) {
        assert!(Instant::now() < deadline, "the flow finishes");
        std::thread::sleep(Duration::from_millis(10));
        rt.frame(0.0);
    }
}

#[test]
fn the_guest_dialog_signs_in_with_the_browser_and_can_cancel() {
    let (mut rt, _dir) = demo_app();
    rt.state.auth.browser = Browser::with_program("true");
    command(&mut rt, ',');
    assert!(has(&rt, "account-server"), "the server stays configurable");
    assert!(!has(&rt, "account-email") && !has(&rt, "account-password"), "no email or password");
    rt.state.account_form.server = rune::TextEdit::new("http://127.0.0.1:8787");
    click(&mut rt, "account-sign-in");
    assert!(matches!(rt.state.auth.step, AuthStep::SigningIn { .. }));
    assert!(text_of(&rt, "account-progress").contains("Continue in your browser"));
    click(&mut rt, "account-sign-in-cancel");
    // Until the attempt's thread has stopped, no second sign-in starts.
    assert!(matches!(rt.state.auth.step, AuthStep::CancellingSignIn));
    rt.state.start_sign_in();
    assert!(matches!(rt.state.auth.step, AuthStep::CancellingSignIn));
    frames_until(&mut rt, |s| matches!(s.auth.step, AuthStep::Idle));
    assert!(rt.state.notice.is_none(), "a cancel is no error");
    assert!(rt.state.account.is_none());
    assert!(has(&rt, "account-sign-in"), "signing in can start again");
}

#[test]
fn a_bad_server_url_says_what_to_enter() {
    let (mut rt, _dir) = demo_app();
    rt.state.account_form.server = rune::TextEdit::new("not a url");
    rt.state.start_sign_in();
    assert!(matches!(rt.state.auth.step, AuthStep::Idle));
    assert!(rt.state.notice.as_deref().is_some_and(|n| n.contains("is not a server URL")));
}

#[test]
fn the_server_field_starts_empty_and_asks_for_an_address() {
    let (mut rt, _dir) = demo_app();
    assert_eq!(rt.state.account_form.server.text, "", "no server is preset");
    rt.state.start_sign_in();
    assert!(matches!(rt.state.auth.step, AuthStep::Idle));
    assert!(rt.state.notice.as_deref().is_some_and(|n| n.contains("Enter your Quest server’s address")));
}

#[test]
fn signed_in_the_dialog_names_the_account() {
    let (mut rt, _dir) = signed_in_demo_app(false);
    command(&mut rt, ',');
    assert_eq!(text_of(&rt, "account-name"), "Signed in as Anna");
    assert_eq!(text_of(&rt, "account-email"), "anna@example.test");
    assert!(!has(&rt, "account-server"));
}

#[test]
fn signing_out_warns_about_unsent_changes_then_shows_the_guest_data() {
    let (mut rt, _dir) = signed_in_demo_app(false);
    rt.state.auth.browser = Browser::with_program("true");
    assert!(!rt.state.snap.tasks.is_empty(), "the account has the demo tasks");
    command(&mut rt, ',');
    click(&mut rt, "account-sign-out");
    let AuthStep::ConfirmSignOut { pending } = rt.state.auth.step else { panic!("asks first") };
    assert!(pending > 0, "the offline account has unsent changes");
    assert!(text_of(&rt, "account-sign-out-warning").contains("haven’t reached the server"));
    click(&mut rt, "account-keep-signed-in");
    assert!(matches!(rt.state.auth.step, AuthStep::Idle));
    assert!(rt.state.account.is_some());
    click(&mut rt, "account-sign-out");
    click(&mut rt, "account-sign-out-confirm");
    frames_until(&mut rt, |s| s.account.is_none() && matches!(s.auth.step, AuthStep::Idle));
    assert!(rt.state.snap.tasks.is_empty(), "the guest store shows, empty");
    assert!(has(&rt, "account-sign-in"));
}

#[test]
fn the_guest_import_dialog_answers_no_with_esc() {
    let (mut rt, _dir) = demo_app();
    rt.state.overlay = Overlay::GuestImport;
    rt.settle_all();
    rt.frame(0.0);
    assert!(has(&rt, "guest-import-yes"));
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.overlay, Overlay::Account, "back to the account, the guest data stays");
}

#[test]
fn signing_out_locks_edits_until_it_is_done() {
    let (mut rt, _dir) = signed_in_demo_app(false);
    rt.state.auth.browser = Browser::with_program("true");
    rt.state.overlay = Overlay::Account;
    let tasks = rt.state.snap.tasks.len();
    rt.state.sign_out();
    assert!(rt.state.auth.locks_edits());
    rt.state.dispatch(quest_domain::Command::CreateTask {
        id: quest_domain::EntityId::new(),
        title: "Lost".into(),
        quest_id: None,
        planned_on: None,
        due_on: None,
        position: quest_domain::Position::Top,
    });
    assert_eq!(rt.state.snap.tasks.len(), tasks, "no change while signing out");
    assert!(rt.state.notice.as_deref().is_some_and(|n| n.contains("Signing out")));
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.overlay, Overlay::Account, "the dialog stays open");
    frames_until(&mut rt, |s| matches!(s.auth.step, AuthStep::Idle));
    assert!(rt.state.account.is_none());
    assert!(!rt.state.auth.locks_edits());
}

#[test]
fn a_failed_import_keeps_the_offer_and_offers_to_try_again() {
    let (mut rt, _dir) = demo_app();
    // The guest has no account to import into: the import fails.
    rt.state.import_guest();
    frames_until(&mut rt, |s| matches!(s.auth.step, AuthStep::Idle));
    assert_eq!(rt.state.overlay, Overlay::GuestImport, "the offer stays");
    rt.settle_all();
    rt.frame(0.0);
    assert!(text_of(&rt, "guest-import-error").contains("The guest data is unchanged."));
    assert!(has(&rt, "guest-import-yes"), "Try again");
    click(&mut rt, "guest-import-no");
    assert!(rt.state.auth.import_error.is_none());
}
