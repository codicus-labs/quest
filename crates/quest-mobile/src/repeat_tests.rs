use chrono_tz::Tz;
use quest_domain::{Frequency, RepeatPin, RepeatRule, Snapshot, Weekdays};

use super::*;
use crate::task_types::DayValue;
use crate::test_support::{berlin, day, task};

fn now() -> Now {
    berlin("2026-11-25T10:00:00+01:00")
}

fn draft() -> RepeatDraft {
    RepeatDraft {
        every: "2".into(),
        frequency: RepeatFrequency::Week,
        pin: RepeatPinChoice::Top,
        weekdays: vec![RepeatWeekday::Mon, RepeatWeekday::Fri],
        last_day: false,
        month_day: String::new(),
        starts: "2026-11-25".into(),
        ends_on_date: true,
        ends: "2026-12-31".into(),
        time_zone: "Europe/Berlin".into(),
    }
}

#[test]
fn a_task_without_repeat_opens_daily_from_its_day_but_not_before_today() {
    let snapshot = Snapshot::default();
    let past = task(1, "Old", None, Some("2026-11-20"));
    let draft = draft_for(&snapshot, &past, now());
    assert_eq!((draft.every.as_str(), draft.frequency, draft.pin), ("1", RepeatFrequency::Day, RepeatPinChoice::None));
    assert_eq!(
        (draft.starts.as_str(), draft.ends_on_date, draft.time_zone.as_str()),
        ("2026-11-25", false, "Europe/Berlin")
    );
    let later = task(2, "Later", None, Some("2026-12-02"));
    assert_eq!(draft_for(&snapshot, &later, now()).starts, "2026-12-02");
}

#[test]
fn a_valid_draft_makes_the_desktop_rule() {
    let made = rule(&draft(), now().today()).unwrap();
    let expected = RepeatRule {
        every: 2,
        frequency: Frequency::Week,
        starts: day("2026-11-25"),
        ends: Some(day("2026-12-31")),
        time_zone: "Europe/Berlin".parse::<Tz>().unwrap(),
        pin: RepeatPin::Top,
        weekdays: Weekdays::on(chrono::Weekday::Mon).toggled(chrono::Weekday::Fri),
        month_day: None,
    };
    assert_eq!(made, expected);
}

#[test]
fn a_wrong_field_reports_the_desktop_message() {
    let bad = RepeatDraft { ends: "2026-11-01".into(), ..draft() };
    let reason = "The end date must be on or after the start.";
    assert_eq!(rule(&bad, now().today()), Err(MobileError::InvalidRepeat { reason: reason.into() }));
    let checked = check(&bad, now().today());
    assert_eq!(checked.error.as_deref(), Some(reason));
    assert_eq!(checked.summary, "Every 2 weeks");
    assert_eq!(checked.starts, Some(DayValue { iso: "2026-11-25".into(), label: "Nov 25, 2026".into() }));
    assert_eq!(checked.ends.map(|d| d.label), Some("Nov 1, 2026".into()));
    let typed = RepeatDraft { starts: "Dec 3, 2026".into(), ends: String::new(), ..draft() };
    let normalized = check(&typed, now().today());
    assert_eq!(normalized.starts.map(|d| d.iso), Some("2026-12-03".into()));
    assert_eq!(normalized.ends, None);
    assert_eq!(check(&RepeatDraft { every: "x".into(), ..draft() }, now().today()).summary, "");
}

#[test]
fn weekdays_show_the_start_until_picked_and_keep_the_last_one() {
    let none = RepeatDraft { weekdays: Vec::new(), ..draft() };
    let checked = check(&none, now().today());
    assert_eq!(checked.shown_weekdays, [RepeatWeekday::Wed]);
    assert_eq!(checked.month_day_placeholder, "25");
    let added = toggle_weekday(none, RepeatWeekday::Fri, now().today());
    assert_eq!(added.weekdays, [RepeatWeekday::Wed, RepeatWeekday::Fri]);
    let only = RepeatDraft { weekdays: vec![RepeatWeekday::Fri], ..draft() };
    assert_eq!(toggle_weekday(only, RepeatWeekday::Fri, now().today()).weekdays, [RepeatWeekday::Fri]);
    let unpicked = RepeatDraft { weekdays: Vec::new(), ..draft() };
    let still = toggle_weekday(unpicked, RepeatWeekday::Wed, now().today());
    assert!(still.weekdays.is_empty(), "the start's weekday stays implicit, as on the desktop");
    let moved = RepeatDraft { starts: "2026-11-27".into(), ..still };
    assert_eq!(check(&moved, now().today()).shown_weekdays, [RepeatWeekday::Fri], "it follows a new start");
}
