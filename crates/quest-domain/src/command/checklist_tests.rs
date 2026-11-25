use chrono::{TimeZone, Utc};

use super::*;
use crate::test_support::Replica;

fn task(r: &mut Replica) -> EntityId {
    let id = EntityId::new();
    r.run(Command::CreateTask {
        id,
        title: "Trip".into(),
        quest_id: None,
        planned_on: None,
        due_on: None,
        position: Position::Bottom,
    });
    id
}

fn texts(r: &Replica, task: EntityId) -> Vec<(&str, bool)> {
    r.snapshot.checklist_of(task).map(|i| (i.text.as_str(), i.done)).collect()
}

#[test]
fn items_are_added_renamed_ticked_moved_and_deleted() {
    let mut r = Replica::new();
    let trip = task(&mut r);
    let [a, b, c] = [EntityId::new(), EntityId::new(), EntityId::new()];
    let add = |id, text: &str, position| Command::AddChecklistItem { id, task_id: trip, text: text.into(), position };
    r.run(add(a, "Pack", Position::Bottom));
    r.run(add(b, "Book", Position::Top));
    r.run(add(c, " Go ", Position::Before(a)));
    assert_eq!(texts(&r, trip), [("Book", false), ("Go", false), ("Pack", false)]);

    r.run(Command::RenameChecklistItem { id: a, text: "Pack bags".into() });
    r.run(Command::SetChecklistItemDone { id: b, done: true });
    r.run(Command::MoveChecklistItem { id: b, position: Position::Bottom });
    assert_eq!(texts(&r, trip), [("Go", false), ("Pack bags", false), ("Book", true)]);
    r.run(Command::SetChecklistItemDone { id: b, done: false });
    assert_eq!(texts(&r, trip)[2], ("Book", false));

    let at = Utc.with_ymd_and_hms(2026, 11, 25, 9, 0, 0).unwrap();
    r.run(Command::DeleteChecklistItem { id: c, at });
    assert_eq!(texts(&r, trip).len(), 2);
    assert_eq!(r.snapshot.trash[0].title.as_deref(), Some("Go"));
    r.run(Command::Restore { kind: EntityKind::ChecklistItem, id: c });
    assert_eq!(texts(&r, trip)[0], ("Go", false));
}

#[test]
fn items_are_validated_and_hidden_with_their_task() {
    let mut r = Replica::new();
    let trip = task(&mut r);
    let (item, missing) = (EntityId::new(), EntityId::new());
    let blank = Command::AddChecklistItem { id: item, task_id: trip, text: "  ".into(), position: Position::Top };
    assert_eq!(plan(&blank, &r.snapshot), Err(CommandError::EmptyTitle));
    let orphan = Command::AddChecklistItem { id: item, task_id: missing, text: "x".into(), position: Position::Top };
    assert_eq!(plan(&orphan, &r.snapshot), Err(CommandError::UnknownTask(missing)));
    let rename = Command::RenameChecklistItem { id: missing, text: "x".into() };
    assert_eq!(plan(&rename, &r.snapshot), Err(CommandError::UnknownChecklistItem(missing)));

    r.run(Command::AddChecklistItem { id: item, task_id: trip, text: "Pack".into(), position: Position::Top });
    let at = Utc.with_ymd_and_hms(2026, 11, 25, 9, 0, 0).unwrap();
    r.run(Command::DeleteTask { id: trip, at });
    assert!(r.snapshot.checklist.is_empty());
    r.run(Command::Restore { kind: EntityKind::Task, id: trip });
    assert_eq!(texts(&r, trip), [("Pack", false)], "restoring the task brings its checklist back");
}

#[test]
fn ticking_on_one_device_and_renaming_on_another_both_stick() {
    let mut a = Replica::on_node(1);
    let trip = task(&mut a);
    let item = EntityId::new();
    a.run(Command::AddChecklistItem { id: item, task_id: trip, text: "Pack".into(), position: Position::Top });
    let mut b = Replica::on_node(2);
    b.merge(&a);
    a.run(Command::SetChecklistItemDone { id: item, done: true });
    b.run(Command::RenameChecklistItem { id: item, text: "Pack bags".into() });
    a.merge(&b);
    b.merge(&a);
    assert_eq!(a.snapshot.checklist, b.snapshot.checklist);
    assert_eq!(texts(&a, trip), [("Pack bags", true)]);
}
