use std::sync::{MutexGuard, PoisonError};

use quest_domain::EntityId;
use quest_view::focus_exit::{self, FocusExit};

use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::time::Now;
use crate::types::Moment;

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FocusExitStatus {
    pub entry_id: String,
    pub left_at_ms: i64,
    pub hint_due: bool,
    pub left_at_label: String,
}

impl QuestCore {
    pub(crate) fn focus_exit_state(&self) -> MutexGuard<'_, FocusExit> {
        self.focus_exit.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[uniffi::export]
impl QuestCore {
    pub fn focus_opened(&self) -> Result<(), MobileError> {
        guard(|| {
            self.focus_exit_state().open();
            Ok(())
        })
    }

    pub fn focus_left(&self, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let at = Now::parse(&now)?.at;
            let snap = self.store().snapshot();
            self.focus_exit_state().leave(&snap, at);
            Ok(())
        })
    }

    pub fn focus_exit_status(&self, now: Moment) -> Result<Option<FocusExitStatus>, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let snap = self.store().snapshot();
            let exit = self.focus_exit_state();
            Ok(exit.departure(&snap).map(|(entry, since)| FocusExitStatus {
                entry_id: entry.to_string(),
                left_at_ms: since.timestamp_millis(),
                hint_due: exit.hint(&snap, now.at).is_some(),
                left_at_label: since.with_timezone(&now.tz).format("%H:%M").to_string(),
            }))
        })
    }

    pub fn keep_focus_time(&self, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let at = Now::parse(&now)?.at;
            let snap = self.store().snapshot();
            self.focus_exit_state().keep(&snap, at);
            Ok(())
        })
    }

    /// Returns the new organizing entry, or None when there is no hint to accept.
    pub fn book_focus_time(&self, now: Moment) -> Result<Option<String>, MobileError> {
        guard(|| {
            let at = Now::parse(&now)?.at;
            let snap = self.store().snapshot();
            let entry = EntityId::new();
            let command = self.focus_exit_state().book(&snap, entry, at);
            let Some(command) = command else { return Ok(None) };
            self.dispatch(command)?;
            self.focus_exit_state().open();
            self.undo_state().rebooked(entry, at);
            Ok(Some(entry.to_string()))
        })
    }

    /// Returns whether the current task was switched to organizing.
    pub fn organize(&self, now: Moment) -> Result<bool, MobileError> {
        guard(|| {
            let at = Now::parse(&now)?.at;
            let Some(command) = focus_exit::organize(&self.store().snapshot(), EntityId::new(), at) else {
                return Ok(false);
            };
            self.dispatch(command)?;
            self.focus_exit_state().open();
            Ok(true)
        })
    }

    /// Returns the resumed task's ID, or None if there is no open origin task.
    pub fn back_to_task(&self, now: Moment) -> Result<Option<String>, MobileError> {
        guard(|| {
            let at = Now::parse(&now)?.at;
            let Some(command @ quest_domain::Command::StartFocus { task_id, .. }) =
                focus_exit::back_to_task(&self.store().snapshot(), EntityId::new(), at)
            else {
                return Ok(None);
            };
            self.dispatch(command)?;
            self.focus_exit_state().open();
            Ok(Some(task_id.to_string()))
        })
    }
}
