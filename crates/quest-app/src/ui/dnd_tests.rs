use chrono::Days;
use quest_domain::{Task, TaskList};
use rune::{Key, Mods, Runtime};

use super::tests::{click, command, demo_app, press, type_text};
use crate::dnd::{DragState, ListId};
use crate::state::State;

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

pub(super) fn scroll_into_view(rt: &mut Runtime<State>, key: &str) {
    let key = key.to_owned();
    rt.update(move |_, tx| tx.scroll_into_view(key));
    settle(rt);
}

#[test]
fn lifting_outside_a_drop_zone_animates_the_source_neighbour() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '2');
    settle(&mut rt);
    let rows = quest_view::lists::today(&rt.state.snap, rt.state.today());
    let neighbour = format!("task-item-{}", rows[1].id);
    let from = center(&rt, &format!("row-title-{}", rows[0].id));
    let start = rect(&rt, &neighbour)[1];
    rt.pointer_down(from.0, from.1);
    rt.frame(0.0);
    rt.pointer_move(from.0, 10.0);
    rt.frame(0.0);
    assert!(rt.state.drag.dragging().is_some() && rt.state.drag.target().is_none());
    let first_frame = rect(&rt, &neighbour)[1];
    settle(&mut rt);
    let end = rect(&rt, &neighbour)[1];
    assert!(end < start - 20.0, "the source gap closes");
    assert!(first_frame > end + 10.0, "the neighbour must not jump to {end}");
}

#[test]
fn the_lift_and_cancel_share_the_existing_faces_geometry() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '2');
    settle(&mut rt);
    let id = task(&rt, "Design the next chapter").id;
    let face = format!("face-{id}");
    let from = center(&rt, &format!("row-title-{id}"));
    let start = rect(&rt, &face)[1];
    rt.pointer_down(from.0, from.1);
    rt.frame(0.0);
    rt.pointer_move(from.0, from.1 + 65.0);
    rt.frame(0.0);
    let lift_start = rect(&rt, "drag-lift-face")[1];
    assert!((lift_start - start).abs() < 2.0, "the lift begins at the source face, got {lift_start} vs {start}");
    settle(&mut rt);
    let lifted = rect(&rt, "drag-lift-face")[1];
    assert!((lifted - start - 65.0).abs() < 2.0);
    press(&mut rt, Key::Escape, Mods::default());
    assert!((rect(&rt, &face)[1] - lifted).abs() < 2.0, "the return begins at the lifted face");
    settle(&mut rt);
    assert!((rect(&rt, &face)[1] - start).abs() < 2.0);
}

#[test]
fn today_reorders_without_opening_a_detail_and_can_undo() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '2');
    settle(&mut rt);
    let before: Vec<_> = quest_view::lists::today(&rt.state.snap, rt.state.today()).iter().map(|t| t.id).collect();
    let from = center(&rt, &format!("row-title-{}", before[1]));
    let to = center(&rt, &format!("row-title-{}", before[0]));
    drag_to(&mut rt, from, to);
    settle(&mut rt);
    assert!(rt.find("drag-lift").is_some());
    assert_eq!(rt.state.drag.target().unwrap().before, Some(before[0]));
    rt.pointer_up(to.0, to.1);
    settle(&mut rt);
    assert_eq!(rt.state.drag, DragState::Idle);
    assert!(rt.state.task_modal.is_none());
    assert_eq!(quest_view::lists::today(&rt.state.snap, rt.state.today())[0].id, before[1]);
    assert!(rt.state.undo.shown.is_none());
    command(&mut rt, 'z');
    assert_eq!(quest_view::lists::today(&rt.state.snap, rt.state.today())[0].id, before[0]);
}

#[test]
fn plan_drop_opens_a_ghost_in_an_empty_day_and_escape_restores_it() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '3');
    settle(&mut rt);
    let id = task(&rt, "Design the next chapter").id;
    let day = rt.state.today() + Days::new(4);
    let from = center(&rt, &format!("row-title-{id}"));
    let to = center(&rt, &format!("plan-head-{day}"));
    drag_to(&mut rt, from, to);
    settle(&mut rt);
    assert_eq!(rt.state.drag.target().unwrap().list, ListId::Rows(TaskList::Day(day)));
    assert!(rt.find("drag-room").is_some());
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.drag, DragState::Idle);
    settle(&mut rt);
    assert!(rt.find("drag-room").is_none());
    assert_ne!(rt.state.snap.task(id).unwrap().planned_on, Some(day));

    let from = center(&rt, &format!("row-title-{id}"));
    drag(&mut rt, from, to);
    assert_eq!(rt.state.snap.task(id).unwrap().planned_on, Some(day));
    assert_eq!(rt.state.undo.shown.as_ref().unwrap().toast, quest_view::undo::Toast::DateChanged);
}

#[test]
fn holding_at_the_plan_edge_scrolls_and_retargets_without_mouse_moves() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '3');
    settle(&mut rt);
    let id = task(&rt, "Design the next chapter").id;
    let from = center(&rt, &format!("row-title-{id}"));
    let [_, top, _, height] = rect(&rt, "plan-page-scroll");
    let bottom = top + height;
    let today = format!("plan-head-{}", rt.state.today());
    let start = rect(&rt, &today)[1];
    let to = (from.0, bottom - 6.0);
    rt.pointer_down(from.0, from.1);
    rt.frame(0.0);
    rt.pointer_move(to.0, to.1);
    rt.frame(0.0);
    let initial_target = rt.state.drag.target();
    let day = (0..120)
        .find_map(|_| {
            rt.frame(1.0 / 60.0);
            let target = rt.state.drag.target()?;
            let ListId::Rows(TaskList::Day(day)) = target.list else { return None };
            (Some(target) != initial_target && rect(&rt, &today)[1] < start - 80.0).then_some(day)
        })
        .expect("scrolling brings a new day under the resting pointer");
    rt.pointer_up(to.0, to.1);
    settle(&mut rt);
    assert_eq!(rt.state.snap.task(id).unwrap().planned_on, Some(day));
}

#[test]
fn sidebar_drop_highlights_quest_and_files_the_task_without_opening_it() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '1');
    settle(&mut rt);
    let id = quest_view::lists::inbox(&rt.state.snap)[0].id;
    let quest = rt.state.snap.active_quests().next().unwrap().id;
    let from = center(&rt, &format!("row-title-{id}"));
    let to = center(&rt, &format!("nav-quest-{quest}"));
    drag_to(&mut rt, from, to);
    assert_eq!(rt.state.drag.target().unwrap().list, ListId::File(TaskList::Quest(quest)));
    rt.pointer_up(to.0, to.1);
    settle(&mut rt);
    assert_eq!(rt.state.snap.task(id).unwrap().quest_id, Some(quest));
    assert_eq!(rt.state.undo.shown.as_ref().unwrap().toast, quest_view::undo::Toast::QuestChanged);
    assert!(rt.state.task_modal.is_none());
}

#[test]
fn keyboard_sort_and_day_quick_add() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '3');
    press(&mut rt, Key::Down, Mods::default());
    let id = rt.state.selected_task.unwrap();
    press(&mut rt, Key::Down, Mods { alt: true, ..Mods::default() });
    assert_eq!(quest_view::lists::today(&rt.state.snap, rt.state.today())[1].id, id);
    let date = rt.state.today() + Days::new(1);
    let (x, y) = center(&rt, &format!("plan-head-{date}"));
    rt.pointer_move(x, y);
    settle(&mut rt);
    click(&mut rt, &format!("plan-add-{date}"));
    assert_eq!(super::quick_add::defaults(&rt.state).dates.planned_on, Some(date));
    type_text(&mut rt, "From the day header");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(task(&rt, "From the day header").planned_on, Some(date));
}

#[test]
fn unplanned_quest_tasks_can_be_dragged_from_the_tray_into_a_day() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '3');
    scroll_into_view(&mut rt, "plan-not-planned");
    click(&mut rt, "plan-not-planned");
    let task = task(&rt, "Sketch a pricing page");
    assert!(task.planned_on.is_none());
    let source = format!("row-title-{}", task.id);
    scroll_into_view(&mut rt, &source);
    let day = rt.state.today() + Days::new(12);
    let from = center(&rt, &source);
    let to = center(&rt, &format!("plan-head-{day}"));
    drag(&mut rt, from, to);
    let moved = rt.state.snap.task(task.id).unwrap();
    assert_eq!((moved.planned_on, moved.due_on), (Some(day), Some(day)));
}

#[test]
fn a_task_also_in_today_expands_and_lifts_at_its_tray_row() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '3');
    let id = task(&rt, "Sketch a pricing page").id;
    rt.update(move |s, _| {
        let today = s.today();
        s.dispatch(quest_domain::Command::SetTaskDue { id, due_on: Some(today), today, at: s.now });
    });
    scroll_into_view(&mut rt, "plan-not-planned");
    click(&mut rt, "plan-not-planned");
    let title = format!("not-planned-title-{id}");
    scroll_into_view(&mut rt, &title);
    let before = rect(&rt, &title);
    click(&mut rt, &title);
    settle(&mut rt);
    assert_eq!(&rect(&rt, "task-modal-title")[..2], &before[..2]);
    assert!(rt.find(&title).is_none());
    press(&mut rt, Key::Escape, Mods::default());
    settle(&mut rt);

    let from = center(&rt, &title);
    let start = rect(&rt, &format!("tray-face-{id}"))[1];
    rt.pointer_down(from.0, from.1);
    rt.frame(0.0);
    rt.pointer_move(from.0, from.1 - 60.0);
    rt.frame(0.0);
    assert!((rect(&rt, "drag-lift-face")[1] - start).abs() < 2.0);
    press(&mut rt, Key::Escape, Mods::default());
    settle(&mut rt);
    assert!(rt.find(&title).is_some());
}

#[test]
fn plan_navigation_crosses_days_and_g_returns_to_today() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '2');
    let num_today = super::lists::shown_tasks(&rt.state).len();
    command(&mut rt, '3');
    settle(&mut rt);
    let today_key = format!("plan-head-{}", rt.state.today());
    let y = rect(&rt, &today_key)[1];
    for _ in 0..=num_today {
        type_text(&mut rt, "j");
    }
    assert_eq!(rt.state.selected_task, Some(task(&rt, "Polish the details that matter").id));
    scroll_into_view(&mut rt, "plan-not-planned");
    assert!(rect(&rt, &today_key)[1] < y);
    type_text(&mut rt, "g");
    settle(&mut rt);
    assert_eq!(rect(&rt, &today_key)[1], y);
    assert_eq!(rt.state.selected_task, None);
}
