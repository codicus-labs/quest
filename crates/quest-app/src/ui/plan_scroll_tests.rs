//! The planner's endless strip in the real view: where it opens, trackpad scrolling weeks either
//! way without a jump, Earlier / Later / Today, and drags resting at the board's edge.

use chrono::{Days, NaiveDate};
use rune::{Key, Mods, Runtime};

use super::dnd_tests::{center, rect, settle, task};
use super::tests::{TempDir, click, command, demo_app, press};
use crate::dnd::DragState;
use crate::state::State;

const MOVING: &str = "Polish the details that matter";

fn plan() -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = demo_app();
    settle(&mut rt);
    command(&mut rt, '2');
    settle(&mut rt);
    (rt, dir)
}

fn column_x(rt: &Runtime<State>, day: NaiveDate) -> f32 {
    rect(rt, &format!("plan-col-{day}"))[0]
}

/// The built day columns with where each is drawn.
fn drawn_columns(rt: &Runtime<State>) -> Vec<(NaiveDate, f32)> {
    rt.elements()
        .into_iter()
        .filter_map(|e| {
            let day = e.key.as_deref()?.strip_prefix("plan-col-")?.parse().ok()?;
            Some((day, rt.presented(&format!("plan-col-{day}"))?.0[0]))
        })
        .collect()
}

/// How far the strip is scrolled as drawn: every built column must agree on it, or a column
/// jumped out of line.
pub(super) fn drawn_offset(rt: &Runtime<State>) -> f32 {
    let board = rect(rt, "plan-board")[0];
    let strip = rt.state.plan.strip;
    let offsets: Vec<f32> = drawn_columns(rt).into_iter().map(|(day, x)| board + strip.x_of(day) - x).collect();
    let first = *offsets.first().expect("columns are built");
    for o in &offsets {
        assert!((o - first).abs() < 0.5, "every column sits where the strip has it: {offsets:?}");
    }
    first
}

fn month(rt: &Runtime<State>) -> String {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some("plan-month")).and_then(|e| e.text).unwrap()
}

#[test]
fn the_plan_opens_on_the_day_before_yesterday_in_its_first_frame() {
    let (mut rt, _dir) = demo_app();
    settle(&mut rt);
    command(&mut rt, '2');
    let first = rt.state.today() - Days::new(2);
    let board = rect(&rt, "plan-board");
    assert!((column_x(&rt, first) - board[0]).abs() < 0.5, "it starts the board in the first frame");
    assert_eq!(rt.state.plan.from(), first);
}

/// Swipes `weeks` weeks (negative: earlier) in trackpad steps of `step` points, a frame each,
/// checking each frame that the strip moved by the step and nothing else.
pub(super) fn swipe(rt: &mut Runtime<State>, weeks: i64, step: f32) {
    let [x, y, w, h] = rect(rt, "plan-board");
    let distance = weeks.abs() as f32 * 7.0 * rt.state.plan.strip.stride();
    let dx = if weeks > 0 { -step } else { step };
    rt.wheel_began();
    let mut moved = 0.0;
    while moved < distance {
        let before = drawn_offset(rt);
        rt.wheel_xy(x + w / 2.0, y + h / 2.0, dx, 0.0);
        rt.frame(1.0 / 60.0);
        let after = drawn_offset(rt);
        assert!((after - before + dx).abs() < 0.5, "moved by the swipe, no jump: {before} → {after}");
        assert!((rt.state.plan.offset() - after).abs() < 0.5, "the app reads the offset drawn");
        moved += step;
    }
    rt.wheel_ended();
    settle(rt);
}

#[test]
fn a_trackpad_swipe_scrolls_weeks_either_way_without_a_jump() {
    let (mut rt, _dir) = plan();
    let start = rt.state.plan.from();
    let start_x = drawn_offset(&rt);
    let stride = rt.state.plan.strip.stride();
    swipe(&mut rt, 3, 40.0);
    let later = drawn_offset(&rt);
    assert!((later - start_x - 21.0 * stride).abs() < 40.0, "three weeks later: {start_x} → {later}");
    assert_eq!(rt.state.plan.from(), start + Days::new(21));
    assert_eq!(month(&rt), "November 2026", "the month follows what shows");
    swipe(&mut rt, -6, 40.0);
    assert_eq!(rt.state.plan.from(), start - Days::new(21));
    assert_eq!(month(&rt), "September 2026");
    let built = drawn_columns(&rt).len();
    assert!(built < 30, "only days around what shows are built: {built}");
}

#[test]
fn a_swipe_re_runs_the_view_only_for_new_weeks_and_months() {
    let (mut rt, _dir) = plan();
    // Debug builds re-run the view a quarter second after a live-only event to check it.
    rt.check_live_only(false);
    let commits = rt.stats.commits;
    // Six weeks cross 42 days, six or seven windows and a month.
    swipe(&mut rt, 6, 40.0);
    let committed = rt.stats.commits - commits;
    assert!(committed <= 9, "the view re-ran {committed} times over six weeks");
}

#[test]
fn earlier_later_and_today_glide() {
    let (mut rt, _dir) = plan();
    let start = rt.state.plan.from();
    let before = drawn_offset(&rt);
    click(&mut rt, "plan-next");
    rt.frame(1.0 / 60.0);
    let mid = drawn_offset(&rt);
    let stride = rt.state.plan.strip.stride();
    assert!(mid > before + 1.0 && mid < before + stride - 1.0, "it glides, it doesn't jump: {before} → {mid}");
    hold(&mut rt, 1.0);
    assert_eq!(rt.state.plan.from(), start + Days::new(1));
    for _ in 0..3 {
        click(&mut rt, "plan-prev");
        hold(&mut rt, 1.0);
    }
    assert_eq!(rt.state.plan.from(), start - Days::new(2));
    swipe(&mut rt, 5, 80.0);
    click(&mut rt, "plan-today");
    hold(&mut rt, 1.0);
    assert_eq!(rt.state.plan.from(), start, "Today brings the day before yesterday back to the left");
}

/// Presses `title`'s card and drags it to `to` in a few moves.
fn lift(rt: &mut Runtime<State>, title: &str, to: (f32, f32)) {
    let from = center(rt, &format!("plan-title-{}", task(rt, title).id));
    rt.pointer_down(from.0, from.1);
    rt.frame(0.0);
    for i in 1..=4 {
        let t = i as f32 / 4.0;
        rt.pointer_move(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        rt.frame(0.0);
    }
}

/// Runs `seconds` of frames without moving the pointer (glides run on real frames).
pub(super) fn hold(rt: &mut Runtime<State>, seconds: f64) {
    for _ in 0..(seconds * 60.0) as usize {
        rt.frame(1.0 / 60.0);
    }
}

fn dragging(rt: &Runtime<State>) -> bool {
    matches!(rt.state.drag, DragState::Dragging { .. })
}

#[test]
fn a_drag_resting_at_the_right_edge_scrolls_on_until_it_leaves() {
    let (mut rt, _dir) = plan();
    let start = rt.state.plan.from();
    let [x, y, w, h] = rect(&rt, "plan-board");
    lift(&mut rt, MOVING, (x + w - 8.0, y + h / 2.0));
    hold(&mut rt, 1.0);
    let scrolled = rt.state.plan.from();
    assert!(scrolled > start + Days::new(3), "a second at the edge scrolls days on: {start} → {scrolled}");
    assert!(dragging(&rt), "the card stays lifted");
    rt.pointer_move(x + w / 2.0, y + h / 2.0);
    rt.frame(1.0 / 60.0);
    let offset = rt.state.plan.offset();
    hold(&mut rt, 0.5);
    assert_eq!(rt.state.plan.offset(), offset, "out of the edge zone it stops at once");
    assert!(dragging(&rt));
}

#[test]
fn a_drag_resting_at_the_left_edge_scrolls_earlier_and_esc_stops_it() {
    let (mut rt, _dir) = plan();
    let start = rt.state.plan.from();
    let [x, y, _, h] = rect(&rt, "plan-board");
    lift(&mut rt, MOVING, (x + 6.0, y + h / 2.0));
    hold(&mut rt, 0.5);
    assert!(rt.state.plan.from() < start, "earlier days come in");
    press(&mut rt, Key::Escape, Mods::default());
    let offset = rt.state.plan.offset();
    hold(&mut rt, 0.5);
    assert!(!dragging(&rt), "Esc ends the drag");
    assert_eq!(rt.state.plan.offset(), offset, "and the scroll with it");
    assert_eq!(task(&rt, MOVING).day, Some(rt.state.today() + Days::new(1)), "the card went back");
}

#[test]
fn a_drop_after_an_edge_scroll_lands_on_the_day_under_the_pointer() {
    let (mut rt, _dir) = plan();
    let today = rt.state.today();
    let target = today + Days::new(9);
    assert!(rt.presented(&format!("plan-col-{target}")).is_none_or(|p| p.0[0] > 1100.0), "not on screen at first");
    let [x, y, w, h] = rect(&rt, "plan-board");
    lift(&mut rt, MOVING, (x + w - 8.0, y + h / 2.0));
    // Scroll on until the target day is on screen, then rest over it.
    for _ in 0..240 {
        rt.frame(1.0 / 60.0);
        if rt.state.plan.from() >= target - Days::new(2) {
            break;
        }
    }
    rt.pointer_move(x + w / 2.0, y + h / 2.0);
    rt.frame(1.0 / 60.0);
    let under = rt.state.plan.strip.day_at(rt.state.plan.offset(), w / 2.0);
    assert!(under >= target - Days::new(1), "a day that wasn't on screen is under the pointer: {under}");
    rt.pointer_up(x + w / 2.0, y + h / 2.0);
    settle(&mut rt);
    assert_eq!(task(&rt, MOVING).day, Some(under), "it lands on the day under the pointer");
    assert!((column_x(&rt, under) - (x + rt.state.plan.strip.x_of(under) - rt.state.plan.offset())).abs() < 0.5);
}
