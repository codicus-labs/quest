use chrono::Duration;
use quest_domain::{Command, DateField, EntityId, Position};
use rune::{Key, Mods, Runtime};

use super::dnd_tests::{settle, task};
use super::tests::{click, command, demo_app, press, type_text};
use crate::clock::Clock;
use crate::state::{State, View};

fn focused(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.focused && e.key.as_deref() == Some(key))
}

#[test]
fn quick_add_keyboard_pickers_keep_the_draft_and_reset_after_submit() {
    let (mut rt, _dir) = demo_app();
    let today = rt.state.today();
    assert!(rt.find("quick-add-fields").is_none());
    click(&mut rt, "quick-add");
    assert!(rt.find("quick-add-fields").is_some());
    type_text(&mut rt, "Send a sketch");
    press(&mut rt, Key::Command('l'), Mods { cmd: true, shift: true, ..Mods::default() });
    assert!(focused(&rt, "quest-picker-search"));
    type_text(&mut rt, "half marathon");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.quick_add.text, "Send a sketch");
    assert!(focused(&rt, "quick-add"));

    press(&mut rt, Key::Tab, Mods::default());
    assert!(focused(&rt, "quick-add-quest"));
    type_text(&mut rt, "p");
    assert!(focused(&rt, "task-calendar"));
    type_text(&mut rt, "m");
    assert_eq!(rt.state.quick_add.text, "Send a sketch");
    assert!(focused(&rt, "quick-add"));
    press(&mut rt, Key::Enter, Mods::default());

    let created = task(&rt, "Send a sketch");
    assert_eq!(created.planned_on, today.succ_opt());
    assert_eq!(rt.state.snap.quest(created.quest_id.unwrap()).unwrap().title, "Run a half marathon");
    assert_eq!(rt.state.quick_draft().fields, rt.state.quick_defaults());
    assert!(focused(&rt, "quick-add"));
    assert!(rt.find("quick-add-fields").is_some());
    press(&mut rt, Key::Escape, Mods::default());
    assert!(rt.find("quick-add-fields").is_none());
}

#[test]
fn quick_add_autocompletes_a_quest_and_applies_dates_in_typing_order() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '1');
    click(&mut rt, "quick-add");
    type_text(&mut rt, "Review the draft #mar");
    press(&mut rt, Key::Enter, Mods::default());
    type_text(&mut rt, " @tom !today");
    press(&mut rt, Key::Enter, Mods::default());
    let added = task(&rt, "Review the draft");
    assert_eq!(added.planned_on, Some(rt.state.today()));
    assert_eq!(added.due_on, added.planned_on);
    assert_eq!(rt.state.snap.quest(added.quest_id.unwrap()).unwrap().title, "Run a half marathon");
    click(&mut rt, "page-title");
    settle(&mut rt);
    assert!(rt.find("quick-add-fields").is_none());
}

#[test]
fn a_lingering_inbox_row_goes_before_the_toast_but_undo_still_works_after_both() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '1');
    let before = super::lists::shown_tasks(&rt.state);
    let id = before[0];
    let completed_at = rt.state.clock.now();
    click(&mut rt, &format!("row-check-{id}"));
    assert!(rt.state.snap.task(id).unwrap().is_done());
    assert_eq!(super::lists::shown_tasks(&rt.state), before);
    assert!(rt.find("toast").is_some());
    rt.state.clock = Clock::Fixed(completed_at + Duration::seconds(4));
    rt.frame(0.0);
    assert!(rt.find(&format!("row-{id}")).is_none());
    assert!(rt.find("toast").is_some());
    rt.state.clock = Clock::Fixed(completed_at + Duration::seconds(6));
    rt.frame(0.0);
    settle(&mut rt);
    assert!(rt.find("toast").is_none());
    command(&mut rt, 'z');
    assert_eq!(super::lists::shown_tasks(&rt.state), before);
    assert!(!rt.state.snap.task(id).unwrap().is_done());
}

#[test]
fn deadline_picker_and_quest_picker_replace_the_same_undo_slot() {
    let (mut rt, _dir) = demo_app();
    let id = EntityId::new();
    let today = rt.state.today();
    let original = today.succ_opt();
    rt.update(move |s, _| {
        s.dispatch(Command::CreateTask {
            id,
            title: "Outline the talk".into(),
            quest_id: None,
            planned_on: Some(today),
            due_on: original,
            position: Position::Top,
        });
        s.go(View::Today);
        s.selected_task = Some(id);
    });
    type_text(&mut rt, "d");
    assert_eq!(rt.state.task_calendar.unwrap().field, DateField::Due);
    type_text(&mut rt, "n");
    assert_ne!(rt.state.snap.task(id).unwrap().due_on, original);
    assert!(rt.find("toast").is_none());
    type_text(&mut rt, "l");
    type_text(&mut rt, "half marathon");
    press(&mut rt, Key::Enter, Mods::default());
    assert!(rt.state.snap.task(id).unwrap().quest_id.is_some());
    command(&mut rt, 'z');
    assert!(rt.state.snap.task(id).unwrap().quest_id.is_none());
    command(&mut rt, 'z');
    assert_ne!(rt.state.snap.task(id).unwrap().due_on, original);
}
