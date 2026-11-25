use quest_domain::{Command, EntityId, WorkTask};
use rune::TextEdit;
use rune_kit::ComboboxState;

use super::{Overlay, State};
use crate::clock;
use crate::entry_editor::{self, Draft, EndFields, FieldError, Times};
use crate::log_work::{self, date_label, time_label};
use crate::undo::Deletable;

#[derive(Default)]
pub struct EntryForm {
    pub entry: Option<EntityId>,
    pub running: bool,
    pub task: Option<EntityId>,
    pub new_task: Option<String>,
    pub task_picker: ComboboxState,
    pub quest: Option<EntityId>,
    pub start_date: TextEdit,
    pub start: TextEdit,
    pub end_date: TextEdit,
    pub end: TextEdit,
    pub end_follows: bool,
}

impl EntryForm {
    pub fn draft(&self) -> Draft<'_> {
        Draft {
            start_date: &self.start_date.text,
            start: &self.start.text,
            end: (!self.running).then_some(EndFields { date: &self.end_date.text, time: &self.end.text }),
        }
    }
}

impl State {
    /// Opens the editor on a stored entry with its current values.
    pub fn open_entry_editor(&mut self, id: EntityId) {
        let Some(entry) = self.snap.time_entries.iter().find(|e| e.id == id) else { return };
        let start = clock::local(entry.started_at);
        let end = clock::local(entry.ended_at.unwrap_or(entry.started_at));
        self.entry_form = EntryForm {
            entry: Some(id),
            running: entry.is_running(),
            task: entry.task_id,
            quest: entry.quest_id,
            start_date: TextEdit::new(date_label(start.date())),
            start: TextEdit::new(time_label(start.time())),
            end_date: TextEdit::new(date_label(end.date())),
            end: TextEdit::new(time_label(end.time())),
            end_follows: start.date() == end.date(),
            ..EntryForm::default()
        };
        self.menu = None;
        self.overlay = Overlay::EntryEditor;
    }

    /// The times the fields name, or the field that is wrong.
    pub fn entry_form_times(&self) -> Result<Times, FieldError> {
        let entry = self
            .entry_form
            .entry
            .and_then(|id| self.snap.time_entries.iter().find(|entry| entry.id == id))
            .ok_or(FieldError::StartDate)?;
        entry_editor::times(&self.entry_form.draft(), entry, self.today(), clock::to_utc)
    }

    /// The start date changed: an end date that follows it moves along.
    pub fn follow_entry_start_date(&mut self) {
        let form = &mut self.entry_form;
        if form.end_follows {
            form.end_date = TextEdit::new(form.start_date.text.clone());
        }
    }

    pub fn pick_entry_task(&mut self, task: Option<EntityId>) {
        let quest = log_work::quest(&self.snap, task, None);
        self.entry_form.task = task;
        self.entry_form.new_task = None;
        self.entry_form.quest = quest;
    }

    pub fn save_entry_editor(&mut self) {
        let Some(id) = self.entry_form.entry else { return };
        let Ok(times) = self.entry_form_times() else { return };
        let form = &self.entry_form;
        let task = if let Some(title) = &form.new_task {
            Some(WorkTask::New {
                id: EntityId::new(),
                title: title.clone(),
                quest_id: form.quest,
                day: clock::local(times.started_at).date(),
            })
        } else {
            form.task.map(WorkTask::Existing)
        };
        self.dispatch(Command::EditEntry {
            id,
            task,
            quest_id: form.quest,
            started_at: times.started_at,
            ended_at: times.ended_at,
        });
        if self.notice.is_none() {
            self.close_entry_editor();
        }
    }

    pub fn close_entry_editor(&mut self) {
        self.date_picker = None;
        self.quest_picker = None;
        self.entry_form = EntryForm::default();
        if self.overlay == Overlay::EntryEditor {
            self.overlay = Overlay::None;
        }
    }

    /// Delete: the entry goes with an undo toast, and the editor closes.
    pub fn delete_edited_entry(&mut self) {
        let Some(id) = self.entry_form.entry else { return };
        self.close_entry_editor();
        self.delete(Deletable::Entry(id));
    }

    /// Open task: drops what was typed and opens the picked task's dialog.
    pub fn open_edited_entry_task(&mut self) {
        let Some(task) = self.entry_form.task else { return };
        self.close_entry_editor();
        self.open_task_modal(task);
    }
}
