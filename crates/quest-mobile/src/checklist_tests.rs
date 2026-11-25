use quest_domain::{ChecklistItem, Command, Position, Snapshot};

use super::*;
use crate::test_support::{id, task};

fn item(n: u128, text: &str, done: bool) -> ChecklistItem {
    ChecklistItem { id: id(n), task_id: id(1), text: text.into(), done, sort_key: format!("{n:04}") }
}

#[test]
fn rows_list_the_task_s_items_in_order() {
    let snapshot = Snapshot {
        tasks: vec![task(1, "Pack", None, None)],
        checklist: vec![item(10, "Tent", true), item(11, "Stove", false)],
        ..Snapshot::default()
    };
    let rows = rows(&snapshot, id(1));
    let shown: Vec<_> = rows.iter().map(|r| (r.text.as_str(), r.done)).collect();
    assert_eq!(shown, [("Tent", true), ("Stove", false)]);
    assert_eq!(rows[0].id, id(10).to_string());
    assert!(super::rows(&snapshot, id(2)).is_empty());
}

#[test]
fn adding_trims_the_text_and_refuses_a_blank_one() {
    let before = ChecklistPosition::Before { item_id: id(10).to_string() };
    assert_eq!(
        add(id(1), id(12), "  Map\nand compass ", &before).unwrap(),
        Command::AddChecklistItem {
            id: id(12),
            task_id: id(1),
            text: "Map and compass".into(),
            position: Position::Before(id(10))
        }
    );
    assert_eq!(add(id(1), id(12), " \n ", &ChecklistPosition::Top), Err(MobileError::EmptyTitle));
    let bad = ChecklistPosition::Before { item_id: "nope".into() };
    assert_eq!(add(id(1), id(12), "Map", &bad), Err(MobileError::InvalidId { value: "nope".into() }));
}

#[test]
fn renaming_or_ticking_to_what_it_is_saves_nothing() {
    let tent = item(10, "Tent", false);
    assert_eq!(rename(&tent, " Tent "), None);
    assert_eq!(rename(&tent, "  "), None);
    assert_eq!(rename(&tent, "Big tent"), Some(Command::RenameChecklistItem { id: id(10), text: "Big tent".into() }));
    assert_eq!(set_done(&tent, false), None);
    assert_eq!(set_done(&tent, true), Some(Command::SetChecklistItemDone { id: id(10), done: true }));
}
