//! `QuestCore`'s Quests page, a quest's page and the archive. Memory-only like the rest of the
//! core; the records and rules live in `quests` and `archive`.

use quest_domain::{Command, EntityId, Position, Quest, QuestStatus};
use quest_view::undo::Deletable;

use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::ids::parse_id;
use crate::quest_types::{ArchiveData, QuestDetail, QuestHue, QuestsData, TrashKind};
use crate::time::Now;
use crate::types::{AddPosition, Moment};
use crate::{archive, quests};

#[uniffi::export]
impl QuestCore {
    /// The Quests page: active and completed quests with their hours.
    pub fn quests(&self, now: Moment) -> Result<QuestsData, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let toast = self.undo_state().toast();
            Ok(quests::quests(&self.store().snapshot(), now, toast))
        })
    }

    /// Creates an active quest from the editor: a title (`EmptyTitle` when blank) and its colour
    /// (none: the first colour no quest uses, desktop rule). Returns its id.
    pub fn create_quest(&self, title: String, hue: Option<QuestHue>) -> Result<String, MobileError> {
        guard(|| {
            let title = quests::new_title(&title)?;
            let id = EntityId::new();
            let color =
                hue.map_or_else(|| quest_view::quests::new_quest_color(&self.store().snapshot()), quests::color);
            self.dispatch(Command::CreateQuest { id, title, color })?;
            Ok(id.to_string())
        })
    }

    /// A quest's page. A deleted quest is `UnknownQuest`: close the page.
    pub fn quest_detail(&self, quest_id: String, now: Moment) -> Result<QuestDetail, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let quest = self.quest(&quest_id)?;
            let toast = self.undo_state().toast();
            Ok(quests::quest_detail(&self.store().snapshot(), &quest, now, toast))
        })
    }

    /// An empty or unchanged title is not saved (no error: the page shows the stored one).
    pub fn rename_quest(&self, quest_id: String, title: String) -> Result<(), MobileError> {
        guard(|| {
            let quest = self.quest(&quest_id)?;
            let rename =
                quests::retitle(&quest.title, &title).map(|title| Command::RenameQuest { id: quest.id, title });
            self.dispatch_some(rename)
        })
    }

    pub fn set_quest_hue(&self, quest_id: String, hue: QuestHue) -> Result<(), MobileError> {
        guard(|| {
            let quest = self.quest(&quest_id)?;
            let color = quests::color(hue);
            self.dispatch_some((quest.color != color).then_some(Command::SetQuestColor { id: quest.id, color }))
        })
    }

    /// Complete quest, Reopen quest or Unarchive, by the quest's status (desktop rule). Tracked
    /// time stays; the snackbar offers to undo it.
    pub fn toggle_quest_status(&self, quest_id: String, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let quest = self.quest(&quest_id)?;
            let to = quests::next_status(quest.status);
            self.dispatch(Command::SetQuestStatus { id: quest.id, status: to })?;
            self.undo_state().quest_status_changed(quest.id, quest.title, quest.status, to, now.at);
            Ok(())
        })
    }

    /// The archive's Unarchive: an archived quest becomes active again (no snackbar, as on the
    /// desktop).
    pub fn unarchive_quest(&self, quest_id: String) -> Result<(), MobileError> {
        guard(|| {
            let quest = self.quest(&quest_id)?;
            let unarchive = Command::SetQuestStatus { id: quest.id, status: QuestStatus::Active };
            self.dispatch_some((quest.status == QuestStatus::Archived).then_some(unarchive))
        })
    }

    /// Deletes the quest softly; the snackbar offers to undo it. Its tasks
    /// and sessions stay and count as other work until it is restored.
    pub fn delete_quest(&self, quest_id: String, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let quest = self.quest(&quest_id)?;
            let item = Deletable::Quest(quest.id);
            self.dispatch(item.delete(now.at))?;
            self.undo_state().deleted(item, quest.title, now.at);
            Ok(())
        })
    }

    /// The page's "Add a task to this quest…": planned today or (`backlog`) unplanned, at the top
    /// or bottom.
    /// Returns the new task's id.
    pub fn add_quest_task(
        &self,
        quest_id: String,
        title: String,
        backlog: bool,
        position: AddPosition,
        now: Moment,
    ) -> Result<String, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let quest = self.quest(&quest_id)?;
            let title = quests::new_title(&title)?;
            let id = EntityId::new();
            let position = match position {
                AddPosition::Top => Position::Top,
                AddPosition::Bottom => Position::Bottom,
            };
            let planned_on = (!backlog).then(|| now.today());
            let quest_id = Some(quest.id);
            self.dispatch(Command::CreateTask { id, title, quest_id, planned_on, due_on: None, position })?;
            Ok(id.to_string())
        })
    }

    /// Archived quests and everything deleted, to restore.
    pub fn archive(&self, now: Moment) -> Result<ArchiveData, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            Ok(archive::archive(&self.store().snapshot(), now))
        })
    }

    /// Brings a deleted thing back.
    pub fn restore(&self, kind: TrashKind, id: String, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let item = archive::deletable(kind, parse_id(&id)?);
            self.dispatch(item.restore())?;
            self.undo_state().restored(item, now.at);
            Ok(())
        })
    }
}

impl QuestCore {
    /// A copy of a quest that exists now (active or completed).
    fn quest(&self, quest_id: &str) -> Result<Quest, MobileError> {
        let id = parse_id(quest_id)?;
        self.store().snapshot().quest(id).cloned().ok_or_else(|| MobileError::UnknownQuest { id: quest_id.to_owned() })
    }
}

#[cfg(test)]
#[path = "core_quests_tests.rs"]
mod tests;
