//! The Calendar, pure: which days a page shows (1, 3 or a week from Monday), the pages before
//! and after, each day's grid and tracked total, and the week's Quest Hours against the target
//! (the desktop's Calendar header).

use chrono::{Days, NaiveDate, Timelike};
use quest_domain::{Snapshot, week_start};
use quest_view::format;

use crate::calendar_types::{CalendarData, CalendarDay, CalendarQuery};
use crate::day_grid::day_grid;
use crate::error::MobileError;
use crate::ids::{iso_day, parse_iso_day};
use crate::time::Now;
use crate::types::UndoToast;

/// A week: the expanded layout pages by weeks from Monday.
const WEEK: u32 = 7;

/// The page's first day and how many days it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Page {
    pub first: NaiveDate,
    pub count: u32,
}

impl Page {
    /// `day_count` is clamped to 1..=7. Without a first day the page holds today: a week from
    /// its Monday, else today in the middle (1: today; 3: yesterday to tomorrow). A week always
    /// starts on Monday.
    pub fn of(query: &CalendarQuery, today: NaiveDate) -> Result<Page, MobileError> {
        let count = query.day_count.clamp(1, WEEK);
        let first = match query.first_day.as_deref() {
            Some(day) => parse_iso_day(day)?,
            None if count == WEEK => today,
            None => back(today, (count - 1) / 2)?,
        };
        let first = if count == WEEK { week_start(first) } else { first };
        Ok(Page { first, count })
    }

    pub fn days(&self) -> impl Iterator<Item = NaiveDate> + '_ {
        (0..self.count).filter_map(|i| self.first.checked_add_days(Days::new(u64::from(i))))
    }

    pub fn previous(&self) -> Result<NaiveDate, MobileError> {
        back(self.first, self.count)
    }

    pub fn next(&self) -> Result<NaiveDate, MobileError> {
        forward(self.first, self.count)
    }

    /// The day in the middle (left of the middle for an even count): its week heads the page.
    pub fn middle(&self) -> Result<NaiveDate, MobileError> {
        forward(self.first, (self.count - 1) / 2)
    }
}

pub(crate) fn calendar(
    snapshot: &Snapshot,
    page: Page,
    now: Now,
    toast: Option<UndoToast>,
) -> Result<CalendarData, MobileError> {
    let today = now.today();
    let monday = week_start(page.middle()?);
    let week_ms = snapshot.work_between(now.midnight(monday), now.midnight(forward(monday, WEEK)?), now.at).quest_ms;
    let target_ms = i64::from(snapshot.settings.weekly_target_minutes) * 60_000;
    let local = now.local(now.at);
    let days = page.days().map(|day| calendar_day(snapshot, day, now)).collect::<Result<Vec<_>, _>>()?;
    Ok(CalendarData {
        previous_first_day: iso_day(page.previous()?),
        next_first_day: iso_day(page.next()?),
        shows_today: page.days().any(|day| day == today),
        week_label: format::week_range(monday),
        week_hours: format::hours(week_ms),
        weekly_target: format::hours(target_ms),
        week_progress: (week_ms as f32 / target_ms.max(1) as f32).clamp(0.0, 1.0),
        scroll_minute: local.hour() * 60,
        month_label: page.first.format("%B %Y").to_string(),
        days,
        toast,
    })
}

fn calendar_day(snapshot: &Snapshot, day: NaiveDate, now: Now) -> Result<CalendarDay, MobileError> {
    // All tracked work: the total matches the sessions drawn under it (desktop).
    let total = snapshot.work_between(now.midnight(day), now.midnight(forward(day, 1)?), now.at).total_ms();
    Ok(CalendarDay {
        grid: day_grid(snapshot, day, now),
        weekday: day.format("%a").to_string(),
        day_number: day.format("%-d").to_string(),
        is_today: day == now.today(),
        total: format::hours(total),
    })
}

fn forward(day: NaiveDate, days: u32) -> Result<NaiveDate, MobileError> {
    day.checked_add_days(Days::new(u64::from(days))).ok_or_else(|| out_of_range(day))
}

fn back(day: NaiveDate, days: u32) -> Result<NaiveDate, MobileError> {
    day.checked_sub_days(Days::new(u64::from(days))).ok_or_else(|| out_of_range(day))
}

fn out_of_range(day: NaiveDate) -> MobileError {
    MobileError::InvalidDate { value: iso_day(day) }
}

#[cfg(test)]
#[path = "calendar_tests.rs"]
mod tests;
