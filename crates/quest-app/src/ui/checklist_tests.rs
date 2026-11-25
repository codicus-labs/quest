use quest_domain::{Command, EntityId};
use rune::{Key, Mods};

use super::dnd_tests::{scroll_into_view, task};
use super::tests::{click, command, demo_app, press, type_text};

#[test]
fn checklist_keyboard_reorders_deletes_and_undoes_in_the_open_row() {
    let (mut rt, _dir) = demo_app();
    let id = task(&rt, "Design the next chapter").id;
    rt.update(move |s, _| s.open_task_modal(id));
    scroll_into_view(&mut rt, "checklist-add");
    click(&mut rt, "checklist-add");
    for name in ["First", "Second"] {
        type_text(&mut rt, name);
        press(&mut rt, Key::Enter, Mods::default());
    }
    let items = |s: &crate::state::State| -> Vec<EntityId> { s.snap.checklist_of(id).map(|i| i.id).collect() };
    let original = items(&rt.state);
    assert_eq!(original.len(), 2);
    let (first, second) = (original[0], original[1]);

    press(&mut rt, Key::Up, Mods::default());
    press(&mut rt, Key::Up, Mods { alt: true, ..Mods::default() });
    assert_eq!(items(&rt.state), [second, first]);
    press(&mut rt, Key::Enter, Mods { cmd: true, ..Mods::default() });
    assert!(rt.state.snap.checklist_item(second).unwrap().done);
    press(&mut rt, Key::Escape, Mods::default());
    command(&mut rt, 'z');
    assert!(!rt.state.snap.checklist_item(second).unwrap().done);

    scroll_into_view(&mut rt, &format!("checklist-item-{second}"));
    click(&mut rt, &format!("checklist-item-{second}"));
    press(&mut rt, Key::Enter, Mods::default());
    let inserted = items(&rt.state)[1];
    assert_eq!(rt.state.snap.checklist_item(inserted).unwrap().text, "");
    press(&mut rt, Key::Backspace, Mods::default());
    assert_eq!(items(&rt.state), [second, first]);
    press(&mut rt, Key::Escape, Mods::default());
    command(&mut rt, 'z');
    assert_eq!(items(&rt.state), [second, inserted, first]);
    assert!(rt.state.task_modal.is_some());
}

#[test]
fn markdown_task_box_changes_the_note_without_opening_the_editor() {
    let (mut rt, _dir) = demo_app();
    let id = task(&rt, "Design the next chapter").id;
    rt.update(move |s, _| {
        s.dispatch(Command::SetTaskDescription { id, description: "- [ ] A step".into() });
        s.open_task_modal(id);
    });
    scroll_into_view(&mut rt, "note-check-2");
    click(&mut rt, "note-check-2");
    assert_eq!(rt.state.snap.task(id).unwrap().description, "- [x] A step");
    assert!(rt.state.note.is_none());
    assert_eq!(rt.state.snap.checklist_of(id).count(), 0);
}
