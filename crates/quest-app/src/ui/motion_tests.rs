//! Motion: Later glides the planner's day columns with their cards as one board, and
//! switching views (or calendar weeks) flies nothing from one place to another.

use chrono::Days;
use quest_domain::EntityId;
use rune::Runtime;

use super::tests::{TempDir, click, command, demo_app};
use crate::state::State;

fn app() -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = demo_app();
    settle(&mut rt);
    (rt, dir)
}

fn settle(rt: &mut Runtime<State>) {
    rt.frame(0.0);
    rt.settle_all();
    rt.frame(0.0);
}

fn drawn(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.presented(key).unwrap_or_else(|| panic!("`{key}` is drawn")).0
}

fn task_id(rt: &Runtime<State>, title: &str) -> EntityId {
    rt.state.snap.tasks.iter().find(|t| t.title == title).map(|t| t.id).expect("demo task")
}

#[test]
fn later_glides_the_columns_with_their_cards() {
    let (mut rt, _dir) = app();
    command(&mut rt, '2');
    settle(&mut rt);
    let tomorrow = rt.state.today() + Days::new(1);
    let card = format!("plan-task-{}", task_id(&rt, "Polish the details that matter"));
    let column = format!("plan-col-{tomorrow}");
    let offset = |rt: &Runtime<State>| {
        let (c, k) = (drawn(rt, &card), drawn(rt, &column));
        (c[0] - k[0], c[1] - k[1])
    };
    let before = offset(&rt);
    let column_x = drawn(&rt, &column)[0];

    click(&mut rt, "plan-next");
    let mut moved = false;
    for _ in 0..12 {
        rt.frame(1.0 / 60.0);
        let (dx, dy) = offset(&rt);
        assert!((dx - before.0).abs() < 0.5 && (dy - before.1).abs() < 0.5, "the card rides its column");
        moved |= (drawn(&rt, &column)[0] - column_x).abs() > 1.0;
    }
    assert!(moved, "the column itself slides");
    settle(&mut rt);
    assert!(drawn(&rt, &column)[0] < column_x - 50.0, "tomorrow ends one place to the left");
    assert_eq!(offset(&rt), before);
}

/// After a view switch, nothing is on its way from somewhere else: what shows on the first
/// frame is where it rests.
fn assert_nothing_flies(rt: &mut Runtime<State>, keys: &[String]) {
    rt.frame(1.0 / 60.0);
    let first: Vec<Option<[f32; 4]>> = keys.iter().map(|k| rt.presented(k).map(|p| p.0)).collect();
    settle(rt);
    for (key, at) in keys.iter().zip(first) {
        let (Some(at), Some(rest)) = (at, rt.presented(key).map(|p| p.0)) else { continue };
        assert!((at[0] - rest[0]).abs() < 0.5 && (at[1] - rest[1]).abs() < 0.5, "`{key}` flew: {at:?} → {rest:?}");
    }
}

#[test]
fn switching_views_flies_no_card() {
    let (mut rt, _dir) = app();
    let today = rt.state.today();
    let ids: Vec<EntityId> = rt.state.snap.tasks_on(today).map(|t| t.id).collect();
    let keys: Vec<String> = ids
        .iter()
        .flat_map(|id| ["home", "plan"].map(|prefix| [format!("{prefix}-face-{id}"), format!("{prefix}-task-{id}")]))
        .flatten()
        .collect();
    for from in '1'..='6' {
        for to in '1'..='6' {
            if from == to {
                continue;
            }
            command(&mut rt, from);
            settle(&mut rt);
            command(&mut rt, to);
            let page_key =
                |key: char| ["home", "plan", "calendar", "focus", "quests", "review"][(key as usize) - ('1' as usize)];
            assert_eq!(rt.presented(page_key(to)).map(|(_, opacity)| opacity), Some(1.0));
            assert!(rt.presented(page_key(from)).is_none());
            assert_nothing_flies(&mut rt, &keys);
        }
    }
}

#[test]
fn switching_calendar_weeks_flies_nothing() {
    let (mut rt, _dir) = app();
    command(&mut rt, '3');
    settle(&mut rt);
    let keys: Vec<String> =
        rt.state.snap.time_entries.iter().map(|e| format!("calendar-grid-entry-{}", e.id)).collect();
    click(&mut rt, "calendar-next");
    settle(&mut rt);
    click(&mut rt, "calendar-prev");
    assert_nothing_flies(&mut rt, &keys);
}
