//! `QuestCore`'s sessions: organizing, Log work and the entry editor. Memory-only like the rest
//! of the core; decisions live in `work_forms`.

use quest_domain::{Command, EntityId};

use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::ids::parse_id;
use crate::time::Now;
use crate::types::Moment;
use crate::work_forms;
use crate::work_types::{EntryCheck, EntryDraft, LogWorkCheck, LogWorkDraft, TaskChoices};

#[uniffi::export]
impl QuestCore {
    /// Starts organizing (time between tasks, no task and no quest); whatever runs stops.
    /// `origin_task_id`: the task to go back to ("Back to …"), e.g. the one that was running.
    /// Returns the new session's id.
    pub fn start_organizing(&self, origin_task_id: Option<String>, now: Moment) -> Result<String, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let origin_task_id = origin_task_id.as_deref().map(parse_id).transpose()?;
            let entry_id = EntityId::new();
            self.dispatch(Command::StartOrganizing { entry_id, origin_task_id, at: now.at })?;
            Ok(entry_id.to_string())
        })
    }

    /// Books the running task session as organizing from `since_ms` (epoch ms) on: the task
    /// session ends there and organizing (back to that task) runs from there. `Rejected` when no
    /// task session runs or `since_ms` is not within it. Returns the organizing session's id.
    pub fn rebook_as_organizing(&self, since_ms: i64, now: Moment) -> Result<String, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let since = chrono::DateTime::from_timestamp_millis(since_ms)
                .ok_or(MobileError::InvalidMoment { epoch_ms: since_ms })?;
            let entry_id = EntityId::new();
            self.dispatch(Command::RebookAsOrganizing { entry_id, since, now: now.at })?;
            Ok(entry_id.to_string())
        })
    }

    /// Tasks to pick for Log work and the entry editor (`query`: what was typed).
    pub fn task_choices(&self, query: String) -> Result<TaskChoices, MobileError> {
        guard(|| Ok(work_forms::task_choices(&self.store().snapshot(), &query)))
    }

    /// Log work from the button: the quarter hour that just ended, today.
    pub fn log_work_now(&self, now: Moment) -> Result<LogWorkDraft, MobileError> {
        guard(|| Ok(work_forms::log_work_now(Now::parse(&now)?)))
    }

    /// The Log work sheet's labels, quest and what saving would refuse.
    pub fn check_log_work(&self, draft: LogWorkDraft, now: Moment) -> Result<LogWorkCheck, MobileError> {
        guard(|| work_forms::check_log_work(&self.store().snapshot(), &draft, Now::parse(&now)?))
    }

    /// Saves the session (a new title becomes a completed task). A wrong field is
    /// `InvalidForm` with the desktop's message. Returns the new entry's id.
    pub fn log_work(&self, draft: LogWorkDraft, now: Moment) -> Result<String, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let entry_id = EntityId::new();
            let command = work_forms::decide_log_work(&self.store().snapshot(), &draft, entry_id, now)
                .map_err(|e| MobileError::InvalidForm { reason: e.message })?;
            self.dispatch(command)?;
            Ok(entry_id.to_string())
        })
    }

    /// The entry editor's fields as it opens. `UnknownEntry` once the session is gone.
    pub fn entry_draft(&self, entry_id: String, now: Moment) -> Result<EntryDraft, MobileError> {
        guard(|| work_forms::entry_draft(&self.store().snapshot(), &entry_id, Now::parse(&now)?))
    }

    /// The entry editor's labels, the task's quest, the duration and what saving would refuse.
    pub fn check_entry(&self, draft: EntryDraft, now: Moment) -> Result<EntryCheck, MobileError> {
        guard(|| work_forms::check_entry(&self.store().snapshot(), &draft, Now::parse(&now)?))
    }

    /// Saves the times and the task; the session counts towards that task's quest. An organizing
    /// session given a task becomes task work.
    pub fn save_entry(&self, draft: EntryDraft, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let command = work_forms::decide_entry(&self.store().snapshot(), &draft, now)?;
            self.dispatch(command)
        })
    }

    /// Deletes the session softly; the snackbar offers to undo it.
    pub fn delete_entry(&self, entry_id: String, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let (item, title) = work_forms::delete_entry(&self.store().snapshot(), &entry_id)?;
            self.dispatch(item.delete(now.at))?;
            self.undo_state().deleted(item, title, now.at);
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "core_work_tests.rs"]
mod tests;
