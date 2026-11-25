use chrono::{DateTime, Duration, Utc};
use quest_view::undo::Deletable;

use super::*;
use crate::test_support::{at, id};

fn t0() -> DateTime<Utc> {
    at("2026-11-25T10:00:00+01:00")
}

#[test]
fn a_delete_shows_its_toast_and_undo_restores_it() {
    let mut undo = MobileUndo::default();
    undo.deleted(Deletable::Task(id(1)), "Draft".into(), t0());
    let toast = undo.toast().unwrap();
    assert_eq!(toast.text, "Deleted “Draft”");
    assert_eq!(toast.action_label, "Undo");
    assert_eq!(toast.expires_at_ms, (t0() + Duration::seconds(6)).timestamp_millis());
    assert_eq!(undo.undo(t0()), Some(UndoAction::Restore(Deletable::Task(id(1)))));
    assert_eq!(undo.toast(), None);
    assert_eq!(undo.undo(t0()), None, "only once");
}

fn completion(task: u128, previous: Option<DateTime<Utc>>) -> TaskEdit {
    TaskEdit::Completion { id: id(task), previous, previous_plan: None }
}

#[test]
fn a_swipe_completion_undoes_to_open_and_a_reopen_to_the_original_completion() {
    let mut undo = MobileUndo::default();
    let complete = completion(1, None);
    let reopen = completion(1, Some(t0()));
    undo.toggled(complete.clone(), "Draft", t0());
    assert_eq!(undo.toast().unwrap().text, "Completed “Draft”");
    assert_eq!(undo.undo(t0()), Some(UndoAction::Task(complete)));
    undo.toggled(reopen.clone(), "Draft", t0() + Duration::minutes(1));
    assert_eq!(undo.toast().unwrap().text, "Reopened “Draft”");
    assert_eq!(undo.undo(t0()), Some(UndoAction::Task(reopen)));
}

#[test]
fn a_completion_replaces_the_delete_and_remains_undoable_after_the_toast() {
    let mut undo = MobileUndo::default();
    undo.deleted(Deletable::Task(id(1)), "Old".into(), t0());
    let edit = completion(2, None);
    undo.toggled(edit.clone(), "Done", t0());
    assert_eq!(undo.expires(), Some(t0() + Duration::seconds(6)));
    undo.tick(t0() + Duration::seconds(5));
    assert!(undo.toast().is_some());
    undo.tick(t0() + Duration::seconds(6));
    assert_eq!(undo.toast(), None);
    assert_eq!(undo.expires(), None);
    assert_eq!(undo.undo(t0() + Duration::seconds(7)), Some(UndoAction::Task(edit)));
    assert_eq!(undo.undo(t0()), None);
}

#[test]
fn dismissing_hides_the_toast_but_keeps_the_slot() {
    let mut undo = MobileUndo::default();
    let edit = completion(2, None);
    undo.toggled(edit.clone(), "Done", t0());
    undo.dismiss(t0());
    assert_eq!(undo.toast(), None);
    assert_eq!(undo.undo(t0()), Some(UndoAction::Task(edit)));
}

#[test]
fn a_quest_completion_shows_its_toast_and_undo_puts_the_status_back() {
    use quest_domain::QuestStatus::{Active, Completed};
    let mut undo = MobileUndo::default();
    undo.quest_status_changed(id(1), "Garden".into(), Active, Completed, t0());
    assert_eq!(undo.toast().unwrap().text, "Completed quest “Garden”");
    assert_eq!(undo.undo(t0()), Some(UndoAction::SetQuestStatus { quest: id(1), status: Active }));
    assert_eq!(undo.toast(), None);
    undo.quest_status_changed(id(1), "Garden".into(), Completed, Active, t0());
    assert_eq!(undo.toast().unwrap().text, "Reopened quest “Garden”");
    undo.tick(t0() + Duration::seconds(7));
    assert_eq!(undo.toast(), None);
    assert_eq!(
        undo.undo(t0() + Duration::seconds(8)),
        Some(UndoAction::SetQuestStatus { quest: id(1), status: Completed }),
        "like ⌘Z, also after the toast went"
    );
}
