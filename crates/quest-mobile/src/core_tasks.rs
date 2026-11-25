//! `QuestCore`'s task actions: the card (rename, timer, delete, undo) and the task sheet.
//! Memory-only like the rest of the core; decisions live in the pure modules.

use quest_domain::{Command, EntityId, Snapshot, TaskList};
use quest_store::SyncStatus;
use quest_view::undo::{Deletable, TaskEdit};

use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::ids::{parse_id, parse_iso_day};
use crate::task_types::{RepeatCheck, RepeatDraft, RepeatWeekday, TaskDetail};
use crate::time::Now;
use crate::types::Moment;
use crate::undo::UndoAction;
use crate::{commands, repeat, task_detail};

impl QuestCore {
    pub(crate) fn change_task_dates(
        &self,
        task: &quest_domain::Task,
        command: Option<Command>,
        now: Now,
    ) -> Result<(), MobileError> {
        let Some(command) = command else { return Ok(()) };
        let before = self.store().snapshot();
        let placement = match &command {
            Command::PlaceTask { placement, .. } => Some(placement.list()),
            _ => None,
        };
        self.dispatch(command)?;
        self.remember_task_date_edit(task, &before, placement, now);
        Ok(())
    }

    pub(crate) fn remember_task_date_edit(
        &self,
        task: &quest_domain::Task,
        before: &Snapshot,
        placement: Option<TaskList>,
        now: Now,
    ) {
        let after = self.store().snapshot();
        let edit = after.task(task.id).and_then(|updated| match placement {
            Some(list) => TaskEdit::placement(task, updated, before, &after, list, now.today()),
            None => TaskEdit::dates(task, updated, before, &after),
        });
        if let Some(edit) = edit {
            let mut undo = self.undo_state();
            if placement.is_some() {
                undo.task_placed(edit, now.at);
            } else {
                undo.task_edited(edit, now.at);
            }
        }
    }
}

#[uniffi::export]
impl QuestCore {
    /// Renames as the desktop does: line breaks become spaces; an empty or unchanged title is
    /// not saved (no error: the UI shows the stored title again).
    pub fn rename_task(&self, task_id: String, title: String) -> Result<(), MobileError> {
        guard(|| self.dispatch_some(commands::rename_task(&self.task(&task_id)?, &title)))
    }

    /// Starts the task's timer; a timer running on another task stops. The task may be in another
    /// workspace than the shown one (Resume on the focus notification).
    pub fn start_timer(&self, task_id: String, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let (snapshot, task) = match self.task(&task_id) {
                Ok(task) => (self.store().snapshot(), task),
                Err(unknown) => self.task_anywhere(parse_id(&task_id)?).ok_or(unknown)?,
            };
            match commands::start_timer(&snapshot, &task, EntityId::new(), now)? {
                Some(command) => self.dispatch_there(&snapshot, command),
                None => Ok(()),
            }
        })
    }

    /// Stops the running timer, if any.
    pub fn stop_timer(&self, now: Moment) -> Result<(), MobileError> {
        guard(|| self.dispatch(Command::StopFocus { at: Now::parse(&now)?.at }))
    }

    /// Deletes the task softly; the snackbar offers to undo it.
    pub fn delete_task(&self, task_id: String, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let task = self.task(&task_id)?;
            let item = Deletable::Task(task.id);
            self.dispatch(item.delete(now.at))?;
            self.undo_state().deleted(item, task.title, now.at);
            Ok(())
        })
    }

    /// The snackbar's Undo: restores a delete, or flips a completion back. Without a snackbar
    /// it restores the last delete (the desktop's ⌘Z), else does nothing.
    pub fn undo(&self, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let Some(action) = self.undo_state().undo(now.at) else { return Ok(()) };
            match action {
                UndoAction::UndoRebook(entry_id) => {
                    self.dispatch(Command::UndoOrganizingRebook { entry_id, at: now.at })
                }
                UndoAction::Task(edit) => self.dispatch(edit.command(now.at)),
                UndoAction::Restore(item) => self.dispatch(item.restore()),
                UndoAction::SetChecklistDone { id, done } => {
                    self.dispatch(quest_domain::Command::SetChecklistItemDone { id, done })
                }
                UndoAction::SetQuestStatus { quest, status } => {
                    self.dispatch(Command::SetQuestStatus { id: quest, status })
                }
            }
        })
    }

    /// Closes the snackbar.
    pub fn dismiss_toast(&self, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            self.undo_state().dismiss(Now::parse(&now)?.at);
            Ok(())
        })
    }

    /// The task sheet. A task deleted meanwhile is `UnknownTask`: close the sheet.
    pub fn task_detail(&self, task_id: String, now: Moment) -> Result<TaskDetail, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let task = self.task(&task_id)?;
            Ok(task_detail::task_detail(&self.store().snapshot(), &task, now))
        })
    }

    /// Saves the plain text description; unchanged saves nothing.
    pub fn set_task_description(&self, task_id: String, description: String) -> Result<(), MobileError> {
        guard(|| self.dispatch_some(commands::set_description(&self.task(&task_id)?, &description)))
    }

    /// Sets the task's quest; `None` is "No quest".
    pub fn set_task_quest(&self, task_id: String, quest_id: Option<String>) -> Result<(), MobileError> {
        guard(|| {
            let task = self.task(&task_id)?;
            let Some(command) = commands::set_quest(&task, quest_id.as_deref())? else {
                return Ok(());
            };
            self.dispatch(command)?;
            self.undo_state()
                .task_edited(TaskEdit::Quest { id: task.id, previous: task.quest_id }, chrono::DateTime::UNIX_EPOCH);
            Ok(())
        })
    }

    /// Plans the task for `planned_on` ("2026-11-24"), or unplans it with `None`, at the bottom
    /// of that day's list. A plan date after the deadline takes the deadline along; moving a date
    /// later counts as a postponement.
    pub fn set_task_planned(
        &self,
        task_id: String,
        planned_on: Option<String>,
        now: Moment,
    ) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let planned_on = planned_on.as_deref().map(parse_iso_day).transpose()?;
            let task = self.task(&task_id)?;
            self.change_task_dates(&task, commands::set_planned(&task, planned_on, now), now)
        })
    }

    /// Sets the deadline `due_on` ("2026-11-24"), or clears it with `None`. A deadline before the
    /// plan date takes the plan date along; moving a date later counts as a postponement.
    pub fn set_task_due(&self, task_id: String, due_on: Option<String>, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let due_on = due_on.as_deref().map(parse_iso_day).transpose()?;
            let task = self.task(&task_id)?;
            self.change_task_dates(&task, commands::set_due(&task, due_on, now), now)
        })
    }

    /// The Repeat dialog's fields as it opens on the task.
    pub fn repeat_draft(&self, task_id: String, now: Moment) -> Result<RepeatDraft, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let task = self.task(&task_id)?;
            Ok(repeat::draft_for(&self.store().snapshot(), &task, now))
        })
    }

    /// Saves the repeat on the task's series; a wrong field is `InvalidRepeat` with the
    /// desktop's message. Occurrences due now appear right away.
    pub fn set_task_repeat(&self, task_id: String, draft: RepeatDraft, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let rule = repeat::rule(&draft, now.today())?;
            self.change_repeat(&task_id, Some(rule), now)
        })
    }

    /// "Stop repeating".
    pub fn clear_task_repeat(&self, task_id: String, now: Moment) -> Result<(), MobileError> {
        guard(|| self.change_repeat(&task_id, None, Now::parse(&now)?))
    }
}

/// The Repeat dialog's live state while typing: picked weekdays, placeholder, summary, error.
#[uniffi::export]
pub fn check_repeat(draft: RepeatDraft, now: Moment) -> Result<RepeatCheck, MobileError> {
    guard(|| Ok(repeat::check(&draft, Now::parse(&now)?.today())))
}

/// A press on a weekday chip of the Repeat dialog; the last picked day stays picked.
#[uniffi::export]
pub fn toggle_repeat_weekday(draft: RepeatDraft, day: RepeatWeekday, now: Moment) -> Result<RepeatDraft, MobileError> {
    guard(|| Ok(repeat::toggle_weekday(draft, day, Now::parse(&now)?.today())))
}

impl QuestCore {
    pub(crate) fn dispatch_some(&self, command: Option<Command>) -> Result<(), MobileError> {
        command.map_or(Ok(()), |command| self.dispatch(command))
    }

    fn change_repeat(
        &self,
        task_id: &str,
        rule: Option<quest_domain::RepeatRule>,
        now: Now,
    ) -> Result<(), MobileError> {
        let task = self.task(task_id)?;
        let snapshot = self.store().snapshot();
        if rule.is_none() && quest_view::repeat::series_of(&snapshot, &task).repeat.is_none() {
            return Ok(());
        }
        let command = commands::set_repeat(&snapshot, &task, rule, now);
        self.dispatch(command)
            .map_err(|_| MobileError::InvalidRepeat { reason: "The repeat schedule could not be saved.".to_owned() })?;
        if rule.is_some() {
            self.generate_due_now(now);
        }
        Ok(())
    }

    /// As the desktop after saving a repeat: due occurrences now, unless still connecting.
    fn generate_due_now(&self, now: Now) {
        let store = self.store();
        if store.status() != SyncStatus::Connecting {
            let _refused = store.dispatch(Command::GenerateDueTasks { now: now.at });
        }
    }
}

#[cfg(test)]
#[path = "core_tasks_tests.rs"]
mod tests;
