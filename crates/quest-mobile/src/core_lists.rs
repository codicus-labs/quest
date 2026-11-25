//! `QuestCore`'s redesigned lists, the Quests tab, the Tracked tab and the mini player.
//! Memory-only; the rules are `quest-view`'s (`lists`, `row_meta`, `tracked`, `player`).

use quest_domain::EntityId;

use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::list_types::{ListKind, PlayerCard, QuestNav, TaskList, TrackedData};
use crate::lists;
use crate::time::Now;
use crate::types::Moment;

#[uniffi::export]
impl QuestCore {
    /// One list: Today (by date), the Inbox, the Logbook or a quest's list. A gone quest is
    /// `UnknownQuest`.
    pub fn task_list(&self, kind: ListKind, now: Moment) -> Result<TaskList, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let snapshot = self.store().snapshot();
            let undo = self.undo_state();
            lists::task_list(&snapshot, &kind, now, undo.toast(), |task| undo.lingering(task, now.at))
        })
    }

    /// Like toggle_task, but keeps a newly completed row in place and shows “Done”.
    pub fn toggle_list_task(&self, task_id: String, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let before = self.task(&task_id)?;
            self.dispatch(crate::commands::toggle_task(&before, now))?;
            self.remember_list_completion(before, now);
            Ok(())
        })
    }

    /// Adds a task to a list with its defaults: Today plans it today, a quest's list gives it the
    /// quest, the Inbox (and the Logbook) nothing. Returns its id.
    pub fn add_to_list(&self, kind: ListKind, title: String, now: Moment) -> Result<String, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let id = EntityId::new();
            for command in lists::add(&self.store().snapshot(), &kind, id, &title, now)? {
                self.dispatch(command)?;
            }
            Ok(id.to_string())
        })
    }

    /// The Quests tab: active and completed quests with this week's hours.
    pub fn quest_nav(&self, now: Moment) -> Result<QuestNav, MobileError> {
        guard(|| Ok(lists::quest_nav(&self.store().snapshot(), Now::parse(&now)?)))
    }

    /// The Tracked tab: today's timeline and this week.
    pub fn tracked(&self, now: Moment) -> Result<TrackedData, MobileError> {
        guard(|| {
            let snapshot = self.store().snapshot();
            Ok(lists::tracked(&snapshot, Now::parse(&now)?, &self.focus_exit_state()))
        })
    }

    /// The mini player above the tabs.
    pub fn player(&self, now: Moment) -> Result<PlayerCard, MobileError> {
        guard(|| Ok(lists::player(&self.store().snapshot(), Now::parse(&now)?)))
    }
}

#[cfg(test)]
#[path = "core_lists_tests.rs"]
mod tests;
