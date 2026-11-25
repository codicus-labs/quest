//! The Log work dialog's fields; `log_work::decide` says what saving them does.

use chrono::{NaiveDate, NaiveTime, Timelike};
use quest_domain::EntityId;
use rune::TextEdit;
use rune_kit::ComboboxState;

use super::{Overlay, State};
use crate::clock;
use crate::log_work::{self, Draft, FieldError};

pub struct LogWorkForm {
    pub task: Option<EntityId>,
    pub title: TextEdit,
    pub task_picker: ComboboxState,
    pub date: TextEdit,
    pub start: TextEdit,
    pub end: TextEdit,
    /// The quest chosen for a new task.
    pub quest: Option<EntityId>,
    /// Why the last save was refused, shown at that field.
    pub error: Option<FieldError>,
}

impl Default for LogWorkForm {
    fn default() -> Self {
        LogWorkForm {
            task: None,
            title: TextEdit::new(""),
            task_picker: ComboboxState::default(),
            date: TextEdit::new(""),
            start: TextEdit::new(""),
            end: TextEdit::new(""),
            quest: None,
            error: None,
        }
    }
}

impl State {
    /// Opens Log work for `day` from `start` to `end`, nothing saved until Log work.
    pub fn open_log_work(&mut self, day: NaiveDate, start: NaiveTime, end: NaiveTime) {
        self.log_work = LogWorkForm {
            date: TextEdit::new(log_work::date_label(day)),
            start: TextEdit::new(log_work::time_label(start)),
            end: TextEdit::new(log_work::time_label(end)),
            ..LogWorkForm::default()
        };
        self.menu = None;
        self.overlay = Overlay::LogWork;
    }

    /// Log work from the header or ⌘K: the quarter hour that just ended.
    pub fn open_log_work_now(&mut self) {
        let now = clock::local(self.clock.now());
        let end = now.time().with_second(0).unwrap_or(now.time());
        let start = end - chrono::Duration::minutes(15);
        let start = if start > end { NaiveTime::MIN } else { start };
        self.open_log_work(now.date(), start, end);
    }

    pub fn pick_log_work_task(&mut self, task: EntityId) {
        let Some(title) = self.snap.task(task).map(|t| t.title.clone()) else { return };
        self.log_work.task = Some(task);
        self.log_work.title = TextEdit::new(title);
        self.log_work.error = None;
    }

    /// Saves the form; a wrong field keeps the dialog open with the error at that field.
    pub fn save_log_work(&mut self) {
        let form = &self.log_work;
        let draft = Draft {
            task: form.task,
            title: &form.title.text,
            date: &form.date.text,
            start: &form.start.text,
            end: &form.end.text,
            quest: form.quest,
        };
        let decided = log_work::decide(&draft, self.today(), clock::to_utc, EntityId::new(), EntityId::new());
        match decided {
            Err(error) => self.log_work.error = Some(error),
            Ok(command) => {
                self.dispatch(command);
                if self.notice.is_none() {
                    self.close_log_work();
                }
            }
        }
    }

    pub fn close_log_work(&mut self) {
        self.date_picker = None;
        self.quest_picker = None;
        self.log_work = LogWorkForm::default();
        if self.overlay == Overlay::LogWork {
            self.overlay = Overlay::None;
        }
    }
}
