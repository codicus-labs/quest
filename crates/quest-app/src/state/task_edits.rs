use chrono::{DateTime, NaiveDate, Utc};
use quest_domain::{Command, DateField, EntityId, Position, Snapshot, Task, TaskDates, TaskList};
use quest_view::{
    calendar, quick_add,
    undo::{self, TaskEdit, UndoEvent},
};
use rune::Tx;

use super::quest_picker::PickFor;
use super::{Overlay, State, View};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Calendar {
    pub task: Option<EntityId>,
    pub field: DateField,
    pub cursor: NaiveDate,
}

impl Calendar {
    pub fn anchor(self) -> String {
        field_key(self.task, self.field)
    }
}

pub fn field_key(task: Option<EntityId>, field: DateField) -> String {
    let which = if field == DateField::Plan { "plan" } else { "due" };
    task.map_or_else(|| format!("quick-add-{which}"), |id| format!("task-{id}-{which}"))
}

#[derive(Clone, Copy)]
pub(super) enum TrackedEdit {
    Quest(EntityId),
    Dates(EntityId),
    Placement(EntityId, TaskList, NaiveDate),
    Completion(EntityId),
}

impl TrackedEdit {
    pub fn from_command(command: &Command) -> Option<Self> {
        match command {
            Command::AssignTask { id, .. } => Some(Self::Quest(*id)),
            Command::MoveTask { id, .. } | Command::SetTaskDue { id, .. } => Some(Self::Dates(*id)),
            Command::PlaceTask { id, placement, today, .. } => Some(Self::Placement(*id, placement.list(), *today)),
            Command::CompleteTask { id, .. }
            | Command::CompleteTaskFromPlan { id, .. }
            | Command::ReopenTask { id } => Some(Self::Completion(*id)),
            Command::EditPlan { edit, .. } => Self::from_command(edit),
            _ => None,
        }
    }

    pub fn id(self) -> EntityId {
        match self {
            Self::Quest(id) | Self::Dates(id) | Self::Placement(id, ..) | Self::Completion(id) => id,
        }
    }
}

impl State {
    pub(super) fn remember_task_edit(&mut self, edit: TrackedEdit, before: Task, old: &Snapshot) {
        let Some(after) = self.snap.task(before.id) else { return };
        let change = match edit {
            TrackedEdit::Dates(_) => TaskEdit::dates(&before, after, old, &self.snap),
            TrackedEdit::Placement(_, list, today) => TaskEdit::placement(&before, after, old, &self.snap, list, today),
            TrackedEdit::Quest(id) => {
                (before.quest_id != after.quest_id).then_some(TaskEdit::Quest { id, previous: before.quest_id })
            }
            TrackedEdit::Completion(id) => {
                let change = TaskEdit::completion(&before, after);
                if change.is_some() {
                    if let Some(at) = after.completed_at {
                        self.lingering_tasks.insert(id, (at, before));
                    } else {
                        self.lingering_tasks.remove(&id);
                    }
                }
                change
            }
        };
        if let Some(change) = change {
            let event = if matches!(edit, TrackedEdit::Placement(..)) {
                UndoEvent::TaskPlaced(change)
            } else {
                UndoEvent::TaskEdited(change)
            };
            self.step_undo(event);
        }
    }

    pub(super) fn undo_task_edit(&mut self, edit: TaskEdit) {
        let command = edit.command(self.clock.now());
        if !self.auth.locks_edits() {
            self.dispatch_task_change(command, false);
        }
    }

    pub fn lingering(&self, task: &Task) -> Option<&Task> {
        self.lingering_tasks
            .get(&task.id)
            .filter(|(at, _)| undo::lingering(*at, task, self.now))
            .map(|(_, before)| before)
    }

    pub(super) fn refresh_task_fields(&mut self, previous: DateTime<Utc>, tx: &mut Tx) -> bool {
        let mut changed = false;
        self.lingering_tasks.retain(|id, (at, _)| {
            let until = undo::linger_until(*at);
            if self.now >= until {
                changed |= previous < until;
                return false;
            }
            tx.wake_after(crate::clock::wake_for(self.now, until));
            self.snap.task(*id).is_some_and(|task| task.completed_at == Some(*at))
        });
        let picker = self.task_calendar.is_some_and(|c| c.task.is_none())
            || self.quest_picker.as_ref().is_some_and(|q| q.target == PickFor::QuickAdd);
        let has_focus = tx.focused().is_some_and(|key| key.starts_with("quick-add"));
        if has_focus && self.task_modal.is_some() {
            self.close_task_modal();
            tx.animated();
            changed = true;
        }
        if !has_focus && !picker && self.quick_add.text.trim().is_empty() {
            let was_open = self.plan_quick_day.take().is_some();
            if was_open || self.quick_fields.is_some() {
                self.reset_quick_add();
                changed = true;
            }
        }
        changed
    }

    pub fn reset_quick_add(&mut self) {
        self.quick_add.clear();
        self.quick_fields = None;
        self.quick_choice = 0;
    }

    pub fn quick_defaults(&self) -> quick_add::Fields {
        let mut fields = quick_add::Fields::for_view(self.view, self.today());
        if self.view == View::Plan {
            fields.dates.planned_on = self.plan_quick_day;
        }
        fields
    }

    pub fn quick_base(&self) -> quick_add::Fields {
        self.quick_fields.unwrap_or_else(|| self.quick_defaults())
    }

    pub fn quick_draft(&self) -> quick_add::Draft {
        quick_add::parse(
            &self.quick_add.text,
            self.quick_base(),
            self.today(),
            &quest_view::quest_picker::recent_quests(&self.snap, self.now),
        )
    }

    pub fn save_quick_add(&mut self) {
        if let Some(command) = self.quick_draft().command(EntityId::new(), Position::Bottom) {
            self.dispatch(command);
            if self.notice.is_none() {
                self.reset_quick_add();
            }
        }
    }

    pub fn field_dates(&self, task: Option<EntityId>) -> Option<TaskDates> {
        match task {
            None => Some(self.quick_draft().fields.dates),
            Some(task) => self.plan_task(task).map(|t| t.dates()),
        }
    }

    pub fn open_calendar(&mut self, task: Option<EntityId>, field: DateField, tx: &mut Tx) {
        let Some(dates) = self.field_dates(task) else { return };
        self.quest_picker = None;
        self.task_calendar = Some(Calendar { task, field, cursor: calendar::initial(dates, field, self.today()) });
        if task.is_none() {
            self.quick_fields = Some(self.quick_base());
        }
        tx.focus_when_shown("task-calendar", true);
    }

    pub fn close_calendar(&mut self, tx: &mut Tx) {
        if let Some(calendar) = self.task_calendar.take() {
            let anchor = if calendar.task.is_none() { "quick-add".into() } else { calendar.anchor() };
            tx.focus_when_shown(anchor, tx.focus_visible());
        }
    }

    pub fn choose_calendar(&mut self, day: Option<NaiveDate>, tx: &mut Tx) {
        let Some(calendar) = self.task_calendar else { return };
        if day.is_some_and(|day| day < self.today()) {
            return;
        }
        self.close_calendar(tx);
        let Some(id) = calendar.task else {
            let mut fields = self.commit_quick_tokens();
            fields.dates = quest_view::calendar::set_date(fields.dates, calendar.field, day);
            self.quick_fields = Some(fields);
            return;
        };
        if self.field_dates(Some(id)).is_none_or(|dates| dates.get(calendar.field) == day) {
            return;
        }
        let at = self.clock.now();
        self.dispatch(match calendar.field {
            DateField::Plan => Command::MoveTask { id, planned_on: day, position: Position::Bottom, at },
            DateField::Due => Command::SetTaskDue { id, due_on: day, today: self.today(), at },
        });
    }

    pub fn commit_quick_tokens(&mut self) -> quick_add::Fields {
        let draft = self.quick_draft();
        if draft.title != self.quick_add.text.trim() {
            self.quick_add = rune::TextEdit::new(draft.title);
        }
        draft.fields
    }

    pub fn quick_hosted(&self) -> bool {
        self.overlay == Overlay::None && matches!(self.view, View::Today | View::Inbox | View::Quest(_))
    }
}
