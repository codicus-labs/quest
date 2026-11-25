use chrono::{Datelike, Days, Months, NaiveDate, Weekday};
use quest_domain::{DateField, TaskDates, set_due, set_planned};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickDate {
    Today,
    Tomorrow,
    NextMonday,
    None,
}

impl QuickDate {
    pub const ALL: [Self; 4] = [Self::Today, Self::Tomorrow, Self::NextMonday, Self::None];

    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Tomorrow => "Tomorrow",
            Self::NextMonday => "Next Monday",
            Self::None => "None",
        }
    }

    pub fn date(self, today: NaiveDate) -> Option<NaiveDate> {
        match self {
            Self::Today => Some(today),
            Self::Tomorrow => today.succ_opt(),
            Self::NextMonday => {
                today.checked_add_days(Days::new(7 - u64::from(today.weekday().num_days_from_monday())))
            }
            Self::None => None,
        }
    }
}

pub fn set_date(dates: TaskDates, field: DateField, day: Option<NaiveDate>) -> TaskDates {
    match field {
        DateField::Plan => set_planned(dates, day),
        DateField::Due => set_due(dates, day),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Day,
    Week,
    Month,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Day {
    pub date: NaiveDate,
    pub in_month: bool,
    pub enabled: bool,
    pub selected: bool,
    pub today: bool,
    pub marker: Option<DateField>,
}

pub fn initial(dates: TaskDates, field: DateField, today: NaiveDate) -> NaiveDate {
    dates
        .get(field)
        .or_else(|| (field == DateField::Due).then_some(dates.planned_on).flatten())
        .unwrap_or(today)
        .max(today)
}

pub fn shift(cursor: NaiveDate, step: Step, forward: bool, today: NaiveDate) -> NaiveDate {
    let next = match (step, forward) {
        (Step::Day, true) => cursor.succ_opt(),
        (Step::Day, false) => cursor.pred_opt(),
        (Step::Week, true) => cursor.checked_add_days(Days::new(7)),
        (Step::Week, false) => cursor.checked_sub_days(Days::new(7)),
        (Step::Month, true) => cursor.checked_add_months(Months::new(1)),
        (Step::Month, false) => cursor.checked_sub_months(Months::new(1)),
    };
    next.unwrap_or(cursor).max(today)
}

pub fn month(cursor: NaiveDate, today: NaiveDate, dates: TaskDates, field: DateField) -> Vec<Day> {
    let first = cursor.with_day(1).unwrap_or(cursor);
    let start = first.checked_sub_days(Days::new(u64::from(first.weekday().num_days_from_monday()))).unwrap_or(first);
    let boundary = match field {
        DateField::Plan => DateField::Due,
        DateField::Due => DateField::Plan,
    };
    std::iter::successors(Some(start), |day| day.succ_opt())
        .take_while(|day| !(*day > cursor && day.month() != cursor.month() && day.weekday() == Weekday::Mon))
        .map(|date| Day {
            date,
            in_month: date.month() == cursor.month(),
            enabled: date >= today,
            selected: dates.get(field) == Some(date),
            today: date == today,
            marker: (dates.get(boundary) == Some(date)).then_some(boundary),
        })
        .collect()
}

#[cfg(test)]
#[path = "calendar_tests.rs"]
mod tests;
