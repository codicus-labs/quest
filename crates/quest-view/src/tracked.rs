//! The tracked-time column, pure: "Tracked today" (the day's sessions on a timeline) and
//! "This week" (quest hours against the weekly goal, per quest, Organizing, No quest, Postponed).

#[cfg(test)]
mod tests;

use chrono::{DateTime, Days, NaiveDate, TimeZone, Utc};
use quest_domain::{EntityId, EntryKind, QuestColor, QuestHours, Snapshot, WeekTasks, WeeklyReview};

/// What a session on the timeline was spent on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    /// Work on a task; `quest` is its quest if that still exists.
    Task {
        task: EntityId,
        quest: Option<EntityId>,
    },
    /// A session without a task (other work).
    NoTask,
    Organizing,
}

/// One session as it sits on the day: minutes since the local midnight, cut to the day.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineBlock {
    pub entry: EntityId,
    pub kind: BlockKind,
    /// The task's title, "Organizing", or "No task".
    pub title: String,
    /// The quest's colour; none for Organizing and work without a quest.
    pub color: Option<QuestColor>,
    pub start_minute: u32,
    pub end_minute: u32,
    pub running: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DayTimeline {
    /// Oldest first.
    pub blocks: Vec<TimelineBlock>,
    /// The hour lines to draw: 08:00 to 18:00, widened to fit every block and now.
    pub first_hour: u32,
    pub last_hour: u32,
    /// Minutes since midnight of the "now" line; none on another day.
    pub now_minute: Option<u32>,
    pub tracked_ms: i64,
}

const DAY_MINUTES: i64 = 24 * 60;

pub fn day_timeline<Tz: TimeZone>(snapshot: &Snapshot, day: NaiveDate, tz: &Tz, now: DateTime<Utc>) -> DayTimeline {
    let (from, to) = (midnight(day, tz), midnight(day + Days::new(1), tz));
    let minute = |at: DateTime<Utc>| u32::try_from((at - from).num_minutes().clamp(0, DAY_MINUTES)).unwrap_or(0);
    let mut entries: Vec<_> =
        snapshot.time_entries.iter().filter(|e| e.started_at < to && e.ended_at.unwrap_or(now) > from).collect();
    entries.sort_by_key(|e| e.started_at);
    let mut tracked_ms = 0;
    let mut blocks = Vec::new();
    for entry in entries {
        let (start, end) = (entry.started_at.max(from), entry.ended_at.unwrap_or(now).min(to));
        tracked_ms += (end - start).num_milliseconds().max(0);
        let (kind, title, color) = describe(snapshot, entry);
        blocks.push(TimelineBlock {
            entry: entry.id,
            kind,
            title,
            color,
            start_minute: minute(start),
            end_minute: minute(end),
            running: entry.is_running(),
        });
    }
    let now_minute = (from <= now && now < to).then(|| minute(now));
    let earliest = blocks.iter().map(|b| b.start_minute).chain(now_minute).min().unwrap_or(u32::MAX);
    let latest = blocks.iter().map(|b| b.end_minute).chain(now_minute).max().unwrap_or(0);
    DayTimeline {
        blocks,
        first_hour: (earliest / 60).min(8),
        last_hour: latest.div_ceil(60).clamp(18, 24),
        now_minute,
        tracked_ms,
    }
}

fn describe(snapshot: &Snapshot, entry: &quest_domain::TimeEntry) -> (BlockKind, String, Option<QuestColor>) {
    if entry.kind == EntryKind::Organizing {
        return (BlockKind::Organizing, "Organizing".to_owned(), None);
    }
    let quest = snapshot.entry_quest(entry);
    let color = quest.and_then(|id| snapshot.quest(id)).map(|q| q.color);
    match entry.task_id {
        Some(task) => {
            let title = snapshot.task(task).map_or_else(|| "Deleted task".to_owned(), |t| t.title.clone());
            (BlockKind::Task { task, quest }, title, color)
        }
        None => (BlockKind::NoTask, "No task".to_owned(), color),
    }
}

/// One active quest's hours this week.
#[derive(Clone, Debug, PartialEq)]
pub struct QuestWeek {
    pub quest: EntityId,
    pub title: String,
    pub color: QuestColor,
    pub ms: i64,
}

/// "This week": quest hours against the global goal, and where the rest went.
#[derive(Clone, Debug, PartialEq)]
pub struct WeekBalance {
    pub quest_ms: i64,
    pub target_ms: i64,
    /// Every active quest in sidebar order, also those without time.
    pub quests: Vec<QuestWeek>,
    pub organizing_ms: i64,
    /// Work on tasks without a quest.
    pub no_quest_ms: i64,
    /// Plan dates and deadlines moved later this week.
    pub postponed: usize,
    /// Hours per quest with time, completed quests too (a quest page's header).
    pub by_quest: Vec<QuestHours>,
}

impl WeekBalance {
    pub fn quest_ms_of(&self, quest: EntityId) -> i64 {
        self.by_quest.iter().find(|q| q.quest_id == quest).map_or(0, |q| q.ms)
    }
}

pub fn week_balance<Tz: TimeZone>(snapshot: &Snapshot, today: NaiveDate, tz: &Tz, now: DateTime<Utc>) -> WeekBalance {
    let review = WeeklyReview::build(snapshot, today, tz, now);
    let postponed = WeekTasks::build(snapshot, today, tz, now).postponements;
    let ms_of = |id: EntityId| review.by_quest.iter().find(|q| q.quest_id == id).map_or(0, |q| q.ms);
    let quests = snapshot
        .active_quests()
        .map(|q| QuestWeek { quest: q.id, title: q.title.clone(), color: q.color, ms: ms_of(q.id) })
        .collect();
    WeekBalance {
        quest_ms: review.quest_ms,
        target_ms: review.target_ms,
        quests,
        organizing_ms: review.organizing_ms,
        no_quest_ms: review.other_ms,
        postponed,
        by_quest: review.by_quest,
    }
}

/// `part` of `whole` as 0.0..=1.0 (a bar or ring); 0 without a whole.
pub fn fraction(part: i64, whole: i64) -> f32 {
    if whole <= 0 {
        return 0.0;
    }
    (part as f32 / whole as f32).clamp(0.0, 1.0)
}

/// The UTC instant a local day starts (the earlier one if the clock went back).
fn midnight<Tz: TimeZone>(day: NaiveDate, tz: &Tz) -> DateTime<Utc> {
    let naive = day.and_hms_opt(0, 0, 0).unwrap_or_default();
    tz.from_local_datetime(&naive).earliest().map_or_else(|| naive.and_utc(), |t| t.with_timezone(&Utc))
}
