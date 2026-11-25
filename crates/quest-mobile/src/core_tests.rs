use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use quest_store::{StoreError, SyncStatus};

use super::*;
use crate::status::sync_state;
use crate::types::{AddPosition, HomeQuery, SignInResult, TaskSection, UrlPolicy};

#[derive(Default)]
struct Counter(AtomicUsize);

impl ChangeListener for Counter {
    fn on_change(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Wednesday, 25 Nov 2026, 10:00 in Berlin.
fn frozen() -> Moment {
    Moment { epoch_ms: 1_795_564_800_000 + 9 * 3_600_000, time_zone: "Europe/Berlin".into() }
}

fn open() -> (tempfile::TempDir, Arc<QuestCore>, Arc<Counter>) {
    let dir = tempfile::tempdir().unwrap();
    let counter = Arc::new(Counter::default());
    let core =
        QuestCore::open(dir.path().display().to_string(), crate::test_support::tokens(), counter.clone()).unwrap();
    (dir, core, counter)
}

fn home(core: &QuestCore) -> HomeData {
    core.home(frozen(), HomeQuery::default()).unwrap()
}

/// Adds an OTHER task for today at the bottom.
fn add(core: &QuestCore, title: &str) -> String {
    core.add_task(title.into(), TaskSection::Other, AddPosition::Bottom, HomeQuery::default(), frozen()).unwrap()
}

#[test]
fn added_tasks_show_on_home_and_toggle_between_done_and_open() {
    let (_dir, core, _) = open();
    assert_eq!(home(&core).day_label, "Wednesday, November 25");

    let id = add(&core, "Draft chapter");
    let tasks = home(&core).other;
    assert_eq!((tasks.len(), tasks[0].id.as_str(), tasks[0].done), (1, id.as_str(), false));

    core.toggle_task(id.clone(), frozen()).unwrap();
    assert!(home(&core).other[0].done);
    core.toggle_task(id, frozen()).unwrap();
    assert!(!home(&core).other[0].done);
}

#[test]
fn rejected_commands_and_bad_input_come_back_as_errors() {
    let (_dir, core, _) = open();
    let empty = core.add_task("  ".into(), TaskSection::Other, AddPosition::Top, HomeQuery::default(), frozen());
    assert_eq!(empty, Err(MobileError::EmptyTitle));
    let unknown = EntityId::new().to_string();
    assert_eq!(core.toggle_task(unknown.clone(), frozen()), Err(MobileError::UnknownTask { id: unknown }));
    assert!(matches!(core.toggle_task("x".into(), frozen()), Err(MobileError::InvalidId { .. })));
    let bad_url = core.begin_sign_in("not a url".into(), UrlPolicy::Release, None);
    assert!(matches!(bad_url, Err(MobileError::InvalidServerUrl { .. })));
}

#[test]
fn starts_signed_out_and_notifies_the_listener() {
    let (_dir, core, counter) = open();
    assert_eq!(core.sync_state(), SyncState::SignedOut);
    assert_eq!(core.account(), None);
    add(&core, "Draft");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while counter.0.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(counter.0.load(Ordering::SeqCst) > 0);
}

#[test]
fn sync_status_maps_to_ui_states() {
    assert_eq!(sync_state(&SyncStatus::Live { pending: 2 }), SyncState::Live { pending: 2 });
    assert_eq!(
        sync_state(&SyncStatus::Offline { pending: 1, retry_in: Duration::from_millis(2500) }),
        SyncState::Offline { pending: 1, retry_in_secs: 2 }
    );
    let failed = sync_state(&SyncStatus::Failed { error: StoreError::UnsupportedProtocol });
    assert!(matches!(failed, SyncState::Failed { message } if message.contains("Update the app")));
    let expired = sync_state(&SyncStatus::Failed { error: StoreError::SessionExpired });
    assert_eq!(expired, SyncState::SessionExpired, "the app offers to sign in again");
}

fn moment(rfc3339: &str) -> Moment {
    let at = chrono::DateTime::parse_from_rfc3339(rfc3339).unwrap();
    Moment { epoch_ms: at.timestamp_millis(), time_zone: "Europe/Berlin".into() }
}

#[test]
fn tick_creates_due_recurring_tasks_and_asks_for_the_next_minute() {
    use quest_domain::{Frequency, Position, RepeatPin, RepeatRule, Weekdays};
    let (_dir, core, _) = open();
    let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 25).unwrap();
    let id = EntityId::new();
    let rule = RepeatRule {
        every: 1,
        frequency: Frequency::Day,
        starts: today,
        ends: None,
        time_zone: chrono_tz::Europe::Berlin,
        pin: RepeatPin::None,
        weekdays: Weekdays::default(),
        month_day: None,
    };
    let create = Command::CreateTask {
        id,
        title: "Stretch".into(),
        quest_id: None,
        day: Some(today),
        position: Position::Bottom,
    };
    core.dispatch(create).unwrap();
    let now = moment("2026-11-25T10:00:00+01:00");
    core.dispatch(Command::SetTaskRepeat { id, rule: Some(rule), now: Now::parse(&now).unwrap().at }).unwrap();

    assert!(core.home(moment("2026-11-26T00:00:00+01:00"), HomeQuery::default()).unwrap().other.is_empty());
    let next = core.tick(moment("2026-11-26T00:00:00+01:00")).unwrap();
    assert_eq!(next, moment("2026-11-26T00:01:00.005+01:00").epoch_ms);
    let tomorrow = core.home(moment("2026-11-26T00:00:00+01:00"), HomeQuery::default()).unwrap();
    assert_eq!(tomorrow.other.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["Stretch"]);
}

#[test]
fn next_wake_ticks_each_second_while_focus_runs() {
    let (_dir, core, _) = open();
    let task = add(&core, "Draft").parse().unwrap();
    let now = moment("2026-11-25T10:00:00.400+01:00");
    assert_eq!(core.next_wake_ms(now.clone()).unwrap(), moment("2026-11-25T10:01:00.005+01:00").epoch_ms);
    let at = Now::parse(&now).unwrap().at;
    core.dispatch(Command::StartFocus { entry_id: EntityId::new(), task_id: task, at }).unwrap();
    assert_eq!(core.next_wake_ms(now.clone()).unwrap(), moment("2026-11-25T10:00:01.005+01:00").epoch_ms);
    assert_eq!(core.tick(now).unwrap(), moment("2026-11-25T10:00:01.005+01:00").epoch_ms);
}

#[test]
fn weekly_target_can_be_read_and_set() {
    let (_dir, core, _) = open();
    assert_eq!(core.settings(), SettingsData { weekly_target_minutes: 1200, weekly_target_label: "20h".into() });
    core.set_weekly_target(600).unwrap();
    assert_eq!(core.settings().weekly_target_label, "10h");
    assert_eq!(core.set_weekly_target(0), Err(MobileError::InvalidWeeklyTarget { minutes: 0 }));
}

#[test]
fn sign_in_starts_in_the_browser_with_the_android_client() {
    let (_dir, core, _) = open();
    let url = core.begin_sign_in("http://127.0.0.1:8787".into(), UrlPolicy::Release, None).unwrap();
    assert!(url.starts_with("http://127.0.0.1:8787/authorize?"), "{url}");
    assert!(url.contains("client_id=quest-android"), "{url}");
    assert!(url.contains("redirect_uri=app.quest.android%3A%2Foauth2redirect"), "{url}");
    assert!(url.contains("code_challenge_method=S256"), "{url}");
    let https = core.begin_sign_in("https://quest.example.com".into(), UrlPolicy::Release, None).unwrap();
    assert!(https.contains("redirect_uri=app.quest.android%3A%2Foauth2redirect"), "{https}");
    let logged_out = core.finish_sign_in("app.quest.android:/oauth2redirect".into()).unwrap();
    assert_eq!(logged_out, SignInResult::NotASignIn, "the return from signing out is no sign-in");
}

#[test]
fn a_build_with_an_app_link_signs_in_at_its_server_through_the_app_link() {
    let (_dir, core, _) = open();
    let app_link = || Some("https://quest.example.com".to_owned());
    let url = core.begin_sign_in("https://quest.example.com/".into(), UrlPolicy::Release, app_link()).unwrap();
    assert!(url.contains("redirect_uri=https%3A%2F%2Fquest.example.com%2Fauth%2Fandroid"), "{url}");
    let elsewhere = core.begin_sign_in("https://other.example.com".into(), UrlPolicy::Release, app_link()).unwrap();
    assert!(elsewhere.contains("redirect_uri=app.quest.android%3A%2Foauth2redirect"), "{elsewhere}");
    let logged_out = core.finish_sign_in("https://quest.example.com/auth/android".into()).unwrap();
    assert_eq!(logged_out, SignInResult::NotASignIn);
}

#[test]
fn an_app_link_sign_in_takes_its_answer_only_at_the_app_link() {
    // A closed local port: the exchange fails without leaving this machine.
    let (server, app_link) = ("https://127.0.0.1:9", || Some("https://127.0.0.1:9".to_owned()));
    let (_dir, core, _) = open();
    let state = |url: &str| url.split("state=").nth(1).and_then(|s| s.split('&').next()).unwrap().to_owned();
    let answer = |target: &str, state: &str| format!("{target}?code=abc&state={state}&iss=https%3A%2F%2F127.0.0.1%3A9");

    let started = core.begin_sign_in(server.into(), UrlPolicy::Release, app_link()).unwrap();
    let wrong_target = core.finish_sign_in(answer("app.quest.android:/oauth2redirect", &state(&started)));
    assert!(
        matches!(&wrong_target, Err(MobileError::SignInFailed { reason }) if reason.contains("unexpected address")),
        "{wrong_target:?}"
    );

    let started = core.begin_sign_in(server.into(), UrlPolicy::Release, app_link()).unwrap();
    let exchanged = core.finish_sign_in(answer("https://127.0.0.1:9/auth/android", &state(&started)));
    let Err(MobileError::SignInFailed { reason }) = exchanged else { panic!("{exchanged:?}") };
    assert!(!reason.contains("unexpected address") && !reason.contains("does not belong"), "{reason}");
}

#[test]
fn an_answer_without_a_waiting_sign_in_asks_to_start_again() {
    let (_dir, core, _) = open();
    let answer = core.finish_sign_in("app.quest.android:/oauth2redirect?code=abc&state=xyz".into());
    assert_eq!(answer, Err(MobileError::SignInExpired));
}

#[test]
fn a_stray_answer_leaves_the_waiting_sign_in_alone() {
    let (_dir, core, _) = open();
    core.begin_sign_in("http://127.0.0.1:8787".into(), UrlPolicy::Release, None).unwrap();
    for _ in 0..2 {
        let stray = core.finish_sign_in("app.quest.android:/oauth2redirect?code=abc&state=forged".into());
        assert!(matches!(stray, Err(MobileError::SignInFailed { .. })), "{stray:?}");
    }
    assert_eq!(core.sign_out_warning(), None);
}

#[test]
fn edits_are_refused_while_signing_out() {
    let (_dir, core, _) = open();
    *core.signing_out.write().unwrap() = true;
    let refused =
        core.add_task("Lost?".into(), TaskSection::Other, AddPosition::Bottom, HomeQuery::default(), frozen());
    assert_eq!(refused.map(|_| ()), Err(MobileError::SigningOut));
    *core.signing_out.write().unwrap() = false;
    add(&core, "Kept");
}

#[test]
fn sign_in_refuses_unencrypted_remote_servers() {
    let (_dir, core, _) = open();
    let insecure = core.begin_sign_in("http://quest.example.com".into(), UrlPolicy::Development, None);
    assert_eq!(insecure, Err(MobileError::InsecureServerUrl { value: "http://quest.example.com".into() }));
    assert_eq!(core.sync_state(), SyncState::SignedOut);
    assert_eq!(core.sign_out(), Err(MobileError::NotSignedIn), "the guest has nothing to sign out of");
    assert_eq!(core.sign_out_warning(), None);
    assert_eq!(crate::check_server_url("http://10.0.2.2:8787".into(), UrlPolicy::Development), Ok(()));
    assert!(crate::check_server_url("http://10.0.2.2:8787".into(), UrlPolicy::Release).is_err());
}
