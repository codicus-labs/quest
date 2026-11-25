use rune::{Key, Mods, Runtime};

use super::dnd_tests::task;
use super::tests::{click, command, demo_app, press, type_text};
use crate::state::{State, View};

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn quest_id(rt: &Runtime<State>, title: &str) -> quest_domain::EntityId {
    rt.state.snap.quests.iter().find(|q| q.title == title).map(|q| q.id).unwrap_or_else(|| panic!("quest {title}"))
}

#[test]
fn plan_today_has_the_same_rows_as_today() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, '2');
    let today = super::lists::shown_tasks(&rt.state);
    command(&mut rt, '3');
    let plan = super::lists::shown_tasks(&rt.state);
    assert_eq!(&plan[..today.len()], today);
}

fn quick_add(rt: &mut Runtime<State>, title: &str) -> quest_domain::Task {
    click(rt, "quick-add");
    type_text(rt, title);
    press(rt, Key::Enter, Mods::default());
    task(rt, title)
}

#[test]
fn quick_add_plans_today_with_the_picked_quest_files_on_a_quest_page_and_leaves_inbox_bare() {
    let (mut rt, _dir) = demo_app();
    let today = rt.state.today();
    let running = quest_id(&rt, "Run a half marathon");
    let plain = quick_add(&mut rt, "Water the plants");
    assert_eq!((plain.quest_id, plain.planned_on), (None, Some(today)), "Today: today, no quest yet");

    click(&mut rt, "quick-add-quest");
    type_text(&mut rt, "half marathon");
    press(&mut rt, Key::Enter, Mods::default());
    let shoes = quick_add(&mut rt, "Buy new shoes");
    assert_eq!((shoes.quest_id, shoes.planned_on), (Some(running), Some(today)), "the picked quest");
    assert!(shown(&rt, &format!("row-{}", shoes.id)), "and it lists on Today");

    command(&mut rt, '1');
    let idea = quick_add(&mut rt, "Learn to juggle");
    assert_eq!((idea.quest_id, idea.planned_on), (None, None), "the Inbox: neither");
    assert!(shown(&rt, &format!("row-{}", idea.id)));

    rt.state.go(View::Quest(running));
    rt.frame(0.0);
    let laces = quick_add(&mut rt, "Lace the new shoes");
    assert_eq!((laces.quest_id, laces.planned_on), (Some(running), None), "a quest's page: that quest");
}
