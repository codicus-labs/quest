//! The task dialog: a task's title, description, day and quest, edited over the existing
//! commands. Title and description save as they change, the day on Return, when focus leaves
//! it or when the dialog closes. An empty title is never saved.

use chrono::{DateTime, NaiveDate, Utc};
use quest_domain::{Command, EntityId, Position, Task};
use rune::TextEdit;

use super::{Overlay, State};
use crate::log_work::{date_label, parse_day};
use crate::undo::Deletable;
use quest_view::home::rename;

#[cfg(test)]
#[path = "task_modal_tests.rs"]
mod tests;

/// The dialog's fields for one task.
pub struct TaskModal {
    pub task: EntityId,
    pub title: TextEdit,
    pub description: TextEdit,
    /// The DATE field: "Nov 24, 2026", empty for the backlog.
    pub date: TextEdit,
}

impl TaskModal {
    /// Fields owned by no task.
    pub fn scratch() -> TaskModal {
        TaskModal {
            task: EntityId::new(),
            title: TextEdit::default(),
            description: TextEdit::default(),
            date: TextEdit::default(),
        }
    }
}

/// What the DATE field shows for a task's day.
pub fn day_label(day: Option<NaiveDate>) -> String {
    day.map(date_label).unwrap_or_default()
}

/// The move the DATE field asks for: none when it names the task's day already or no day at
/// all (the field then shows the task's day again). Empty means the backlog.
pub fn move_to(task: &Task, text: &str, today: NaiveDate, at: DateTime<Utc>) -> Option<Command> {
    let planned_on = if text.trim().is_empty() { None } else { Some(parse_day(text, today)?) };
    if planned_on == task.planned_on {
        return None;
    }
    Some(Command::MoveTask { id: task.id, planned_on, position: Position::Bottom, at })
}

/// Whether everything typed is stored: an empty title or a day not yet taken shows "Saving…".
pub fn is_saved(task: &Task, form: &TaskModal) -> bool {
    form.title.text.trim() == task.title
        && form.description.text == task.description
        && form.date.text == day_label(task.planned_on)
}

impl State {
    /// Opens the dialog on `task`. Focus then shows it while nothing runs, as a picked card did.
    pub fn open_task_modal(&mut self, task: EntityId) {
        let Some(t) = self.plan_task(task) else { return };
        self.task_modal = Some(TaskModal {
            task,
            title: TextEdit::new(t.title.clone()),
            description: TextEdit::new(t.description.clone()),
            date: TextEdit::new(day_label(t.planned_on)),
        });
        self.menu = None;
        self.overlay = Overlay::Task;
        if self.snap.task(task).is_some() {
            self.focus_task = Some(task);
            self.sync_focus_draft();
        }
    }

    /// Closes the dialog, saving what it holds (but an empty title).
    pub fn close_task_modal(&mut self) {
        self.date_picker = None;
        self.quest_picker = None;
        self.save_task_title();
        self.save_task_description();
        self.save_task_day();
        self.task_modal = None;
        if self.overlay == Overlay::Task {
            self.overlay = Overlay::None;
        }
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

    pub fn save_task_description(&mut self) {
        let Some(form) = &self.task_modal else { return };
        let (id, description) = (form.task, form.description.text.clone());
        if self.plan_task(id).is_none_or(|t| t.description == description) {
            return;
        }
        self.dispatch(Command::SetTaskDescription { id, description });
    }

    /// Moves the task to the DATE field's day; a field naming no day shows the task's again.
    pub fn save_task_day(&mut self) {
        let today = self.today();
        let Some(form) = &self.task_modal else { return };
        let id = form.task;
        let Some(task) = self.plan_task(id) else { return };
        if let Some(command) = move_to(&task, &form.date.text, today, self.clock.now()) {
            self.dispatch(command);
        }
        let label = self.plan_task(id).map(|t| day_label(t.planned_on));
        if let (Some(form), Some(label)) = (&mut self.task_modal, label) {
            form.date = TextEdit::new(label);
        }
    }

    /// Saves the DATE field once focus left it with a change (Rune has no blur callback).
    /// True when it saved.
    pub fn save_task_day_on_blur(&mut self) -> bool {
        let Some(form) = &self.task_modal else { return false };
        let shown = self.plan_task(form.task).map(|t| day_label(t.planned_on));
        if form.date.focused || shown.is_none_or(|label| label == form.date.text) {
            return false;
        }
        self.save_task_day();
        true
    }

    pub fn move_task_to_backlog(&mut self) {
        let Some(form) = &mut self.task_modal else { return };
        form.date = TextEdit::new("");
        self.save_task_day();
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
        self.overlay = Overlay::None;
        self.delete(Deletable::Task(form.task));
    }
}
