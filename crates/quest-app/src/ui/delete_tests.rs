//! Deleting with undo: a task (card menu, ⌫, ⌘K), a quest with its milestones and a session each
//! go to the archive with a toast; Undo or ⌘Z brings them back, and the archive restores them.

use quest_domain::{EntityId, EntityKind};
use rune::{Key, Mods, Runtime};

use super::shortcut_tests::hover;
use super::tests::{TempDir, click, command, demo_app, press, type_text};
use crate::menu::TaskAction;
use crate::state::{Overlay, State, View};

fn app() -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = demo_app();
    rt.frame(0.0);
    rt.settle_all();
    rt.frame(0.0);
    (rt, dir)
}

fn task_id(rt: &Runtime<State>, title: &str) -> EntityId {
    rt.state.snap.tasks.iter().find(|t| t.title == title).map(|t| t.id).expect("demo task")
}

fn quest_id(rt: &Runtime<State>, title: &str) -> EntityId {
    rt.state.snap.quests.iter().find(|q| q.title == title).map(|q| q.id).expect("demo quest")
}

fn in_trash(rt: &Runtime<State>, kind: EntityKind, id: EntityId) -> bool {
    rt.state.snap.trash.iter().any(|t| t.kind == kind && t.id == id)
}

fn toast(rt: &Runtime<State>) -> String {
    rt.elements()
        .into_iter()
        .find(|e| !e.exiting && e.key.as_deref() == Some("toast"))
        .and_then(|e| e.label)
        .unwrap_or_default()
}

/// The key of `action`'s row in the menu open on `task`.
fn menu_row(rt: &Runtime<State>, task: EntityId, action: TaskAction) -> String {
    let t = rt.state.snap.task(task).expect("task");
    let has_quest = t.quest_id.and_then(|q| rt.state.snap.quest(q)).is_some();
    let index = super::menu::rows(t, has_quest).iter().position(|a| *a == Some(action)).expect("the menu offers it");
    format!("card-menu-item{index}")
}

fn undo(rt: &mut Runtime<State>) {
    command(rt, 'z');
}

const DRAFT: &str = "Draft this week’s essay";

#[test]
fn a_task_deleted_from_its_menu_comes_back_with_command_z() {
    let (mut rt, _dir) = app();
    let id = task_id(&rt, DRAFT);
    click(&mut rt, &format!("home-menu-{id}"));
    assert!(rt.find("card-menu-menu").is_some(), "the menu opens");
    let delete = menu_row(&rt, id, TaskAction::Delete);
    click(&mut rt, &delete);
    assert!(rt.state.snap.task(id).is_none());
    assert!(in_trash(&rt, EntityKind::Task, id));
    assert!(rt.find("card-menu-menu").is_none(), "the menu closed");
    assert_eq!(toast(&rt), format!("Deleted “{DRAFT}”"));
    undo(&mut rt);
    assert!(rt.state.snap.task(id).is_some(), "⌘Z restored it");
    assert!(rt.find("toast").is_none());
}

#[test]
fn backspace_deletes_the_card_under_the_pointer_and_the_toast_undoes_it() {
    let (mut rt, _dir) = app();
    let id = task_id(&rt, DRAFT);
    hover(&mut rt, id);
    assert_eq!(rt.state.hovered_task, Some(id));
    press(&mut rt, Key::Backspace, Mods::default());
    assert!(rt.state.snap.task(id).is_none());
    click(&mut rt, "toast-action");
    assert!(rt.state.snap.task(id).is_some());
}

#[test]
fn the_palette_deletes_a_task() {
    let (mut rt, _dir) = app();
    let id = task_id(&rt, DRAFT);
    command(&mut rt, 'k');
    type_text(&mut rt, "draft this");
    for _ in 0..3 {
        press(&mut rt, Key::Down, Mods::default());
    }
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.overlay, Overlay::None);
    assert!(rt.state.snap.task(id).is_none(), "the fourth entry deletes it");
}

#[test]
fn a_deleted_quest_takes_its_milestones_and_undo_brings_both_back() {
    let (mut rt, _dir) = app();
    let quest = rt.state.snap.quests[0].id;
    let milestones = rt.state.snap.milestones_of(quest).count();
    assert!(milestones > 0, "the demo quest has milestones");
    rt.state.go(View::Quest(quest));
    rt.frame(0.0);
    click(&mut rt, "quest-delete");
    assert_eq!(rt.state.view, View::Quests, "its page closes");
    assert!(rt.state.snap.quest(quest).is_none());
    assert_eq!(rt.state.snap.milestones_of(quest).count(), 0);
    click(&mut rt, "toast-action");
    assert!(rt.state.snap.quest(quest).is_some());
    assert_eq!(rt.state.snap.milestones_of(quest).count(), milestones);
}

#[test]
fn a_focused_session_is_deleted_with_backspace_and_undone() {
    let (mut rt, _dir) = app();
    let today = rt.state.today();
    let entry = rt
        .state
        .snap
        .time_entries
        .iter()
        .find(|e| !e.is_running() && crate::clock::local_day(e.started_at) == today)
        .map(|e| e.id)
        .expect("a session today");
    rt.update(|_, tx| tx.focus_when_shown(format!("home-grid-entry-{entry}"), true));
    rt.frame(0.0);
    press(&mut rt, Key::Backspace, Mods::default());
    assert!(in_trash(&rt, EntityKind::TimeEntry, entry));
    undo(&mut rt);
    assert!(rt.state.snap.time_entries.iter().any(|e| e.id == entry));
}

#[test]
fn the_archive_restores_deleted_things_and_unarchives_quests() {
    let (mut rt, _dir) = app();
    let id = task_id(&rt, DRAFT);
    rt.state.delete(crate::undo::Deletable::Task(id));
    let quest = quest_id(&rt, &rt.state.snap.quests[1].title.clone());
    rt.state.dispatch(quest_domain::Command::SetQuestStatus { id: quest, status: quest_domain::QuestStatus::Archived });
    command(&mut rt, '5');
    click(&mut rt, "quests-archive");
    assert_eq!(rt.state.view, View::Archive);
    assert!(rt.find(&format!("archive-item-{id}")).is_some(), "the deleted task is listed");
    click(&mut rt, &format!("archive-restore-{id}"));
    assert!(rt.state.snap.task(id).is_some());
    assert_eq!(rt.state.undo.last, None, "⌘Z no longer refers to it");
    click(&mut rt, &format!("archive-unarchive-{quest}"));
    assert_eq!(rt.state.snap.quest(quest).map(|q| q.status), Some(quest_domain::QuestStatus::Active));
}
