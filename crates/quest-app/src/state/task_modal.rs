use quest_domain::{Command, EntityId};
use quest_view::home::rename;
use rune::TextEdit;

use super::State;
use crate::undo::Deletable;

pub struct TaskModal {
    pub task: EntityId,
    pub tray_alias: bool,
    pub review_group: Option<&'static str>,
    pub title: TextEdit,
    pub checklist: super::checklist::ChecklistEdits,
}

impl TaskModal {
    pub fn scratch() -> TaskModal {
        TaskModal {
            task: EntityId::new(),
            tray_alias: false,
            review_group: None,
            title: TextEdit::default(),
            checklist: Default::default(),
        }
    }
}

impl State {
    /// Opens the dialog on `task`. Focus then shows it while nothing runs, as a picked card did.
    pub fn open_task_modal(&mut self, task: EntityId) {
        if self.task_modal.as_ref().is_some_and(|form| form.task != task) {
            self.close_task_modal();
        }
        let Some(t) = self.plan_task(task) else { return };
        self.task_modal = Some(TaskModal {
            task,
            tray_alias: false,
            review_group: None,
            title: TextEdit::new(t.title.clone()),
            checklist: Default::default(),
        });
        self.show_task_time = false;
        self.menu = None;
        self.selected_task = Some(task);
        if self.snap.task(task).is_some() {
            self.focus_task = Some(task);
            self.sync_focus_draft();
        }
    }

    /// Closes the dialog, saving what it holds (but an empty title).
    pub fn close_task_modal(&mut self) {
        self.date_picker = None;
        self.task_calendar = None;
        self.quest_picker = None;
        self.save_task_title();
        if self.note.as_ref().is_some_and(|note| note.host == super::notes::NoteHost::Detail) {
            self.finish_note();
        }
        self.task_modal = None;
    }

    /// The dialog's fields for its inputs. A late edit after it closed lands in scratch fields
    /// no task owns.
    pub fn task_modal_fields(&mut self) -> &mut TaskModal {
        match &mut self.task_modal {
            Some(form) => form,
            None => &mut self.closed_task_modal,
        }
    }

    pub fn save_task_title(&mut self) {
        let Some(form) = &self.task_modal else { return };
        let Some(command) = self.plan_task(form.task).and_then(|t| rename(&t, &form.title.text)) else { return };
        self.dispatch(command);
    }

    pub fn toggle_task_detail(&mut self, id: EntityId) {
        if self.task_modal.as_ref().is_some_and(|form| form.task == id) {
            self.close_task_modal();
        } else {
            self.open_task_modal(id);
        }
    }

    pub fn assign_task_quest(&mut self, quest: Option<EntityId>) {
        let Some(form) = &self.task_modal else { return };
        let id = form.task;
        if self.plan_task(id).is_none_or(|t| t.quest_id == quest) {
            return;
        }
        self.dispatch(Command::AssignTask { id, quest_id: quest });
    }

    /// Delete task: gone with an undo toast, and the dialog closes.
    pub fn delete_modal_task(&mut self) {
        let Some(form) = self.task_modal.take() else { return };
        self.delete(Deletable::Task(form.task));
    }
}
