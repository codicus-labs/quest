//! The Repeat dialog for mobile, pure: the draft it opens with (desktop `open_repeat_editor`),
//! its live check and saving, all through `quest_view::repeat` like the desktop.

use chrono::{NaiveDate, Weekday};
use quest_domain::{Frequency, MonthDay, RepeatPin, RepeatRule, Snapshot, Task, Weekdays};
use quest_view::log_work::parse_day;
use quest_view::repeat::{self, RepeatForm};

use crate::error::MobileError;
use crate::ids::iso_day;
use crate::task_detail::day_value;
use crate::task_types::{RepeatCheck, RepeatDraft, RepeatFrequency, RepeatPinChoice, RepeatWeekday};
use crate::time::Now;

/// The dialog's fields when it opens on `task`: its series' rule, else daily from the task's
/// day (today at the earliest) in the caller's time zone.
pub(crate) fn draft_for(snapshot: &Snapshot, task: &Task, now: Now) -> RepeatDraft {
    let today = now.today();
    let rule = repeat::series_of(snapshot, task).repeat.map(|s| s.rule).unwrap_or(RepeatRule {
        every: 1,
        frequency: Frequency::Day,
        starts: task.day.unwrap_or(today).max(today),
        ends: None,
        time_zone: now.tz,
        pin: RepeatPin::None,
        weekdays: Weekdays::default(),
        month_day: None,
    });
    RepeatDraft {
        every: rule.every.to_string(),
        frequency: frequency_choice(rule.frequency),
        pin: pin_choice(rule.pin),
        weekdays: rule.weekdays.iter().map(weekday_choice).collect(),
        last_day: rule.month_day == Some(MonthDay::Last),
        month_day: match rule.month_day {
            Some(MonthDay::Day(day)) => day.to_string(),
            _ => String::new(),
        },
        starts: iso_day(rule.starts),
        ends_on_date: rule.ends.is_some(),
        ends: rule.ends.map(iso_day).unwrap_or_default(),
        time_zone: rule.time_zone.to_string(),
    }
}

pub(crate) fn check(draft: &RepeatDraft, today: NaiveDate) -> RepeatCheck {
    let form = form(draft);
    RepeatCheck {
        shown_weekdays: form.shown_weekdays(today).iter().map(weekday_choice).collect(),
        month_day_placeholder: form.default_month_day(today),
        summary: repeat::parse_every(&draft.every)
            .map(|every| repeat::cadence(every, form.frequency))
            .unwrap_or_default(),
        error: form.rule(today).err().map(str::to_owned),
        starts: parse_day(&draft.starts, today).map(day_value),
        ends: parse_day(&draft.ends, today).map(day_value),
    }
}

/// The rule to save, or the desktop's message for the field to correct.
pub(crate) fn rule(draft: &RepeatDraft, today: NaiveDate) -> Result<RepeatRule, MobileError> {
    form(draft).rule(today).map_err(|reason| MobileError::InvalidRepeat { reason: reason.to_owned() })
}

/// A press on `day`'s chip; the last picked day stays.
pub(crate) fn toggle_weekday(draft: RepeatDraft, day: RepeatWeekday, today: NaiveDate) -> RepeatDraft {
    let form = form(&draft);
    let picked = repeat::toggle_weekday(form.weekdays, form.shown_weekdays(today), weekday(day));
    let weekdays = picked.iter().map(weekday_choice).collect();
    RepeatDraft { weekdays, ..draft }
}

fn form(draft: &RepeatDraft) -> RepeatForm<'_> {
    RepeatForm {
        every: &draft.every,
        frequency: frequency(draft.frequency),
        pin: pin(draft.pin),
        weekdays: draft.weekdays.iter().fold(Weekdays::default(), |days, day| {
            if days.contains(weekday(*day)) { days } else { days.toggled(weekday(*day)) }
        }),
        last_day: draft.last_day,
        month_day: &draft.month_day,
        starts: &draft.starts,
        ends_on_date: draft.ends_on_date,
        ends: &draft.ends,
        time_zone: &draft.time_zone,
    }
}

fn frequency(choice: RepeatFrequency) -> Frequency {
    match choice {
        RepeatFrequency::Day => Frequency::Day,
        RepeatFrequency::Week => Frequency::Week,
        RepeatFrequency::Month => Frequency::Month,
        RepeatFrequency::Year => Frequency::Year,
    }
}

fn frequency_choice(frequency: Frequency) -> RepeatFrequency {
    match frequency {
        Frequency::Day => RepeatFrequency::Day,
        Frequency::Week => RepeatFrequency::Week,
        Frequency::Month => RepeatFrequency::Month,
        Frequency::Year => RepeatFrequency::Year,
    }
}

fn pin(choice: RepeatPinChoice) -> RepeatPin {
    match choice {
        RepeatPinChoice::None => RepeatPin::None,
        RepeatPinChoice::Top => RepeatPin::Top,
        RepeatPinChoice::Bottom => RepeatPin::Bottom,
    }
}

fn pin_choice(pin: RepeatPin) -> RepeatPinChoice {
    match pin {
        RepeatPin::None => RepeatPinChoice::None,
        RepeatPin::Top => RepeatPinChoice::Top,
        RepeatPin::Bottom => RepeatPinChoice::Bottom,
    }
}

fn weekday(choice: RepeatWeekday) -> Weekday {
    match choice {
        RepeatWeekday::Mon => Weekday::Mon,
        RepeatWeekday::Tue => Weekday::Tue,
        RepeatWeekday::Wed => Weekday::Wed,
        RepeatWeekday::Thu => Weekday::Thu,
        RepeatWeekday::Fri => Weekday::Fri,
        RepeatWeekday::Sat => Weekday::Sat,
        RepeatWeekday::Sun => Weekday::Sun,
    }
}

fn weekday_choice(day: Weekday) -> RepeatWeekday {
    match day {
        Weekday::Mon => RepeatWeekday::Mon,
        Weekday::Tue => RepeatWeekday::Tue,
        Weekday::Wed => RepeatWeekday::Wed,
        Weekday::Thu => RepeatWeekday::Thu,
        Weekday::Fri => RepeatWeekday::Fri,
        Weekday::Sat => RepeatWeekday::Sat,
        Weekday::Sun => RepeatWeekday::Sun,
    }
}

#[cfg(test)]
#[path = "repeat_tests.rs"]
mod tests;
