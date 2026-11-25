//! The weekly review: how much tracked time went to each quest, per day and in total.

use chrono::{DateTime, Datelike, Days, NaiveDate, TimeZone, Utc};

use crate::change::EntityId;
use crate::snapshot::{Snapshot, overlap_ms};

/// The Monday of the week containing `day`.
pub fn week_start(day: NaiveDate) -> NaiveDate {
    day - Days::new(u64::from(day.weekday().num_days_from_monday()))
}

#[derive(Clone, Debug, PartialEq)]
pub struct QuestHours {
    pub quest_id: EntityId,
    pub ms: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeeklyReview {
    pub week_start: NaiveDate,
    /// Quest work per day, Monday first.
    pub quest_ms_per_day: [i64; 7],
    /// Quests with tracked time, most time first.
    pub by_quest: Vec<QuestHours>,
    /// Each day's quest work split by quest (Monday first), in `by_quest`'s order, without zeros.
    pub by_quest_per_day: [Vec<QuestHours>; 7],
    pub quest_ms: i64,
    /// Tracked time not linked to a quest.
    pub other_ms: i64,
    pub target_ms: i64,
}

impl WeeklyReview {
    /// Days with a recorded or elapsed share of the week, for the daily average (at least 1).
    pub fn daily_average_ms(&self, today: NaiveDate) -> i64 {
        let elapsed = (today - self.week_start).num_days().clamp(0, 6) + 1;
        self.quest_ms / elapsed
    }

    /// Share of all tracked time that was quest work, 0..=100.
    pub fn quest_share_percent(&self) -> u8 {
        let total = self.quest_ms + self.other_ms;
        if total == 0 {
            return 0;
        }
        u8::try_from(self.quest_ms * 100 / total).unwrap_or(100)
    }

    pub fn build<Tz: TimeZone>(snapshot: &Snapshot, any_day: NaiveDate, tz: &Tz, now: DateTime<Utc>) -> Self {
        let start = week_start(any_day);
        let midnight = |day: NaiveDate| -> DateTime<Utc> {
            let naive = day.and_hms_opt(0, 0, 0).unwrap_or_default();
            tz.from_local_datetime(&naive).earliest().map_or_else(|| naive.and_utc(), |t| t.with_timezone(&Utc))
        };
        let bounds: Vec<DateTime<Utc>> = (0..=7).map(|i| midnight(start + Days::new(i))).collect();

        let mut quest_ms_per_day = [0; 7];
        let mut by_quest: Vec<QuestHours> = Vec::new();
        let mut by_quest_per_day: [Vec<QuestHours>; 7] = Default::default();
        let mut other_ms = 0;
        for entry in &snapshot.time_entries {
            let quest = snapshot.entry_quest(entry);
            for (day, window) in bounds.windows(2).enumerate() {
                let ms = overlap_ms(entry, window[0], window[1], now);
                if ms == 0 {
                    continue;
                }
                let Some(quest_id) = quest else {
                    other_ms += ms;
                    continue;
                };
                quest_ms_per_day[day] += ms;
                add(&mut by_quest, quest_id, ms);
                add(&mut by_quest_per_day[day], quest_id, ms);
            }
        }
        by_quest.sort_by_key(|q| std::cmp::Reverse(q.ms));
        let rank = |id: EntityId| by_quest.iter().position(|q| q.quest_id == id);
        for day in &mut by_quest_per_day {
            day.sort_by_key(|q| rank(q.quest_id));
        }
        WeeklyReview {
            week_start: start,
            quest_ms_per_day,
            quest_ms: quest_ms_per_day.iter().sum(),
            by_quest,
            by_quest_per_day,
            other_ms,
            target_ms: i64::from(snapshot.settings.weekly_target_minutes) * 60_000,
        }
    }
}

fn add(hours: &mut Vec<QuestHours>, quest_id: EntityId, ms: i64) {
    match hours.iter_mut().find(|q| q.quest_id == quest_id) {
        Some(total) => total.ms += ms,
        None => hours.push(QuestHours { quest_id, ms }),
    }
}
