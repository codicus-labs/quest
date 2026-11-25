use std::{borrow::Cow, collections::BTreeMap};

use chrono::{DateTime, Datelike, Days, NaiveDate, TimeZone, Utc};
use quest_domain::{Snapshot, Task};

use crate::lists;

pub const DAILY_COUNT: u64 = 14;

pub struct Day<'a> {
    pub date: NaiveDate,
    pub tasks: Vec<Cow<'a, Task>>,
    pub due: Vec<&'a Task>,
}

impl Day<'_> {
    pub fn has_tasks(&self) -> bool {
        !self.tasks.is_empty() || !self.due.is_empty()
    }
}

pub struct Month<'a> {
    pub start: NaiveDate,
    pub partial: bool,
    pub days: Vec<Day<'a>>,
}

pub struct Plan<'a> {
    pub days: Vec<Day<'a>>,
    pub later: Vec<Month<'a>>,
    pub done_today: Vec<&'a Task>,
    pub not_planned: Vec<&'a Task>,
}

impl<'a> Plan<'a> {
    pub fn all_days(&self) -> impl Iterator<Item = &Day<'a>> {
        self.days.iter().chain(self.later.iter().flat_map(|month| &month.days))
    }
}

fn day<'a, 'b>(buckets: &'b mut BTreeMap<NaiveDate, Day<'a>>, date: NaiveDate) -> &'b mut Day<'a> {
    buckets.entry(date).or_insert_with(|| Day { date, tasks: Vec::new(), due: Vec::new() })
}

pub fn daily_dates(today: NaiveDate) -> impl Iterator<Item = NaiveDate> {
    (0..DAILY_COUNT).filter_map(move |offset| today.checked_add_days(Days::new(offset)))
}

pub fn build<'a>(snapshot: &'a Snapshot, today: NaiveDate, now: DateTime<Utc>, tz: &impl TimeZone) -> Plan<'a> {
    let mut buckets = BTreeMap::new();
    day(&mut buckets, today).tasks = lists::today(snapshot, today).into_iter().map(Cow::Borrowed).collect();
    for date in daily_dates(today).skip(1) {
        day(&mut buckets, date).tasks.extend(snapshot.planned_occurrences(date, now).map(Cow::Owned));
    }
    let mut not_planned = Vec::new();
    for task in snapshot.tasks.iter().filter(|task| !task.is_done()) {
        if let Some(date) = task.planned_on.filter(|date| *date > today) {
            day(&mut buckets, date).tasks.push(Cow::Borrowed(task));
        }
        if task.planned_on.is_none() && task.quest_id.is_some() {
            not_planned.push(task);
        }
        if let Some(date) = task.due_on.filter(|date| *date > today && Some(*date) != task.planned_on) {
            day(&mut buckets, date).due.push(task);
        }
    }
    let mut days = Vec::new();
    let mut months = BTreeMap::<NaiveDate, Month<'_>>::new();
    let daily_end = today.checked_add_days(Days::new(DAILY_COUNT - 1)).unwrap_or(NaiveDate::MAX);
    let partial_month =
        daily_end.succ_opt().filter(|next| next.month() == daily_end.month()).and_then(|next| next.with_day(1));
    if let Some(start) = partial_month {
        months.insert(start, Month { start, partial: true, days: Vec::new() });
    }
    for (_, mut bucket) in buckets {
        bucket.tasks.sort_by(|a, b| (&a.day_order, a.id).cmp(&(&b.day_order, b.id)));
        if bucket.date <= daily_end {
            days.push(bucket);
        } else if let Some(start) = bucket.date.with_day(1) {
            months.entry(start).or_insert_with(|| Month { start, partial: false, days: Vec::new() }).days.push(bucket);
        }
    }
    Plan {
        days,
        later: months.into_values().collect(),
        done_today: lists::done_today(snapshot, today, tz),
        not_planned,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeekDay {
    pub date: NaiveDate,
    pub has_tasks: bool,
    pub jump_enabled: bool,
}

pub fn weeks(plan: &Plan<'_>, today: NaiveDate) -> Vec<Vec<WeekDay>> {
    let monday = quest_domain::week_start(today);
    let last = plan.days.last().map_or(today, |day| day.date);
    (0..)
        .map_while(|offset| monday.checked_add_days(Days::new(offset)))
        .take_while(|date| *date <= last || quest_domain::week_start(*date) <= last)
        .map(|date| {
            let bucket = plan.days.iter().find(|day| day.date == date);
            WeekDay {
                date,
                has_tasks: bucket.is_some_and(Day::has_tasks) || (date == today && !plan.done_today.is_empty()),
                jump_enabled: bucket.is_some(),
            }
        })
        .collect::<Vec<_>>()
        .chunks(7)
        .map(<[WeekDay]>::to_vec)
        .collect()
}

#[cfg(test)]
mod tests;
