use quest_domain::{Command, Settings, Snapshot};

use super::*;

#[test]
fn shows_the_weekly_target_in_minutes_and_hours() {
    let snapshot = Snapshot { settings: Settings { weekly_target_minutes: 90 }, ..Snapshot::default() };
    let data = settings(&snapshot);
    assert_eq!((data.weekly_target_minutes, data.weekly_target_label.as_str()), (90, "1.5h"));
    assert_eq!(settings(&Snapshot::default()).weekly_target_label, "20h");
}

#[test]
fn accepts_targets_from_one_hour_to_a_full_week() {
    assert_eq!(set_weekly_target(60), Ok(Command::SetWeeklyTarget { minutes: 60 }));
    assert_eq!(set_weekly_target(168 * 60), Ok(Command::SetWeeklyTarget { minutes: 168 * 60 }));
}

#[test]
fn refuses_targets_outside_the_range() {
    for minutes in [0, 59, 168 * 60 + 1] {
        assert_eq!(set_weekly_target(minutes), Err(MobileError::InvalidWeeklyTarget { minutes }));
    }
}
