use quest_domain::Snapshot;

use super::*;
use crate::test_support::{berlin, day, entry, quest, task};

/// Wednesday, 25 Nov 2026, 10:40 in Berlin.
fn now() -> Now {
    berlin("2026-11-25T10:40:00+01:00")
}

fn query(first_day: Option<&str>, day_count: u32) -> CalendarQuery {
    CalendarQuery { first_day: first_day.map(str::to_owned), day_count }
}

fn page(first_day: Option<&str>, day_count: u32) -> Page {
    Page::of(&query(first_day, day_count), now().today()).unwrap()
}

fn snapshot() -> Snapshot {
    let mut snapshot = Snapshot {
        quests: vec![quest(1, "Book")],
        tasks: vec![task(10, "Write", Some(1), Some("2026-11-25")), task(11, "Mail", None, Some("2026-11-25"))],
        time_entries: vec![
            // Quest work: 1h on Monday, 1.5h today; other work: 30 min today.
            entry(20, Some(10), Some(1), "2026-11-23T09:00:00+01:00", Some("2026-11-23T10:00:00+01:00")),
            entry(21, Some(10), Some(1), "2026-11-25T08:00:00+01:00", Some("2026-11-25T09:30:00+01:00")),
            entry(22, Some(11), None, "2026-11-25T09:30:00+01:00", Some("2026-11-25T10:00:00+01:00")),
            // Last week: not in this week's hours.
            entry(23, Some(10), Some(1), "2026-11-20T09:00:00+01:00", Some("2026-11-20T12:00:00+01:00")),
        ],
        ..Snapshot::default()
    };
    snapshot.settings.weekly_target_minutes = 10 * 60;
    snapshot
}

#[test]
fn pages_without_a_day_hold_today_and_weeks_start_on_monday() {
    assert_eq!(page(None, 1), Page { first: day("2026-11-25"), count: 1 });
    assert_eq!(page(None, 3), Page { first: day("2026-11-24"), count: 3 }, "today in the middle");
    assert_eq!(page(None, 7), Page { first: day("2026-11-23"), count: 7 });
    assert_eq!(page(Some("2026-11-27"), 7).first, day("2026-11-23"), "a week from its Monday");
    assert_eq!(page(Some("2026-11-27"), 3).first, day("2026-11-27"));
    assert_eq!(page(None, 0).count, 1);
    assert_eq!(page(None, 12).count, 7);
    assert!(Page::of(&query(Some("soon"), 1), now().today()).is_err());
}

#[test]
fn swipes_page_by_the_days_shown() {
    let one = page(Some("2026-11-25"), 1);
    assert_eq!((one.previous().unwrap(), one.next().unwrap()), (day("2026-11-24"), day("2026-11-26")));
    let three = page(Some("2026-11-24"), 3);
    assert_eq!((three.previous().unwrap(), three.next().unwrap()), (day("2026-11-21"), day("2026-11-27")));
    let week = page(None, 7);
    assert_eq!((week.previous().unwrap(), week.next().unwrap()), (day("2026-11-16"), day("2026-11-30")));
}

#[test]
fn a_page_shows_each_days_grid_and_total_with_the_weeks_quest_hours() {
    let data = calendar(&snapshot(), page(None, 3), now(), None).unwrap();
    let heads: Vec<_> =
        data.days.iter().map(|d| (d.weekday.as_str(), d.day_number.as_str(), d.is_today, d.total.as_str())).collect();
    assert_eq!(heads, [("Tue", "24", false, "0h"), ("Wed", "25", true, "2h"), ("Thu", "26", false, "0h")]);
    assert_eq!(data.days[1].grid.blocks.len(), 2);
    assert_eq!(data.days[1].grid.now_minute, Some(640.0));
    assert_eq!((data.previous_first_day.as_str(), data.next_first_day.as_str()), ("2026-11-21", "2026-11-27"));
    assert!(data.shows_today);
    assert_eq!(data.week_label, "Nov 23 – 29");
    assert_eq!((data.week_hours.as_str(), data.weekly_target.as_str()), ("2.5h", "10h"));
    assert!((data.week_progress - 0.25).abs() < 1e-6);
    assert_eq!(data.scroll_minute, 600, "the start of the current hour");
    assert_eq!(data.month_label, "November 2026");
}

#[test]
fn the_header_counts_the_week_of_the_pages_middle_day() {
    // Sat 21 to Mon 23: the middle day is Sunday, so last week counts (3h on Friday).
    let data = calendar(&snapshot(), page(Some("2026-11-21"), 3), now(), None).unwrap();
    assert_eq!((data.week_label.as_str(), data.week_hours.as_str()), ("Nov 16 – 22", "3h"));
    assert!(!data.shows_today);
}
