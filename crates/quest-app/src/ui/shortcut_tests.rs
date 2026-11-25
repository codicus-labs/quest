//! The keyboard shortcuts: Space X D W ⌘⌫ on the selected row or the one under the pointer, J/K
//! and the arrows moving the selection, Focus's Space, ⌘↵ and Esc, T for the tracked column.

use chrono::{Datelike, Days};
use quest_domain::EntityId;
use rune::{Key, Mods, Runtime};

use super::dnd_tests::{center, settle, task};
use super::tests::{click, command, demo_app, go, press, type_text};
use crate::state::{Overlay, State, View};

/// Moves the pointer onto `id`'s row.
pub(super) fn hover(rt: &mut Runtime<State>, id: EntityId) {
    let (x, y) = center(rt, &format!("row-{id}"));
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
fn list_timer_keeps_its_position_when_started_and_stopped() {
    let (mut rt, _dir) = demo_app();
    settle(&mut rt);
    let id = task(&rt, "Draft this week’s essay").id;
    let button = format!("row-play-{id}");
    let before = center(&rt, &button);
    click(&mut rt, &button);
    assert!(runs(&rt, id));
    assert_eq!(center(&rt, &button), before);
    click(&mut rt, &button);
    assert!(rt.state.snap.running_entry().is_none());
    assert_eq!(center(&rt, &button), before);
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
    assert_eq!(rt.state.view, View::Today, "and stays on Today");
    type_text(&mut rt, " ");
    assert!(rt.state.snap.running_entry().is_none(), "Space again pauses it");

    type_text(&mut rt, "x");
    assert!(task(&rt, "Draft this week’s essay").is_done(), "X completes");
    type_text(&mut rt, "x");
    assert!(!task(&rt, "Draft this week’s essay").is_done(), "X reopens");

    let today = rt.state.today();
    type_text(&mut rt, "p");
    assert!(rt.find("task-calendar").is_some());
    type_text(&mut rt, "m");
    assert_eq!(task(&rt, "Draft this week’s essay").planned_on, Some(today + Days::new(1)), "P M: tomorrow");
    // The row left Today (and the pointer); its card under the pointer elsewhere stands in.
    rt.frame(0.0);
    assert_eq!(rt.state.hovered_task, None);
    rt.state.hovered_task = Some(essay);
    type_text(&mut rt, "w");
    let monday = today + Days::new(u64::from(7 - today.weekday().num_days_from_monday()));
    assert_eq!(task(&rt, "Draft this week’s essay").planned_on, Some(monday), "W: next week");

    let (key, mods) = cmd(Key::Backspace);
    press(&mut rt, key, mods);
    assert!(rt.state.snap.task(essay).is_none(), "⌘⌫ deletes");
    assert!(rt.find("toast").is_some(), "with an undo toast");
    command(&mut rt, 'z');
    assert!(rt.state.snap.task(essay).is_some(), "⌘Z brings it back");
}

fn row_id(key: &str) -> Option<EntityId> {
    key.strip_prefix("row-").and_then(|id| id.parse().ok())
}

/// Crossing Today's rows a third of a second apart, as a hand does.
fn cross_the_cards(rt: &mut Runtime<State>) -> (u64, EntityId) {
    let on_screen = |k: &String| row_id(k).is_some() && center(rt, k).1 < rt.window_viewport().1;
    let cards: Vec<String> = rt.elements().into_iter().filter_map(|e| e.key).filter(on_screen).collect();
    assert!(cards.len() > 3, "Today shows several rows");
    let commits = rt.stats.commits;
    for key in &cards {
        let (x, y) = center(rt, key);
        rt.pointer_move(x, y);
        rt.frame(0.3);
    }
    let last = cards.last().and_then(|k| row_id(k)).expect("a row id");
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
fn without_a_selection_or_a_row_under_the_pointer_the_task_keys_do_nothing() {
    let (mut rt, _dir) = demo_app();
    let essay = task(&rt, "Draft this week’s essay").id;
    click(&mut rt, &format!("row-title-{essay}"));
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.selected_task, None, "Esc clears the selection");
    rt.state.hovered_task = None;
    type_text(&mut rt, "x");
    press(&mut rt, Key::Backspace, Mods::default());
    let essay = task(&rt, "Draft this week’s essay");
    assert!(!essay.is_done(), "X did nothing");
}

#[test]
fn j_k_and_the_arrows_move_the_selection_and_x_completes_it() {
    let (mut rt, _dir) = demo_app();
    settle(&mut rt);
    let rows = crate::ui::lists::shown_tasks(&rt.state);
    type_text(&mut rt, "j");
    assert_eq!(rt.state.selected_task, Some(rows[0]), "J starts at the top");
    type_text(&mut rt, "j");
    press(&mut rt, Key::Down, Mods::default());
    assert_eq!(rt.state.selected_task, Some(rows[2]));
    type_text(&mut rt, "k");
    press(&mut rt, Key::Up, Mods::default());
    assert_eq!(rt.state.selected_task, Some(rows[0]));
    type_text(&mut rt, "k");
    assert_eq!(rt.state.selected_task, Some(rows[0]), "the top holds");
    assert!(rt.find(&format!("row-{}", rows[0])).is_some());

    rt.state.hovered_task = None;
    type_text(&mut rt, "x");
    assert!(rt.state.snap.task(rows[0]).is_some_and(|t| t.is_done()), "X completes the selected row");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.task_modal.as_ref().map(|form| form.task), Some(rows[0]), "Return expands the row");
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
    assert_eq!(rt.state.view, View::Today);
}

#[test]
fn focus_keys_pause_resume_complete_and_leave() {
    let (mut rt, _dir) = demo_app();
    let design = task(&rt, "Design the next chapter").id;
    go(&mut rt, View::Focus);
    type_text(&mut rt, " ");
    assert!(rt.state.snap.running_entry().is_none(), "Space pauses");
    assert!(rt.find("focus-title").is_some(), "Focus keeps showing the paused task");
    type_text(&mut rt, " ");
    assert!(runs(&rt, design), "Space resumes");
    let (key, mods) = cmd(Key::Enter);
    press(&mut rt, key, mods);
    assert!(task(&rt, "Design the next chapter").is_done(), "⌘↵ completes");
    assert_eq!(rt.state.view, View::Today);
    go(&mut rt, View::Focus);
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.view, View::Today, "Esc goes to Today");
}

#[test]
fn focus_edits_the_title_and_description() {
    let (mut rt, _dir) = demo_app();
    let design = task(&rt, "Design the next chapter").id;
    go(&mut rt, View::Focus);
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
fn t_and_option_command_i_toggle_the_tracked_column_and_question_mark_shows_the_sheet() {
    let (mut rt, _dir) = demo_app();
    assert!(rt.find("tracked").is_some(), "the column starts open");
    type_text(&mut rt, "t");
    assert!(rt.find("tracked").is_none(), "T closes it");
    command(&mut rt, '3');
    assert!(rt.find("tracked").is_none(), "closed stays closed across views");
    press(&mut rt, Key::Command('i'), Mods { cmd: true, alt: true, ..Mods::default() });
    assert!(rt.find("tracked").is_some(), "⌥⌘I opens it");
    click(&mut rt, "right-column-toggle");
    assert!(rt.find("tracked").is_none(), "so does its button");

    type_text(&mut rt, "?");
    assert_eq!(rt.state.overlay, Overlay::Shortcuts);
    assert!(rt.find("shortcuts").is_some());
    type_text(&mut rt, "t");
    assert!(rt.find("tracked").is_none(), "keys wait while the sheet is open");
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.overlay, Overlay::None);
    command(&mut rt, ',');
    assert_eq!(rt.state.overlay, Overlay::Account);
}
