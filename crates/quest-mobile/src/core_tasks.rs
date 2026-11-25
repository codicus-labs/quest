//! `QuestCore`'s task actions: the card (rename, timer, delete, undo), the task sheet and the
//! day grid. Memory-only like the rest of the core; decisions live in the pure modules.

use quest_domain::{Command, EntityId, Position};
use quest_store::SyncStatus;
use quest_view::undo::Deletable;

use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::ids::{parse_id, parse_iso_day};
use crate::task_types::{DayGrid, RepeatCheck, RepeatDraft, RepeatWeekday, TaskDetail};
use crate::time::Now;
use crate::types::Moment;
use crate::undo::{PutBack, UndoAction};
use crate::{commands, day_grid, repeat, task_detail};

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
                UndoAction::Restore(item) => self.dispatch(item.restore()),
                UndoAction::Reopen { task, back_to } => {
                    self.dispatch(Command::ReopenTask { id: task })?;
                    let Some(day) = back_to else { return Ok(()) };
                    self.dispatch(Command::MoveTask { id: task, day: Some(day), position: Position::Bottom })
                }
                UndoAction::Complete(id) => self.dispatch(Command::CompleteTask { id, at: now.at }),
                UndoAction::PutBack(PutBack::Entry { id, started_at, ended_at }) => {
                    // It was running: keep the end a stop has given it since (it can't run again).
                    let stopped = || self.store().snapshot().time_entries.iter().find(|e| e.id == id)?.ended_at;
                    let ended_at = ended_at.or_else(stopped);
                    self.dispatch(Command::SetEntryTimes { id, started_at, ended_at })
                }
                UndoAction::PutBack(PutBack::Block { id, day, block }) => {
                    self.dispatch(Command::ScheduleTask { id, day, block })
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
        guard(|| self.dispatch_some(commands::set_quest(&self.task(&task_id)?, quest_id.as_deref())?))
    }

    /// Moves the task to `day` ("2026-11-24"), or to the backlog with `None`, at the bottom of
    /// that list.
    pub fn set_task_day(&self, task_id: String, day: Option<String>, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            Now::parse(&now)?;
            let day = day.as_deref().map(parse_iso_day).transpose()?;
            self.dispatch_some(commands::set_day(&self.task(&task_id)?, day))
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

    /// The time grid of `day` ("2026-11-24"): sessions, planned blocks and, on today, the
    /// now-line.
    pub fn day_grid(&self, day: String, now: Moment) -> Result<DayGrid, MobileError> {
        guard(|| {
            let (day, now) = (parse_iso_day(&day)?, Now::parse(&now)?);
            Ok(day_grid::day_grid(&self.store().snapshot(), day, now))
        })
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
