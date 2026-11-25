use chrono::{DateTime, Duration, Utc};
use quest_domain::{Command, EntityId, EntityKind, QuestStatus};

use super::*;

fn t0() -> DateTime<Utc> {
    "2026-10-15T12:40:00Z".parse().unwrap()
}

fn deleted(item: Deletable) -> UndoEvent {
    UndoEvent::Deleted { item, title: "Draft".into() }
}

#[test]
fn a_delete_shows_a_toast_for_six_seconds() {
    let item = Deletable::Task(EntityId::new());
    let (state, effect) = UndoState::default().step(deleted(item), t0());
    assert_eq!(effect, UndoEffect::None);
    assert_eq!(state.expires(), Some(t0() + Duration::seconds(6)));
    let (still, _) = state.step(UndoEvent::Tick, t0() + Duration::seconds(5));
    assert!(still.shown.is_some());
    let (gone, _) = state.step(UndoEvent::Tick, t0() + Duration::seconds(6));
    assert_eq!(gone.shown, None);
    assert_eq!(gone.last, Some(Undoable::Delete(item)), "⌘Z still knows the last delete after the toast went");
}

#[test]
fn undo_restores_the_last_delete_once() {
    let first = Deletable::Quest(EntityId::new());
    let second = Deletable::ChecklistItem(EntityId::new());
    let (state, _) = UndoState::default().step(deleted(first), t0());
    let (state, _) = state.step(deleted(second), t0());
    let (state, effect) = state.step(UndoEvent::Undo, t0());
    assert_eq!(effect, UndoEffect::Restore(second));
    assert_eq!(state.shown, None, "the toast about it goes");
    let (_, again) = state.step(UndoEvent::Undo, t0());
    assert_eq!(again, UndoEffect::None, "one level of undo");
}

#[test]
fn restoring_from_the_archive_forgets_the_undo() {
    let item = Deletable::Entry(EntityId::new());
    let (state, _) = UndoState::default().step(deleted(item), t0());
    let (state, _) = state.step(UndoEvent::Restored(item), t0());
    assert_eq!(state, UndoState::default());
}

#[test]
fn deletables_map_to_commands() {
    let id = EntityId::new();
    assert_eq!(Deletable::Entry(id).delete(t0()), Command::DeleteEntry { id, at: t0() });
    assert_eq!(Deletable::Quest(id).restore(), Command::Restore { kind: EntityKind::Quest, id });
    assert_eq!(Deletable::from_kind(EntityKind::ChecklistItem, id), Some(Deletable::ChecklistItem(id)));
    assert_eq!(Deletable::from_kind(EntityKind::Settings, id), None);
}

#[test]
fn undo_puts_a_completed_quest_back_and_a_later_delete_takes_over() {
    let quest = EntityId::new();
    let completed = UndoEvent::QuestStatusChanged {
        quest,
        title: "Run".into(),
        from: QuestStatus::Active,
        to: QuestStatus::Completed,
    };
    let (state, _) = UndoState::default().step(completed.clone(), t0());
    assert!(matches!(
        &state.shown,
        Some(Shown { toast: Toast::QuestStatus { status: QuestStatus::Completed, .. }, .. })
    ));
    let (undone, effect) = state.step(UndoEvent::Undo, t0());
    assert_eq!(effect, UndoEffect::SetQuestStatus { quest, status: QuestStatus::Active });
    assert_eq!(undone.shown, None, "its toast goes");

    let task = Deletable::Task(EntityId::new());
    let (state, _) = state.step(deleted(task), t0());
    let (_, effect) = state.step(UndoEvent::Undo, t0());
    assert_eq!(effect, UndoEffect::Restore(task), "⌘Z takes back the newest change");
}
