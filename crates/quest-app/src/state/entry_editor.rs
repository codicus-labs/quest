//! The entry editor a timeline block opens: start, end, what the session is booked on (a task or
//! Organizing), and delete. `quest_view::entry_editor::times` reads the times.

use quest_domain::{Command, EntityId, EntryKind, TimeEntry, WorkTask};
use quest_view::entry_editor::{self, Draft, EndFields, FieldError, Times};
use quest_view::log_work::{date_label, time_label};
use rune::TextEdit;
use rune_kit::ComboboxState;

use super::{Overlay, State};
use crate::clock;
use crate::undo::Deletable;

/// What a session counts as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Booking {
    /// Work on a task; none for work without one.
    Task(Option<EntityId>),
    Organizing,
}

pub struct EntryForm {
    pub entry: EntityId,
    pub running: bool,
    pub booking: Booking,
    pub picker: ComboboxState,
    /// The days stay; only the times are edited.
    start_date: String,
    end_date: String,
    pub start: TextEdit,
    pub end: TextEdit,
}

impl EntryForm {
    /// Takes edits for an editor that already closed (`State::entry_form_mut`).
    pub fn scratch() -> EntryForm {
        EntryForm {
            entry: EntityId::DEFAULT_WORKSPACE,
            running: false,
            booking: Booking::Organizing,
            picker: ComboboxState::default(),
            start_date: String::new(),
            end_date: String::new(),
            start: TextEdit::new(""),
            end: TextEdit::new(""),
        }
    }

    fn draft(&self) -> Draft<'_> {
        Draft {
            start_date: &self.start_date,
            start: &self.start.text,
            end: (!self.running).then_some(EndFields { date: &self.end_date, time: &self.end.text }),
        }
    }
}

impl State {
    pub fn open_entry_editor(&mut self, id: EntityId) {
        let Some(entry) = self.entry(id) else { return };
        let start = clock::local(entry.started_at);
        let end = clock::local(entry.ended_at.unwrap_or(entry.started_at));
        let booking = match entry.kind {
            EntryKind::Organizing => Booking::Organizing,
            EntryKind::Task => Booking::Task(entry.task_id),
        };
        self.entry_form = Some(EntryForm {
            entry: id,
            running: entry.is_running(),
            booking,
            picker: ComboboxState::default(),
            start_date: date_label(start.date()),
            end_date: date_label(end.date()),
            start: TextEdit::new(time_label(start.time())),
            end: TextEdit::new(time_label(end.time())),
        });
        self.menu = None;
        self.overlay = Overlay::EntryEditor;
    }

    fn entry(&self, id: EntityId) -> Option<&TimeEntry> {
        self.snap.time_entries.iter().find(|e| e.id == id)
    }

    /// The times the fields name, or the field that is wrong.
    pub fn entry_form_times(&self) -> Result<Times, FieldError> {
        let form = self.entry_form.as_ref().ok_or(FieldError::Start)?;
        let entry = self.entry(form.entry).ok_or(FieldError::Start)?;
        entry_editor::times(&form.draft(), entry, self.today(), clock::to_utc)
    }

    /// The form's fields; a closed editor's edits land nowhere.
    pub fn entry_form_mut(&mut self) -> &mut EntryForm {
        self.entry_form.as_mut().unwrap_or(&mut self.closed_entry_form)
    }

    pub fn book_entry(&mut self, booking: Booking) {
        if let Some(form) = &mut self.entry_form {
            form.booking = booking;
        }
    }

    /// Saves the times, then the booking: a task's session takes the task's quest; Organizing
    /// keeps the task it came from.
    pub fn save_entry_editor(&mut self) {
        let Some(form) = &self.entry_form else { return };
        let (id, booking) = (form.entry, form.booking);
        let Some(entry) = self.entry(id) else { return };
        let Ok(times) = self.entry_form_times() else { return };
        let was_organizing = entry.kind == EntryKind::Organizing;
        let (started_at, ended_at) = (times.started_at, times.ended_at);
        let command = match booking {
            Booking::Organizing if was_organizing => Command::SetEntryTimes { id, started_at, ended_at },
            Booking::Organizing => Command::EditEntry {
                id,
                task: entry.task_id.map(WorkTask::Existing),
                quest_id: entry.quest_id,
                started_at,
                ended_at,
            },
            Booking::Task(task) => {
                let quest_id = match task {
                    Some(task) => crate::log_work::quest(&self.snap, Some(task), None),
                    None => entry.quest_id,
                };
                Command::EditEntry { id, task: task.map(WorkTask::Existing), quest_id, started_at, ended_at }
            }
        };
        self.dispatch(command);
        if booking == Booking::Organizing && !was_organizing && self.notice.is_none() {
            self.dispatch(Command::BookEntryAsOrganizing { id });
        }
        if self.notice.is_none() {
            self.close_entry_editor();
        }
    }

    pub fn close_entry_editor(&mut self) {
        self.entry_form = None;
        if self.overlay == Overlay::EntryEditor {
            self.overlay = Overlay::None;
        }
    }

    /// The session goes with an undo toast, and the editor closes.
    pub fn delete_edited_entry(&mut self) {
        let Some(id) = self.entry_form.as_ref().map(|f| f.entry) else { return };
        self.close_entry_editor();
        self.delete(Deletable::Entry(id));
    }
}
