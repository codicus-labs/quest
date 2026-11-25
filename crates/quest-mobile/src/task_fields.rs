use quest_domain::{Command, DateField, EntityId, Position, TaskDates};
use quest_view::{
    calendar::{self, QuickDate, Step},
    nav::View,
    quest_picker,
    quick_add::{self, Draft, Fields},
};

use crate::guard::guard;
use crate::home::chip;
use crate::ids::{parse_id, parse_iso_day};
use crate::task_detail::day_value;
use crate::task_types::LoggedEntry;
use crate::time::Now;
use crate::{DayValue, MobileError, Moment, QuestChip, QuestCore};

#[derive(Clone, Debug, uniffi::Enum)]
pub enum QuickAddContext {
    Inbox,
    Today,
    Quest { id: String },
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct QuickFields {
    pub quest_id: Option<String>,
    pub planned_on: Option<String>,
    pub due_on: Option<String>,
}

impl QuickFields {
    fn parse(&self) -> Result<Fields, MobileError> {
        let planned_on = self.planned_on.as_deref().map(parse_iso_day).transpose()?;
        let due_on = self.due_on.as_deref().map(parse_iso_day).transpose()?;
        Ok(Fields {
            quest: self.quest_id.as_deref().map(parse_id).transpose()?,
            dates: calendar::set_date(TaskDates { planned_on, due_on }, DateField::Due, due_on),
        })
    }

    fn from_fields(fields: Fields) -> Self {
        Self {
            quest_id: fields.quest.map(|id| id.to_string()),
            planned_on: fields.dates.planned_on.map(|d| d.to_string()),
            due_on: fields.dates.due_on.map(|d| d.to_string()),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct QuickPreview {
    pub title: String,
    pub fields: QuickFields,
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum DateTarget {
    Plan,
    Deadline,
}

impl From<DateTarget> for DateField {
    fn from(target: DateTarget) -> Self {
        match target {
            DateTarget::Plan => Self::Plan,
            DateTarget::Deadline => Self::Due,
        }
    }
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum CalendarStep {
    Day,
    Week,
    Month,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct DateChoice {
    pub label: String,
    pub date: Option<DayValue>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct CalendarCell {
    pub day: DayValue,
    pub in_month: bool,
    pub enabled: bool,
    pub selected: bool,
    pub today: bool,
    pub marker: Option<DateTarget>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct CalendarMonth {
    pub cursor: DayValue,
    pub label: String,
    pub quick_choices: Vec<DateChoice>,
    pub cells: Vec<CalendarCell>,
}

fn draft(snapshot: &quest_domain::Snapshot, text: &str, defaults: Fields, now: Now) -> Draft {
    quick_add::parse(text, defaults, now.today(), &quest_picker::recent_quests(snapshot, now.at))
}

#[uniffi::export]
impl QuestCore {
    pub fn task_logged_entries(&self, task_id: String, now: Moment) -> Result<Vec<LoggedEntry>, MobileError> {
        guard(|| {
            let task = self.task(&task_id)?;
            let now = Now::parse(&now)?;
            let snapshot = self.store().snapshot();
            Ok(crate::task_detail::logged_entries(&snapshot, task.id, now).collect())
        })
    }

    pub fn preview_quick_add(
        &self,
        text: String,
        defaults: QuickFields,
        now: Moment,
    ) -> Result<QuickPreview, MobileError> {
        guard(|| {
            let draft = draft(&self.store().snapshot(), &text, defaults.parse()?, Now::parse(&now)?);
            Ok(QuickPreview { title: draft.title, fields: QuickFields::from_fields(draft.fields) })
        })
    }

    pub fn quick_add_quest_suggestions(&self, query: String, now: Moment) -> Result<Vec<QuestChip>, MobileError> {
        guard(|| {
            Ok(quick_add::quest_suggestions(&self.store().snapshot(), Now::parse(&now)?.at, &query)
                .into_iter()
                .map(chip)
                .collect())
        })
    }

    pub fn task_quest_options(&self, query: String, now: Moment) -> Result<Vec<QuestChip>, MobileError> {
        guard(|| {
            Ok(quest_picker::recent_quests(&self.store().snapshot(), Now::parse(&now)?.at)
                .into_iter()
                .filter(|quest| quest_picker::matches(&quest.title, &query))
                .map(chip)
                .collect())
        })
    }

    pub fn add_quick_draft(
        &self,
        text: String,
        description: String,
        defaults: QuickFields,
        now: Moment,
    ) -> Result<String, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let fields = defaults.parse()?;
            self.edit(|store| {
                let id = EntityId::new();
                let create = draft(&store.snapshot(), &text, fields, now)
                    .command(id, Position::Bottom)
                    .ok_or(MobileError::EmptyTitle)?;
                let mut commands = vec![create];
                if !description.is_empty() {
                    commands.push(Command::SetTaskDescription { id, description });
                }
                store.dispatch_all(&commands).map_err(|e| MobileError::Rejected { reason: e.to_string() })?;
                Ok(id.to_string())
            })
        })
    }
}

#[uniffi::export]
pub fn quick_add_defaults(context: QuickAddContext, now: Moment) -> Result<QuickFields, MobileError> {
    guard(|| {
        let view = match context {
            QuickAddContext::Inbox => View::Inbox,
            QuickAddContext::Today => View::Today,
            QuickAddContext::Quest { id } => View::Quest(parse_id(&id)?),
        };
        Ok(QuickFields::from_fields(Fields::for_view(view, Now::parse(&now)?.today())))
    })
}

#[uniffi::export]
pub fn update_quick_date(
    fields: QuickFields,
    target: DateTarget,
    date: Option<String>,
) -> Result<QuickFields, MobileError> {
    guard(|| {
        let day = date.as_deref().map(parse_iso_day).transpose()?;
        let mut fields = fields.parse()?;
        fields.dates = calendar::set_date(fields.dates, target.into(), day);
        Ok(QuickFields::from_fields(fields))
    })
}

#[uniffi::export]
pub fn task_calendar(
    cursor: Option<String>,
    fields: QuickFields,
    target: DateTarget,
    now: Moment,
) -> Result<CalendarMonth, MobileError> {
    guard(|| {
        let today = Now::parse(&now)?.today();
        let dates = fields.parse()?.dates;
        let field = target.into();
        let cursor = cursor
            .as_deref()
            .map(parse_iso_day)
            .transpose()?
            .unwrap_or_else(|| calendar::initial(dates, field, today))
            .max(today);
        let quick_choices = QuickDate::ALL
            .into_iter()
            .map(|choice| DateChoice { label: choice.label().into(), date: choice.date(today).map(day_value) })
            .collect();
        let cells = calendar::month(cursor, today, dates, field)
            .into_iter()
            .map(|day| CalendarCell {
                day: day_value(day.date),
                in_month: day.in_month,
                enabled: day.enabled,
                selected: day.selected,
                today: day.today,
                marker: day.marker.map(|field| match field {
                    DateField::Plan => DateTarget::Plan,
                    DateField::Due => DateTarget::Deadline,
                }),
            })
            .collect();
        Ok(CalendarMonth { cursor: day_value(cursor), label: cursor.format("%B %Y").to_string(), quick_choices, cells })
    })
}

#[uniffi::export]
pub fn shift_calendar_date(
    cursor: String,
    step: CalendarStep,
    forward: bool,
    now: Moment,
) -> Result<DayValue, MobileError> {
    guard(|| {
        let step = match step {
            CalendarStep::Day => Step::Day,
            CalendarStep::Week => Step::Week,
            CalendarStep::Month => Step::Month,
        };
        Ok(day_value(calendar::shift(parse_iso_day(&cursor)?, step, forward, Now::parse(&now)?.today())))
    })
}

#[uniffi::export]
pub fn completion_linger_ms() -> i64 {
    quest_view::undo::LINGER_SECONDS * 1_000
}
