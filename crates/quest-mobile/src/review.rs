//! The weekly review, pure: the domain's `WeeklyReview` (the Quest Hours rules) as labels and
//! bar fractions, plus the recent weeks. Charts follow the desktop: a day's bar is relative to
//! the largest day, but at least an hour.

use chrono::{Days, NaiveDate};
use quest_domain::{Quest, Snapshot, WeeklyReview, week_start};
use quest_view::format;

use crate::home::color_hex;
use crate::ids::iso_day;
use crate::review_types::{ReviewData, ReviewDay, ReviewQuest, ReviewSegment, ReviewWeek};
use crate::time::Now;

/// The smallest top of a chart, so a few minutes don't fill it (desktop: 1 h).
const CHART_MIN_MS: i64 = 3_600_000;
/// Weeks in the history chart, ending with the reviewed one.
const HISTORY_WEEKS: u64 = 8;

pub(crate) fn review(snapshot: &Snapshot, any_day: NaiveDate, now: Now) -> ReviewData {
    let week = WeeklyReview::build(snapshot, any_day, &now.tz, now.at);
    let start = week.week_start;
    let today = now.today();
    let target_pct = if week.target_ms > 0 { week.quest_ms * 100 / week.target_ms } else { 0 };
    let total = week.quest_ms + week.other_ms;
    let quest_work = format::hours(week.quest_ms);
    let other = format::hours(week.other_ms);
    let share = week.quest_share_percent();
    let weeks = history(snapshot, start, now);
    let quests = by_quest(snapshot, &week);
    ReviewData {
        week_start: iso_day(start),
        week_label: format::week_range(start),
        previous_week: iso_day(start - Days::new(7)),
        next_week: iso_day(start + Days::new(7)),
        is_this_week: start == week_start(today),
        quest_hours: quest_work.clone(),
        target: format!("/ {}", format::hours(week.target_ms)),
        target_note: format!("{target_pct}% of target"),
        target_progress: fraction(week.quest_ms, week.target_ms),
        daily_average: format::hours(week.daily_average_ms(today)),
        quest_share: format!("{share}%"),
        days: days(snapshot, &week, today),
        by_quest_a11y: by_quest_a11y(&quests),
        by_quest: quests,
        split_a11y: format!("Quest work {quest_work}, other {other}: {share}% quest work"),
        quest_work: format!("Quest work · {quest_work}"),
        other: format!("Other · {other}"),
        quest_fraction: fraction(week.quest_ms, total),
        weeks_a11y: format!(
            "Quest hours of recent weeks: {}",
            weeks.iter().map(|w| format!("{} {}", w.label, w.hours)).collect::<Vec<_>>().join(", ")
        ),
        weeks,
    }
}

fn by_quest_a11y(quests: &[ReviewQuest]) -> String {
    if quests.is_empty() {
        return "No quest time this week".to_owned();
    }
    let parts: Vec<String> = quests.iter().map(|q| format!("{} {}", q.name, q.hours)).collect();
    format!("Hours by quest: {}", parts.join(", "))
}

/// `part` of `whole`, 0 to 1; 0 when there is no whole.
fn fraction(part: i64, whole: i64) -> f32 {
    if whole <= 0 { 0.0 } else { (part as f32 / whole as f32).clamp(0.0, 1.0) }
}

fn days(snapshot: &Snapshot, week: &WeeklyReview, today: NaiveDate) -> Vec<ReviewDay> {
    let top = week.quest_ms_per_day.iter().copied().max().unwrap_or(0).max(CHART_MIN_MS);
    (0..7u64)
        .map(|i| {
            let day = week.week_start + Days::new(i);
            let ms = week.quest_ms_per_day[i as usize];
            let parts: Vec<(&Quest, i64)> = week.by_quest_per_day[i as usize]
                .iter()
                .filter_map(|q| snapshot.quest(q.quest_id).map(|quest| (quest, q.ms)))
                .collect();
            let segments = parts
                .iter()
                .map(|(quest, ms)| ReviewSegment {
                    quest_id: quest.id.to_string(),
                    color_hex: color_hex(quest.color).to_owned(),
                    fraction: fraction(*ms, top),
                })
                .collect();
            let hours = format::hours(ms);
            let split: Vec<String> =
                parts.iter().map(|(quest, ms)| format!("{} {}", quest.title, format::hours(*ms))).collect();
            let a11y_label = if split.is_empty() {
                format!("{}: {hours}", day.format("%A"))
            } else {
                format!("{}: {hours}. {}", day.format("%A"), split.join(", "))
            };
            ReviewDay {
                label: day.format("%a").to_string(),
                hours,
                is_today: day == today,
                fraction: fraction(ms, top),
                segments,
                a11y_label,
            }
        })
        .collect()
}

fn by_quest(snapshot: &Snapshot, week: &WeeklyReview) -> Vec<ReviewQuest> {
    let top = week.by_quest.first().map_or(1, |q| q.ms.max(1));
    week.by_quest
        .iter()
        .filter_map(|hours| {
            let quest = snapshot.quest(hours.quest_id)?;
            Some(ReviewQuest {
                id: quest.id.to_string(),
                name: quest.title.clone(),
                color_hex: color_hex(quest.color).to_owned(),
                hours: format::hours(hours.ms),
                fraction: fraction(hours.ms, top),
                a11y_label: format!("{}: {}", quest.title, format::hours(hours.ms)),
            })
        })
        .collect()
}

/// The reviewed week and the ones before it, oldest first.
fn history(snapshot: &Snapshot, reviewed: NaiveDate, now: Now) -> Vec<ReviewWeek> {
    let weeks: Vec<(NaiveDate, i64)> = (0..HISTORY_WEEKS)
        .rev()
        .map(|back| reviewed - Days::new(7 * back))
        .map(|monday| (monday, WeeklyReview::build(snapshot, monday, &now.tz, now.at).quest_ms))
        .collect();
    let top = weeks.iter().map(|(_, ms)| *ms).max().unwrap_or(0).max(CHART_MIN_MS);
    weeks
        .into_iter()
        .map(|(monday, ms)| ReviewWeek {
            week_start: iso_day(monday),
            label: monday.format("%b %-d").to_string(),
            hours: format::hours(ms),
            fraction: fraction(ms, top),
            selected: monday == reviewed,
            a11y_label: format!("Week of {}: {}", monday.format("%b %-d"), format::hours(ms)),
        })
        .collect()
}

#[cfg(test)]
#[path = "review_tests.rs"]
mod tests;
