use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Datelike, Days, Months, NaiveDate, TimeDelta, Utc, Weekday};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::change::{EntityId, EntityKind, FieldWrite};
use crate::command::{Command, CommandError, plan};
use crate::model::{Task, TimeBlock, fields};
use crate::snapshot::Snapshot;
use crate::sort_key::sort_key_between;

const MAX_OCCURRENCES_PER_PASS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    Day,
    Week,
    Month,
    Year,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatPin {
    #[default]
    None,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonthDay {
    Day(u32),
    Last,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Weekdays(u8);

impl Weekdays {
    pub fn on(day: Weekday) -> Self {
        Self(1 << day.num_days_from_monday())
    }

    pub fn contains(self, day: Weekday) -> bool {
        self.0 & Self::on(day).0 != 0
    }

    pub fn toggled(self, day: Weekday) -> Self {
        Self(self.0 ^ Self::on(day).0)
    }

    pub fn is_empty(self) -> bool {
        self.0 & 0b111_1111 == 0
    }

    pub fn iter(self) -> impl Iterator<Item = Weekday> {
        const DAYS: [Weekday; 7] =
            [Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri, Weekday::Sat, Weekday::Sun];
        DAYS.into_iter().filter(move |day| self.contains(*day))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepeatRule {
    pub every: u32,
    pub frequency: Frequency,
    pub starts: NaiveDate,
    pub ends: Option<NaiveDate>,
    pub time_zone: Tz,
    #[serde(default)]
    pub pin: RepeatPin,
    #[serde(default)]
    pub weekdays: Weekdays,
    #[serde(default)]
    pub month_day: Option<MonthDay>,
}

impl RepeatRule {
    pub fn date_at(self, now: DateTime<Utc>) -> NaiveDate {
        now.with_timezone(&self.time_zone).date_naive()
    }

    fn valid(self) -> bool {
        (1..=999).contains(&self.every)
            && self.ends.is_none_or(|end| end >= self.starts)
            && !matches!(self.month_day, Some(MonthDay::Day(day)) if !(1..=31).contains(&day))
    }

    fn day_selection(self) -> Weekdays {
        if self.weekdays.is_empty() { Weekdays::on(self.starts.weekday()) } else { self.weekdays }
    }

    fn date(self, index: u32) -> Option<NaiveDate> {
        if self.frequency == Frequency::Week {
            let days = self.day_selection();
            let count = days.iter().count() as u32;
            let weekday = days.iter().nth((index % count) as usize)?;
            let weeks = i64::from(self.every) * i64::from(index / count);
            let delta = 7 * weeks + i64::from(weekday.num_days_from_monday())
                - i64::from(self.starts.weekday().num_days_from_monday());
            return self.starts.checked_add_signed(TimeDelta::try_days(delta)?);
        }
        let step = self.every.checked_mul(index)?;
        match self.frequency {
            Frequency::Day => self.starts.checked_add_days(Days::new(u64::from(step))),
            Frequency::Month => match self.month_day {
                None => self.starts.checked_add_months(Months::new(step)),
                Some(selection) => {
                    let first = self.starts.with_day(1)?.checked_add_months(Months::new(step))?;
                    let last = first.checked_add_months(Months::new(1))?.pred_opt()?;
                    let wanted = match selection {
                        MonthDay::Day(day) => day.min(last.day()),
                        MonthDay::Last => last.day(),
                    };
                    first.with_day(wanted)
                }
            },
            Frequency::Year => self.starts.checked_add_months(Months::new(step.checked_mul(12)?)),
            Frequency::Week => unreachable!(),
        }
    }

    fn first_index(self, from: NaiveDate) -> u32 {
        let from = from.max(self.starts);
        let months = (from.year() - self.starts.year()) * 12 + from.month() as i32 - self.starts.month() as i32;
        let days = (from - self.starts).num_days() as u32;
        let (steps, dates_per_step) = match self.frequency {
            Frequency::Day => (days, 1),
            Frequency::Week => {
                ((days + self.starts.weekday().num_days_from_monday()) / 7, self.day_selection().iter().count() as u32)
            }
            Frequency::Month => (months as u32, 1),
            Frequency::Year => (months as u32 / 12, 1),
        };
        steps / self.every * dates_per_step
    }

    fn dates_from(self, from: NaiveDate) -> impl Iterator<Item = NaiveDate> {
        let from = from.max(self.starts);
        self.valid()
            .then(|| self.first_index(from))
            .into_iter()
            .flat_map(move |index| (index..).map_while(move |index| self.date(index)))
            .filter(move |day| *day >= from)
            .take_while(move |day| self.ends.is_none_or(|end| *day <= end))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepeatSchedule {
    pub rule: RepeatRule,
    effective_from: NaiveDate,
    source_day: Option<NaiveDate>,
}

impl RepeatSchedule {
    pub fn next_on_or_after(self, day: NaiveDate) -> Option<NaiveDate> {
        self.rule.dates_from(day.max(self.effective_from)).next()
    }
}

// User edits live in the ordinary task fields and take precedence over this syncable initial value.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Occurrence {
    pub series: EntityId,
    pub day: NaiveDate,
    pub title: String,
    pub description: String,
    pub sort_key: String,
    pub quest_id: Option<EntityId>,
    pub block: Option<TimeBlock>,
}

impl Occurrence {
    fn new(task: &Task, day: NaiveDate, sort_key: String) -> Self {
        Self {
            series: task.id,
            day,
            title: task.title.clone(),
            description: task.description.clone(),
            sort_key,
            quest_id: task.quest_id,
            block: task.block,
        }
    }

    fn task(&self, id: EntityId) -> Task {
        Task {
            id,
            title: self.title.clone(),
            quest_id: self.quest_id,
            day: Some(self.day),
            sort_key: self.sort_key.clone(),
            block: self.block,
            completed_at: None,
            description: self.description.clone(),
            repeat: None,
            series: Some(self.series),
        }
    }

    fn write(&self, id: EntityId) -> FieldWrite {
        task_write(id, fields::TASK_OCCURRENCE, json!(self))
    }
}

impl Snapshot {
    /// Projects the plan's occurrences for one day without creating any records.
    pub fn planned_occurrences(&self, day: NaiveDate, now: DateTime<Utc>) -> impl Iterator<Item = Task> {
        self.occurrences_on(day, now).map(|(id, occurrence)| occurrence.task(id))
    }

    fn occurrences_on(&self, day: NaiveDate, now: DateTime<Utc>) -> impl Iterator<Item = (EntityId, Occurrence)> {
        let mut order = None;
        self.tasks.iter().filter_map(move |task| {
            let schedule = task.repeat?;
            if day < schedule.rule.date_at(now)
                || schedule.source_day == Some(day)
                || schedule.next_on_or_after(day) != Some(day)
            {
                return None;
            }
            let id = occurrence_id(task.id, day);
            if self.task(id).is_some() || self.trash.iter().any(|item| item.kind == EntityKind::Task && item.id == id) {
                return None;
            }
            let order = order.get_or_insert_with(|| DayOrder::from_tasks(self, day));
            Some((id, Occurrence::new(task, day, order.insert(schedule.rule.pin))))
        })
    }
}

pub(crate) fn edit_plan(
    snapshot: &Snapshot,
    occurrence: Option<(EntityId, NaiveDate)>,
    now: DateTime<Utc>,
    edit: &Command,
) -> Result<Vec<FieldWrite>, CommandError> {
    let mut writes = Vec::new();
    let mut tasks = Vec::new();
    if let Some((series, day)) = occurrence {
        let id = occurrence_id(series, day);
        if snapshot.task(id).is_none() {
            let initial = snapshot
                .occurrences_on(day, now)
                .find(|(candidate, _)| *candidate == id)
                .map(|(_, initial)| initial)
                .ok_or(CommandError::UnknownTask(id))?;
            tasks.push(initial.task(id));
            writes.push(initial.write(id));
        }
    }
    if let Command::MoveTask { day: Some(day), .. } = edit {
        // A drop can use a preview as a sorting anchor without saving that preview.
        tasks.extend(
            snapshot
                .planned_occurrences(*day, now)
                .filter(|task| occurrence.is_none_or(|(series, date)| task.id != occurrence_id(series, date))),
        );
    }
    if tasks.is_empty() {
        return plan(edit, snapshot);
    }
    let mut projected = snapshot.clone();
    projected.tasks.extend(tasks);
    projected.tasks.sort_by(|a, b| a.sort_key.cmp(&b.sort_key).then(a.id.cmp(&b.id)));
    writes.extend(plan(edit, &projected)?);
    Ok(writes)
}

pub(crate) fn set_rule(
    snapshot: &Snapshot,
    id: EntityId,
    rule: Option<RepeatRule>,
    now: DateTime<Utc>,
) -> Result<Vec<FieldWrite>, CommandError> {
    let task = snapshot.task(id).ok_or(CommandError::UnknownTask(id))?;
    if rule == task.repeat.map(|schedule| schedule.rule) {
        return Ok(Vec::new());
    }
    let Some(rule) = rule else { return Ok(vec![task_write(id, fields::TASK_REPEAT, serde_json::Value::Null)]) };
    if !rule.valid() {
        return Err(CommandError::InvalidRepeatRule);
    }
    let mut writes = Vec::new();
    let source_day = match task.repeat {
        Some(previous) => previous.source_day,
        None if task.day.is_none() && !task.is_done() => {
            let first = rule.dates_from(rule.starts).next();
            if let Some(day) = first {
                let key = DayOrder::from_tasks(snapshot, day).insert(rule.pin);
                writes.push(task_write(id, fields::TASK_DAY, json!(day)));
                writes.push(task_write(id, fields::SORT_KEY, json!(key)));
            }
            first
        }
        None => task.day,
    };
    let effective_from = if task.repeat.is_some() { rule.date_at(now) } else { rule.starts };
    let schedule = RepeatSchedule { rule, effective_from, source_day };
    writes.push(task_write(id, fields::TASK_REPEAT, json!(schedule)));
    Ok(writes)
}

#[derive(Default)]
struct DayOrder {
    first: Option<String>,
    last: Option<String>,
}

impl DayOrder {
    fn from_tasks(snapshot: &Snapshot, day: NaiveDate) -> Self {
        let mut tasks = snapshot.tasks_on(day);
        let first = tasks.next().map(|task| task.sort_key.clone());
        let last = tasks.last().map(|task| task.sort_key.clone()).or_else(|| first.clone());
        Self { first, last }
    }

    fn insert(&mut self, pin: RepeatPin) -> String {
        let key = match pin {
            RepeatPin::Top => sort_key_between(None, self.first.as_deref()),
            RepeatPin::None | RepeatPin::Bottom => sort_key_between(self.last.as_deref(), None),
        };
        if self.first.is_none() || pin == RepeatPin::Top {
            self.first = Some(key.clone());
        }
        if self.last.is_none() || pin != RepeatPin::Top {
            self.last = Some(key.clone());
        }
        key
    }
}

pub(crate) fn generate_due(snapshot: &Snapshot, now: DateTime<Utc>) -> Vec<FieldWrite> {
    if !snapshot.tasks.iter().any(|task| task.repeat.is_some()) {
        return Vec::new();
    }
    let present: HashSet<_> = snapshot
        .tasks
        .iter()
        .map(|task| task.id)
        .chain(snapshot.trash.iter().filter(|item| item.kind == EntityKind::Task).map(|item| item.id))
        .collect();
    let mut writes = Vec::new();
    let mut keys = HashMap::new();
    for task in &snapshot.tasks {
        let Some(schedule) = task.repeat else { continue };
        let today = schedule.rule.date_at(now);
        for day in schedule.rule.dates_from(schedule.effective_from).take_while(|day| *day <= today) {
            let id = occurrence_id(task.id, day);
            if schedule.source_day == Some(day) || present.contains(&id) {
                continue;
            }
            let order = keys.entry(day).or_insert_with(|| DayOrder::from_tasks(snapshot, day));
            let initial = Occurrence::new(task, day, order.insert(schedule.rule.pin));
            writes.push(initial.write(id));
            if writes.len() == MAX_OCCURRENCES_PER_PASS {
                return writes;
            }
        }
    }
    writes
}

fn task_write(id: EntityId, field: &'static str, value: serde_json::Value) -> FieldWrite {
    FieldWrite { kind: EntityKind::Task, id, field, value }
}

fn occurrence_id(series: EntityId, day: NaiveDate) -> EntityId {
    EntityId(Uuid::new_v5(&series.0, day.to_string().as_bytes()))
}

#[cfg(test)]
mod tests;
