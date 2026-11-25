//! Nothing jumps: a period's back and forward buttons stay where they are while one steps through
//! periods with labels of different widths, the search stays reachable at the minimum width, and Focus's
//! back button starts past the window's traffic lights.

use rune::{ChromeInsets, Runtime};

use super::dnd_tests::{rect, settle};
use super::tests::{click, demo_app};
use crate::state::{State, View};

/// Steps back `weeks` times with `prev` and returns the x of `prev` and `next` and the label
/// after each step.
fn step_back(rt: &mut Runtime<State>, keys: [&str; 3], weeks: usize) -> Vec<(f32, f32, String)> {
    let [prev, label, next] = keys;
    let mut seen = Vec::new();
    for _ in 0..weeks {
        click(rt, prev);
        settle(rt);
        let text = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(label)).and_then(|e| e.text);
        seen.push((rect(rt, prev)[0], rect(rt, next)[0], text.unwrap_or_default()));
    }
    seen
}

/// Six weeks back from the demo's Oct 15 cross a month ("Sep 28 – Oct 4") between short and long
/// labels ("Oct 5 – 11").
fn buttons_stay_put(view: View, keys: [&str; 3]) {
    let (mut rt, _dir) = demo_app();
    rt.state.go(view);
    settle(&mut rt);
    let seen = step_back(&mut rt, keys, 6);

    let labels: std::collections::BTreeSet<usize> = seen.iter().map(|(_, _, label)| label.chars().count()).collect();
    assert!(labels.len() > 1, "the weeks have labels of different lengths: {seen:?}");
    let (prev_x, next_x, _) = seen[0];
    for (prev, next, label) in &seen {
        assert_eq!((*prev, *next), (prev_x, next_x), "the buttons stay put at `{label}`");
    }
}

#[test]
fn review_week_buttons_stay_put() {
    buttons_stay_put(View::Review, ["review-prev", "review-range", "review-next"]);
}

#[test]
fn focus_heading_starts_past_the_traffic_lights() {
    let (mut rt, _dir) = demo_app();
    let mut insets = ChromeInsets::default();
    insets.leading = 78.0;
    rt.set_chrome_insets(insets);
    rt.state.go(View::Focus);
    settle(&mut rt);

    let [heading_x, ..] = rect(&rt, "focus-heading");
    assert_eq!(heading_x, super::chrome::leading_pad(insets.leading), "where the top bar's logo starts");
    assert!(heading_x >= insets.leading + 16.0, "the heading clears the traffic lights");
}

#[test]
fn the_magnifier_above_the_page_opens_the_palette_at_the_minimum_width() {
    let (mut rt, _dir) = demo_app();
    rt.set_viewport((1000.0, 640.0), 1.0);
    settle(&mut rt);
    let [_, _, search_w, search_h] = rect(&rt, "search");
    assert_eq!((search_w, search_h), (32.0, 32.0));
    click(&mut rt, "search");
    assert_eq!(rt.state.overlay, crate::state::Overlay::Palette);
}
