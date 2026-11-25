use quest_domain::EntityId;
use rune::Runtime;

use super::tests::{TempDir, command, demo_app};
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
    let keys: Vec<String> = ids.iter().map(|id| format!("row-{id}")).collect();
    for from in '1'..='5' {
        for to in '1'..='5' {
            if from == to {
                continue;
            }
            command(&mut rt, from);
            settle(&mut rt);
            command(&mut rt, to);
            let page_key = |key: char| {
                ["list-page", "list-page", "plan-page", "review", "list-page"][(key as usize) - ('1' as usize)]
            };
            assert_eq!(rt.presented(page_key(to)).map(|(_, opacity)| opacity), Some(1.0), "⌘{from} → ⌘{to}");
            if page_key(from) != page_key(to) {
                assert!(rt.presented(page_key(from)).is_none());
            }
            assert_nothing_flies(&mut rt, &keys);
        }
    }
}
