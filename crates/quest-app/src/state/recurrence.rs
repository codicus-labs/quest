use std::borrow::Cow;
use std::sync::Arc;

use chrono::{NaiveDate, Weekday};
use quest_domain::{Command, EntityId, Frequency, MonthDay, RepeatPin, RepeatRule, Task, Weekdays};
use quest_store::SyncStatus;
use quest_view::repeat::{self, RepeatForm};
use rune::TextEdit;

use super::{Overlay, State, View};
use crate::log_work::date_label;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RepeatChoice {
    Frequency,
    Pin,
    MonthDay,
    Ends,
}

pub struct RepeatEditor {
    pub series: Option<EntityId>,
    return_to: Overlay,
    pub every: TextEdit,
    pub frequency: Frequency,
    pub pin: RepeatPin,
    pub weekdays: Weekdays,
    pub last_day: bool,
    pub month_day: TextEdit,
    pub starts: TextEdit,
    pub ends_on_date: bool,
    pub ends: TextEdit,
    pub time_zone: TextEdit,
    pub choice: Option<RepeatChoice>,
    pub error: Option<&'static str>,
}

impl Default for RepeatEditor {
    fn default() -> Self {
        Self {
            series: None,
            return_to: Overlay::None,
            every: TextEdit::new("1"),
            frequency: Frequency::Day,
            pin: RepeatPin::None,
            weekdays: Weekdays::default(),
            last_day: false,
            month_day: TextEdit::default(),
            starts: TextEdit::default(),
            ends_on_date: false,
            ends: TextEdit::default(),
            time_zone: TextEdit::default(),
            choice: None,
            error: None,
        }
    }
}

impl RepeatEditor {
    fn form(&self) -> RepeatForm<'_> {
        RepeatForm {
            every: &self.every.text,
            frequency: self.frequency,
            pin: self.pin,
            weekdays: self.weekdays,
            last_day: self.last_day,
            month_day: &self.month_day.text,
            starts: &self.starts.text,
            ends_on_date: self.ends_on_date,
            ends: &self.ends.text,
            time_zone: &self.time_zone.text,
        }
    }

    pub fn shown_weekdays(&self, today: NaiveDate) -> Weekdays {
        self.form().shown_weekdays(today)
    }

    fn rule(&self, today: NaiveDate) -> Result<RepeatRule, &'static str> {
        self.form().rule(today)
    }
}

impl State {
    pub fn plan_task(&self, id: EntityId) -> Option<Cow<'_, Task>> {
        if let Some(task) = self.snap.task(id) {
            return Some(Cow::Borrowed(task));
        }
        if self.view != View::Plan {
            return None;
        }
        self.plan
            .built
            .days()
            .flat_map(|day| self.snap.planned_occurrences(day, self.now))
            .find(|task| task.id == id)
            .map(Cow::Owned)
    }

    pub(super) fn plan_edit(&self, command: Command) -> Command {
        if self.view != View::Plan {
            return command;
        }
        let id = match &command {
            Command::RenameTask { id, .. }
            | Command::SetTaskDescription { id, .. }
            | Command::AssignTask { id, .. }
            | Command::MoveTask { id, .. }
            | Command::ScheduleTask { id, .. }
            | Command::CompleteTaskFromPlan { id, .. }
            | Command::DeleteTask { id, .. }
            | Command::StartFocus { task_id: id, .. } => *id,
            _ => return command,
        };
        let occurrence = if let Some(Cow::Owned(task)) = self.plan_task(id) { task.series.zip(task.day) } else { None };
        if occurrence.is_some() || matches!(command, Command::MoveTask { day: Some(_), .. }) {
            Command::EditPlan { occurrence, now: self.now, edit: Box::new(command) }
        } else {
            command
        }
    }

    pub fn series_of<'a>(&'a self, task: &'a Task) -> &'a Task {
        repeat::series_of(&self.snap, task)
    }

    pub fn open_repeat_editor(&mut self, task: EntityId) {
        self.save_task_day();
        let Some(task) = self.plan_task(task) else { return };
        let series = self.series_of(&task);
        let rule = series.repeat.map(|schedule| schedule.rule).unwrap_or(RepeatRule {
            every: 1,
            frequency: Frequency::Day,
            starts: task.day.unwrap_or_else(|| self.today()).max(self.today()),
            ends: None,
            time_zone: self.repeat_zone,
            pin: RepeatPin::None,
            weekdays: Weekdays::default(),
            month_day: None,
        });
        self.repeat_editor = RepeatEditor {
            series: Some(series.id),
            return_to: self.overlay,
            every: TextEdit::new(rule.every.to_string()),
            frequency: rule.frequency,
            pin: rule.pin,
            weekdays: rule.weekdays,
            last_day: rule.month_day == Some(MonthDay::Last),
            month_day: TextEdit::new(match rule.month_day {
                Some(MonthDay::Day(day)) => day.to_string(),
                _ => String::new(),
            }),
            starts: TextEdit::new(date_label(rule.starts)),
            ends_on_date: rule.ends.is_some(),
            ends: TextEdit::new(rule.ends.map(date_label).unwrap_or_default()),
            time_zone: TextEdit::new(rule.time_zone.to_string()),
            choice: None,
            error: None,
        };
        self.menu = None;
        self.date_picker = None;
        self.overlay = Overlay::RepeatEditor;
    }

    pub fn close_repeat_editor(&mut self) {
        if self.overlay == Overlay::RepeatEditor {
            self.overlay = self.repeat_editor.return_to;
        }
        self.date_picker = None;
        self.repeat_editor = RepeatEditor::default();
    }

    pub fn toggle_repeat_weekday(&mut self, day: Weekday) {
        let shown = self.repeat_editor.shown_weekdays(self.today());
        self.repeat_editor.weekdays = repeat::toggle_weekday(self.repeat_editor.weekdays, shown, day);
    }

    pub fn pick_repeat_start(&mut self, day: NaiveDate) {
        self.repeat_editor.starts = TextEdit::new(date_label(day));
    }

    pub fn pick_repeat_end(&mut self, day: NaiveDate) {
        self.repeat_editor.ends = TextEdit::new(date_label(day));
    }

    pub fn save_repeat_editor(&mut self) {
        match self.repeat_editor.rule(self.today()) {
            Ok(rule) => self.change_repeat(Some(rule)),
            Err(error) => self.repeat_editor.error = Some(error),
        }
    }

    pub fn stop_repeating(&mut self) {
        self.change_repeat(None);
    }

    fn change_repeat(&mut self, rule: Option<RepeatRule>) {
        let Some(id) = self.repeat_editor.series else { return };
        self.dispatch(Command::SetTaskRepeat { id, rule, now: self.clock.now() });
        if self.notice.is_some() {
            self.repeat_editor.error = Some("The repeat schedule could not be saved.");
            return;
        }
        if let Some(form) = &mut self.task_modal
            && form.task == id
            && let Some(task) = self.snap.task(id)
        {
            form.date = TextEdit::new(super::task_modal::day_label(task.day));
        }
        self.close_repeat_editor();
        if rule.is_some() {
            self.generate_due_tasks();
        }
    }

    pub(super) fn generate_due_tasks(&mut self) -> bool {
        if self.sync == SyncStatus::Connecting || self.auth.locks_edits() {
            self.recurrence_minute = None;
            return false;
        }
        let minute = self.now.timestamp().div_euclid(60);
        self.recurrence_minute = Some(minute);
        let before = self.store.snapshot();
        if self.store.dispatch(Command::GenerateDueTasks { now: self.now }).is_err() {
            return false;
        }
        self.adopt_snapshot();
        if Arc::ptr_eq(&before, &self.snap) {
            return false;
        }
        self.sync_focus_draft();
        true
    }
}
