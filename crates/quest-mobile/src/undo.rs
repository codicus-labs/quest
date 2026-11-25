//! The undo snackbar, pure. Deletes and quest completions go through the desktop's
//! `quest_view::undo::UndoState` unchanged. Mobile adds a case the desktop lacks: a swipe that
//! completes or reopens a task shows "Completed “…”" / "Reopened “…”" with Undo, which flips it
//! back. The core performs the returned `UndoAction`.

use chrono::{DateTime, Duration, NaiveDate, Utc};
use quest_domain::{EntityId, QuestStatus};
use quest_view::undo::{Deletable, Shown, TOAST_SECONDS, Toast, UndoEffect, UndoEvent, UndoState};

use crate::types::UndoToast;

/// What Undo asks the core to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UndoAction {
    Restore(Deletable),
    /// Reopens the task and, if completing it moved it to today, puts it back on `back_to`.
    Reopen {
        task: EntityId,
        back_to: Option<NaiveDate>,
    },
    Complete(EntityId),
    /// Puts a completed or reopened quest back to `status`.
    SetQuestStatus {
        quest: EntityId,
        status: QuestStatus,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MobileUndo {
    deletes: UndoState,
    /// A mobile toast: a completion, with what its Undo does.
    own: Option<Own>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Own {
    action: UndoAction,
    text: String,
    until: DateTime<Utc>,
}

impl MobileUndo {
    pub fn deleted(&mut self, item: Deletable, title: String, now: DateTime<Utc>) {
        self.own = None;
        self.step(UndoEvent::Deleted { item, title }, now);
    }

    /// Restored from the archive: a toast or ⌘Z about it no longer applies.
    pub fn restored(&mut self, item: Deletable, now: DateTime<Utc>) {
        self.step(UndoEvent::Restored(item), now);
    }

    /// A quest went `from` → `to`: "Completed quest “…”" / "Reopened quest “…”" with Undo.
    pub fn quest_status_changed(
        &mut self,
        quest: EntityId,
        title: String,
        from: QuestStatus,
        to: QuestStatus,
        now: DateTime<Utc>,
    ) {
        self.own = None;
        self.step(UndoEvent::QuestStatusChanged { quest, title, from, to }, now);
    }

    /// `moved_from`: the day the completion moved the task away from, if it did.
    pub fn toggled(
        &mut self,
        task: EntityId,
        completed: bool,
        moved_from: Option<NaiveDate>,
        title: String,
        now: DateTime<Utc>,
    ) {
        let (verb, action) = if completed {
            ("Completed", UndoAction::Reopen { task, back_to: moved_from })
        } else {
            ("Reopened", UndoAction::Complete(task))
        };
        self.show_own(action, format!("{verb} “{title}”"), now);
    }

    fn show_own(&mut self, action: UndoAction, text: String, now: DateTime<Utc>) {
        self.step(UndoEvent::Dismiss, now);
        let until = now + Duration::seconds(TOAST_SECONDS);
        self.own = Some(Own { action, text, until });
    }

    /// The toast's Undo: flips a shown completion back, else
    /// takes back the last delete or quest completion (also after its toast went, like the
    /// desktop's ⌘Z). A mobile
    /// toast whose time ran out but that a tick has not removed yet is still what the user
    /// tapped: it is used up and nothing else is undone.
    pub fn undo(&mut self, now: DateTime<Utc>) -> Option<UndoAction> {
        if let Some(own) = self.own.take() {
            return (now < own.until).then_some(own.action);
        }
        match self.step(UndoEvent::Undo, now) {
            UndoEffect::Restore(item) => Some(UndoAction::Restore(item)),
            UndoEffect::SetQuestStatus { quest, status } => Some(UndoAction::SetQuestStatus { quest, status }),
            UndoEffect::None => None,
        }
    }

    pub fn dismiss(&mut self, now: DateTime<Utc>) {
        self.own = None;
        self.step(UndoEvent::Dismiss, now);
    }

    /// Time passed: a toast whose time is up goes.
    pub fn tick(&mut self, now: DateTime<Utc>) {
        self.own = self.own.take().filter(|t| now < t.until);
        self.step(UndoEvent::Tick, now);
    }

    /// When the toast on screen goes by itself.
    pub fn expires(&self) -> Option<DateTime<Utc>> {
        self.own.as_ref().map(|t| t.until).or_else(|| self.deletes.expires())
    }

    pub fn toast(&self) -> Option<UndoToast> {
        let (text, until) = match (&self.own, &self.deletes.shown) {
            (Some(own), _) => (own.text.clone(), own.until),
            (None, Some(Shown { toast: Toast::Deleted { title, .. }, until })) => {
                (format!("Deleted “{title}”"), *until)
            }
            (None, Some(Shown { toast: Toast::QuestStatus { title, status, .. }, until })) => {
                let verb = if *status == QuestStatus::Completed { "Completed" } else { "Reopened" };
                (format!("{verb} quest “{title}”"), *until)
            }
            (None, _) => return None,
        };
        Some(UndoToast { text, action_label: "Undo".to_owned(), expires_at_ms: until.timestamp_millis() })
    }

    fn step(&mut self, event: UndoEvent, now: DateTime<Utc>) -> UndoEffect {
        let (next, effect) = self.deletes.step(event, now);
        self.deletes = next;
        effect
    }
}

#[cfg(test)]
#[path = "undo_tests.rs"]
mod tests;
