use chrono::{DateTime, Datelike, Days, NaiveDate, TimeZone, Utc};
use quest_domain::{DateField, EntityId, Snapshot, WeekTasks, WeeklyReview};

#[derive(Debug, PartialEq)]
pub struct Review {
    pub tasks: WeekTasks,
    pub time: WeeklyReview,
    pub previous_tasks: WeekTasks,
    pub previous_quest_ms: i64,
    pub most_postponed: Vec<Postponed>,
    pub carried_over: Vec<Since>,
    pub overdue: Vec<Since>,
    pub shares: Vec<Share>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Postponed {
    pub task_id: EntityId,
    pub count: usize,
    pub plan: Vec<Vec<NaiveDate>>,
    pub deadline: Vec<Vec<NaiveDate>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Since {
    pub task_id: EntityId,
    pub day: NaiveDate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bucket {
    Quest(EntityId),
    Organizing,
    NoQuest,
}

#[derive(Debug, PartialEq)]
pub struct Share {
    pub bucket: Bucket,
    pub ms: i64,
    pub fraction: f64,
}

impl Review {
    pub fn build<Tz: TimeZone>(snapshot: &Snapshot, day: NaiveDate, tz: &Tz, now: DateTime<Utc>) -> Self {
        let tasks = WeekTasks::build(snapshot, day, tz, now);
        let start = tasks.week_start;
        let previous = start - Days::new(7);
        let time = WeeklyReview::build(snapshot, start, tz, now);
        let previous_tasks = WeekTasks::build(snapshot, previous, tz, now);
        let previous_quest_ms = WeeklyReview::build(snapshot, previous, tz, now).quest_ms;
        let week_days = start..start + Days::new(7);
        let most_postponed = tasks
            .most_postponed
            .iter()
            .take(3)
            .map(|rank| {
                let mut plan = Vec::new();
                let mut deadline = Vec::new();
                let events = snapshot
                    .postponements_of(rank.task_id)
                    .filter(|p| week_days.contains(&p.at.with_timezone(tz).date_naive()));
                for event in events {
                    let chains = match event.field {
                        DateField::Plan => &mut plan,
                        DateField::Due => &mut deadline,
                    };
                    append(chains, event.from, event.to);
                }
                Postponed { task_id: rank.task_id, count: rank.total(), plan, deadline }
            })
            .collect();
        let since = |task_id, field| {
            let day = snapshot.task(task_id)?.dates().get(field)?;
            (day >= start).then_some(Since { task_id, day })
        };
        let carried_over = tasks.carried_over.iter().filter_map(|r| since(r.task_id, DateField::Plan)).collect();
        let overdue = tasks.overdue.iter().filter_map(|r| since(r.task_id, DateField::Due)).collect();
        let total = time.total_ms().max(1) as f64;
        let shares = time
            .by_quest
            .iter()
            .map(|q| (Bucket::Quest(q.quest_id), q.ms))
            .chain([(Bucket::Organizing, time.organizing_ms), (Bucket::NoQuest, time.other_ms)])
            .map(|(bucket, ms)| Share { bucket, ms, fraction: ms as f64 / total })
            .collect();
        Self { tasks, time, previous_tasks, previous_quest_ms, most_postponed, carried_over, overdue, shares }
    }
}

pub fn offset_week(day: NaiveDate, offset: i32) -> Option<NaiveDate> {
    let monday = day.checked_sub_days(Days::new(u64::from(day.weekday().num_days_from_monday())))?;
    let days = Days::new(u64::from(offset.unsigned_abs()) * 7);
    let start = if offset < 0 { monday.checked_sub_days(days)? } else { monday.checked_add_days(days)? };
    start.checked_sub_days(Days::new(7))?;
    start.checked_add_days(Days::new(7))?;
    Some(start)
}

fn append(chains: &mut Vec<Vec<NaiveDate>>, from: NaiveDate, to: NaiveDate) {
    if let Some(chain) = chains.last_mut().filter(|chain| chain.last() == Some(&from)) {
        chain.push(to);
    } else {
        chains.push(vec![from, to]);
    }
}

#[cfg(test)]
mod tests;
