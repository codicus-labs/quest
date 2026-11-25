use super::*;

#[test]
fn a_day_with_no_local_midnight_starts_at_the_first_valid_instant() {
    let day = NaiveDate::from_ymd_opt(2026, 3, 8).unwrap();
    let zone = chrono_tz::America::Havana;
    let at = midnight(day, &zone);
    let first = at.with_timezone(&zone).naive_local();
    assert_eq!(first, day.and_hms_opt(1, 0, 0).unwrap());
    assert_eq!((midnight(day + Days::new(1), &zone) - at).num_hours(), 23);
}
