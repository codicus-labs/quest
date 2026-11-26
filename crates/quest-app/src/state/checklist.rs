use std::collections::HashMap;

use quest_domain::{Command, EntityId, Position};
use quest_view::{checklist, undo::UndoEvent};
use rune::TextEdit;

use super::{State, View};

#[derive(Default)]
pub struct ChecklistEdits {
    pub items: HashMap<EntityId, TextEdit>,
    pub add: TextEdit,
}

impl State {
    pub fn checklist_fields(&mut self) -> &mut ChecklistEdits {
        if self.view == View::Focus {
            &mut self.focus_fields().checklist
        } else {
            &mut self.task_modal_fields().checklist
        }
    }

    pub fn checklist_edit(&mut self, id: EntityId) -> &mut TextEdit {
        let current = self.snap.checklist_item(id).map(|item| item.text.clone()).unwrap_or_default();
        let edit = self.checklist_fields().items.entry(id).or_insert_with(|| TextEdit::new(&current));
        if !edit.focused && edit.text != current {
            *edit = TextEdit::new(current);
        }
        edit
    }

    pub fn save_checklist_text(&mut self, id: EntityId) {
        let Some(edit) = (if self.view == View::Focus {
            self.focus_draft.as_ref().map(|form| &form.checklist)
        } else {
            self.task_modal.as_ref().map(|form| &form.checklist)
        })
        .and_then(|form| form.items.get(&id)) else {
            return;
        };
        if self.snap.checklist_item(id).is_some_and(|item| item.text != edit.text) {
            self.dispatch(Command::RenameChecklistItem { id, text: edit.text.clone() });
        }
    }

    pub fn insert_checklist_item(&mut self, task: EntityId, after: Option<EntityId>, text: String) -> Option<EntityId> {
        let position = match after {
            Some(item) => checklist::after(&self.snap, item)?,
            None => Position::Bottom,
        };
        let id = EntityId::new();
        self.dispatch(Command::AddChecklistItem { id, task_id: task, text, position });
        self.snap.checklist_item(id).map(|item| item.id)
    }

    pub fn move_checklist_item(&mut self, id: EntityId, down: bool) {
        if let Some(position) = checklist::move_one(&self.snap, id, down) {
            self.dispatch(Command::MoveChecklistItem { id, position });
        }
    }

    pub fn toggle_checklist_item(&mut self, id: EntityId) {
        let Some(previous) = self.snap.checklist_item(id).map(|item| item.done) else { return };
        self.dispatch(Command::SetChecklistItemDone { id, done: !previous });
        if self.notice.is_none() {
            self.step_undo(UndoEvent::ChecklistDone { id, previous });
        }
    }
}
