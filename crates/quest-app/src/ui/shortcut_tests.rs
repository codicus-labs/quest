//! The keyboard shortcuts: Space X T D W ⌘⌫ on the task under the pointer, Focus's Space, ⌘↵ and
//! Esc, Tab through views, and Play on a card staying in its view.

use chrono::{Datelike, Days};
use quest_domain::EntityId;
use rune::{Key, Mods, Runtime};

use super::dnd_tests::{center, settle, task};
use super::tests::{click, click_beside_title, command, demo_app, press, type_text};
use crate::state::{Overlay, State, View};

/// Moves the pointer onto `id`'s Home card.
pub(super) fn hover(rt: &mut Runtime<State>, id: EntityId) {
    let (x, y) = center(rt, &format!("home-task-{id}"));
    rt.pointer_move(x, y);
    rt.frame(0.0);
}

fn cmd(key: Key) -> (Key, Mods) {
    (key, Mods { cmd: true, ..Mods::default() })
}

fn runs(rt: &Runtime<State>, id: EntityId) -> bool {
    rt.state.snap.running_entry().is_some_and(|e| e.task_id == Some(id))
}

#[test]
fn keys_act_on_the_task_under_the_pointer() {
    let (mut rt, _dir) = demo_app();
    settle(&mut rt);
    let essay = task(&rt, "Draft this week’s essay").id;
    hover(&mut rt, essay);
    assert_eq!(rt.state.hovered_task, Some(essay));

    type_text(&mut rt, " ");
    assert!(runs(&rt, essay), "Space starts it");
    assert_eq!(rt.state.view, View::Home, "and stays on Home");
    type_text(&mut rt, " ");
    assert!(rt.state.snap.running_entry().is_none(), "Space again pauses it");

    type_text(&mut rt, "x");
    assert!(task(&rt, "Draft this week’s essay").is_done(), "X completes");
    type_text(&mut rt, "x");
    assert!(!task(&rt, "Draft this week’s essay").is_done(), "X reopens");

    let today = rt.state.today();
    type_text(&mut rt, "d");
    assert_eq!(task(&rt, "Draft this week’s essay").day, Some(today + Days::new(1)), "D: the next day");
    // The card left Home (and the pointer); its card under the pointer elsewhere stands in.
    rt.frame(0.0);
    assert_eq!(rt.state.hovered_task, None);
    rt.state.hovered_task = Some(essay);
    type_text(&mut rt, "w");
    let monday = today + Days::new(u64::from(7 - today.weekday().num_days_from_monday()));
    assert_eq!(task(&rt, "Draft this week’s essay").day, Some(monday), "W: next week");
    type_text(&mut rt, "t");
    assert_eq!(task(&rt, "Draft this week’s essay").day, Some(today), "T: today");

    rt.state.hovered_task = None;
    settle(&mut rt);
    hover(&mut rt, essay);
    let (key, mods) = cmd(Key::Backspace);
    press(&mut rt, key, mods);
    assert!(rt.state.snap.task(essay).is_none(), "⌘⌫ deletes");
    assert!(rt.find("toast").is_some(), "with an undo toast");
    command(&mut rt, 'z');
    assert!(rt.state.snap.task(essay).is_some(), "⌘Z brings it back");
}

/// Crossing the Home cards a third of a second apart, as a hand does.
fn cross_the_cards(rt: &mut Runtime<State>) -> (u64, EntityId) {
    let on_screen = |k: &String| k.starts_with("home-task-") && center(rt, k).1 < rt.window_viewport().1;
    let cards: Vec<String> = rt.elements().into_iter().filter_map(|e| e.key).filter(on_screen).collect();
    assert!(cards.len() > 3, "Home shows several cards");
    let commits = rt.stats.commits;
    for key in &cards {
        let (x, y) = center(rt, key);
        rt.pointer_move(x, y);
        rt.frame(0.3);
    }
    let last =
        cards.last().and_then(|k| k.strip_prefix("home-task-")).and_then(|id| id.parse().ok()).expect("a card id");
    (rt.stats.commits - commits, last)
}

#[test]
fn crossing_cards_commits_nothing_and_the_keys_follow_the_pointer() {
    let (mut rt, _dir) = demo_app();
    settle(&mut rt);
    // Without the debug build's live-only check (see the next test) hover only sets state.
    rt.check_live_only(false);
    let (commits, last) = cross_the_cards(&mut rt);
    assert_eq!(commits, 0, "hover keeps its card in state the view doesn't read");
    assert_eq!(rt.state.hovered_task, Some(last));
    let done = |rt: &Runtime<State>| rt.state.snap.task(last).is_some_and(|t| t.is_done());
    let before = done(&rt);
    type_text(&mut rt, "x");
    assert_ne!(done(&rt), before, "X acts on the card under the pointer");
}

/// Rune's debug builds re-run the view for a live-only event a quarter second after the last
/// commit, to check it; the hover-rows warning counts those runs (RUNE_NOTES, Round 9).
#[test]
fn the_debug_live_only_check_is_what_commits_while_crossing_cards() {
    let (mut rt, _dir) = demo_app();
    settle(&mut rt);
    let (commits, _) = cross_the_cards(&mut rt);
    assert!(commits > 0, "the check re-runs the view");
}

#[test]
fn without_a_card_under_the_pointer_the_task_keys_do_nothing() {
    let (mut rt, _dir) = demo_app();
    let essay = task(&rt, "Draft this week’s essay").id;
    // A click no longer picks a card for the keys.
    click_beside_title(&mut rt, "home", essay);
    click(&mut rt, "task-modal-close");
    rt.state.hovered_task = None;
    type_text(&mut rt, "x");
    press(&mut rt, Key::Backspace, Mods::default());
    let essay = task(&rt, "Draft this week’s essay");
    assert!(!essay.is_done(), "X did nothing");
}

#[test]
fn play_on_a_card_starts_the_timer_in_place() {
    let (mut rt, _dir) = demo_app();
    settle(&mut rt);
    let essay = task(&rt, "Draft this week’s essay").id;
    click(&mut rt, &format!("home-play-{essay}"));
    assert!(runs(&rt, essay));
    assert_eq!(rt.state.view, View::Home);
}

#[test]
fn focus_complete_ends_the_session_marks_done_and_goes_home() {
    let (mut rt, _dir) = demo_app();
    let design = task(&rt, "Design the next chapter").id;
    assert!(runs(&rt, design), "the demo runs this one");
    type_text(&mut rt, "f");
    assert_eq!(rt.state.view, View::Focus);
    click(&mut rt, "focus-complete");
    assert!(task(&rt, "Design the next chapter").is_done());
    assert!(rt.state.snap.running_entry().is_none(), "the session ended");
    assert_eq!(rt.state.view, View::Home);
}

#[test]
fn focus_keys_pause_resume_complete_and_leave() {
    let (mut rt, _dir) = demo_app();
    let design = task(&rt, "Design the next chapter").id;
    command(&mut rt, '4');
    type_text(&mut rt, " ");
    assert!(rt.state.snap.running_entry().is_none(), "Space pauses");
    assert!(rt.find("focus-title").is_some(), "Focus keeps showing the paused task");
    type_text(&mut rt, " ");
    assert!(runs(&rt, design), "Space resumes");
    let (key, mods) = cmd(Key::Enter);
    press(&mut rt, key, mods);
    assert!(task(&rt, "Design the next chapter").is_done(), "⌘↵ completes");
    assert_eq!(rt.state.view, View::Home);
    command(&mut rt, '4');
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.view, View::Home, "Esc goes Home");
}

#[test]
fn focus_edits_the_title_and_description() {
    let (mut rt, _dir) = demo_app();
    let design = task(&rt, "Design the next chapter").id;
    command(&mut rt, '4');
    click(&mut rt, "focus-description");
    type_text(&mut rt, "Start from the reader's question.");
    press(&mut rt, Key::Enter, Mods::default());
    type_text(&mut rt, "Then the outline.");
    let saved = rt.state.snap.task(design).map(|t| t.description.clone());
    assert_eq!(saved.as_deref(), Some("Start from the reader's question.\nThen the outline."), "multi-line");
    assert_eq!(rt.state.view, View::Focus, "letters in a field are not shortcuts");
    click(&mut rt, "focus-title");
    press(&mut rt, Key::End, Mods::default());
    type_text(&mut rt, " again");
    assert_eq!(task(&rt, "Design the next chapter again").id, design);
}

#[test]
fn letters_and_tab_switch_views_and_question_mark_shows_the_sheet() {
    let (mut rt, _dir) = demo_app();
    type_text(&mut rt, "c");
    assert_eq!(rt.state.view, View::Calendar);
    press(&mut rt, Key::Tab, Mods::default());
    assert_eq!(rt.state.view, View::Focus);
    press(&mut rt, Key::Tab, Mods { shift: true, ..Mods::default() });
    assert_eq!(rt.state.view, View::Calendar);
    type_text(&mut rt, "q");
    assert_eq!(rt.state.view, View::Quests);
    type_text(&mut rt, "?");
    assert_eq!(rt.state.overlay, Overlay::Shortcuts);
    assert!(rt.find("shortcuts").is_some());
    type_text(&mut rt, "h");
    assert_eq!(rt.state.view, View::Quests, "keys wait while the sheet is open");
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.overlay, Overlay::None);
    command(&mut rt, ',');
    assert_eq!(rt.state.overlay, Overlay::Account);
}
