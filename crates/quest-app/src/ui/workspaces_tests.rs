//! Workspaces in the real view: the switcher, Manage workspaces, what a switch resets, and the
//! focus timer that is one per person across workspaces.

use quest_domain::{Command, EntityId};
use rune::{Key, Mods, Runtime};

use super::log_work_tests::{fill, text_of};
use super::tests::{click, command, demo_app, press, type_text};
use crate::state::{Overlay, State, View};
use crate::undo::Deletable;

const DEFAULT: EntityId = EntityId::DEFAULT_WORKSPACE;

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key))
}

fn task_titled(rt: &Runtime<State>, title: &str) -> EntityId {
    rt.state.snap.tasks.iter().find(|t| t.title == title).map(|t| t.id).expect("demo task")
}

/// Opens Manage workspaces from the switcher (its last menu item).
fn open_manage(rt: &mut Runtime<State>) {
    click(rt, "workspace-switcher");
    assert!(rt.state.workspace_menu);
    let manage = rt.state.snap.live_workspaces().count() + 1;
    click(rt, &format!("workspace-menu-item{manage}"));
    assert_eq!(rt.state.overlay, Overlay::Workspaces);
}

/// Creates a workspace through Manage workspaces; the app then shows it.
fn create(rt: &mut Runtime<State>, name: &str) -> EntityId {
    open_manage(rt);
    fill(rt, "workspace-new", name);
    press(rt, Key::Enter, Mods::default());
    rt.state.snap.workspace
}

#[test]
fn creating_renaming_switching_deleting_and_restoring() {
    let (mut rt, _dir) = demo_app();
    assert_eq!(text_of(&rt, "workspace-name"), "Default");
    let demo_tasks = rt.state.snap.tasks.len();

    let side = create(&mut rt, "Side");
    assert_ne!(side, DEFAULT);
    assert_eq!(rt.state.overlay, Overlay::None, "creating closes the dialog");
    assert_eq!(text_of(&rt, "workspace-name"), "Side");
    assert!(rt.state.snap.tasks.is_empty(), "a new workspace starts empty");

    open_manage(&mut rt);
    fill(&mut rt, "workspace-new", "default");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(text_of(&rt, "workspace-error"), "Another workspace already has this name.");
    fill(&mut rt, &format!("workspace-name-{side}"), "Side project");
    click(&mut rt, &format!("workspace-rename-{side}"));
    assert_eq!(rt.state.snap.active_workspace().name, "Side project");

    click(&mut rt, &format!("workspace-delete-{side}"));
    assert!(text_of(&rt, "workspace-delete-question").contains("It is empty."));
    click(&mut rt, "workspace-delete-confirm");
    assert_eq!(rt.state.snap.workspace, DEFAULT, "deleting the shown workspace shows a remaining one");
    assert_eq!(rt.state.snap.tasks.len(), demo_tasks);
    assert_eq!(rt.state.overlay, Overlay::Workspaces, "the dialog stays open");
    assert!(shown(&rt, &format!("workspace-restore-{side}")));

    click(&mut rt, &format!("workspace-delete-{DEFAULT}"));
    assert!(!shown(&rt, "workspace-delete-question"), "the last workspace can't be deleted");

    click(&mut rt, &format!("workspace-restore-{side}"));
    assert!(rt.state.snap.live_workspace(side).is_some());
    press(&mut rt, Key::Escape, Mods::default());
    click(&mut rt, "workspace-switcher");
    click(&mut rt, "workspace-menu-item1");
    assert_eq!(rt.state.snap.workspace, side);
    assert_eq!(text_of(&rt, "workspace-name"), "Side project");
}

#[test]
fn the_default_workspace_can_be_deleted_while_another_one_remains() {
    let (mut rt, _dir) = demo_app();
    let side = create(&mut rt, "Side");
    open_manage(&mut rt);
    click(&mut rt, &format!("workspace-delete-{DEFAULT}"));
    assert!(text_of(&rt, "workspace-delete-question").contains("It holds"), "says what it holds");
    click(&mut rt, "workspace-delete-confirm");
    assert_eq!(rt.state.snap.workspace, side);
    assert_eq!(rt.state.snap.live_workspaces().count(), 1);
}

#[test]
fn the_palette_switches_workspaces() {
    let (mut rt, _dir) = demo_app();
    let side = create(&mut rt, "Side");
    command(&mut rt, 'k');
    type_text(&mut rt, "switch workspace");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.overlay, Overlay::Palette, "it asks which workspace");
    type_text(&mut rt, "default");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.snap.workspace, DEFAULT);
    command(&mut rt, 'k');
    type_text(&mut rt, "side");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.snap.workspace, side, "a workspace's name is a jump too");
}

#[test]
fn a_switch_drops_dialogs_selection_and_undo_of_the_old_workspace() {
    let (mut rt, _dir) = demo_app();
    let side = create(&mut rt, "Side");
    click(&mut rt, "workspace-switcher");
    click(&mut rt, "workspace-menu-item0");
    assert_eq!(rt.state.snap.workspace, DEFAULT);

    let task = task_titled(&rt, "Draft this week’s essay");
    let quest = rt.state.snap.quests[0].id;
    rt.state.delete(Deletable::Task(task_titled(&rt, "Answer emails")));
    assert!(rt.state.undo.expires().is_some(), "⌘Z could take the delete back");
    rt.state.quest_filter = Some(quest);
    rt.state.go(View::Quest(quest));
    rt.state.open_task_modal(task);
    rt.frame(0.0);

    rt.state.switch_workspace(side);
    rt.frame(0.0);
    assert_eq!(rt.state.snap.workspace, side);
    assert_eq!((rt.state.overlay, rt.state.task_modal.is_none()), (Overlay::None, true));
    assert_eq!(rt.state.view, View::Quests, "a quest's page goes back to Quests");
    assert_eq!(rt.state.quest_filter, None);
    assert!(rt.state.undo.expires().is_none());
    command(&mut rt, 'z');
    assert!(rt.state.snap.tasks.is_empty(), "⌘Z brings nothing from the other workspace");
}

#[test]
fn a_workspace_deleted_on_another_device_falls_back_and_closes_its_dialogs() {
    let (mut rt, _dir) = demo_app();
    let side = create(&mut rt, "Side");
    rt.state.dispatch(Command::CreateTask {
        id: EntityId::new(),
        title: "Side task".into(),
        quest_id: None,
        planned_on: None,
        due_on: None,
        position: quest_domain::Position::Top,
    });
    rt.state.open_task_modal(task_titled(&rt, "Side task"));
    // As if another device's delete arrived: straight into the store, past the app's state.
    rt.state.store.dispatch(Command::DeleteWorkspace { id: side, at: rt.state.clock.now() }).unwrap();
    rt.frame(0.0);
    assert_eq!(rt.state.snap.workspace, DEFAULT);
    assert_eq!(rt.state.overlay, Overlay::None);
    assert!(rt.state.task_modal.is_none());
}

#[test]
fn the_focus_timer_runs_on_across_a_switch_and_starting_elsewhere_stops_it() {
    let (mut rt, _dir) = demo_app();
    let running = rt.state.snap.running_entry().map(|e| e.id).expect("the demo runs a session");
    let side = create(&mut rt, "Side");
    assert!(shown(&rt, "timer-elsewhere"), "the timer stays visible");
    command(&mut rt, '3');
    assert!(shown(&rt, "timer-elsewhere"), "also on Focus");

    click(&mut rt, "timer-elsewhere");
    assert_eq!((rt.state.snap.workspace, rt.state.view), (DEFAULT, View::Focus));
    assert_eq!(rt.state.snap.running_entry().map(|e| e.id), Some(running));

    rt.state.switch_workspace(side);
    rt.state.dispatch(Command::CreateTask {
        id: EntityId::new(),
        title: "Side task".into(),
        quest_id: None,
        planned_on: Some(rt.state.today()),
        due_on: None,
        position: quest_domain::Position::Top,
    });
    rt.state.start_focus(task_titled(&rt, "Side task"));
    rt.frame(0.0);
    assert_eq!(rt.state.snap.running.len(), 1, "one timer per person");
    assert!(rt.state.snap.running_entry().is_some());
    rt.settle_all();
    rt.frame(0.0);
    assert!(!shown(&rt, "timer-elsewhere"));
}

#[test]
fn restoring_waits_while_another_workspace_has_its_name() {
    let (mut rt, _dir) = demo_app();
    let old = create(&mut rt, "Work");
    rt.state.open_workspaces();
    rt.state.ask_delete_workspace(old);
    rt.state.delete_workspace();
    rt.state.workspaces.new_name = rune::TextEdit::new("work");
    rt.state.create_workspace();
    rt.state.open_workspaces();
    rt.frame(0.0);
    click(&mut rt, &format!("workspace-restore-{old}"));
    assert!(rt.state.snap.live_workspace(old).is_none(), "two live workspaces never share a name");
    assert!(text_of(&rt, "workspace-error").starts_with("Rename the other “Work” first."));
}
