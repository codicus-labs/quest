//! The quest page: open it from the quest's dialog, rename, recolour, complete, add a task; its
//! milestones to read only (no section without any), and nowhere to add one, also not from a
//! task; and a card's body: a click selects it, its buttons stay buttons.

use quest_domain::{EntityId, QuestColor, QuestStatus};
use rune::{Key, Mods, Runtime};

use super::dnd_tests::rect;
use super::tests::{TempDir, click, click_beside_title, command, demo_app, press, type_text};
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
    command(rt, '5');
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
    click(&mut rt, "quest-add-task");
    type_text(&mut rt, "Outline chapter four");
    enter(&mut rt);
    let task = rt.state.snap.tasks.iter().find(|t| t.title == "Outline chapter four").expect("created");
    let today = rt.state.today();
    assert_eq!((task.quest_id, task.day), (Some(id), Some(today)), "on this quest, today (the default)");
    assert!(rt.find(&format!("quest-task-{}", task.id)).is_some(), "listed on the page");
    click(&mut rt, "quest-add-backlog");
    click(&mut rt, "quest-add-task");
    type_text(&mut rt, "Someday chapter");
    enter(&mut rt);
    let later = rt.state.snap.tasks.iter().find(|t| t.title == "Someday chapter").expect("created");
    assert_eq!((later.quest_id, later.day), (Some(id), None), "on this quest, in the backlog");
}

/// Elements that look like a way to add, edit or delete a milestone.
fn milestone_controls(rt: &Runtime<State>) -> Vec<String> {
    rt.elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .filter(|e| {
            let words =
                [e.key.as_deref(), e.label.as_deref(), e.text.as_deref()].map(|w| w.unwrap_or("").to_lowercase());
            let editing = (e.focusable || ["button", "textbox", "menuitem"].contains(&e.role))
                && words.iter().any(|w| w.contains("milestone"));
            editing || words.iter().any(|w| w.contains("what did you achieve") || w.contains("mark as milestone"))
        })
        .map(|e| e.key.unwrap_or_default())
        .collect()
}

#[test]
fn milestones_show_to_read_only() {
    let (mut rt, _dir) = app();
    let id = open_first_quest(&mut rt);
    let milestone = rt.state.snap.milestones_of(id).last().cloned().expect("a demo milestone");
    assert!(rt.find("quest-milestones").is_some(), "the section shows");
    let row = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&format!("milestone-{}", milestone.id)));
    let label = row.and_then(|e| e.label).expect("the milestone is listed");
    assert_eq!(label, format!("{}, {}", milestone.title, milestone.achieved_on.format("%b %-d, %Y")));
    assert_eq!(milestone_controls(&rt), Vec::<String>::new(), "nothing to add, edit or delete");

    click(&mut rt, &format!("milestone-title-{}", milestone.id));
    click(&mut rt, &format!("milestone-date-{}", milestone.id));
    assert_eq!(
        (rt.state.overlay, rt.state.quest_edit),
        (Overlay::None, crate::state::QuestEdit::None),
        "clicks do nothing"
    );
    assert_eq!(rt.state.snap.milestones_of(id).last(), Some(&milestone));
}

#[test]
fn a_quest_without_milestones_has_no_section() {
    let (mut rt, _dir) = app();
    let quest =
        rt.state.snap.active_quests().find(|q| rt.state.snap.milestones_of(q.id).next().is_none()).map(|q| q.id);
    let quest = quest.expect("a demo quest without milestones");
    rt.update(|s, _| s.go(View::Quest(quest)));
    rt.settle_all();
    rt.frame(0.0);
    assert!(rt.find("quest-tasks").is_some(), "the page shows its tasks");
    assert!(rt.find("quest-milestones").is_none());
    assert!(
        !rt.elements()
            .iter()
            .any(|e| e.text.as_deref() == Some("MILESTONES") || e.text.as_deref() == Some("Milestones"))
    );
}

#[test]
fn a_task_offers_no_milestone_in_its_menu_or_on_completion() {
    let (mut rt, _dir) = app();
    let today = rt.state.today();
    let task = rt.state.snap.tasks_on(today).find(|t| t.quest_id.is_some() && !t.is_done()).cloned();
    let task = task.expect("quest work");
    let milestones = rt.state.snap.milestones.len();
    click(&mut rt, &format!("home-menu-{}", task.id));
    rt.settle_all();
    rt.frame(0.0);
    assert!(rt.find("card-menu-menu").is_some(), "the menu opens");
    let items: Vec<String> = rt
        .elements()
        .into_iter()
        .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("card-menu-item")))
        .filter_map(|e| e.text.or(e.label))
        .collect();
    assert!(items.iter().any(|i| i == "Open quest"), "the quest's actions are there: {items:?}");
    assert!(!items.iter().any(|i| i.to_lowercase().contains("milestone")), "{items:?}");
    press(&mut rt, Key::Escape, Mods::default());

    click(&mut rt, &format!("home-check-{}", task.id));
    assert!(rt.state.snap.task(task.id).unwrap().is_done());
    rt.settle_all();
    rt.frame(0.0);
    assert!(rt.find("toast").is_none(), "completing offers nothing");
    assert_eq!(rt.state.undo.shown, None);
    assert_eq!(rt.state.snap.milestones.len(), milestones);
}

#[test]
fn a_click_on_a_card_opens_its_dialog_and_its_buttons_do_not_drag() {
    let (mut rt, _dir) = app();
    let today = rt.state.today();
    let order = |rt: &Runtime<State>| rt.state.snap.tasks_on(today).map(|t| t.id).collect::<Vec<_>>();
    let before = order(&rt);
    let id = rt.state.snap.tasks_on(today).find(|t| !t.is_done()).map(|t| t.id).unwrap();
    click_beside_title(&mut rt, "home", id);
    assert_eq!(rt.state.task_modal.as_ref().map(|m| m.task), Some(id), "a click without travel opens the dialog");
    assert_eq!(order(&rt), before, "and moves nothing");
    press(&mut rt, Key::Escape, Mods::default());
    assert!(rt.state.task_modal.is_none() && rt.find("task-modal").is_none(), "Esc closes it");

    // A press on the check that wanders a little stays a press on the check.
    let (x, y) = rt.find(&format!("home-check-{id}")).unwrap();
    rt.pointer_down(x, y);
    rt.frame(0.0);
    rt.pointer_move(x + 2.0, y + 2.0);
    rt.frame(0.0);
    assert_eq!(rt.state.drag, DragState::Idle, "the check doesn't start a drag");
    rt.pointer_up(x + 2.0, y + 2.0);
    rt.frame(0.0);
    assert!(rt.state.snap.task(id).unwrap().is_done());

    click(&mut rt, &format!("home-chip-{id}"));
    assert_eq!(rt.state.palette.mode, Mode::AssignQuest(id), "the chip picks a quest");
    assert_eq!(rt.state.drag, DragState::Idle);
}

#[test]
fn a_press_on_a_card_drags_only_past_the_threshold() {
    let (mut rt, _dir) = app();
    let id = rt.state.snap.tasks_on(rt.state.today()).find(|t| !t.is_done()).map(|t| t.id).unwrap();
    let (x, y) = rt.find(&format!("home-title-{id}")).unwrap();
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
