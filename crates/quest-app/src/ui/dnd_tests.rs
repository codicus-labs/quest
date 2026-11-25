//! Drag-and-drop interaction tests: real pointer gestures (down, moves, up) on the demo in
//! Rune's headless `Runtime`, plus Esc, clicks and the keyboard ways.

use chrono::Days;
use quest_domain::Task;
use rune::{Key, Mods, Runtime};

use super::tests::{TempDir, click, command, demo_app, plan_key, press};
use crate::dnd::DragState;
use crate::state::State;

/// The planner, whose today column the drags here use.
fn app() -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = demo_app();
    command(&mut rt, '3');
    settle(&mut rt);
    (rt, dir)
}

/// Lets springs land (frames at dt 0 never advance them) and drop zones report their rects.
pub(super) fn settle(rt: &mut Runtime<State>) {
    rt.frame(0.0);
    rt.settle_all();
    rt.frame(0.0);
}

pub(super) fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.presented(key).unwrap_or_else(|| panic!("`{key}` is drawn")).0
}

pub(super) fn center(rt: &Runtime<State>, key: &str) -> (f32, f32) {
    rt.find(key).unwrap_or_else(|| panic!("`{key}` is on screen"))
}

/// Presses at `from`, moves there in a few steps (a frame each, as a window delivers them).
fn drag_to(rt: &mut Runtime<State>, from: (f32, f32), to: (f32, f32)) {
    rt.pointer_down(from.0, from.1);
    rt.frame(0.0);
    for i in 1..=4 {
        let t = i as f32 / 4.0;
        rt.pointer_move(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        rt.frame(0.0);
    }
}

pub(super) fn drag(rt: &mut Runtime<State>, from: (f32, f32), to: (f32, f32)) {
    drag_to(rt, from, to);
    rt.pointer_up(to.0, to.1);
    settle(rt);
}

pub(super) fn task(rt: &Runtime<State>, title: &str) -> Task {
    rt.state.snap.tasks.iter().find(|t| t.title == title).cloned().unwrap_or_else(|| panic!("task {title}"))
}

fn open_titles(rt: &Runtime<State>) -> Vec<String> {
    let today = rt.state.today();
    rt.state.snap.tasks_on(today).filter(|t| !t.is_done()).map(|t| t.title.clone()).collect()
}

#[test]
fn reorder_in_a_plan_column_by_the_handle() {
    let (mut rt, _dir) = app();
    let easy = task(&rt, "Easy 5 km. Leave the headphones at home.").id;
    let design = task(&rt, "Design the next chapter").id;
    let [_, top, _, _] = rect(&rt, &format!("plan-task-{design}"));
    let from = center(&rt, &format!("plan-title-{easy}"));
    drag_to(&mut rt, from, (from.0, top + 10.0));
    rt.settle_all();
    rt.frame(0.0);
    assert_eq!(rect(&rt, &format!("plan-task-{easy}"))[1], top, "its room opened where it lands");
    assert!(rt.find("drag-lift").is_some(), "the lifted card follows the pointer");
    rt.pointer_up(from.0, top + 10.0);
    rt.frame(0.0);
    assert_eq!(open_titles(&rt)[0], "Easy 5 km. Leave the headphones at home.");
    assert_eq!(rt.state.drag, DragState::Idle);
    assert!(rt.find("drag-lift").is_none());
}

#[test]
fn plan_cards_move_between_day_columns_and_to_the_backlog() {
    let (mut rt, _dir) = app();
    command(&mut rt, '3');
    settle(&mut rt);
    let polish = task(&rt, "Polish the details that matter");
    let today = rt.state.today();
    assert_eq!(polish.planned_on, Some(today + Days::new(1)));
    // Before the first card, so the card stays in view for the next drag.
    let [x, _, w, _] = rect(&rt, &plan_key(&rt, "col", 4));
    let first = rect(&rt, &format!("{}-top", plan_key(&rt, "add", 4)));
    let from = center(&rt, &format!("plan-title-{}", polish.id));
    drag(&mut rt, from, (x + w / 2.0, first[1] + 60.0));
    assert_eq!(task(&rt, "Polish the details that matter").planned_on, Some(today + Days::new(2)));

    click(&mut rt, "plan-backlog");
    settle(&mut rt);
    let [x, y, w, h] = rect(&rt, &plan_key(&rt, "col", 5));
    let from = center(&rt, &format!("plan-title-{}", polish.id));
    drag(&mut rt, from, (x + w / 2.0, y + h - 40.0));
    assert_eq!(task(&rt, "Polish the details that matter").planned_on, None, "dropped into Someday");
}

#[test]
fn escape_cancels_a_drag_and_nothing_changes() {
    let (mut rt, _dir) = app();
    let order = open_titles(&rt);
    let easy = task(&rt, "Easy 5 km. Leave the headphones at home.").id;
    let from = center(&rt, &format!("plan-title-{easy}"));
    drag_to(&mut rt, from, (from.0, from.1 - 300.0));
    assert!(rt.state.drag.dragging().is_some());
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.drag, DragState::Idle);
    rt.pointer_up(from.0, from.1 - 300.0);
    rt.frame(0.0);
    assert_eq!(open_titles(&rt), order);
    assert!(rt.find("drag-lift").is_none());
}

#[test]
fn a_dropped_card_outside_any_list_snaps_back() {
    let (mut rt, _dir) = app();
    let order = open_titles(&rt);
    let easy = task(&rt, "Easy 5 km. Leave the headphones at home.");
    let from = center(&rt, &format!("plan-title-{}", easy.id));
    drag(&mut rt, from, (from.0, 20.0));
    assert_eq!(open_titles(&rt), order);
    assert_eq!(task(&rt, "Easy 5 km. Leave the headphones at home."), easy, "unchanged");
}

#[test]
fn option_arrows_move_a_focused_card() {
    let (mut rt, _dir) = app();
    let make = task(&rt, "Make the first impression feel effortless").id;
    rt.update(|_, tx| tx.focus_when_shown(format!("plan-task-{make}"), true));
    rt.frame(0.0);
    let alt = Mods { alt: true, ..Mods::default() };
    press(&mut rt, Key::Up, alt);
    assert_eq!(open_titles(&rt)[0], "Make the first impression feel effortless");
    press(&mut rt, Key::Right, alt);
    let today = rt.state.today();
    assert_eq!(task(&rt, "Make the first impression feel effortless").planned_on, Some(today + Days::new(1)));
}
