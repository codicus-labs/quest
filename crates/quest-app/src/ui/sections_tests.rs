//! QUEST WORK and OTHER on the planner: V hides other work and the filter shows one quest's work;
//! the quick-add field's defaults on Today, the Inbox and a quest's page.

use rune::{Key, Mods, Runtime};

use super::dnd_tests::{settle, task};
use super::tests::{click, command, demo_app, press, type_text};
use crate::state::{State, View};

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn quest_id(rt: &Runtime<State>, title: &str) -> quest_domain::EntityId {
    rt.state.snap.quests.iter().find(|q| q.title == title).map(|q| q.id).unwrap_or_else(|| panic!("quest {title}"))
}

fn plan(rt: &mut Runtime<State>) {
    command(rt, '3');
    settle(rt);
}

#[test]
fn v_hides_other_work_on_plan() {
    let (mut rt, _dir) = demo_app();
    plan(&mut rt);
    let errand = task(&rt, "Pick up the dry cleaning").id;
    let quest_work = task(&rt, "Design the next chapter").id;
    assert!(shown(&rt, &format!("plan-task-{errand}")), "other work shows");
    type_text(&mut rt, "v");
    assert!(!shown(&rt, &format!("plan-task-{errand}")), "V hides it");
    assert!(shown(&rt, &format!("plan-task-{quest_work}")), "quest work stays");
    type_text(&mut rt, "v");
    assert!(shown(&rt, &format!("plan-task-{errand}")), "V shows it again");
}

#[test]
fn a_task_changes_section_with_its_quest() {
    let (mut rt, _dir) = demo_app();
    plan(&mut rt);
    let errand = task(&rt, "Pick up the dry cleaning").id;
    let quest = quest_id(&rt, "Launch my idea");
    rt.state.dispatch(quest_domain::Command::AssignTask { id: errand, quest_id: Some(quest) });
    type_text(&mut rt, "v");
    assert!(shown(&rt, &format!("plan-task-{errand}")), "with a quest it is quest work, shown with OTHER hidden");
}

#[test]
fn the_filter_shows_one_quests_work() {
    let (mut rt, _dir) = demo_app();
    plan(&mut rt);
    let writing = quest_id(&rt, "Write every week");
    let essay = task(&rt, "Draft this week’s essay").id;
    let design = task(&rt, "Design the next chapter").id;
    click(&mut rt, "plan-filter");
    type_text(&mut rt, "Write every");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.quest_filter, Some(writing));
    assert!(shown(&rt, &format!("plan-task-{essay}")));
    assert!(!shown(&rt, &format!("plan-task-{design}")), "other quests' work is filtered out");

    click(&mut rt, "plan-filter-chip");
    assert_eq!(rt.state.quest_filter, None, "the chip clears the filter");
    assert!(shown(&rt, &format!("plan-task-{design}")));
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
