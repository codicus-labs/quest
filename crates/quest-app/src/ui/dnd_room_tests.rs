//! A dragged card makes room: its list opens a gap where it would drop, the cards around it
//! spring aside, and on a drop or a snap back the lifted card flies into place.

use chrono::Days;
use quest_domain::{EntityId, Task};
use rune::{Key, Mods, Runtime};

use super::tests::{TempDir, command, demo_app, plan_key, press};
use crate::dnd::{DragState, ListId, Target};
use crate::state::State;

const FRAME: f64 = 1.0 / 60.0;

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

fn task(rt: &Runtime<State>, title: &str) -> Task {
    rt.state.snap.tasks.iter().find(|t| t.title == title).cloned().unwrap_or_else(|| panic!("task {title}"))
}

fn home_order(rt: &Runtime<State>) -> Vec<EntityId> {
    let today = rt.state.today();
    rt.state.snap.tasks_on(today).filter(|t| !t.is_done()).map(|t| t.id).collect()
}

fn press_and_lift(rt: &mut Runtime<State>, from: (f32, f32)) {
    rt.pointer_down(from.0, from.1);
    rt.frame(0.0);
    rt.pointer_move(from.0, from.1 + 6.0);
    rt.frame(0.0);
}

fn move_to(rt: &mut Runtime<State>, to: (f32, f32)) {
    rt.pointer_move(to.0, to.1);
    rt.frame(0.0);
}

#[test]
fn siblings_spring_aside_and_the_target_holds_while_they_move() {
    let (mut rt, _dir) = app();
    let order = home_order(&rt);
    let (first, second) = (order[0], order[1]);
    let first_rect = rect(&rt, &format!("home-task-{first}"));
    let second_rect = rect(&rt, &format!("home-task-{second}"));
    let from = center(&rt, &format!("home-title-{first}"));
    press_and_lift(&mut rt, from);
    assert_eq!(rect(&rt, &format!("home-task-{second}")), second_rect, "a small drag keeps the room in place");

    // Down by the second card's height: the room moves below it, and it springs up.
    let down = second_rect[1] + second_rect[3] - first_rect[1] - first_rect[3];
    move_to(&mut rt, (from.0, from.1 + down));
    let target = rt.state.drag.target();
    assert_eq!(target, Some(Target::List { list: ListId::Home, before: order.get(2).copied() }));
    let mut ys = Vec::new();
    for _ in 0..6 {
        rt.frame(FRAME);
        ys.push(rect(&rt, &format!("home-task-{second}"))[1]);
        // The pointer holds still while the card springs: the target must too.
        move_to(&mut rt, (from.0, from.1 + down));
        assert_eq!(rt.state.drag.target(), target, "no flicker while siblings move");
    }
    assert!(ys[0] > first_rect[1] && ys[0] < second_rect[1], "on its way up, not there at once: {ys:?}");
    settle(&mut rt);
    assert_eq!(rect(&rt, &format!("home-task-{second}"))[1], first_rect[1], "landed in the first place");

    rt.pointer_up(from.0, from.1 + down);
    settle(&mut rt);
    assert_eq!(home_order(&rt)[..2], [second, first]);
}

#[test]
fn another_list_opens_a_ghost_and_leaving_every_list_closes_it() {
    let (mut rt, _dir) = app();
    command(&mut rt, '2');
    settle(&mut rt);
    let polish = task(&rt, "Polish the details that matter").id;
    let own = rect(&rt, &format!("plan-task-{polish}"));
    let from = center(&rt, &format!("plan-title-{polish}"));
    press_and_lift(&mut rt, from);
    assert!(rt.find("drag-room").is_none(), "its own list moves the card itself into the room");

    let [x, _, w, _] = rect(&rt, &plan_key(&rt, "col", 4));
    let add = rect(&rt, &format!("{}-top", plan_key(&rt, "add", 4)));
    let next = rt.state.today() + Days::new(2);
    let first_next = rt.state.snap.tasks_on(next).next().map(|t| t.id).expect("a card on that day");
    let next_rect = rect(&rt, &format!("plan-task-{first_next}"));
    move_to(&mut rt, (x + w / 2.0, from.1 + next_rect[1] - own[1]));
    settle(&mut rt);
    let ghost = rect(&rt, "drag-room");
    assert_eq!(ghost[3], own[3], "the ghost is as tall as the card");
    assert_eq!(ghost[1], next_rect[1], "it opens where the first card was");
    let pushed = rect(&rt, &format!("plan-task-{first_next}"));
    assert_eq!(pushed[1], next_rect[1] + own[3] + 10.0, "the first card moved down by the room");
    assert!(add[1] < ghost[1]);
    assert!(rt.presented(&format!("plan-task-{polish}")).is_some_and(|(_, opacity)| opacity < 0.5), "left behind");

    move_to(&mut rt, (x + w / 2.0, 10.0));
    settle(&mut rt);
    assert!(rt.find("drag-room").is_none(), "off every list the room closes");
    assert_eq!(rect(&rt, &format!("plan-task-{first_next}"))[1], next_rect[1]);
    assert_eq!(rect(&rt, &format!("plan-task-{polish}")), own, "its own place is back");
    rt.pointer_up(x + w / 2.0, 10.0);
    settle(&mut rt);
    assert_eq!(task(&rt, "Polish the details that matter").day, Some(rt.state.today() + Days::new(1)));
}

#[test]
fn a_drop_where_nothing_changes_runs_no_command() {
    let (mut rt, _dir) = app();
    let order = home_order(&rt);
    let before = task(&rt, "Design the next chapter");
    let from = center(&rt, &format!("home-title-{}", before.id));
    press_and_lift(&mut rt, from);
    move_to(&mut rt, (from.0, from.1 + 20.0));
    assert!(matches!(rt.state.drag, DragState::Dragging { .. }));
    rt.pointer_up(from.0, from.1 + 20.0);
    settle(&mut rt);
    assert_eq!(home_order(&rt), order);
    assert_eq!(task(&rt, "Design the next chapter"), before, "no move, so its sort key is untouched");
}

#[test]
fn escape_flies_the_card_back_to_its_place() {
    let (mut rt, _dir) = app();
    let id = home_order(&rt)[0];
    let face = format!("home-face-{id}");
    let home = rect(&rt, &face);
    let from = center(&rt, &format!("home-title-{id}"));
    press_and_lift(&mut rt, from);
    move_to(&mut rt, (from.0 + 200.0, from.1 + 220.0));
    settle(&mut rt);
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.drag, DragState::Idle);
    rt.frame(FRAME);
    let flying = rect(&rt, &format!("{face}-0"));
    assert!(flying[0] > home[0] + 100.0 && flying[1] > home[1] + 100.0, "starts where the lifted card was: {flying:?}");
    settle(&mut rt);
    let landed = rect(&rt, &format!("{face}-0"));
    assert_eq!((landed[0], landed[1]), (home[0], home[1]), "and springs home");
}

/// The lifted card's rect next to its own card's, mid-drag, after `key`'s body is pressed.
fn lifted_beside(rt: &mut Runtime<State>, card: &str, grip: &str) -> ([f32; 4], [f32; 4]) {
    let own = rect(rt, card);
    let from = center(rt, grip);
    press_and_lift(rt, from);
    move_to(rt, (from.0 + 40.0, from.1 + 300.0));
    rt.settle_all();
    rt.frame(0.0);
    (rect(rt, "drag-lift"), own)
}

#[test]
fn the_lifted_card_keeps_its_cards_size() {
    let (mut rt, _dir) = app();
    let id = home_order(&rt)[1];
    let (lift, own) = lifted_beside(&mut rt, &format!("home-task-{id}"), &format!("home-title-{id}"));
    assert_eq!((lift[2], lift[3]), (own[2], own[3]), "Home");

    let (mut rt, _dir) = app();
    command(&mut rt, '2');
    settle(&mut rt);
    let id = task(&rt, "Polish the details that matter").id;
    let (lift, own) = lifted_beside(&mut rt, &format!("plan-task-{id}"), &format!("plan-title-{id}"));
    assert_eq!((lift[2], lift[3]), (own[2], own[3]), "Plan wraps the title as its column does");
    let [x, _, w, _] = lift;
    let grip = rect(&rt, &format!("lift-menu-{id}"));
    assert!(grip[0] >= x && grip[0] + grip[2] <= x + w, "the menu button sits inside the card: {grip:?} in {lift:?}");
}
