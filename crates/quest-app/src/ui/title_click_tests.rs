//! A planner card's title is no field of its own: a click on it opens the task's dialog (open and
//! done tasks), where the title is edited, and a drag that starts on it moves the card.

use chrono::Days;
use quest_domain::EntityId;
use rune::Runtime;

use super::dnd_tests::{center, drag, rect, settle, task};
use super::tests::{TempDir, click, command, demo_app, plan_key};
use crate::state::{Overlay, State, View};

const OPEN: &str = "Design the next chapter";

/// The demo with its running session stopped, so a click visibly starts none.
fn app() -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = demo_app();
    rt.state.stop_focus();
    settle(&mut rt);
    (rt, dir)
}

/// `demo_app`'s window height.
const WINDOW_H: f32 = 900.0;

/// Wheels the list under a card until all of it is above the window's bottom (Plan's done cards
/// sit below the fold).
pub(super) fn scroll_into_view(rt: &mut Runtime<State>, key: &str) {
    for _ in 0..10 {
        let [x, y, w, h] = rect(rt, key);
        if y + h <= WINDOW_H - 20.0 {
            return;
        }
        rt.wheel(x + w / 2.0, (WINDOW_H - 60.0).min(y + 10.0), -200.0);
        settle(rt);
    }
    panic!("`{key}` scrolled into view");
}

/// Clicks `title`'s card title in `view` and checks the dialog opened on it, with nothing else
/// changed: no field on the card, the task as it was, no session, the same view.
fn title_click_opens_the_dialog(view: char, prefix: &str, title: &str) {
    let (mut rt, _dir) = app();
    command(&mut rt, view);
    settle(&mut rt);
    let before = task(&rt, title);
    let view_before = rt.state.view;
    let title_key = format!("{prefix}-title-{}", before.id);
    scroll_into_view(&mut rt, &format!("{prefix}-task-{}", before.id));

    click(&mut rt, &title_key);
    settle(&mut rt);

    assert_eq!(rt.state.task_modal.as_ref().map(|m| m.task), Some(before.id), "the title opens the dialog");
    assert_eq!(rt.state.overlay, Overlay::None);
    let card_title = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(title_key.as_str()));
    assert_eq!(card_title.map(|e| (e.role, e.text)), Some(("text", Some(title.to_owned()))), "the card shows text");
    let modal_title = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("task-modal-title"));
    assert_eq!(modal_title.and_then(|e| e.text), Some(title.to_owned()), "the dialog edits the title");
    assert_eq!(rt.state.snap.task(before.id), Some(&before), "the task is unchanged");
    assert!(rt.state.snap.running_entry().is_none(), "no session started");
    assert_eq!(rt.state.view, view_before, "the view stays");
}

#[test]
fn plan_title_of_an_open_task_opens_its_dialog() {
    title_click_opens_the_dialog('3', "plan", OPEN);
}

#[test]
fn a_drag_from_the_title_moves_the_card_and_opens_nothing() {
    let (mut rt, _dir) = app();
    command(&mut rt, '3');
    settle(&mut rt);
    let id: EntityId = task(&rt, OPEN).id;
    let today = rt.state.today();
    let [x, y, w, h] = rect(&rt, &plan_key(&rt, "col", 4));
    let from = center(&rt, &format!("plan-title-{id}"));
    drag(&mut rt, from, (x + w / 2.0, y + h - 40.0));
    assert_eq!(rt.state.snap.task(id).unwrap().planned_on, Some(today + Days::new(2)), "the card moved");
    assert!(rt.state.task_modal.is_none(), "no dialog opened");
    assert_eq!(rt.state.view, View::Plan);
}
