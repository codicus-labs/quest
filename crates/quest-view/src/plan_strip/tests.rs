use chrono::{Days, NaiveDate};

use super::*;

fn day(s: &str) -> NaiveDate {
    s.parse().expect("a date")
}

fn strip() -> Strip {
    // 5 columns of 200 with 4 gaps of 12.
    Strip::around(day("2026-10-15"), 1048.0)
}

#[test]
fn five_columns_fill_the_viewport() {
    let strip = strip();
    assert_eq!(strip.column_w(), 200.0);
    assert_eq!(strip.stride(), 212.0);
    let today = day("2026-10-15");
    let offset = strip.offset_for(today);
    assert_eq!(strip.first_visible(offset), today);
    assert_eq!(strip.day_at(offset, 0.0), today);
    assert_eq!(strip.day_at(offset, 4.0 * 212.0 + 10.0), today + Days::new(4));
}

#[test]
fn a_built_day_sits_where_the_whole_strip_has_it_whatever_the_window() {
    let strip = strip();
    let day = day("2026-11-02");
    for weeks in [-30_i64, -3, 0, 3, 30] {
        let offset = strip.offset_for(day) + weeks as f32 * 7.0 * strip.stride();
        let window = strip.window(offset, None);
        // The view puts a spacer as wide as the days before the window, then the days.
        let lead = strip.x_of(window.first);
        for (i, d) in window.days().enumerate() {
            assert_eq!(lead + i as f32 * strip.stride(), strip.x_of(d), "{d} after scrolling {weeks} weeks");
        }
    }
}

#[test]
fn the_window_covers_what_shows_and_moves_a_week_at_a_time() {
    let strip = strip();
    let start = day("2026-10-13");
    let mut offset = strip.offset_for(start);
    let first = strip.window(offset, None);
    let mut changes = 0;
    // Scroll three weeks later, then six earlier, a point at a time.
    for step in std::iter::repeat_n(1.0, (21.0 * strip.stride()) as usize)
        .chain(std::iter::repeat_n(-1.0, (42.0 * strip.stride()) as usize))
    {
        let before = strip.window(offset, None);
        offset += step;
        let window = strip.window(offset, None);
        let shown_from = strip.day_at(offset, 0.0);
        let shown_to = strip.day_at(offset, strip.viewport_w - 1.0);
        assert!(window.first <= shown_from, "the window starts before what shows");
        assert!(window.first + Days::new(window.count as u64) > shown_to, "and ends after it");
        if window != before {
            changes += 1;
            assert_eq!((window.first - before.first).num_days().abs() % 7, 0, "whole weeks");
        }
    }
    assert!(changes <= 10, "the window moved {changes} times over 9 weeks of scrolling");
    assert!(changes >= 8, "it does move along: {changes}");
    assert_eq!(strip.first_visible(offset), start - Days::new(21));
    assert!(first.count <= 21, "a few weeks are built, not years: {first:?}");
}

#[test]
fn the_window_keeps_a_dragged_cards_day() {
    let strip = strip();
    let today = day("2026-10-15");
    let offset = strip.offset_for(today + Days::new(60));
    let window = strip.window(offset, Some(today));
    assert_eq!(window.first, today);
    assert!(window.days().any(|d| d == today + Days::new(64)));
}

#[test]
fn the_strip_reaches_years_either_way_and_stops_at_its_ends() {
    let strip = strip();
    let today = day("2026-10-15");
    // Near either end; the last few days can only show further right.
    let far = Days::new(3 * 365 - 10);
    assert_eq!(strip.first_visible(strip.offset_for(today - far)), today - far);
    assert_eq!(strip.first_visible(strip.offset_for(today + far)), today + far);
    assert_eq!(strip.clamp(-50.0), 0.0);
    assert_eq!(strip.clamp(f32::MAX), strip.max_offset());
    let end = strip.window(strip.max_offset(), None);
    assert_eq!(end.first + Days::new(end.count as u64 - 1), strip.start + Days::new(2 * REACH_DAYS));
}

#[test]
fn a_resize_keeps_the_first_day_in_place() {
    let strip = strip();
    let monday = day("2026-10-19");
    let offset = strip.offset_for(monday) + strip.stride() * 0.25;
    let (wider, moved) = strip.resized(offset, 1548.0);
    assert_eq!(wider.column_w(), 300.0);
    assert_eq!(moved, wider.offset_for(monday) + wider.stride() * 0.25);
    assert_eq!(wider.first_visible(moved), monday);
}

#[test]
fn edges_scroll_faster_nearer_the_edge_and_not_in_the_middle() {
    let (left, right) = (100.0, 1100.0);
    assert_eq!(edge_speed(600.0, left, right), 0.0);
    assert_eq!(edge_speed(left + EDGE_ZONE + 1.0, left, right), 0.0, "just inside the zone's border");
    let near = edge_speed(right - 10.0, left, right);
    let far = edge_speed(right - 60.0, left, right);
    assert!(near > far && far > 0.0, "later, faster at the edge: {far} < {near}");
    assert_eq!(edge_speed(right, left, right), EDGE_MAX_SPEED);
    assert_eq!(edge_speed(right + 40.0, left, right), EDGE_MAX_SPEED, "past the edge is the edge");
    assert!(edge_speed(left + 10.0, left, right) < 0.0, "earlier at the left");
    assert_eq!(edge_speed(left + 10.0, left, right), -near);
}

#[test]
fn scrolling_at_a_speed_moves_by_time_and_stops_at_the_ends() {
    let strip = strip();
    assert_eq!(scrolled(&strip, 1000.0, 500.0, 0.5), 1250.0);
    assert_eq!(scrolled(&strip, 100.0, -500.0, 1.0), 0.0);
    assert_eq!(scrolled(&strip, strip.max_offset(), 500.0, 1.0), strip.max_offset());
}

#[test]
fn the_built_window_holds_while_what_shows_stays_well_inside_it() {
    let strip = strip();
    let mut offset = strip.offset_for(day("2026-10-13"));
    let mut built = strip.window(offset, None);
    let mut changes = 0;
    // Nine weeks later a point at a time, then back and forth across where the window moved last.
    let later = std::iter::repeat_n(1.0, (63.0 * strip.stride()) as usize);
    let wiggle = (0..20).flat_map(|_| std::iter::repeat_n(-1.0, 300).chain(std::iter::repeat_n(1.0, 300)));
    for step in later.chain(wiggle) {
        offset += step;
        let next = strip.next_window(built, offset, None);
        let shown_from = strip.day_at(offset, 0.0);
        let shown_to = strip.day_at(offset, strip.viewport_w - 1.0);
        assert!(next.first + Days::new(HOLD_DAYS as u64) <= shown_from, "days built before what shows");
        assert!(next.first + Days::new(next.count as u64 - HOLD_DAYS as u64) > shown_to, "and after it");
        if next != built {
            changes += 1;
        }
        built = next;
    }
    assert!(changes <= 10, "the window moved {changes} times over 9 weeks and some wiggling");
    assert!(changes >= 7, "it does move along: {changes}");
}

#[test]
fn the_built_window_takes_in_a_dragged_cards_day() {
    let strip = strip();
    let today = day("2026-10-15");
    let offset = strip.offset_for(today + Days::new(60));
    let built = strip.window(offset, None);
    let next = strip.next_window(built, offset, Some(today));
    assert_eq!(next.first, today);
}

#[test]
fn the_month_is_the_one_the_middle_of_the_board_shows() {
    let strip = strip();
    // From Thu Oct 29 the middle column is Sat Oct 31; a day on, it is Sun Nov 1.
    assert_eq!(strip.month_at(strip.offset_for(day("2026-10-29"))), day("2026-10-01"));
    assert_eq!(strip.month_at(strip.offset_for(day("2026-10-30"))), day("2026-11-01"));
    let mid_day = strip.offset_for(day("2026-10-29")) + strip.stride() * 0.4;
    assert_eq!(strip.month_at(mid_day), day("2026-10-01"), "until most of the next day shows");
}
