use quest_domain::{QuestColor, Snapshot};

use super::*;
use crate::test_support::{berlin, day, entry, quest, task};

/// Wednesday, 25 Nov 2026, 10:00 in Berlin.
fn now() -> Now {
    berlin("2026-11-25T10:00:00+01:00")
}

fn snapshot() -> Snapshot {
    let garden = quest_domain::Quest { color: QuestColor::Green, ..quest(2, "Garden") };
    Snapshot {
        quests: vec![quest(1, "Photo series"), garden],
        tasks: vec![task(10, "Shoot", Some(1), None), task(11, "Dig", Some(2), None), task(12, "Mail", None, None)],
        time_entries: vec![
            // Monday: photo 2 h, garden 30 min; Tuesday: garden 1 h; other work 30 min.
            entry(20, Some(10), Some(1), "2026-11-23T08:00:00+01:00", Some("2026-11-23T10:00:00+01:00")),
            entry(21, Some(11), Some(2), "2026-11-23T11:00:00+01:00", Some("2026-11-23T11:30:00+01:00")),
            entry(22, Some(11), Some(2), "2026-11-24T11:00:00+01:00", Some("2026-11-24T12:00:00+01:00")),
            entry(23, Some(12), None, "2026-11-24T13:00:00+01:00", Some("2026-11-24T13:30:00+01:00")),
            // The week before: 5 h.
            entry(24, Some(10), Some(1), "2026-11-17T08:00:00+01:00", Some("2026-11-17T13:00:00+01:00")),
        ],
        ..Snapshot::default()
    }
}

#[test]
fn the_cards_show_hours_against_the_target_average_and_share() {
    let data = review(&snapshot(), day("2026-11-25"), now());
    assert_eq!((data.week_start.as_str(), data.week_label.as_str()), ("2026-11-23", "Nov 23 – 29"));
    assert_eq!((data.previous_week.as_str(), data.next_week.as_str()), ("2026-11-16", "2026-11-30"));
    assert!(data.is_this_week);
    assert_eq!((data.quest_hours.as_str(), data.target.as_str()), ("3.5h", "/ 20h"));
    assert_eq!(data.target_note, "17% of target");
    assert!((data.target_progress - 0.175).abs() < 1e-6);
    assert_eq!(data.daily_average, "1.2h", "3.5 h over Monday to Wednesday");
    assert_eq!(data.quest_share, "87%");
    assert_eq!((data.quest_work.as_str(), data.other.as_str()), ("Quest work · 3.5h", "Other · 0.5h"));
    assert!((data.quest_fraction - 0.875).abs() < 1e-6);
    assert_eq!(data.split_a11y, "Quest work 3.5h, other 0.5h: 87% quest work");
}

#[test]
fn days_stack_quests_in_the_weeks_order() {
    let data = review(&snapshot(), day("2026-11-25"), now());
    let monday = &data.days[0];
    assert_eq!((monday.label.as_str(), monday.hours.as_str(), monday.fraction), ("Mon", "2.5h", 1.0));
    let colors: Vec<&str> = monday.segments.iter().map(|s| s.color_hex.as_str()).collect();
    assert_eq!(colors, ["#5c8fd9", "#62b87a"], "photo (most time this week) at the bottom");
    assert_eq!(monday.segments.iter().map(|s| s.fraction).collect::<Vec<_>>(), [0.8, 0.2]);
    assert_eq!(monday.a11y_label, "Monday: 2.5h. Photo series 2h, Garden 0.5h");
    assert_eq!(data.days[1].segments.len(), 1);
    assert!(data.days[2].is_today);
    assert_eq!(data.days[2].a11y_label, "Wednesday: 0h");
    assert_eq!(data.days[6].fraction, 0.0);
}

#[test]
fn a_quiet_week_still_scales_to_an_hour() {
    let mut snap = snapshot();
    snap.time_entries.truncate(1);
    snap.time_entries[0].ended_at = Some(crate::test_support::at("2026-11-23T08:30:00+01:00"));
    let data = review(&snap, day("2026-11-25"), now());
    assert_eq!(data.days[0].fraction, 0.5);
}

#[test]
fn hours_by_quest_are_relative_to_the_largest() {
    let data = review(&snapshot(), day("2026-11-25"), now());
    let quests: Vec<(&str, &str, f32)> =
        data.by_quest.iter().map(|q| (q.name.as_str(), q.hours.as_str(), q.fraction)).collect();
    assert_eq!(quests, [("Photo series", "2h", 1.0), ("Garden", "1.5h", 0.75)]);
    assert_eq!(data.by_quest[1].a11y_label, "Garden: 1.5h");
    assert_eq!(data.by_quest_a11y, "Hours by quest: Photo series 2h, Garden 1.5h");
}

#[test]
fn recent_weeks_end_with_the_reviewed_one() {
    let data = review(&snapshot(), day("2026-11-25"), now());
    assert_eq!(data.weeks.len(), 8);
    let last = &data.weeks[7];
    assert_eq!(
        (last.week_start.as_str(), last.label.as_str(), last.hours.as_str(), last.selected),
        ("2026-11-23", "Nov 23", "3.5h", true)
    );
    assert_eq!((data.weeks[6].hours.as_str(), data.weeks[6].fraction), ("5h", 1.0));
    assert!((last.fraction - 0.7).abs() < 1e-6);
    assert_eq!(last.a11y_label, "Week of Nov 23: 3.5h");
    assert!(data.weeks_a11y.starts_with("Quest hours of recent weeks: Oct 5 0h, "));
    assert!(data.weeks_a11y.ends_with("Nov 16 5h, Nov 23 3.5h"));
}

#[test]
fn another_week_is_not_this_week() {
    let data = review(&snapshot(), day("2026-11-18"), now());
    assert!(!data.is_this_week);
    assert_eq!(data.quest_hours, "5h");
    assert_eq!(data.daily_average, "0.7h", "a past week averages over all seven days");
    assert!(data.days.iter().all(|d| !d.is_today));
    let empty = review(&Snapshot::default(), day("2026-11-25"), now());
    assert_eq!((empty.quest_share.as_str(), empty.quest_fraction), ("0%", 0.0));
    assert!(empty.by_quest.is_empty());
    assert_eq!(empty.by_quest_a11y, "No quest time this week");
}

#[test]
fn days_split_at_local_midnight_also_in_a_week_with_a_clock_change() {
    let snap = Snapshot {
        quests: vec![quest(1, "Photo series")],
        tasks: vec![task(10, "Shoot", Some(1), None)],
        time_entries: vec![
            // Monday 23:00 to Tuesday 01:00.
            entry(20, Some(10), Some(1), "2026-10-19T23:00:00+02:00", Some("2026-10-20T01:00:00+02:00")),
            // Sunday 01:30 to 03:30 across the change back to winter time: three real hours.
            entry(21, Some(10), Some(1), "2026-10-25T01:30:00+02:00", Some("2026-10-25T03:30:00+01:00")),
        ],
        ..Snapshot::default()
    };
    let data = review(&snap, day("2026-10-21"), now());
    let hours: Vec<&str> = data.days.iter().map(|d| d.hours.as_str()).collect();
    assert_eq!(hours, ["1h", "1h", "0h", "0h", "0h", "0h", "3h"]);
    assert_eq!(data.quest_hours, "5h");
}
