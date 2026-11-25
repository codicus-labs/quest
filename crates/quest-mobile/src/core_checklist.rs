//! `QuestCore`'s checklist of a task: list, add, rename, tick, move, delete. Memory-only like
//! the rest of the core; the rules live in `checklist`.

use quest_domain::{ChecklistItem, Command, EntityId};
use quest_view::undo::Deletable;

use crate::checklist;
use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::ids::parse_id;
use crate::task_types::{ChecklistPosition, ChecklistRow};
use crate::time::Now;
use crate::types::Moment;

#[uniffi::export]
impl QuestCore {
    /// The task's checklist in its order (also in `TaskDetail::checklist`).
    pub fn checklist(&self, task_id: String) -> Result<Vec<ChecklistRow>, MobileError> {
        guard(|| {
            let task = self.task(&task_id)?;
            Ok(checklist::rows(&self.store().snapshot(), task.id))
        })
    }

    /// Adds an open item at `position`; a blank text is `EmptyTitle`. Returns its id.
    pub fn add_checklist_item(
        &self,
        task_id: String,
        text: String,
        position: ChecklistPosition,
    ) -> Result<String, MobileError> {
        guard(|| {
            let task = self.task(&task_id)?;
            let id = EntityId::new();
            self.dispatch(checklist::add(task.id, id, &text, &position)?)?;
            Ok(id.to_string())
        })
    }

    /// An empty or unchanged text is not saved (no error: the UI shows the stored one).
    pub fn rename_checklist_item(&self, item_id: String, text: String) -> Result<(), MobileError> {
        guard(|| self.dispatch_some(checklist::rename(&self.checklist_item(&item_id)?, &text)))
    }

    /// Ticks the item off (`done`) or opens it again.
    pub fn set_checklist_item_done(&self, item_id: String, done: bool) -> Result<(), MobileError> {
        guard(|| self.dispatch_some(checklist::set_done(&self.checklist_item(&item_id)?, done)))
    }

    /// Moves the item within its checklist.
    pub fn move_checklist_item(&self, item_id: String, position: ChecklistPosition) -> Result<(), MobileError> {
        guard(|| {
            let item = self.checklist_item(&item_id)?;
            self.dispatch(Command::MoveChecklistItem { id: item.id, position: checklist::position_of(&position)? })
        })
    }

    /// Deletes the item softly; the snackbar offers to undo it.
    pub fn delete_checklist_item(&self, item_id: String, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let item = self.checklist_item(&item_id)?;
            let deletable = Deletable::ChecklistItem(item.id);
            self.dispatch(deletable.delete(now.at))?;
            self.undo_state().deleted(deletable, item.text, now.at);
            Ok(())
        })
    }
}

impl QuestCore {
    /// A copy of a checklist item that exists now.
    fn checklist_item(&self, item_id: &str) -> Result<ChecklistItem, MobileError> {
        let id = parse_id(item_id)?;
        let snapshot = self.store().snapshot();
        snapshot.checklist_item(id).cloned().ok_or_else(|| MobileError::UnknownChecklistItem { id: item_id.to_owned() })
    }
}

#[cfg(test)]
#[path = "core_checklist_tests.rs"]
mod tests;
