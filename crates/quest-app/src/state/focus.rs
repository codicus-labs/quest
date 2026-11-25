//! What Focus is about and its editable title and description.

use quest_domain::{Command, EntityId, Task};
pub use quest_view::focus::FocusTarget;
use rune::{TextEdit, Tx};

use super::{State, View};

/// The Focus view's fields for `task`, edited in place and saved as they change.
pub struct FocusDraft {
    pub task: EntityId,
    pub title: TextEdit,
    pub description: TextEdit,
}

impl State {
    /// The running task; without one, the task started or selected last, else the open task
    /// worked on last. Nothing when there is no such open task (Focus then offers a choice).
    pub fn focus_target(&self) -> FocusTarget<'_> {
        quest_view::focus::target(&self.snap, self.focus_task)
    }

    pub fn focus_subject(&self) -> Option<&Task> {
        self.focus_target().task()
    }

    /// Keeps Focus's fields on the task it shows; edits in progress on that task stay.
    pub fn sync_focus_draft(&mut self) {
        let Some(task) = self.focus_subject() else {
            self.focus_draft = None;
            return;
        };
        if self.focus_draft.as_ref().is_some_and(|d| d.task == task.id) {
            return;
        }
        let draft = FocusDraft {
            task: task.id,
            title: TextEdit::new(task.title.clone()),
            description: TextEdit::new(task.description.clone()),
        };
        self.focus_draft = Some(draft);
    }

    /// Focus's fields for its text inputs. Only empty when Focus shows no task, and then no
    /// field is on screen: a stray edit lands in a draft no task owns, and saving skips it.
    pub fn focus_fields(&mut self) -> &mut FocusDraft {
        self.focus_draft.get_or_insert_with(|| FocusDraft {
            task: EntityId::new(),
            title: TextEdit::default(),
            description: TextEdit::default(),
        })
    }

    /// Saves the edited title; an empty one is not saved (the task keeps its name).
    pub fn save_focus_title(&mut self) {
        let Some(draft) = &self.focus_draft else { return };
        let (id, title) = (draft.task, draft.title.text.trim().to_owned());
        if title.is_empty() || self.snap.task(id).is_none_or(|t| t.title == title) {
            return;
        }
        self.dispatch(Command::RenameTask { id, title });
    }

    pub fn save_focus_description(&mut self) {
        let Some(draft) = &self.focus_draft else { return };
        let (id, description) = (draft.task, draft.description.text.clone());
        if self.snap.task(id).is_none_or(|t| t.description == description) {
            return;
        }
        self.dispatch(Command::SetTaskDescription { id, description });
    }

    pub fn start_focus(&mut self, task: EntityId) {
        self.focus_task = Some(task);
        let at = self.clock.now();
        self.dispatch(Command::StartFocus { entry_id: EntityId::new(), task_id: task, at });
    }

    pub fn stop_focus(&mut self) {
        let at = self.clock.now();
        self.dispatch(Command::StopFocus { at });
    }

    /// Space on a task: pauses it if it runs, else starts it. The view stays.
    pub fn toggle_focus(&mut self, task: EntityId) {
        let running = self.snap.running_entry().is_some_and(|e| e.task_id == Some(task));
        if running { self.stop_focus() } else { self.start_focus(task) }
    }

    /// Focus's Complete: the session ends, the task is done, and the app goes Home.
    pub fn complete_focus(&mut self, task: EntityId, tx: &mut Tx) {
        if self.snap.task(task).is_none_or(Task::is_done) {
            return;
        }
        self.toggle_done(task);
        if self.notice.is_some() {
            return;
        }
        self.focus_task = None;
        self.go_with(View::Home, tx);
    }
}
