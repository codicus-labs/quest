//! A task's dialog, from Today and Plan, for open and done tasks: a double click on a row, a click
//! on a planner card (its title too), "Open" in its menu or ↵ opens it; title and description save as they change
//! and show on the card; closing keeps them and leaves the planner where it was scrolled.

use quest_domain::EntityId;
use rune::{Key, Mods, Runtime};

use super::dnd_tests::{rect, settle, task};
use super::plan_scroll_tests::drawn_offset;
use super::shortcut_tests::hover;
use super::tests::{TempDir, click, command, demo_app, double_click, press, type_text};
use super::title_click_tests::scroll_into_view;
use crate::state::{Overlay, State};

const OPEN: &str = "Design the next chapter";
const DONE: &str = "Read a chapter, take a few notes";

fn app(view: char) -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = demo_app();
    rt.state.stop_focus();
    command(&mut rt, view);
    settle(&mut rt);
    (rt, dir)
}

/// A list row ("row") or a planner card ("plan").
fn card_key(prefix: &str, id: EntityId) -> String {
    if prefix == "row" { format!("row-{id}") } else { format!("{prefix}-task-{id}") }
}

/// Opens the card's dialog: a double click on a row, a click left of the "…" button on a narrow
/// planner card.
fn click_card(rt: &mut Runtime<State>, prefix: &str, id: EntityId) {
    scroll_into_view(rt, &card_key(prefix, id));
    if prefix == "row" {
        return double_click(rt, &card_key(prefix, id));
    }
    let [x, y, _, h] = rect(rt, &format!("{prefix}-menu-{id}"));
    rt.pointer_down(x - 30.0, y + h / 2.0);
    rt.pointer_up(x - 30.0, y + h / 2.0);
    rt.frame(0.0);
}

fn modal_task(rt: &Runtime<State>) -> Option<EntityId> {
    rt.state.task_modal.as_ref().map(|m| m.task)
}

fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text)
}

/// Puts the caret at the end of one of the dialog's fields, as a click into it would.
fn edit_at_end(rt: &mut Runtime<State>, field: fn(&mut State) -> &mut rune::TextEdit) {
    rt.update(move |s, _| {
        let edit = field(s);
        let end = edit.text.len();
        edit.focused = true;
        edit.set_selection(end, end);
    });
    rt.frame(0.0);
}

/// Opens `title`'s dialog by clicking its card, edits title and description, closes it (× or
/// Esc), and checks the card, the store, a second opening and the planner's place.
fn edits_in_the_dialog(view: char, prefix: &str, title: &str, close_with_esc: bool) {
    let (mut rt, _dir) = app(view);
    let id = task(&rt, title).id;
    let done = task(&rt, title).is_done();
    if prefix == "plan" {
        // Somewhere other than where the planner opens.
        let [x, y, w, h] = rect(&rt, "plan-board");
        rt.wheel_xy(x + w / 2.0, y + h / 2.0, -60.0, 0.0);
        settle(&mut rt);
    }
    scroll_into_view(&mut rt, &card_key(prefix, id));
    let place = (prefix == "plan").then(|| drawn_offset(&rt));

    click_card(&mut rt, prefix, id);
    assert_eq!(modal_task(&rt), Some(id), "a click on the card opens its dialog");
    assert_eq!(rt.state.overlay, Overlay::Task);
    assert!(rt.find("task-modal-scrim").is_some(), "the view behind is dimmed");
    assert_eq!(text_of(&rt, "task-modal-title").as_deref(), Some(title));
    assert_eq!(text_of(&rt, "task-modal-status").as_deref(), Some("Saved"));

    edit_at_end(&mut rt, |s| &mut s.task_modal_fields().title);
    type_text(&mut rt, " v2");
    edit_at_end(&mut rt, |s| &mut s.task_modal_fields().description);
    type_text(&mut rt, " Notes.");
    let renamed = format!("{title} v2");
    assert_eq!(rt.state.snap.task(id).unwrap().title, renamed, "the title saves as it changes");
    assert_eq!(text_of(&rt, &format!("{prefix}-title-{id}")), Some(renamed.clone()), "the card shows it at once");

    if close_with_esc {
        press(&mut rt, Key::Escape, Mods::default());
    } else {
        click(&mut rt, "task-modal-close");
    }
    settle(&mut rt);
    assert_eq!(rt.state.overlay, Overlay::None);
    assert!(rt.find("task-modal").is_none(), "the dialog closed");
    let saved = rt.state.snap.task(id).unwrap().clone();
    assert_eq!(saved.title, renamed);
    assert!(saved.description.ends_with(" Notes."), "{:?}", saved.description);
    assert!(!saved.title.contains('\n') && saved.is_done() == done, "done stays done, open stays open");
    if let Some(place) = place {
        assert!((drawn_offset(&rt) - place).abs() < 0.5, "the planner stays where it was");
    }

    click_card(&mut rt, prefix, id);
    assert_eq!(modal_task(&rt), Some(id));
    assert_eq!(text_of(&rt, "task-modal-title"), Some(renamed));
    assert_eq!(rt.state.task_modal.as_ref().unwrap().description.text, saved.description);
    assert!(rt.find("task-modal-description-preview").is_some());
    if let Some(place) = place {
        assert!((drawn_offset(&rt) - place).abs() < 0.5, "opening doesn't scroll the planner either");
    }
}

#[test]
fn today_edits_an_open_task() {
    edits_in_the_dialog('2', "row", OPEN, false);
}

#[test]
fn today_edits_a_done_task() {
    edits_in_the_dialog('2', "row", DONE, true);
}

#[test]
fn plan_edits_an_open_task() {
    edits_in_the_dialog('3', "plan", OPEN, true);
}

#[test]
fn plan_edits_a_done_task() {
    edits_in_the_dialog('3', "plan", DONE, false);
}

#[test]
fn a_click_selects_a_row_and_return_or_a_double_click_open_it_the_check_doesnt() {
    let (mut rt, _dir) = app('2');
    let id = task(&rt, OPEN).id;
    click(&mut rt, &format!("row-title-{id}"));
    assert_eq!((rt.state.selected_task, modal_task(&rt)), (Some(id), None), "a click only selects");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(modal_task(&rt), Some(id), "↵ on the selected row");
    press(&mut rt, Key::Escape, Mods::default());
    settle(&mut rt);
    click(&mut rt, &format!("row-check-{id}"));
    assert!(modal_task(&rt).is_none(), "the check only checks");
    assert!(task(&rt, OPEN).is_done());
    click(&mut rt, &format!("row-check-{id}"));

    rt.state.selected_task = None;
    settle(&mut rt);
    hover(&mut rt, id);
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(modal_task(&rt), Some(id), "↵ over the row");
    press(&mut rt, Key::Escape, Mods::default());
    settle(&mut rt);
    double_click(&mut rt, &format!("row-title-{id}"));
    assert_eq!(modal_task(&rt), Some(id), "a double click on the title");
}

#[test]
fn an_empty_title_is_never_saved() {
    let (mut rt, _dir) = app('2');
    let id = task(&rt, OPEN).id;
    click_card(&mut rt, "row", id);
    rt.update(|s, _| {
        let title = &mut s.task_modal_fields().title;
        title.focused = true;
        title.set_selection(0, title.text.len());
    });
    rt.frame(0.0);
    press(&mut rt, Key::Backspace, Mods::default());
    assert_eq!(rt.state.snap.task(id).unwrap().title, OPEN);
    assert_eq!(text_of(&rt, "task-modal-status").as_deref(), Some("Saving…"), "it isn't stored");
    press(&mut rt, Key::Escape, Mods::default());
    assert!(modal_task(&rt).is_none());
    assert_eq!(rt.state.snap.task(id).unwrap().title, OPEN, "nor on closing");
}

#[test]
fn date_quest_and_delete_act_on_the_task() {
    let (mut rt, _dir) = app('2');
    let id = task(&rt, OPEN).id;
    let today = rt.state.today();
    click_card(&mut rt, "row", id);

    rt.update(|s, _| {
        let date = &mut s.task_modal_fields().date;
        date.focused = true;
        date.set_selection(0, date.text.len());
    });
    rt.frame(0.0);
    let tomorrow = today + chrono::Days::new(1);
    type_text(&mut rt, &crate::log_work::date_label(tomorrow));
    assert_eq!(rt.state.snap.task(id).unwrap().planned_on, Some(today), "not while typing");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.snap.task(id).unwrap().planned_on, Some(tomorrow), "Return takes the day");
    click(&mut rt, "task-modal-backlog");
    assert_eq!(rt.state.snap.task(id).unwrap().planned_on, None, "Backlog");

    click(&mut rt, "task-modal-quest");
    click(&mut rt, "quest-picker-none");
    assert_eq!(rt.state.snap.task(id).unwrap().quest_id, None, "No quest");
    let first = rt.state.snap.active_quests().next().unwrap().id;
    click(&mut rt, "task-modal-quest");
    click(&mut rt, &format!("quest-picker-{first}"));
    assert_eq!(rt.state.snap.task(id).unwrap().quest_id, Some(first));
    assert_eq!(text_of(&rt, "task-modal-repeat"), None, "Repeat is a row, not a field");
    assert!(text_of(&rt, "task-modal-logged-total").is_some_and(|t| t.ends_with(" total")));

    click(&mut rt, "task-modal-delete");
    assert!(rt.state.snap.task(id).is_none(), "deleted");
    settle(&mut rt);
    assert!(modal_task(&rt).is_none() && rt.find("task-modal").is_none(), "and the dialog closed");
    click(&mut rt, "toast-action");
    assert!(rt.state.snap.task(id).is_some(), "Undo brings it back");
}

#[test]
fn notes_keep_markdown_when_editing_and_hide_markers_when_viewing() {
    let (mut rt, _dir) = app('2');
    let id = task(&rt, OPEN).id;
    rt.update(move |s, _| s.open_task_modal(id));
    settle(&mut rt);
    if rt.find("task-modal-description-preview").is_some() {
        click(&mut rt, "task-modal-description-preview");
    } else {
        click(&mut rt, "task-modal-description");
    }
    press(&mut rt, Key::Command('a'), Mods { cmd: true, ..Mods::default() });
    type_text(&mut rt, "# A large heading");
    assert_eq!(text_of(&rt, "task-modal-description").as_deref(), Some("# A large heading"));
    click(&mut rt, "task-modal-title");
    assert!(rt.find("task-modal-description").is_none());
    assert!(rt.find("task-modal-description-preview").is_some());
    assert!(rt.elements().iter().any(|element| element.text.as_deref() == Some("A large heading")));
    click(&mut rt, "task-modal-description-preview");
    assert_eq!(text_of(&rt, "task-modal-description").as_deref(), Some("# A large heading"));
}
