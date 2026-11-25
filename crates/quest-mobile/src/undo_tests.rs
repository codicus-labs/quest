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

#[test]
fn a_swipe_completion_undoes_to_open_and_a_reopen_to_done() {
    let mut undo = MobileUndo::default();
    undo.toggled(id(1), true, None, "Draft".into(), t0());
    assert_eq!(undo.toast().unwrap().text, "Completed “Draft”");
    assert_eq!(undo.undo(t0()), Some(UndoAction::Reopen { task: id(1), back_to: None }));
    undo.toggled(id(1), false, None, "Draft".into(), t0());
    assert_eq!(undo.toast().unwrap().text, "Reopened “Draft”");
    assert_eq!(undo.undo(t0()), Some(UndoAction::Complete(id(1))));
}

#[test]
fn toasts_go_after_six_seconds_and_undo_then_falls_back_to_the_last_delete() {
    let mut undo = MobileUndo::default();
    undo.deleted(Deletable::Task(id(1)), "Old".into(), t0());
    undo.toggled(id(2), true, None, "Done".into(), t0());
    assert_eq!(undo.expires(), Some(t0() + Duration::seconds(6)));
    undo.tick(t0() + Duration::seconds(5));
    assert!(undo.toast().is_some());
    undo.tick(t0() + Duration::seconds(6));
    assert_eq!(undo.toast(), None);
    assert_eq!(undo.expires(), None);
    assert_eq!(undo.undo(t0() + Duration::seconds(7)), Some(UndoAction::Restore(Deletable::Task(id(1)))));
}

#[test]
fn an_expired_completion_toast_not_yet_ticked_away_never_restores_an_old_delete() {
    let mut undo = MobileUndo::default();
    undo.deleted(Deletable::Task(id(1)), "Old".into(), t0());
    undo.toggled(id(2), true, None, "Done".into(), t0());
    assert_eq!(undo.undo(t0() + Duration::seconds(7)), None);
    assert_eq!(undo.toast(), None);
}

#[test]
fn dismissing_hides_the_toast() {
    let mut undo = MobileUndo::default();
    undo.toggled(id(2), true, None, "Done".into(), t0());
    undo.dismiss(t0());
    assert_eq!(undo.toast(), None);
    assert_eq!(undo.undo(t0()), None);
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
