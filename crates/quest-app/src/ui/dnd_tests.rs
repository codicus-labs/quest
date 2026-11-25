//! Drag-and-drop interaction tests: real pointer gestures (down, moves, up) on the demo in
//! Rune's headless `Runtime`, one per kind of drag, plus Esc, clicks and the keyboard ways.

use chrono::{Datelike, Days, NaiveTime, Timelike};
use quest_domain::{EntityId, Task};
use rune::{Key, Mods, Runtime};

use super::tests::{TempDir, click, command, demo_app, plan_key, press};
use crate::clock;
use crate::dnd::DragState;
use crate::state::{Overlay, State};
use crate::ui::timegrid::{CALENDAR_SCALE, HOME_SCALE};

fn app() -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = demo_app();
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

pub(super) fn entry_of(rt: &Runtime<State>, title: &str) -> EntityId {
    let task = task(rt, title).id;
    rt.state.snap.time_entries.iter().find(|e| e.task_id == Some(task)).map(|e| e.id).expect("a session")
}

pub(super) fn entry_times(rt: &Runtime<State>, id: EntityId) -> (NaiveTime, Option<NaiveTime>) {
    let entry = rt.state.snap.time_entries.iter().find(|e| e.id == id).expect("entry exists");
    (clock::local(entry.started_at).time(), entry.ended_at.map(|t| clock::local(t).time()))
}

pub(super) fn hm(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

fn open_titles(rt: &Runtime<State>) -> Vec<String> {
    let today = rt.state.today();
    rt.state.snap.tasks_on(today).filter(|t| !t.is_done()).map(|t| t.title.clone()).collect()
}

/// The point on Home's day grid at `minute` after midnight.
pub(super) fn home_grid_at(rt: &Runtime<State>, minute: i32) -> (f32, f32) {
    let [x, y, w, _] = rect(rt, "home-grid-day-0");
    (x + w / 2.0, y + HOME_SCALE.y(minute))
}

#[test]
fn reorder_in_the_home_list_by_the_handle() {
    let (mut rt, _dir) = app();
    let easy = task(&rt, "Easy 5 km. Leave the headphones at home.").id;
    let design = task(&rt, "Design the next chapter").id;
    let [_, top, _, _] = rect(&rt, &format!("home-task-{design}"));
    let from = center(&rt, &format!("home-title-{easy}"));
    drag_to(&mut rt, from, (from.0, top + 10.0));
    rt.settle_all();
    rt.frame(0.0);
    assert_eq!(rect(&rt, &format!("home-task-{easy}"))[1], top, "its room opened where it lands");
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
    command(&mut rt, '2');
    settle(&mut rt);
    let polish = task(&rt, "Polish the details that matter");
    let today = rt.state.today();
    assert_eq!(polish.day, Some(today + Days::new(1)));
    // Before the first card, so the card stays in view for the next drag.
    let [x, _, w, _] = rect(&rt, &plan_key(&rt, "col", 4));
    let first = rect(&rt, &format!("{}-top", plan_key(&rt, "add", 4)));
    let from = center(&rt, &format!("plan-title-{}", polish.id));
    drag(&mut rt, from, (x + w / 2.0, first[1] + 60.0));
    assert_eq!(task(&rt, "Polish the details that matter").day, Some(today + Days::new(2)));

    click(&mut rt, "plan-backlog");
    settle(&mut rt);
    let [x, y, w, h] = rect(&rt, &plan_key(&rt, "col", 5));
    let from = center(&rt, &format!("plan-title-{}", polish.id));
    drag(&mut rt, from, (x + w / 2.0, y + h - 40.0));
    assert_eq!(task(&rt, "Polish the details that matter").day, None, "dropped into Someday");
}

#[test]
fn a_task_dropped_on_the_day_grid_is_planned_on_the_quarter_hour() {
    let (mut rt, _dir) = app();
    let easy = task(&rt, "Easy 5 km. Leave the headphones at home.");
    // On the title's line, in the card's middle: the lifted card's middle is what hits the grid.
    let (x, _) = center(&rt, &format!("home-task-{}", easy.id));
    let (_, y) = center(&rt, &format!("home-title-{}", easy.id));
    let from = (x, y);
    let to = home_grid_at(&rt, 10 * 60 + 4);
    drag_to(&mut rt, from, to);
    let ghost = rt.presented("home-grid-ghost-time").expect("a ghost block with its times");
    assert!(ghost.1 > 0.0);
    rt.pointer_up(to.0, to.1);
    rt.frame(0.0);
    let block = task(&rt, "Easy 5 km. Leave the headphones at home.").block.expect("planned");
    assert_eq!((block.start, block.minutes), (hm(10, 0), 45), "its own length, snapped to 10:00");
}

#[test]
fn the_calendar_list_drags_onto_the_week() {
    let (mut rt, _dir) = app();
    command(&mut rt, '3');
    settle(&mut rt);
    let pricing = task(&rt, "Sketch a pricing page");
    let from = center(&rt, &format!("calendar-pick-{}", pricing.id));
    let [x, y, w, _] = rect(&rt, "calendar-grid-day-1");
    drag(&mut rt, from, (x + w / 2.0, y + CALENDAR_SCALE.y(9 * 60)));
    let moved = task(&rt, "Sketch a pricing page");
    assert_eq!(moved.day, Some(rt.state.calendar_week + Days::new(1)));
    assert_eq!(moved.block.map(|b| (b.start, b.minutes)), Some((hm(9, 0), 30)), "a default half hour");
}

#[test]
fn planned_blocks_move_across_days_and_stretch_at_the_bottom() {
    let (mut rt, _dir) = app();
    command(&mut rt, '3');
    settle(&mut rt);
    let draft = task(&rt, "Draft this week’s essay").id;
    let today = rt.state.today();
    let index = today.weekday().num_days_from_monday();
    let from = center(&rt, &format!("calendar-grid-block-{draft}"));
    let [next_x, _, w, _] = rect(&rt, &format!("calendar-grid-day-{}", index + 1));
    drag(&mut rt, from, (next_x + w / 2.0, from.1 - CALENDAR_SCALE.y(7 * 60 + 60)));
    let moved = task(&rt, "Draft this week’s essay");
    assert_eq!(moved.day, Some(today + Days::new(1)));
    assert_eq!(moved.block.map(|b| (b.start, b.minutes)), Some((hm(15, 0), 60)));

    let end = center(&rt, &format!("calendar-grid-block-end-{draft}"));
    drag(&mut rt, end, (end.0, end.1 + CALENDAR_SCALE.y(7 * 60 + 30)));
    let block = task(&rt, "Draft this week’s essay").block.expect("still planned");
    assert_eq!((block.start, block.minutes), (hm(15, 0), 90));
}

#[test]
fn tracked_sessions_move_and_change_either_edge() {
    let (mut rt, _dir) = app();
    let map = entry_of(&rt, "Map the ideas worth pursuing");
    assert_eq!(entry_times(&rt, map), (hm(9, 0), Some(hm(10, 30))));
    let body = center(&rt, &format!("home-grid-entry-{map}"));
    drag(&mut rt, body, (body.0, body.1 - HOME_SCALE.y(7 * 60 + 60)));
    assert_eq!(entry_times(&rt, map), (hm(8, 0), Some(hm(9, 30))));

    let start = center(&rt, &format!("home-grid-entry-start-{map}"));
    drag(&mut rt, start, (start.0, start.1 + HOME_SCALE.y(7 * 60 + 30)));
    assert_eq!(entry_times(&rt, map), (hm(8, 30), Some(hm(9, 30))));

    let end = center(&rt, &format!("home-grid-entry-end-{map}"));
    drag(&mut rt, end, (end.0, end.1 + HOME_SCALE.y(7 * 60 + 15)));
    assert_eq!(entry_times(&rt, map), (hm(8, 30), Some(hm(9, 45))));
}

#[test]
fn the_running_session_only_moves_its_start_and_keeps_running() {
    let (mut rt, _dir) = app();
    let running = rt.state.snap.running_entry().map(|e| e.id).expect("the demo runs a session");
    assert!(rt.find(&format!("home-grid-entry-end-{running}")).is_none(), "no end handle while it runs");
    let start = center(&rt, &format!("home-grid-entry-start-{running}"));
    let to = home_grid_at(&rt, 13 * 60 + 30);
    drag(&mut rt, start, (start.0, to.1));
    assert_eq!(entry_times(&rt, running), (hm(13, 30), None));
}

#[test]
fn dragging_empty_space_opens_log_work_with_its_times_and_saves_nothing_yet() {
    let (mut rt, _dir) = app();
    let before = rt.state.snap.time_entries.len();
    let from = home_grid_at(&rt, 13 * 60 + 15);
    let to = home_grid_at(&rt, 13 * 60 + 45);
    drag(&mut rt, from, (from.0 - 60.0, to.1));
    assert_eq!(rt.state.snap.time_entries.len(), before, "nothing is logged by the drag");
    assert_eq!(rt.state.overlay, Overlay::LogWork);
    let form = &rt.state.log_work;
    assert_eq!((form.start.text.as_str(), form.end.text.as_str()), ("13:15", "13:45"));
    assert_eq!(form.date.text, crate::log_work::date_label(rt.state.today()));
    assert!(rt.find("log-work-task-value").is_some(), "the dialog is drawn");
}

#[test]
fn escape_cancels_a_drag_and_nothing_changes() {
    let (mut rt, _dir) = app();
    let order = open_titles(&rt);
    let easy = task(&rt, "Easy 5 km. Leave the headphones at home.").id;
    let from = center(&rt, &format!("home-title-{easy}"));
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
    let from = center(&rt, &format!("home-title-{}", easy.id));
    drag(&mut rt, from, (from.0, 20.0));
    assert_eq!(open_titles(&rt), order);
    assert_eq!(task(&rt, "Easy 5 km. Leave the headphones at home."), easy, "unchanged");
}

#[test]
fn a_focused_session_is_deleted_with_backspace() {
    let (mut rt, _dir) = app();
    let map = entry_of(&rt, "Map the ideas worth pursuing");
    rt.update(|_, tx| tx.focus_when_shown(format!("home-grid-entry-{map}"), true));
    rt.frame(0.0);
    press(&mut rt, Key::Backspace, Mods::default());
    settle(&mut rt);
    assert!(rt.state.snap.time_entries.iter().all(|e| e.id != map));
}

#[test]
fn option_arrows_move_a_focused_card() {
    let (mut rt, _dir) = app();
    let make = task(&rt, "Make the first impression feel effortless").id;
    rt.update(|_, tx| tx.focus_when_shown(format!("home-task-{make}"), true));
    rt.frame(0.0);
    let alt = Mods { alt: true, ..Mods::default() };
    press(&mut rt, Key::Up, alt);
    assert_eq!(open_titles(&rt)[0], "Make the first impression feel effortless");
    press(&mut rt, Key::Right, alt);
    let today = rt.state.today();
    assert_eq!(task(&rt, "Make the first impression feel effortless").day, Some(today + Days::new(1)));
}

#[test]
fn option_arrows_shift_a_focused_session() {
    let (mut rt, _dir) = app();
    let map = entry_of(&rt, "Map the ideas worth pursuing");
    rt.update(|_, tx| tx.focus_when_shown(format!("home-grid-entry-{map}"), true));
    rt.frame(0.0);
    press(&mut rt, Key::Down, Mods { alt: true, ..Mods::default() });
    assert_eq!(entry_times(&rt, map), (hm(9, 15), Some(hm(10, 45))));
    let start = entry_times(&rt, map).0;
    assert_eq!(start.minute() % 15, 0);
}
