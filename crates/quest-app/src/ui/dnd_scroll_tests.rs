//! Drops after the view moved under its drop zones: the planner shifted a day earlier or later,
//! the calendar on to another week, a list or grid scrolled.

use chrono::{Days, NaiveTime};
use quest_domain::Task;
use rune::Runtime;

use super::plan_scroll_tests::hold;
use super::tests::{TempDir, click, command, demo_app, plan_key};
use crate::state::State;
use crate::ui::timegrid::CALENDAR_SCALE;

fn app() -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = demo_app();
    settle(&mut rt);
    (rt, dir)
}

fn settle(rt: &mut Runtime<State>) {
    rt.frame(0.0);
    rt.settle_all();
    rt.frame(0.0);
}

fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.presented(key).unwrap_or_else(|| panic!("`{key}` is drawn")).0
}

fn center(rt: &Runtime<State>, key: &str) -> (f32, f32) {
    rt.find(key).unwrap_or_else(|| panic!("`{key}` is on screen"))
}

fn drag(rt: &mut Runtime<State>, from: (f32, f32), to: (f32, f32)) {
    rt.pointer_down(from.0, from.1);
    rt.frame(0.0);
    for i in 1..=4 {
        let t = i as f32 / 4.0;
        rt.pointer_move(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        rt.frame(0.0);
    }
    rt.pointer_up(to.0, to.1);
    settle(rt);
}

fn task(rt: &Runtime<State>, title: &str) -> Task {
    rt.state.snap.tasks.iter().find(|t| t.title == title).cloned().unwrap_or_else(|| panic!("task {title}"))
}

/// Drags `title` by its body into plan column `index`, below its add field.
fn drag_to_column(rt: &mut Runtime<State>, title: &str, index: usize) {
    let id = task(rt, title).id;
    let [x, _, w, _] = rect(rt, &plan_key(rt, "col", index));
    let add = rect(rt, &format!("{}-top", plan_key(rt, "add", index)));
    let from = center(rt, &format!("plan-title-{id}"));
    drag(rt, from, (x + w / 2.0, add[1] + 60.0));
}

#[test]
fn plan_drops_land_on_the_day_shown_after_moving_later() {
    let (mut rt, _dir) = app();
    command(&mut rt, '2');
    settle(&mut rt);
    click(&mut rt, "plan-next");
    hold(&mut rt, 1.0);
    let today = rt.state.today();
    assert_eq!(rt.state.plan.from(), today - Days::new(1));
    drag_to_column(&mut rt, "Polish the details that matter", 3);
    assert_eq!(task(&rt, "Polish the details that matter").day, Some(today + Days::new(2)), "column 3 shows today + 2");
    drag_to_column(&mut rt, "Polish the details that matter", 4);
    assert_eq!(task(&rt, "Polish the details that matter").day, Some(today + Days::new(3)), "the day that scrolled in");
}

#[test]
fn plan_drops_land_on_the_day_shown_after_moving_earlier() {
    let (mut rt, _dir) = app();
    command(&mut rt, '2');
    settle(&mut rt);
    click(&mut rt, "plan-prev");
    hold(&mut rt, 1.0);
    let today = rt.state.today();
    assert_eq!(rt.state.plan.from(), today - Days::new(3));
    drag_to_column(&mut rt, "Polish the details that matter", 3);
    assert_eq!(task(&rt, "Polish the details that matter").day, Some(today), "column 3 shows today");
    drag_to_column(&mut rt, "Polish the details that matter", 4);
    assert_eq!(task(&rt, "Polish the details that matter").day, Some(today + Days::new(1)));
}

#[test]
fn calendar_drops_land_on_the_week_shown() {
    let (mut rt, _dir) = app();
    command(&mut rt, '3');
    settle(&mut rt);
    click(&mut rt, "calendar-next");
    settle(&mut rt);
    let pricing = task(&rt, "Sketch a pricing page");
    let from = center(&rt, &format!("calendar-pick-{}", pricing.id));
    let [x, y, w, _] = rect(&rt, "calendar-grid-day-2");
    drag(&mut rt, from, (x + w / 2.0, y + CALENDAR_SCALE.y(9 * 60)));
    let week = rt.state.calendar_week;
    assert_eq!(task(&rt, "Sketch a pricing page").day, Some(week + Days::new(2)));
}

#[test]
fn a_scrolled_calendar_grid_takes_drops_at_the_time_under_the_pointer() {
    let (mut rt, _dir) = app();
    command(&mut rt, '3');
    settle(&mut rt);
    let [x, y, w, _] = rect(&rt, "calendar-grid-day-1");
    let before = y;
    rt.wheel(x + w / 2.0, y + 200.0, -240.0);
    settle(&mut rt);
    let [x, y, w, _] = rect(&rt, "calendar-grid-day-1");
    assert!(y < before - 100.0, "the grid scrolled: {before} → {y}");
    let pricing = task(&rt, "Sketch a pricing page");
    let from = center(&rt, &format!("calendar-pick-{}", pricing.id));
    drag(&mut rt, from, (x + w / 2.0, y + CALENDAR_SCALE.y(14 * 60)));
    let block = task(&rt, "Sketch a pricing page").block.expect("planned");
    assert_eq!(block.start, NaiveTime::from_hms_opt(14, 0, 0).unwrap());
}

#[test]
fn a_scrolled_plan_column_drops_before_the_card_under_the_pointer() {
    let (mut rt, _dir) = app();
    command(&mut rt, '2');
    settle(&mut rt);
    let today = rt.state.today();
    let titles: Vec<String> = rt.state.snap.tasks_on(today).map(|t| t.title.clone()).collect();
    let [x, y, w, h] = rect(&rt, &plan_key(&rt, "col", 2));
    let quest_work = |t: &&quest_domain::Task| !t.is_done() && t.quest_id.is_some();
    let last = rt.state.snap.tasks_on(today).filter(quest_work).last().expect("open quest work").id;
    let [_, rest, _, _] = rect(&rt, &format!("plan-task-{last}"));
    rt.wheel(x + w / 2.0, y + h / 2.0, -200.0);
    settle(&mut rt);
    let [_, top, _, _] = rect(&rt, &format!("plan-task-{last}"));
    assert!(top < rest - 50.0, "the column scrolled: {rest} → {top}");
    let moving = task(&rt, "Polish the details that matter").id;
    let from = center(&rt, &format!("plan-title-{moving}"));
    let own_top = rect(&rt, &format!("plan-task-{moving}"))[1];
    // The lifted card's top over the card's: its room opens there.
    drag(&mut rt, from, (x + w / 2.0, from.1 + top - own_top));
    let order: Vec<_> = rt.state.snap.tasks_on(today).filter(|t| t.quest_id.is_some()).map(|t| t.id).collect();
    let at = order.iter().position(|id| *id == moving).expect("moved to today");
    assert_eq!(order.get(at + 1), Some(&last), "dropped right before the card it was over ({titles:?})");
}
