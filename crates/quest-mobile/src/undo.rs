//! The undo snackbar, pure. Deletes and quest completions go through the desktop's
//! `quest_view::undo::UndoState` unchanged. Mobile adds a case the desktop lacks: a swipe that
//! completes or reopens a task shows "Completed “…”" / "Reopened “…”" with Undo, which flips it
//! back. The core performs the returned `UndoAction`.

use chrono::{DateTime, Duration, Utc};
use quest_domain::{EntityId, QuestStatus, Task};
use quest_view::undo::{Deletable, Shown, TOAST_SECONDS, TaskEdit, Toast, UndoEffect, UndoEvent, UndoState};

use crate::types::UndoToast;

/// What Undo asks the core to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum UndoAction {
    Task(TaskEdit),
    Restore(Deletable),
    /// Puts a completed or reopened quest back to `status`.
    SetQuestStatus {
        quest: EntityId,
        status: QuestStatus,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct MobileUndo {
    deletes: UndoState,
    completion_label: Option<(String, DateTime<Utc>)>,
    lingering: Vec<(Task, DateTime<Utc>)>,
}

impl MobileUndo {
    pub fn deleted(&mut self, item: Deletable, title: String, now: DateTime<Utc>) {
        self.completion_label = None;
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
        self.completion_label = None;
        self.step(UndoEvent::QuestStatusChanged { quest, title, from, to }, now);
    }

    pub fn toggled(&mut self, edit: TaskEdit, title: &str, now: DateTime<Utc>) {
        let verb = if matches!(edit, TaskEdit::Completion { previous: None, .. }) { "Completed" } else { "Reopened" };
        self.task_edited(edit, now);
        self.completion_label = Some((format!("{verb} “{title}”"), now + Duration::seconds(TOAST_SECONDS)));
    }

    pub fn list_toggled(&mut self, edit: TaskEdit, before: Task, now: DateTime<Utc>) {
        self.lingering.retain(|(task, _)| task.id != before.id);
        if before.is_done() {
            self.toggled(edit, &before.title, now);
            return;
        }
        self.task_edited(edit, now);
        self.completion_label = Some(("Done".into(), now + Duration::seconds(5)));
        self.lingering.push((before, now));
    }

    pub fn lingering(&self, task: &Task, now: DateTime<Utc>) -> Option<&Task> {
        self.lingering.iter().find_map(|(before, at)| {
            (before.id == task.id && quest_view::undo::lingering(*at, task, now)).then_some(before)
        })
    }

    pub fn next_wake(&self) -> Option<DateTime<Utc>> {
        self.expires().into_iter().chain(self.lingering.iter().map(|(_, at)| quest_view::undo::linger_until(*at))).min()
    }

    pub fn undo(&mut self, now: DateTime<Utc>) -> Option<UndoAction> {
        self.completion_label = None;
        match self.step(UndoEvent::Undo, now) {
            UndoEffect::Restore(item) => Some(UndoAction::Restore(item)),
            UndoEffect::SetQuestStatus { quest, status } => Some(UndoAction::SetQuestStatus { quest, status }),
            UndoEffect::Task(edit) => Some(UndoAction::Task(edit)),
            UndoEffect::None => None,
        }
    }

    pub fn task_edited(&mut self, edit: TaskEdit, now: DateTime<Utc>) {
        self.completion_label = None;
        self.step(UndoEvent::TaskEdited(edit), now);
    }

    pub fn dismiss(&mut self, now: DateTime<Utc>) {
        self.completion_label = None;
        self.step(UndoEvent::Dismiss, now);
    }

    /// Time passed: a toast whose time is up goes.
    pub fn tick(&mut self, now: DateTime<Utc>) {
        self.lingering.retain(|(_, at)| now < quest_view::undo::linger_until(*at));
        self.completion_label = self.completion_label.take().filter(|(_, until)| now < *until);
        self.step(UndoEvent::Tick, now);
    }

    /// When the toast on screen goes by itself.
    pub fn expires(&self) -> Option<DateTime<Utc>> {
        self.completion_label.as_ref().map(|(_, until)| *until).or_else(|| self.deletes.expires())
    }

    pub fn toast(&self) -> Option<UndoToast> {
        let (text, until) = match (&self.completion_label, &self.deletes.shown) {
            (Some((text, until)), _) => (text.clone(), *until),
            (None, Some(Shown { toast: Toast::Deleted { title, .. }, until })) => {
                (format!("Deleted “{title}”"), *until)
            }
            (None, Some(Shown { toast: Toast::QuestStatus { title, status, .. }, until })) => {
                let verb = if *status == QuestStatus::Completed { "Completed" } else { "Reopened" };
                (format!("{verb} quest “{title}”"), *until)
            }
            (None, Some(Shown { toast: Toast::Done, until })) => ("Done".into(), *until),
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
