use super::*;

fn day(month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, month, day).unwrap()
}

#[test]
fn choices_and_navigation_stay_in_the_future() {
    let monday = day(11, 23);
    assert_eq!(QuickDate::Today.date(monday), Some(monday));
    assert_eq!(QuickDate::Tomorrow.date(monday), Some(day(11, 24)));
    assert_eq!(QuickDate::NextMonday.date(monday), Some(day(11, 30)));
    assert_eq!(QuickDate::None.date(monday), None);
    assert_eq!(shift(monday, Step::Week, false, monday), monday);
    assert_eq!(shift(day(1, 31), Step::Month, true, day(1, 1)), day(2, 28));
    assert_eq!(shift(day(11, 30), Step::Day, true, monday), day(12, 1));
}

#[test]
fn vim_navigation_and_quick_dates_do_not_overlap() {
    let today = day(11, 25);
    let mut cursor = today;
    for letter in ["l", "l", "j"] {
        let Some(Selection::Move(next)) = letter_selection(letter, cursor, today) else { panic!("not a move") };
        cursor = next;
    }
    assert_eq!(cursor, day(12, 4));
    for (letter, expected) in
        [("h", day(12, 3)), ("k", day(11, 27)), ("[", today), ("]", day(1, 4).with_year(2027).unwrap())]
    {
        assert_eq!(letter_selection(letter, cursor, today), Some(Selection::Move(expected)));
    }
    for (letter, expected) in [("t", today), ("m", day(11, 26)), ("n", day(11, 30)), ("w", day(11, 30))] {
        assert_eq!(letter_selection(letter, cursor, today), Some(Selection::Apply(Some(expected))));
    }
}

#[test]
fn weeks_start_monday_and_mark_the_other_boundary() {
    let today = day(11, 20);
    let dates = TaskDates { planned_on: Some(day(11, 21)), due_on: Some(day(11, 25)) };
    for (field, marker, boundary) in [(DateField::Plan, DateField::Due, 25), (DateField::Due, DateField::Plan, 21)] {
        let cells = month(today, today, dates, field);
        assert_eq!(cells.len() % 7, 0);
        assert_eq!(cells[0].date.weekday(), Weekday::Mon);
        assert!(!cells[0].in_month && !cells[0].enabled);
        assert_eq!(cells.iter().find(|d| d.marker == Some(marker)).unwrap().date, day(11, boundary));
        assert_eq!(cells.iter().find(|d| d.selected).unwrap().date, dates.get(field).unwrap());
    }
}
