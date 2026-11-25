use quest_domain::EntityId;
use rune::{Key, Mods, Runtime};

use super::dnd_tests::{rect, scroll_into_view, settle, task};
use super::shortcut_tests::hover;
use super::tests::{TempDir, click, command, demo_app, press, type_text};
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

fn click_card(rt: &mut Runtime<State>, _prefix: &str, id: EntityId) {
    scroll_into_view(rt, &format!("row-{id}"));
    click(rt, &format!("row-title-{id}"));
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

fn edits_in_the_dialog(view: char, prefix: &str, title: &str, close_with_esc: bool) {
    let (mut rt, _dir) = app(view);
    let id = task(&rt, title).id;
    let done = task(&rt, title).is_done();
    rt.update(move |s, _| s.selected_task = Some(id));
    settle(&mut rt);

    let title_key = format!("row-title-{id}");
    let check_key = format!("row-check-{id}");
    scroll_into_view(&mut rt, &title_key);
    let before = rect(&rt, &title_key);
    let check = rect(&rt, &check_key);
    click_card(&mut rt, prefix, id);
    settle(&mut rt);
    let after = rect(&rt, "task-modal-title");
    assert_eq!(&before[..2], &after[..2], "the title becomes the input in place");
    assert_eq!(rect(&rt, &check_key), check);
    assert_eq!(modal_task(&rt), Some(id), "a click on the card opens its dialog");
    assert_eq!(rt.state.overlay, Overlay::None);
    assert!(rt.find("task-modal-scrim").is_none());
    assert_eq!(text_of(&rt, "task-modal-title").as_deref(), Some(title));

    edit_at_end(&mut rt, |s| &mut s.task_modal_fields().title);
    type_text(&mut rt, " v2");
    edit_at_end(&mut rt, |s| &mut s.task_modal_fields().description);
    type_text(&mut rt, " Notes.");
    let renamed = format!("{title} v2");
    assert_eq!(rt.state.snap.task(id).unwrap().title, renamed, "the title saves as it changes");
    assert!(rt.find(&title_key).is_none(), "the expanded row has only one title");

    if close_with_esc {
        press(&mut rt, Key::Escape, Mods::default());
    } else {
        click(&mut rt, "task-modal-title");
        press(&mut rt, Key::Enter, Mods::default());
    }
    settle(&mut rt);
    assert_eq!(rt.state.overlay, Overlay::None);
    assert!(rt.find("task-modal").is_none(), "the dialog closed");
    let saved = rt.state.snap.task(id).unwrap().clone();
    assert_eq!(saved.title, renamed);
    assert_eq!(text_of(&rt, &title_key), Some(renamed.clone()));
    assert!(saved.description.ends_with(" Notes."), "{:?}", saved.description);
    assert!(!saved.title.contains('\n') && saved.is_done() == done, "done stays done, open stays open");

    click_card(&mut rt, prefix, id);
    assert_eq!(modal_task(&rt), Some(id));
    assert_eq!(text_of(&rt, "task-modal-title"), Some(renamed));
    assert_eq!(rt.state.task_modal.as_ref().unwrap().description.text, saved.description);
    assert!(rt.find("task-modal-description").is_some());
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
fn plan_today_includes_completed_tasks() {
    let (rt, _dir) = app('3');
    let done = task(&rt, DONE);
    assert!(done.is_done());
    assert!(rt.find(&format!("row-{}", done.id)).is_some());
}

#[test]
fn a_click_opens_and_closes_a_row_but_the_check_doesnt() {
    let (mut rt, _dir) = app('2');
    let id = task(&rt, OPEN).id;
    click(&mut rt, &format!("row-title-{id}"));
    assert_eq!((rt.state.selected_task, modal_task(&rt)), (Some(id), Some(id)));
    let [x, y, _, h] = rect(&rt, &format!("row-{id}"));
    rt.pointer_down(x + 4.0, y + h / 2.0);
    rt.pointer_up(x + 4.0, y + h / 2.0);
    rt.frame(0.0);
    assert!(modal_task(&rt).is_none());
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
    assert!(modal_task(&rt).is_none());
}

#[test]
fn blank_space_dismisses_the_detail_but_its_timer_and_check_still_work() {
    let (mut rt, _dir) = app('2');
    let id = task(&rt, OPEN).id;
    click_card(&mut rt, "row", id);
    click(&mut rt, &format!("row-play-{id}"));
    assert_eq!(rt.state.snap.running_entry().unwrap().task_id, Some(id));
    click(&mut rt, &format!("row-play-{id}"));
    assert!(rt.state.snap.running_entry().is_none());
    click(&mut rt, &format!("row-check-{id}"));
    assert!(rt.state.snap.task(id).unwrap().is_done());
    assert_eq!(modal_task(&rt), Some(id));
    rt.pointer_down(600.0, 820.0);
    rt.pointer_up(600.0, 820.0);
    rt.frame(0.0);
    assert!(modal_task(&rt).is_none());
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
    assert_eq!(rt.state.task_modal.as_ref().unwrap().title.text, "");
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

    let plan = format!("task-{id}-plan");
    click(&mut rt, &plan);
    press(&mut rt, Key::Right, Mods::default());
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.snap.task(id).unwrap().planned_on, today.succ_opt());
    assert_eq!(quest_view::lists::postponed(&rt.state.snap, id).plan, 1);
    command(&mut rt, 'z');
    assert_eq!(rt.state.snap.task(id).unwrap().planned_on, Some(today));
    assert_eq!(quest_view::lists::postponed(&rt.state.snap, id).plan, 0);
    click(&mut rt, &plan);
    press(&mut rt, Key::Escape, Mods::default());
    assert!(rt.state.task_calendar.is_none());
    assert!(rt.elements().iter().any(|e| e.focused && e.key.as_deref() == Some(plan.as_str())));

    click(&mut rt, "task-modal-quest");
    click(&mut rt, "quest-picker-none");
    assert_eq!(rt.state.snap.task(id).unwrap().quest_id, None, "No quest");
    let first = rt.state.snap.active_quests().next().unwrap().id;
    click(&mut rt, "task-modal-quest");
    click(&mut rt, &format!("quest-picker-{first}"));
    assert_eq!(rt.state.snap.task(id).unwrap().quest_id, Some(first));
    assert_eq!(text_of(&rt, "task-modal-repeat"), None, "Repeat is a row, not a field");
    let first_entry =
        rt.state.snap.time_entries.iter().filter(|e| e.task_id == Some(id)).max_by_key(|e| e.started_at).unwrap().id;
    click(&mut rt, "task-time");
    assert!(rt.state.show_task_time);
    rt.update(|_, tx| tx.focus_when_shown("task-time", false));
    rt.frame(0.0);
    press(&mut rt, Key::Enter, Mods::default());
    assert!(!rt.state.show_task_time);
    press(&mut rt, Key::Enter, Mods::default());
    click(&mut rt, &format!("task-time-{first_entry}"));
    assert_eq!(rt.state.overlay, Overlay::EntryEditor);
    press(&mut rt, Key::Escape, Mods::default());

    click(&mut rt, "task-modal-delete");
    assert!(rt.state.snap.task(id).is_none(), "deleted");
    settle(&mut rt);
    assert!(modal_task(&rt).is_none() && rt.find("task-modal").is_none(), "and the dialog closed");
    click(&mut rt, "toast-action");
    assert!(rt.state.snap.task(id).is_some(), "Undo brings it back");
}

#[test]
fn notes_show_and_save_plain_text() {
    let (mut rt, _dir) = app('2');
    let id = task(&rt, OPEN).id;
    rt.update(move |s, _| s.open_task_modal(id));
    settle(&mut rt);
    click(&mut rt, "task-modal-description");
    press(&mut rt, Key::Command('a'), Mods { cmd: true, ..Mods::default() });
    type_text(&mut rt, "# A large heading");
    assert_eq!(text_of(&rt, "task-modal-description").as_deref(), Some("# A large heading"));
    click(&mut rt, "task-modal-title");
    assert_eq!(text_of(&rt, "task-modal-description").as_deref(), Some("# A large heading"));
    assert_eq!(rt.state.snap.task(id).unwrap().description, "# A large heading");
}
