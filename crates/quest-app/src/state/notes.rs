use std::time::{Duration, Instant};

use quest_domain::{Command, EntityId};
use rune::{TextEdit, Tx};

use super::{Overlay, State, View};

const SAVE_DELAY: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NoteHost {
    Detail,
    Focus,
}

impl NoteHost {
    pub fn preview(self) -> &'static str {
        match self {
            Self::Detail => "task-modal-description",
            Self::Focus => "focus-description",
        }
    }

    pub fn source(self) -> &'static str {
        match self {
            Self::Detail => "task-modal-description-source",
            Self::Focus => "focus-description-source",
        }
    }
}

pub struct NoteDraft {
    pub host: NoteHost,
    pub task: EntityId,
    pub edit: TextEdit,
    save_at: Option<Instant>,
}

impl State {
    pub fn edit_notes(&mut self, tx: &mut Tx) -> bool {
        if self.overlay != Overlay::None || self.auth.locks_edits() {
            return false;
        }
        if self.view == View::Focus {
            return self.focus_subject().map(|task| task.id).is_some_and(|id| self.open_note(NoteHost::Focus, id, tx));
        }
        let Some(id) = self.task_modal.as_ref().map(|form| form.task).or_else(|| self.shortcut_task()) else {
            return false;
        };
        if self.task_modal.is_none() {
            self.open_task_modal(id);
            tx.scroll_into_view("task-modal");
        }
        self.open_note(NoteHost::Detail, id, tx)
    }

    pub fn open_note(&mut self, host: NoteHost, id: EntityId, tx: &mut Tx) -> bool {
        if self.note.as_ref().is_some_and(|note| note.host == host && note.task == id) {
            return true;
        }
        self.finish_note();
        let Some(task) = self.plan_task(id) else { return false };
        let mut edit = TextEdit::new(task.description.clone());
        let end = edit.text.len();
        edit.set_selection(end, end);
        edit.focused = true;
        self.note = Some(NoteDraft { host, task: id, edit, save_at: None });
        tx.focus_when_shown(host.source(), false);
        true
    }

    pub fn note_source(&mut self) -> &mut TextEdit {
        self.note.as_mut().map_or(&mut self.closed_note, |note| &mut note.edit)
    }

    pub fn note_changed(&mut self, tx: &mut Tx) {
        if let Some(note) = &mut self.note {
            note.save_at = Some(Instant::now() + SAVE_DELAY);
            tx.wake_after(SAVE_DELAY);
        }
    }

    pub fn close_note(&mut self, tx: &mut Tx) {
        if let Some(host) = self.note.as_ref().map(|note| note.host) {
            self.finish_note();
            tx.focus_when_shown(host.preview(), true);
        }
    }

    pub fn finish_note(&mut self) {
        if let Some(mut note) = self.note.take()
            && note.save_at.is_some()
        {
            self.save_note(note.task, std::mem::take(&mut note.edit.text));
        }
    }

    fn save_note(&mut self, id: EntityId, description: String) {
        if self.plan_task(id).is_some_and(|task| task.description != description) {
            self.dispatch(Command::SetTaskDescription { id, description });
        }
    }

    pub fn refresh_note(&mut self, tx: &mut Tx) -> bool {
        let Some(note) = &self.note else { return false };
        let shown = self.overlay == Overlay::None
            && match note.host {
                NoteHost::Focus => {
                    self.view == View::Focus && self.focus_subject().is_some_and(|task| task.id == note.task)
                }
                NoteHost::Detail => {
                    self.view != View::Focus && self.task_modal.as_ref().is_some_and(|form| form.task == note.task)
                }
            };
        if !shown {
            self.finish_note();
            return true;
        }
        if !note.edit.focused {
            self.close_note(tx);
            return true;
        }
        if let Some(save_at) = note.save_at {
            let remaining = save_at.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                let (id, source) = (note.task, note.edit.text.clone());
                self.note.as_mut().unwrap().save_at = None;
                self.save_note(id, source);
                return true;
            }
            tx.wake_after(remaining);
        }
        false
    }

    pub fn toggle_note_marker(&mut self, id: EntityId, source: &str, offset: usize) {
        let Some(task) = self.plan_task(id) else { return };
        if task.description != source {
            return;
        }
        let description = quest_view::markdown::toggle_task_marker(source, offset);
        self.save_note(id, description);
    }
}
