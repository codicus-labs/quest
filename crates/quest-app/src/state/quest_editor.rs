//! The quest dialog over the Quests page: a quest's title, description and colour, saved as they
//! change; Complete quest / Reopen quest and Delete, each with Undo; and the way to the quest's
//! page. "New quest" opens it empty: the quest exists once its title is taken (Return or
//! Create quest). An empty title is never saved.

use quest_domain::{Command, EntityId, QuestColor, QuestStatus};
use rune::TextEdit;

use super::{Overlay, State, View};
use crate::undo::{Deletable, UndoEvent};

/// The dialog's fields; `quest` is `None` while a new quest has no title yet.
pub struct QuestEditor {
    pub quest: Option<EntityId>,
    pub title: TextEdit,
    pub description: TextEdit,
    /// The colour a new quest gets; an existing quest's colour is its own.
    pub color: QuestColor,
}

impl QuestEditor {
    /// Fields owned by no quest.
    pub fn scratch() -> QuestEditor {
        QuestEditor {
            quest: None,
            title: TextEdit::default(),
            description: TextEdit::default(),
            color: QuestColor::Blue,
        }
    }
}

impl State {
    pub fn open_quest_editor(&mut self, quest: EntityId) {
        let Some(q) = self.snap.quest(quest) else { return };
        self.quest_editor = Some(QuestEditor {
            quest: Some(quest),
            title: TextEdit::new(q.title.clone()),
            description: TextEdit::new(q.description.clone()),
            color: q.color,
        });
        self.overlay = Overlay::Quest;
    }

    /// "New quest": the dialog with an empty title, focused.
    pub fn open_new_quest(&mut self) {
        let mut title = TextEdit::default();
        title.focused = true;
        self.quest_editor = Some(QuestEditor {
            quest: None,
            title,
            description: TextEdit::default(),
            color: quest_view::quests::new_quest_color(&self.snap),
        });
        self.overlay = Overlay::Quest;
    }

    /// Closes the dialog, saving what it holds (a new quest only once it has a title).
    pub fn close_quest_editor(&mut self) {
        self.save_quest_title();
        self.save_quest_description();
        self.quest_editor = None;
        if self.overlay == Overlay::Quest {
            self.overlay = Overlay::None;
        }
    }

    /// The dialog's fields for its inputs; a late edit after it closed lands in scratch fields.
    pub fn quest_editor_fields(&mut self) -> &mut QuestEditor {
        match &mut self.quest_editor {
            Some(form) => form,
            None => &mut self.closed_quest_editor,
        }
    }

    /// Renames the quest live. A new quest is created only by `create_quest_from_editor`.
    pub fn save_quest_title(&mut self) {
        let Some(form) = &self.quest_editor else { return };
        let (Some(id), title) = (form.quest, form.title.text.trim().to_owned()) else { return };
        if title.is_empty() || self.snap.quest(id).is_none_or(|q| q.title == title) {
            return;
        }
        self.dispatch(Command::RenameQuest { id, title });
    }

    pub fn save_quest_description(&mut self) {
        let Some(form) = &self.quest_editor else { return };
        let (Some(id), description) = (form.quest, form.description.text.clone()) else { return };
        if self.snap.quest(id).is_none_or(|q| q.description == description) {
            return;
        }
        self.dispatch(Command::SetQuestDescription { id, description });
    }

    /// Creates the new quest from the dialog's title, colour and description; the dialog then
    /// edits it.
    pub fn create_quest_from_editor(&mut self) {
        let Some(form) = &self.quest_editor else { return };
        let title = form.title.text.trim().to_owned();
        if form.quest.is_some() || title.is_empty() {
            return;
        }
        let (id, color, description) = (EntityId::new(), form.color, form.description.text.clone());
        self.dispatch(Command::CreateQuest { id, title, color });
        if self.snap.quest(id).is_none() {
            return;
        }
        if !description.is_empty() {
            self.dispatch(Command::SetQuestDescription { id, description });
        }
        if let Some(form) = &mut self.quest_editor {
            form.quest = Some(id);
        }
    }

    pub fn set_quest_editor_color(&mut self, color: QuestColor) {
        let Some(form) = &mut self.quest_editor else { return };
        form.color = color;
        let Some(id) = form.quest else { return };
        if self.snap.quest(id).is_some_and(|q| q.color != color) {
            self.dispatch(Command::SetQuestColor { id, color });
        }
    }

    /// Complete quest / Reopen quest: the quest changes tab, the dialog closes, Undo takes it
    /// back.
    pub fn toggle_editor_quest_completed(&mut self) {
        let Some(id) = self.quest_editor.as_ref().and_then(|f| f.quest) else { return };
        let Some(status) = self.snap.quest(id).map(|q| q.status) else { return };
        let to = if status == QuestStatus::Completed { QuestStatus::Active } else { QuestStatus::Completed };
        self.close_quest_editor();
        self.set_quest_status(id, to);
    }

    /// Moves a quest to `to`, with a toast whose Undo (and ⌘Z) moves it back.
    pub fn set_quest_status(&mut self, quest: EntityId, to: QuestStatus) {
        let Some((from, title)) = self.snap.quest(quest).map(|q| (q.status, q.title.clone())) else { return };
        if from == to {
            return;
        }
        self.dispatch(Command::SetQuestStatus { id: quest, status: to });
        if self.snap.quest(quest).is_some_and(|q| q.status == to) {
            self.step_undo(UndoEvent::QuestStatusChanged { quest, title, from, to });
        }
    }

    /// Delete: the quest goes to the archive with an undo toast, and the dialog closes.
    pub fn delete_editor_quest(&mut self) {
        let Some(id) = self.quest_editor.as_ref().and_then(|f| f.quest) else { return };
        self.close_quest_editor();
        self.delete(Deletable::Quest(id));
    }

    /// "Open quest page": the full page with the quest's tasks and milestones.
    pub fn open_editor_quest_page(&mut self) {
        let Some(id) = self.quest_editor.as_ref().and_then(|f| f.quest) else { return };
        self.close_quest_editor();
        self.go(View::Quest(id));
    }
}
