use std::sync::Arc;

use super::*;
use crate::core::ChangeListener;
use crate::test_support::moment;
use crate::types::{AddPosition, HomeQuery, TaskSection};

struct Quiet;

impl ChangeListener for Quiet {
    fn on_change(&self) {}
}

fn open() -> (tempfile::TempDir, Arc<QuestCore>) {
    let dir = tempfile::tempdir().unwrap();
    let core =
        QuestCore::open(dir.path().display().to_string(), crate::test_support::tokens(), Arc::new(Quiet)).unwrap();
    (dir, core)
}

fn now() -> Moment {
    moment("2026-11-25T10:00:00+01:00")
}

fn texts(core: &QuestCore, task: &str) -> Vec<(String, bool)> {
    core.checklist(task.into()).unwrap().into_iter().map(|r| (r.text, r.done)).collect()
}

#[test]
fn a_checklist_is_added_to_ticked_moved_renamed_and_deleted_with_undo() {
    let (_dir, core) = open();
    let task =
        core.add_task("Pack".into(), TaskSection::Other, AddPosition::Bottom, HomeQuery::default(), now()).unwrap();
    let tent = core.add_checklist_item(task.clone(), "Tent".into(), ChecklistPosition::Bottom).unwrap();
    let stove = core.add_checklist_item(task.clone(), "Stove".into(), ChecklistPosition::Top).unwrap();
    assert_eq!(texts(&core, &task), [("Stove".into(), false), ("Tent".into(), false)]);

    core.set_checklist_item_done(tent.clone(), true).unwrap();
    core.move_checklist_item(tent.clone(), ChecklistPosition::Before { item_id: stove.clone() }).unwrap();
    core.rename_checklist_item(stove.clone(), "Gas stove".into()).unwrap();
    assert_eq!(texts(&core, &task), [("Tent".into(), true), ("Gas stove".into(), false)]);
    assert_eq!(core.task_detail(task.clone(), now()).unwrap().checklist.len(), 2);

    core.delete_checklist_item(tent.clone(), now()).unwrap();
    assert_eq!(texts(&core, &task), [("Gas stove".into(), false)]);
    assert_eq!(core.home(now(), HomeQuery::default()).unwrap().toast.unwrap().text, "Deleted “Tent”");
    core.undo(now()).unwrap();
    assert_eq!(texts(&core, &task).len(), 2);

    let gone = core.rename_checklist_item(id_of_nothing(), "x".into()).unwrap_err();
    assert!(matches!(gone, MobileError::UnknownChecklistItem { .. }));
    let blank = core.add_checklist_item(task, " ".into(), ChecklistPosition::Bottom).unwrap_err();
    assert_eq!(blank, MobileError::EmptyTitle);
}

fn id_of_nothing() -> String {
    EntityId::new().to_string()
}
