//! Nothing jumps: a period's back and forward buttons stay where they are while one steps through
//! periods with labels of different widths, the top bar keeps room after its tabs, and Focus's
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
fn focus_back_button_starts_past_the_traffic_lights() {
    let (mut rt, _dir) = demo_app();
    let mut insets = ChromeInsets::default();
    insets.leading = 78.0;
    rt.set_chrome_insets(insets);
    rt.state.go(View::Focus);
    settle(&mut rt);

    let [back_x, ..] = rect(&rt, "focus-back");
    assert_eq!(back_x, super::topbar::leading_pad(insets.leading), "where the top bar's logo starts");
    assert!(back_x >= insets.leading + 16.0, "the back button clears the traffic lights");
}

/// `demo_app` with the demo's session running in another workspace: the bar shows the timer pill.
fn with_timer_pill() -> (Runtime<State>, super::tests::TempDir) {
    let (mut rt, dir) = demo_app();
    let work = quest_domain::EntityId::new();
    rt.state.dispatch(quest_domain::Command::CreateWorkspace { id: work, name: "Work".into() });
    rt.state.switch_workspace(work);
    settle(&mut rt);
    (rt, dir)
}

fn search_text_shown(rt: &Runtime<State>) -> bool {
    rt.elements().into_iter().any(|e| e.text.as_deref() == Some("Search or jump…"))
}

#[test]
fn a_narrow_bar_with_the_timer_pill_shows_the_search_as_a_magnifier() {
    let (mut rt, _dir) = with_timer_pill();
    rt.set_viewport((1000.0, 750.0), 1.0);
    settle(&mut rt);

    let [tab_x, _, tab_w, _] = rect(&rt, "tab-review");
    let [pill_x, ..] = rect(&rt, "timer-elsewhere");
    assert!(pill_x - (tab_x + tab_w) >= 20.0, "room between Review and the timer pill");
    let [_, _, search_w, search_h] = rect(&rt, "search");
    assert_eq!((search_w, search_h), (36.0, 36.0), "the search is a magnifier");
    assert!(!search_text_shown(&rt), "no cut-off search text");

    click(&mut rt, "search");
    assert_eq!(rt.state.overlay, crate::state::Overlay::Palette, "the magnifier opens the palette");
}

#[test]
fn a_wide_bar_shows_the_whole_search_field() {
    let (mut rt, _dir) = with_timer_pill();
    rt.set_viewport((1400.0, 750.0), 1.0);
    settle(&mut rt);
    assert!(search_text_shown(&rt), "with room, the field and its text are back");
    assert!(rect(&rt, "search")[2] >= 210.0);

    let (mut rt, _dir) = demo_app();
    settle(&mut rt);
    assert!(search_text_shown(&rt), "at 1100 pt without the pill the whole field fits");
}
