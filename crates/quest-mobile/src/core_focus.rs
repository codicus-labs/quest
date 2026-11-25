//! `QuestCore`'s Focus: the screen, the running session for the notification, and Complete.

use quest_domain::Command;

use crate::core::QuestCore;
use crate::error::MobileError;
use crate::focus;
use crate::guard::guard;
use crate::ids::parse_id;
use crate::time::Now;
use crate::types::{FocusView, Moment, RunningSession};

#[uniffi::export]
impl QuestCore {
    /// The Focus screen. `focus_task` is the task started or picked last on this device; an
    /// unknown, deleted or done one is ignored.
    pub fn focus(&self, now: Moment, focus_task: Option<String>) -> Result<FocusView, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let focus_task = focus_task.as_deref().and_then(|id| parse_id(id).ok());
            Ok(focus::focus(&self.store().snapshot(), focus_task, now))
        })
    }

    /// The running timer's task, for the ongoing notification. None when no task's timer runs
    /// (a session without a task, or on a deleted one, has nothing to show). The timer is one per
    /// person: in another workspace than the shown one, it shows with that workspace's name.
    pub fn running_session(&self, now: Moment) -> Result<Option<RunningSession>, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let store = self.store();
            let snapshot = store.snapshot();
            let Some(running) = snapshot.running_focus() else { return Ok(None) };
            if running.workspace == snapshot.workspace {
                return Ok(focus::running_session(&snapshot, now, None));
            }
            let name = snapshot.workspace(running.workspace).map(|w| w.name.clone());
            let Some(there) = store.snapshot_in(running.workspace) else { return Ok(None) };
            Ok(focus::running_session(&there, now, name))
        })
    }

    /// Focus's Complete (the desktop's `complete_focus`): completes an open task, which stops
    /// its timer, with the same undo snackbar as the check. A done, deleted or unknown task is
    /// left alone (never reopened, as the check would). The notification's task may be in another
    /// workspace than the shown one; it completes there, without the snackbar.
    pub fn complete_task(&self, task_id: String, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let Ok(id) = parse_id(&task_id) else { return Ok(()) };
            let Some((there, task)) = self.task_anywhere(id).filter(|(_, t)| !t.is_done()) else {
                return Ok(());
            };
            self.dispatch_there(&there, Command::CompleteTask { id, at: now.at })?;
            if there.workspace == self.store().snapshot().workspace {
                self.undo_state().toggled(id, true, None, task.title, now.at);
            }
            Ok(())
        })
    }

    /// Whether a task is open (not done, not deleted) in any live workspace: a paused focus
    /// notification stays while its task is.
    pub fn is_task_open(&self, task_id: String) -> Result<bool, MobileError> {
        guard(|| {
            let open = parse_id(&task_id).ok().and_then(|id| self.task_anywhere(id)).is_some_and(|(_, t)| !t.is_done());
            Ok(open)
        })
    }
}

#[cfg(test)]
#[path = "core_focus_tests.rs"]
mod tests;
