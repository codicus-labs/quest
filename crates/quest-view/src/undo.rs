//! Deleting safely, pure: what can be deleted (`Deletable`), and the undo state behind the toast
//! at the bottom ("Deleted “…” · Undo" or "Completed quest “…” · Undo"). `UndoState::step`
//! decides; the app performs the `UndoEffect` it returns. Time is passed in.

#[cfg(test)]
mod tests;

use chrono::{DateTime, Duration, Utc};
use quest_domain::{Command, EntityId, EntityKind, QuestStatus};

/// How long a toast shows.
pub const TOAST_SECONDS: i64 = 6;

/// Something the user can delete (softly: it goes to the archive and can be restored).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deletable {
    Task(EntityId),
    Quest(EntityId),
    ChecklistItem(EntityId),
    Entry(EntityId),
}

impl Deletable {
    pub fn kind(self) -> EntityKind {
        match self {
            Deletable::Task(_) => EntityKind::Task,
            Deletable::Quest(_) => EntityKind::Quest,
            Deletable::ChecklistItem(_) => EntityKind::ChecklistItem,
            Deletable::Entry(_) => EntityKind::TimeEntry,
        }
    }

    pub fn id(self) -> EntityId {
        match self {
            Deletable::Task(id) | Deletable::Quest(id) | Deletable::ChecklistItem(id) | Deletable::Entry(id) => id,
        }
    }

    pub fn from_kind(kind: EntityKind, id: EntityId) -> Option<Deletable> {
        match kind {
            EntityKind::Task => Some(Deletable::Task(id)),
            EntityKind::Quest => Some(Deletable::Quest(id)),
            EntityKind::ChecklistItem => Some(Deletable::ChecklistItem(id)),
            EntityKind::TimeEntry => Some(Deletable::Entry(id)),
            EntityKind::Postponement | EntityKind::Settings | EntityKind::Workspace => None,
        }
    }

    pub fn delete(self, at: DateTime<Utc>) -> Command {
        match self {
            Deletable::Task(id) => Command::DeleteTask { id, at },
            Deletable::Quest(id) => Command::DeleteQuest { id, at },
            Deletable::ChecklistItem(id) => Command::DeleteChecklistItem { id, at },
            Deletable::Entry(id) => Command::DeleteEntry { id, at },
        }
    }

    pub fn restore(self) -> Command {
        Command::Restore { kind: self.kind(), id: self.id() }
    }
}

/// What ⌘Z takes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Undoable {
    Delete(Deletable),
    /// A quest was completed or reopened: it goes back to `back_to`.
    QuestStatus {
        quest: EntityId,
        back_to: QuestStatus,
    },
}

/// What the toast says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Toast {
    Deleted {
        item: Deletable,
        title: String,
    },
    /// A quest was completed or reopened; `status` is the one it has now.
    QuestStatus {
        quest: EntityId,
        title: String,
        status: QuestStatus,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub toast: Toast,
    pub until: DateTime<Utc>,
}

/// The last undoable change (⌘Z takes it back, also after its toast is gone) and the toast on
/// screen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UndoState {
    pub last: Option<Undoable>,
    pub shown: Option<Shown>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UndoEvent {
    Deleted {
        item: Deletable,
        title: String,
    },
    /// A quest went `from` → `to` (completed or reopened).
    QuestStatusChanged {
        quest: EntityId,
        title: String,
        from: QuestStatus,
        to: QuestStatus,
    },
    /// ⌘Z or the toast's Undo.
    Undo,
    /// Restored some other way (the archive): ⌘Z no longer refers to it.
    Restored(Deletable),
    Dismiss,
    /// Time passed: the toast goes once its time is up.
    Tick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndoEffect {
    None,
    Restore(Deletable),
    SetQuestStatus { quest: EntityId, status: QuestStatus },
}

impl UndoState {
    pub fn step(&self, event: UndoEvent, now: DateTime<Utc>) -> (UndoState, UndoEffect) {
        let until = now + Duration::seconds(TOAST_SECONDS);
        let mut next = self.clone();
        let effect = match event {
            UndoEvent::Deleted { item, title } => {
                next.last = Some(Undoable::Delete(item));
                next.shown = Some(Shown { toast: Toast::Deleted { item, title }, until });
                UndoEffect::None
            }
            UndoEvent::QuestStatusChanged { quest, title, from, to } => {
                next.last = Some(Undoable::QuestStatus { quest, back_to: from });
                next.shown = Some(Shown { toast: Toast::QuestStatus { quest, title, status: to }, until });
                UndoEffect::None
            }
            UndoEvent::Undo => match next.last.take() {
                None => UndoEffect::None,
                Some(Undoable::Delete(item)) => {
                    next.forget_toast_about(item);
                    UndoEffect::Restore(item)
                }
                Some(Undoable::QuestStatus { quest, back_to }) => {
                    if matches!(&next.shown, Some(Shown { toast: Toast::QuestStatus { quest: shown, .. }, .. }) if *shown == quest)
                    {
                        next.shown = None;
                    }
                    UndoEffect::SetQuestStatus { quest, status: back_to }
                }
            },
            UndoEvent::Restored(item) => {
                if next.last == Some(Undoable::Delete(item)) {
                    next.last = None;
                }
                next.forget_toast_about(item);
                UndoEffect::None
            }
            UndoEvent::Dismiss => {
                next.shown = None;
                UndoEffect::None
            }
            UndoEvent::Tick => {
                next.shown = next.shown.filter(|s| now < s.until);
                UndoEffect::None
            }
        };
        (next, effect)
    }

    /// When the toast on screen goes by itself, if one shows.
    pub fn expires(&self) -> Option<DateTime<Utc>> {
        self.shown.as_ref().map(|s| s.until)
    }

    fn forget_toast_about(&mut self, item: Deletable) {
        if matches!(&self.shown, Some(Shown { toast: Toast::Deleted { item: shown, .. }, .. }) if *shown == item) {
            self.shown = None;
        }
    }
}
