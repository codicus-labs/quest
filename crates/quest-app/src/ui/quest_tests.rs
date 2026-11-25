//! The quest page: open it from the quest's dialog, rename, recolour, complete, add a task; and
//! a planner card's body: a click opens it, its buttons stay buttons.

use quest_domain::{EntityId, QuestColor, QuestStatus};
use rune::{Key, Mods, Runtime};

use super::dnd_tests::rect;
use super::tests::{TempDir, click, command, demo_app, go, press, type_text};
use crate::dnd::DragState;
use crate::palette::Mode;
use crate::state::{Overlay, State, View};
use crate::theme::color_name;

fn app() -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = demo_app();
    rt.frame(0.0);
    rt.settle_all();
    rt.frame(0.0);
    (rt, dir)
}

fn enter(rt: &mut Runtime<State>) {
    press(rt, Key::Enter, Mods::default());
}

fn open_first_quest(rt: &mut Runtime<State>) -> EntityId {
    go(rt, View::Quests);
    let quest = rt.state.snap.active_quests().next().map(|q| q.id).expect("a demo quest");
    click(rt, &format!("quest-{quest}"));
    assert_eq!(rt.state.overlay, Overlay::Quest, "a card opens the quest's dialog");
    click(rt, "quest-editor-page");
    assert_eq!(rt.state.view, View::Quest(quest));
    quest
}

#[test]
fn the_quest_page_renames_recolours_completes_and_reopens() {
    let (mut rt, _dir) = app();
    let id = open_first_quest(&mut rt);
    let title = rt.state.snap.quest(id).unwrap().title.clone();
    click(&mut rt, "quest-title");
    type_text(&mut rt, " II");
    enter(&mut rt);
    assert_eq!(rt.state.snap.quest(id).unwrap().title, format!("{title} II"));

    let other = QuestColor::ALL.into_iter().find(|c| *c != rt.state.snap.quest(id).unwrap().color).unwrap();
    click(&mut rt, &format!("quest-color-{}", color_name(other).to_lowercase()));
    assert_eq!(rt.state.snap.quest(id).unwrap().color, other);

    click(&mut rt, "quest-status");
    assert_eq!(rt.state.snap.quest(id).unwrap().status, QuestStatus::Completed);
    click(&mut rt, "quest-status");
    assert_eq!(rt.state.snap.quest(id).unwrap().status, QuestStatus::Active);
}

#[test]
fn escape_cancels_a_rename() {
    let (mut rt, _dir) = app();
    let id = open_first_quest(&mut rt);
    let title = rt.state.snap.quest(id).unwrap().title.clone();
    click(&mut rt, "quest-title");
    type_text(&mut rt, " nope");
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.snap.quest(id).unwrap().title, title);
    assert_eq!(rt.state.view, View::Quest(id), "Esc closed the field, not the page");
}

#[test]
fn the_quest_page_adds_tasks() {
    let (mut rt, _dir) = app();
    let id = open_first_quest(&mut rt);
    type_text(&mut rt, "n");
    type_text(&mut rt, "Outline chapter four");
    enter(&mut rt);
    let task = rt.state.snap.tasks.iter().find(|t| t.title == "Outline chapter four").expect("created");
    assert_eq!((task.quest_id, task.planned_on), (Some(id), None), "N adds to this quest, unplanned");
    assert!(rt.find(&format!("row-{}", task.id)).is_some(), "listed on the page");
    assert_eq!(rt.state.view, View::Quest(id), "N stays on the page");
}

#[test]
fn a_click_on_a_card_opens_its_dialog_and_its_buttons_do_not_drag() {
    let (mut rt, _dir) = app();
    command(&mut rt, '3');
    let today = rt.state.today();
    let order = |rt: &Runtime<State>| rt.state.snap.tasks_on(today).map(|t| t.id).collect::<Vec<_>>();
    let before = order(&rt);
    let id = rt.state.snap.tasks_on(today).find(|t| !t.is_done()).map(|t| t.id).unwrap();
    click(&mut rt, &format!("plan-title-{id}"));
    assert_eq!(rt.state.task_modal.as_ref().map(|m| m.task), Some(id), "a click without travel opens the dialog");
    assert_eq!(order(&rt), before, "and moves nothing");
    press(&mut rt, Key::Escape, Mods::default());
    assert!(rt.state.task_modal.is_none() && rt.find("task-modal").is_none(), "Esc closes it");

    click(&mut rt, &format!("plan-chip-{id}"));
    assert_eq!(rt.state.palette.mode, Mode::AssignQuest(id), "the chip picks a quest");
    assert_eq!(rt.state.drag, DragState::Idle);
    press(&mut rt, Key::Escape, Mods::default());

    // A press on the check that wanders a little stays a press on the check.
    let (x, y) = rt.find(&format!("plan-check-{id}")).unwrap();
    rt.pointer_down(x, y);
    rt.frame(0.0);
    rt.pointer_move(x + 2.0, y + 2.0);
    rt.frame(0.0);
    assert_eq!(rt.state.drag, DragState::Idle, "the check doesn't start a drag");
    rt.pointer_up(x + 2.0, y + 2.0);
    rt.frame(0.0);
    assert!(rt.state.snap.task(id).unwrap().is_done());
}

#[test]
fn a_press_on_a_card_drags_only_past_the_threshold() {
    let (mut rt, _dir) = app();
    command(&mut rt, '3');
    let id = rt.state.snap.tasks_on(rt.state.today()).find(|t| !t.is_done()).map(|t| t.id).unwrap();
    let (x, y) = rt.find(&format!("plan-title-{id}")).unwrap();
    rt.pointer_down(x, y);
    rt.frame(0.0);
    rt.pointer_move(x + 2.0, y + 2.0);
    rt.frame(0.0);
    assert!(rt.state.drag.dragging().is_none(), "3 pt is still a click");
    rt.pointer_move(x + 4.0, y + 6.0);
    rt.frame(0.0);
    assert!(rt.state.drag.dragging().is_some(), "past 4 pt the card lifts");
    rt.pointer_up(x + 4.0, y + 6.0);
    rt.frame(0.0);
    assert_eq!(rt.state.drag, DragState::Idle);
}

/// The rename field's text sits exactly where the name was, at its size: the heading's line
/// box neither moves nor resizes, nor does anything below it.
fn renames_in_place(rt: &mut Runtime<State>, shown: &str, field: &str, below: &str) {
    let name = rect(rt, shown);
    let under = rect(rt, below);
    click(rt, shown);
    rt.settle_all();
    rt.frame(0.0);
    let edit = rect(rt, field);
    assert_eq!((edit[0], edit[1], edit[3]), (name[0], name[1], name[3]), "`{field}` replaces `{shown}` in place");
    assert_eq!(rect(rt, below), under, "nothing below moves");
}

#[test]
fn renaming_keeps_the_quest_name_at_its_size() {
    let (mut rt, _dir) = app();
    open_first_quest(&mut rt);
    renames_in_place(&mut rt, "quest-title", "quest-rename", "quest-week");
}
