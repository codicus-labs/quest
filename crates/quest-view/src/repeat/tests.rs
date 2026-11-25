use chrono::{NaiveDate, Weekday};
use quest_domain::{Frequency, MonthDay, RepeatPin, Weekdays};

use super::*;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 23).unwrap()
}

fn form() -> RepeatForm<'static> {
    RepeatForm {
        every: "2",
        frequency: Frequency::Week,
        pin: RepeatPin::Top,
        weekdays: Weekdays::default(),
        last_day: false,
        month_day: "",
        starts: "2026-11-25",
        ends_on_date: false,
        ends: "",
        time_zone: "Europe/Berlin",
    }
}

#[test]
fn a_valid_form_makes_its_rule() {
    let rule = form().rule(today()).unwrap();
    assert_eq!((rule.every, rule.frequency, rule.pin), (2, Frequency::Week, RepeatPin::Top));
    assert_eq!(rule.starts, NaiveDate::from_ymd_opt(2026, 11, 25).unwrap());
    assert_eq!(rule.ends, None);
    assert_eq!(rule.time_zone, chrono_tz::Europe::Berlin);
}

#[test]
fn each_field_has_the_desktop_message() {
    let cases = [
        (RepeatForm { every: "0", ..form() }, "Enter an interval from 1 to 999."),
        (RepeatForm { every: "1000", ..form() }, "Enter an interval from 1 to 999."),
        (RepeatForm { starts: "soon", ..form() }, "Enter a start date."),
        (RepeatForm { ends_on_date: true, ends: "", ..form() }, "Enter an end date."),
        (
            RepeatForm { ends_on_date: true, ends: "2026-11-24", ..form() },
            "The end date must be on or after the start.",
        ),
        (RepeatForm { time_zone: "Mars/Base", ..form() }, "Enter a timezone such as Europe/Berlin or UTC."),
        (
            RepeatForm { frequency: Frequency::Month, month_day: "32", ..form() },
            "Enter a day of the month from 1 to 31.",
        ),
    ];
    for (form, message) in cases {
        assert_eq!(form.rule(today()), Err(message), "{form:?}");
    }
}

#[test]
fn month_days_only_count_for_monthly_rules() {
    let monthly = RepeatForm { frequency: Frequency::Month, month_day: "31", ..form() };
    assert_eq!(monthly.rule(today()).unwrap().month_day, Some(MonthDay::Day(31)));
    let last = RepeatForm { last_day: true, ..monthly };
    assert_eq!(last.rule(today()).unwrap().month_day, Some(MonthDay::Last));
    assert_eq!(RepeatForm { month_day: "", ..monthly }.rule(today()).unwrap().month_day, None);
    assert_eq!(RepeatForm { month_day: "99", ..form() }.rule(today()).unwrap().month_day, None);
}

#[test]
fn weekdays_follow_the_start_until_picked_and_keep_the_last_one() {
    assert_eq!(form().shown_weekdays(today()), Weekdays::on(Weekday::Wed));
    assert_eq!(form().default_month_day(today()), "25");
    let wed = Weekdays::on(Weekday::Wed);
    assert_eq!(toggle_weekday(wed, wed, Weekday::Wed), wed);
    assert_eq!(toggle_weekday(Weekdays::default(), wed, Weekday::Wed), Weekdays::default(), "unpicked stays empty");
    assert_eq!(toggle_weekday(Weekdays::default(), wed, Weekday::Fri), wed.toggled(Weekday::Fri));
    let daily = RepeatForm { frequency: Frequency::Day, weekdays: wed, ..form() };
    assert_eq!(daily.rule(today()).unwrap().weekdays, Weekdays::default(), "weekdays are only for weekly rules");
}

#[test]
fn cadence_reads_like_the_dialog() {
    assert_eq!(cadence(1, Frequency::Day), "Every day");
    assert_eq!(cadence(3, Frequency::Month), "Every 3 months");
}
