//! Strict validation of the model's answer. The answer is untrusted: anything malformed, out of
//! range or pointing at unknown data is dropped (and listed in `dropped`) or marked uncertain.
//! Keys are renumbered (`q1`, `t1`, `w1`), so they are unique across the whole proposal.

use std::collections::BTreeMap;

use chrono::{Days, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Timelike, Weekday};
use chrono_tz::Tz;
use quest_domain::{EntityId, Frequency, MonthDay, TimeBlock};
use quest_protocol::{
    CaptureProposal, CaptureRequest, DropReason, Dropped, ProposedMilestone, ProposedQuest, ProposedRepeat,
    ProposedTask, ProposedWork, QuestRef, TempKey, WorkTarget,
};
use serde_json::{Map, Value};

use crate::text::{multi_line, shorten, single_line};

/// The answer cannot be used at all.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the AI answer is not a proposal: {0}")]
pub struct InvalidOutput(pub String);

/// Most items per list; more are dropped.
pub(crate) const MAX_ITEMS: usize = 30;
pub(crate) const MAX_TITLE_CHARS: usize = 200;
pub(crate) const MAX_DESCRIPTION_CHARS: usize = 2000;
const MAX_MILESTONES: usize = 10;
const MAX_KEY_CHARS: usize = 40;
/// Dates further away than this are a misunderstanding (or garbage).
const MAX_DAYS_BACK: u64 = 366;
const MAX_DAYS_AHEAD: u64 = 3 * 366;
const MAX_REPEAT_EVERY: u32 = 99;
const MIN_BLOCK_MINUTES: u32 = 5;
/// How far past "now" a logged session may end.
const WORK_END_TOLERANCE_MINUTES: i64 = 60;
/// How the dropped list names an item without a usable title.
const UNTITLED: &str = "(untitled)";
const DROPPED_TITLE_CHARS: usize = 60;

/// Turns the model's answer (shaped like [`crate::output_schema`]) into a proposal for `request`.
pub fn validate(output: &Value, request: &CaptureRequest, time_zone: Tz) -> Result<CaptureProposal, InvalidOutput> {
    let root = output.as_object().ok_or_else(|| InvalidOutput("expected a JSON object".into()))?;
    let lists = ["new_quests", "tasks", "work_logs"];
    if !lists.iter().any(|name| root.contains_key(*name)) {
        return Err(InvalidOutput("none of new_quests, tasks, work_logs is present".into()));
    }
    if let Some(name) = lists.iter().find(|name| root.get(**name).is_some_and(|v| !v.is_array() && !v.is_null())) {
        return Err(InvalidOutput(format!("{name} is not a list")));
    }
    let now = request.now.with_timezone(&time_zone).naive_local();
    let mut checker = Checker { request, now, dropped: Vec::new(), quest_keys: BTreeMap::new() };
    let quests = checker.list(root, "new_quests", Checker::quest);
    let tasks = checker.list(root, "tasks", Checker::task);
    let work = checker.list(root, "work_logs", Checker::work);
    let quests = quests.into_iter().enumerate().map(|(i, q)| ProposedQuest { key: key('q', i), ..q }).collect();
    let tasks = tasks.into_iter().enumerate().map(|(i, t)| ProposedTask { key: key('t', i), ..t }).collect();
    let work = work.into_iter().enumerate().map(|(i, w)| ProposedWork { key: key('w', i), ..w }).collect();
    Ok(CaptureProposal { quests, tasks, work, dropped: checker.dropped })
}

fn key(prefix: char, index: usize) -> TempKey {
    TempKey(format!("{prefix}{}", index + 1))
}

/// Why one item was dropped.
struct Drop(DropReason);

struct Checker<'a> {
    request: &'a CaptureRequest,
    now: NaiveDateTime,
    dropped: Vec<Dropped>,
    /// The model's quest keys → the renumbered ones, for the quests kept.
    quest_keys: BTreeMap<String, TempKey>,
}

impl Checker<'_> {
    fn today(&self) -> NaiveDate {
        self.now.date()
    }

    /// Checks every item of `root[name]`; too many items or bad ones go to `dropped`.
    fn list<T>(
        &mut self,
        root: &Map<String, Value>,
        name: &str,
        check: impl Fn(&mut Self, &Map<String, Value>) -> Result<T, Drop>,
    ) -> Vec<T> {
        let Some(items) = root.get(name).and_then(Value::as_array) else { return Vec::new() };
        let mut kept = Vec::new();
        for item in items {
            let Some(fields) = item.as_object() else {
                self.drop_item(UNTITLED, DropReason::Incomplete);
                continue;
            };
            let title = shown_title(fields);
            if kept.len() == MAX_ITEMS {
                self.drop_item(&title, DropReason::TooLong);
                continue;
            }
            match check(self, fields) {
                Ok(value) => kept.push(value),
                Err(Drop(reason)) => self.drop_item(&title, reason),
            }
        }
        kept
    }

    fn drop_item(&mut self, title: &str, reason: DropReason) {
        self.dropped.push(Dropped { title: shorten(title, DROPPED_TITLE_CHARS), reason });
    }

    fn quest(&mut self, fields: &Map<String, Value>) -> Result<ProposedQuest, Drop> {
        let raw_key = string(fields, "key").map(single_line).filter(|k| !k.is_empty() && k.len() <= MAX_KEY_CHARS);
        let raw_key = raw_key.ok_or(Drop(DropReason::Incomplete))?;
        if self.quest_keys.contains_key(&raw_key) {
            return Err(Drop(DropReason::Incomplete));
        }
        let title = title(fields, "title")?;
        let description = description(fields)?;
        let mut milestones = Vec::new();
        for item in fields.get("milestones").and_then(Value::as_array).into_iter().flatten() {
            match self.milestone(item) {
                Ok(_) if milestones.len() == MAX_MILESTONES => self.drop_item(&title, DropReason::TooLong),
                Ok(milestone) => milestones.push(milestone),
                Err(Drop(reason)) => {
                    let shown = item.as_object().map(shown_title).unwrap_or_else(|| UNTITLED.to_owned());
                    self.drop_item(&shown, reason);
                }
            }
        }
        let key = key('q', self.quest_keys.len());
        self.quest_keys.insert(raw_key, key.clone());
        Ok(ProposedQuest { key, title, description, milestones })
    }

    fn milestone(&self, item: &Value) -> Result<ProposedMilestone, Drop> {
        let fields = item.as_object().ok_or(Drop(DropReason::Incomplete))?;
        let title = title(fields, "title")?;
        let achieved_on = match string(fields, "date") {
            None => self.today(),
            Some(text) => self.date(text)?,
        };
        Ok(ProposedMilestone { title, achieved_on })
    }

    fn task(&mut self, fields: &Map<String, Value>) -> Result<ProposedTask, Drop> {
        let title = title(fields, "title")?;
        let description = description(fields)?;
        let (quest, mut quest_uncertain) = self.quest_ref(fields);
        quest_uncertain |= low_confidence(fields, "quest_confidence");
        let day = string(fields, "day").map(|text| self.date(text)).transpose()?;
        // A faulty block or repeat is left out; the task stays.
        let block = self.task_block(fields, day).unwrap_or_else(|Drop(reason)| {
            self.drop_item(&title, reason);
            None
        });
        let repeat = self.task_repeat(fields, day).unwrap_or_else(|Drop(reason)| {
            self.drop_item(&title, reason);
            None
        });
        Ok(ProposedTask { key: TempKey(String::new()), title, description, quest, quest_uncertain, day, block, repeat })
    }

    fn task_block(&self, fields: &Map<String, Value>, day: Option<NaiveDate>) -> Result<Option<TimeBlock>, Drop> {
        let Some(start) = string(fields, "start_time") else {
            // A duration without a start is an estimate: there are none.
            return Ok(None);
        };
        if day.is_none() {
            return Err(Drop(DropReason::Incomplete));
        }
        let start = time(start).ok_or(Drop(DropReason::InvalidDate))?;
        // A start without a length: an hour, like a block dropped on the calendar.
        let minutes = integer(fields, "duration_minutes").unwrap_or(60);
        block(start, minutes).map(Some)
    }

    fn task_repeat(&self, fields: &Map<String, Value>, day: Option<NaiveDate>) -> Result<Option<ProposedRepeat>, Drop> {
        let Some(value) = fields.get("repeat").filter(|v| !v.is_null()) else { return Ok(None) };
        let day = day.ok_or(Drop(DropReason::Incomplete))?;
        self.repeat(value, day).map(Some)
    }

    fn repeat(&self, value: &Value, starts: NaiveDate) -> Result<ProposedRepeat, Drop> {
        let fields = value.as_object().ok_or(Drop(DropReason::Incomplete))?;
        let every = integer(fields, "every").filter(|n| (1..=i64::from(MAX_REPEAT_EVERY)).contains(n));
        let every = every.ok_or(Drop(DropReason::Incomplete))? as u32;
        let frequency = match string(fields, "unit") {
            Some("day") => Frequency::Day,
            Some("week") => Frequency::Week,
            Some("month") => Frequency::Month,
            Some("year") => Frequency::Year,
            _ => return Err(Drop(DropReason::Incomplete)),
        };
        let mut weekdays = Vec::new();
        for day in fields.get("weekdays").and_then(Value::as_array).into_iter().flatten() {
            let day = day.as_str().and_then(weekday).ok_or(Drop(DropReason::Incomplete))?;
            if !weekdays.contains(&day) {
                weekdays.push(day);
            }
        }
        let month_day = match integer(fields, "month_day") {
            None => None,
            Some(-1) => Some(MonthDay::Last),
            Some(day @ 1..=31) => Some(MonthDay::Day(day as u32)),
            Some(_) => return Err(Drop(DropReason::InvalidDate)),
        };
        let ends = string(fields, "until").map(|text| self.date(text)).transpose()?;
        if ends.is_some_and(|end| end < starts) {
            return Err(Drop(DropReason::InvalidDate));
        }
        let weekdays = if frequency == Frequency::Week { weekdays } else { Vec::new() };
        let month_day = month_day.filter(|_| frequency == Frequency::Month);
        Ok(ProposedRepeat { every, frequency, weekdays, month_day, ends })
    }

    fn work(&mut self, fields: &Map<String, Value>) -> Result<ProposedWork, Drop> {
        let day = self.date(string(fields, "date").ok_or(Drop(DropReason::Incomplete))?)?;
        let start = string(fields, "start_time").ok_or(Drop(DropReason::Incomplete))?;
        let start = time(start).ok_or(Drop(DropReason::InvalidDate))?;
        let end = string(fields, "end_time").ok_or(Drop(DropReason::Incomplete))?;
        let end = end_time(end).ok_or(Drop(DropReason::InvalidDate))?;
        if !crate::apply::is_time_range(start, end) {
            return Err(Drop(DropReason::InvalidTimeRange));
        }
        // Work is done already; a rounded end ("9 to 11" said at 10:30) may lie a little ahead.
        let end_day = if end == NaiveTime::MIN { day.succ_opt() } else { Some(day) };
        let latest_end = self.now + TimeDelta::minutes(WORK_END_TOLERANCE_MINUTES);
        if end_day.is_none_or(|end_day| end_day.and_time(end) > latest_end) {
            return Err(Drop(DropReason::InvalidTimeRange));
        }
        let mut uncertain = low_confidence(fields, "confidence");
        let task_id = string(fields, "task_id");
        let known = task_id.and_then(|id| id.parse::<EntityId>().ok()).filter(|id| self.known_task(*id));
        let task = match (known, string(fields, "new_task_title")) {
            (Some(id), _) => WorkTarget::Existing { id },
            (None, Some(_)) => {
                uncertain |= task_id.is_some();
                let (quest, quest_uncertain) = self.quest_ref(fields);
                uncertain |= quest_uncertain;
                WorkTarget::New { title: title(fields, "new_task_title")?, quest }
            }
            (None, None) if task_id.is_some() => return Err(Drop(DropReason::UnknownTask)),
            (None, None) => return Err(Drop(DropReason::Incomplete)),
        };
        Ok(ProposedWork { key: TempKey(String::new()), task, uncertain, day, start, end })
    }

    /// The quest an item names: an existing one from the context or a kept new one. Anything else
    /// is no quest, marked uncertain.
    fn quest_ref(&self, fields: &Map<String, Value>) -> (Option<QuestRef>, bool) {
        let existing = string(fields, "quest_id");
        let new = string(fields, "new_quest_key").map(single_line);
        if let Some(id) = existing.and_then(|id| id.parse::<EntityId>().ok()).filter(|id| self.known_quest(*id)) {
            return (Some(QuestRef::Existing { id }), new.is_some());
        }
        if let Some(key) = new.as_ref().and_then(|key| self.quest_keys.get(key)) {
            return (Some(QuestRef::New { key: key.clone() }), existing.is_some());
        }
        (None, existing.is_some() || new.is_some())
    }

    fn known_quest(&self, id: EntityId) -> bool {
        self.request.context.quests.iter().any(|q| q.id == id)
    }

    fn known_task(&self, id: EntityId) -> bool {
        self.request.context.tasks.iter().any(|t| t.id == id)
    }

    /// A `YYYY-MM-DD` date within a sensible distance of today.
    fn date(&self, text: &str) -> Result<NaiveDate, Drop> {
        let day = NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").map_err(|_| Drop(DropReason::InvalidDate))?;
        let today = self.today();
        let earliest = today.checked_sub_days(Days::new(MAX_DAYS_BACK)).unwrap_or(today);
        let latest = today.checked_add_days(Days::new(MAX_DAYS_AHEAD)).unwrap_or(today);
        if !(earliest..=latest).contains(&day) {
            return Err(Drop(DropReason::InvalidDate));
        }
        Ok(day)
    }
}

/// A planned block that ends by midnight.
fn block(start: NaiveTime, minutes: i64) -> Result<TimeBlock, Drop> {
    let start_minutes = i64::from(start.num_seconds_from_midnight() / 60);
    if minutes < i64::from(MIN_BLOCK_MINUTES) || start_minutes + minutes > 24 * 60 {
        return Err(Drop(DropReason::InvalidTimeRange));
    }
    Ok(TimeBlock { start, minutes: minutes as u32 })
}

/// `HH:MM` on a 24-hour clock, whole minutes.
fn time(text: &str) -> Option<NaiveTime> {
    let (hours, minutes) = text.trim().split_once(':')?;
    let both_digits = |part: &str| (1..=2).contains(&part.len()) && part.chars().all(|c| c.is_ascii_digit());
    if !both_digits(hours) || minutes.len() != 2 || !both_digits(minutes) {
        return None;
    }
    NaiveTime::from_hms_opt(hours.parse().ok()?, minutes.parse().ok()?, 0)
}

/// Like [`time`], with `24:00` for midnight at the end of the day (returned as `00:00`).
fn end_time(text: &str) -> Option<NaiveTime> {
    if text.trim() == "24:00" {
        return Some(NaiveTime::MIN);
    }
    time(text)
}

fn weekday(text: &str) -> Option<Weekday> {
    Some(match text {
        "mon" => Weekday::Mon,
        "tue" => Weekday::Tue,
        "wed" => Weekday::Wed,
        "thu" => Weekday::Thu,
        "fri" => Weekday::Fri,
        "sat" => Weekday::Sat,
        "sun" => Weekday::Sun,
        _ => return None,
    })
}

fn string<'a>(fields: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    fields.get(name).and_then(Value::as_str).filter(|text| !text.trim().is_empty())
}

/// Whole numbers only; `2.5` or `"2"` are not.
fn integer(fields: &Map<String, Value>, name: &str) -> Option<i64> {
    fields.get(name).and_then(Value::as_i64)
}

fn low_confidence(fields: &Map<String, Value>, name: &str) -> bool {
    string(fields, name) != Some("high")
}

/// A required title on one line, not too long.
fn title(fields: &Map<String, Value>, name: &str) -> Result<String, Drop> {
    let title = string(fields, name).map(single_line).ok_or(Drop(DropReason::Incomplete))?;
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(Drop(DropReason::TooLong));
    }
    Ok(title)
}

fn description(fields: &Map<String, Value>) -> Result<String, Drop> {
    let description = string(fields, "description").map(multi_line).unwrap_or_default();
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(Drop(DropReason::TooLong));
    }
    Ok(description)
}

/// How the dropped list names an item.
fn shown_title(fields: &Map<String, Value>) -> String {
    ["title", "new_task_title"]
        .into_iter()
        .find_map(|name| string(fields, name))
        .map(single_line)
        .unwrap_or_else(|| UNTITLED.to_owned())
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
