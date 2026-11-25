//! Interaction tests: the real view and key handler in Rune's headless `Runtime`, on a demo store
//! in a temporary directory, with the clock frozen at the reference time.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::NaiveDateTime;
use quest_store::{AccountInfo, MemoryVault, Profiles, ProfilesConfig, SignInOutcome, SignedIn, StoredTokens, Url};
use rune::{Env, Key, KeyEvent, Mods, Runtime};

use crate::clock::Clock;
use crate::keys;
use crate::state::{Overlay, State, View};

pub(super) struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The runtime comes first so it (and its store) drops before the directory is removed.
pub(super) fn demo_app() -> (Runtime<State>, TempDir) {
    test_app(true)
}

pub(super) fn test_app(demo: bool) -> (Runtime<State>, TempDir) {
    let dir = TempDir(std::env::temp_dir().join(format!("quest-app-test-{}", quest_domain::EntityId::new())));
    let now: NaiveDateTime = "2026-10-15T14:40:00".parse().expect("valid time");
    let clock = Clock::from_local(Some(now));
    let config = ProfilesConfig { data_dir: dir.0.clone(), vault: Arc::new(MemoryVault::default()) };
    let (profiles, store) = Profiles::open(config, || {}).expect("store opens in a temp dir");
    if demo {
        crate::seed_demo(&store, clock).expect("demo seeds");
    }
    let state = State::new(Arc::new(store), profiles, clock);
    let mut rt = Runtime::new(state, super::view, (1440.0, 900.0), Env::new(true, 1.0));
    rt.on_key = Some(keys::on_key);
    rt.poll = Some(|s: &mut State, clock, tx| s.refresh(clock, tx));
    rt.frame(0.0);
    (rt, dir)
}

/// Signs in to an account at a server that isn't there (the store stays offline), with the demo
/// data seeded into the account's store. `ai_enabled`: the account turned AI features on.
pub(super) fn signed_in_demo_app(ai_enabled: bool) -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = test_app(false);
    let account = AccountInfo {
        user_id: quest_protocol::UserId(quest_domain::EntityId::new().0),
        issuer: "http://127.0.0.1:9".to_owned(),
        subject: "anna-sub".to_owned(),
        email: Some("anna@example.test".to_owned()),
        name: Some("Anna".to_owned()),
        ai_enabled,
        ai_available: true,
    };
    let tokens =
        StoredTokens { access_token: "a".into(), refresh_token: Some("r".into()), access_expires_at_ms: u64::MAX };
    let signed_in = SignedIn {
        server_url: Url::parse("http://127.0.0.1:9").expect("a URL"),
        client_id: "quest-desktop".to_owned(),
        tokens,
        account,
    };
    let Ok(SignInOutcome::Switched { store, .. }) = rt.state.auth.profiles.sign_in(&rt.state.store, signed_in) else {
        panic!("the guest switches to the account");
    };
    crate::seed_demo(&store, rt.state.clock).expect("demo seeds");
    rt.state.use_store(store);
    rt.frame(0.0);
    (rt, dir)
}

pub(super) fn press(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(0.0);
}

pub(super) fn command(rt: &mut Runtime<State>, c: char) {
    press(rt, Key::Command(c), Mods { cmd: true, ..Mods::default() });
}

pub(super) fn type_text(rt: &mut Runtime<State>, text: &str) {
    press(rt, Key::Text(text.into()), Mods::default());
}

/// Goes to `view` as the sidebar would.
pub(super) fn go(rt: &mut Runtime<State>, view: View) {
    rt.state.go(view);
    rt.frame(0.0);
}

/// The key of a planner column's `part` ("col", "add", "list") at `index`; the backlog is last.
pub(super) fn plan_key(rt: &Runtime<State>, part: &str, index: usize) -> String {
    if index >= crate::plan_strip::VISIBLE_DAYS {
        return format!("plan-{part}-backlog");
    }
    format!("plan-{part}-{}", rt.state.plan.from() + chrono::Days::new(index as u64))
}

/// Presses `key` once whatever is fading in has landed (an element at opacity 0 takes no press).
pub(super) fn click(rt: &mut Runtime<State>, key: &str) {
    rt.settle_all();
    rt.frame(0.0);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("`{key}` is on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
}

/// Two quick clicks on `key`.
pub(super) fn double_click(rt: &mut Runtime<State>, key: &str) {
    rt.settle_all();
    rt.frame(0.0);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("`{key}` is on screen"));
    for _ in 0..2 {
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
    }
    rt.frame(0.0);
}

/// Clicks a card just right of its title, on the title's line: its free space, away from the
/// buttons.
pub(super) fn click_beside_title(rt: &mut Runtime<State>, prefix: &str, id: quest_domain::EntityId) {
    rt.settle_all();
    rt.frame(0.0);
    let key = format!("{prefix}-title-{id}");
    let [x, y, w, h] = rt.presented(&key).unwrap_or_else(|| panic!("`{key}` is drawn")).0;
    let at = (x + w + 40.0, y + h / 2.0);
    rt.pointer_down(at.0, at.1);
    rt.pointer_up(at.0, at.1);
    rt.frame(0.0);
}

#[test]
fn command_digits_switch_views_and_escape_leaves_focus_for_today() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '1');
    assert_eq!(rt.state.view, View::Inbox);
    command(&mut rt, '3');
    assert_eq!(rt.state.view, View::Plan);
    assert!(rt.find("plan").is_some(), "the planner is drawn");
    command(&mut rt, '5');
    assert_eq!(rt.state.view, View::Logbook);
    let first_quest = rt.state.snap.active_quests().next().map(|q| q.id).expect("a demo quest");
    command(&mut rt, '6');
    assert_eq!(rt.state.view, View::Quest(first_quest), "⌘6 opens the first quest");
    go(&mut rt, View::Focus);
    assert_eq!(rt.state.view, View::Focus);
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.view, View::Today, "Esc goes back to Today");
}

#[test]
fn the_palette_jumps_to_a_view() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, 'k');
    assert_eq!(rt.state.overlay, Overlay::Palette);
    type_text(&mut rt, "review");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.overlay, Overlay::None);
    assert_eq!(rt.state.view, View::Review);
}

#[test]
fn n_focuses_the_add_field_and_enter_creates_a_task() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '3');
    type_text(&mut rt, "n");
    assert_eq!(rt.state.view, View::Today);
    let before = rt.state.snap.tasks.len();
    type_text(&mut rt, "Stretch for ten minutes");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.snap.tasks.len(), before + 1);
    assert!(rt.state.snap.tasks.iter().any(|t| t.title == "Stretch for ten minutes"));
}

#[test]
fn space_on_the_selected_row_moves_focus_to_that_task() {
    let (mut rt, _dir) = demo_app();
    let task =
        rt.state.snap.tasks.iter().find(|t| t.title == "Draft this week’s essay").map(|t| t.id).expect("demo task");
    click(&mut rt, &format!("row-title-{task}"));
    assert_eq!(rt.state.selected_task, Some(task), "a click selects the row");
    type_text(&mut rt, " ");
    let running = rt.state.snap.running_entry().expect("a focus session runs");
    assert_eq!(running.task_id, Some(task));
}

#[test]
fn the_avatar_opens_the_account_panel() {
    let (mut rt, _dir) = demo_app();
    click(&mut rt, "avatar");
    assert_eq!(rt.state.overlay, Overlay::Account);
    assert!(rt.find("account-sign-in").is_some());
}

#[test]
fn an_idle_window_re_runs_nothing_while_a_timer_runs() {
    let (mut rt, _dir) = demo_app();
    assert!(rt.state.snap.running_entry().is_some(), "the demo has a running timer");
    for view in ['1', '2'] {
        command(&mut rt, view);
        for _ in 0..30 {
            rt.frame(1.0 / 60.0);
        }
        let commits = rt.stats.commits;
        // Three seconds with the clock frozen: nothing shown changes, so nothing commits.
        for _ in 0..180 {
            rt.frame(1.0 / 60.0);
        }
        assert_eq!(rt.stats.commits, commits, "view ⌘{view} committed while idle");
    }
}
