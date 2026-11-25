//! QUEST WORK and OTHER: V hides other work on Home and Plan, the filter shows one quest's work,
//! and "Add quest work…" files new tasks under the filtered (or the chip's) quest.

use rune::{Key, Mods, Runtime};

use super::dnd_tests::{settle, task};
use super::tests::{click, command, demo_app, press, type_text};
use crate::state::State;

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn quest_id(rt: &Runtime<State>, title: &str) -> quest_domain::EntityId {
    rt.state.snap.quests.iter().find(|q| q.title == title).map(|q| q.id).unwrap_or_else(|| panic!("quest {title}"))
}

#[test]
fn v_hides_other_work_on_home_and_plan() {
    let (mut rt, _dir) = demo_app();
    settle(&mut rt);
    let errand = task(&rt, "Pick up the dry cleaning").id;
    let quest_work = task(&rt, "Design the next chapter").id;
    assert!(shown(&rt, &format!("home-task-{errand}")), "other work shows under OTHER");
    type_text(&mut rt, "v");
    assert!(!shown(&rt, &format!("home-task-{errand}")), "V hides it on Home");
    assert!(shown(&rt, &format!("home-task-{quest_work}")), "quest work stays");
    command(&mut rt, '2');
    settle(&mut rt);
    assert!(!shown(&rt, &format!("plan-task-{errand}")), "and on Plan");
    assert!(shown(&rt, &format!("plan-task-{quest_work}")));
    type_text(&mut rt, "v");
    assert!(shown(&rt, &format!("plan-task-{errand}")), "V shows it again");
    command(&mut rt, '1');
    assert!(shown(&rt, &format!("home-task-{errand}")));
}

#[test]
fn a_task_changes_section_with_its_quest() {
    let (mut rt, _dir) = demo_app();
    let errand = task(&rt, "Pick up the dry cleaning").id;
    let quest = quest_id(&rt, "Launch my idea");
    rt.state.dispatch(quest_domain::Command::AssignTask { id: errand, quest_id: Some(quest) });
    type_text(&mut rt, "v");
    assert!(shown(&rt, &format!("home-task-{errand}")), "with a quest it is quest work, shown with OTHER hidden");
}

#[test]
fn the_filter_shows_one_quests_work_and_adds_to_it() {
    let (mut rt, _dir) = demo_app();
    let writing = quest_id(&rt, "Write every week");
    let essay = task(&rt, "Draft this week’s essay").id;
    let design = task(&rt, "Design the next chapter").id;
    click(&mut rt, "home-filter");
    type_text(&mut rt, "Write every");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.quest_filter, Some(writing));
    assert!(shown(&rt, &format!("home-task-{essay}")));
    assert!(!shown(&rt, &format!("home-task-{design}")), "other quests' work is filtered out");

    click(&mut rt, "home-add");
    type_text(&mut rt, "Essay intro");
    press(&mut rt, Key::Enter, Mods::default());
    let added = task(&rt, "Essay intro");
    assert_eq!((added.quest_id, added.day), (Some(writing), Some(rt.state.today())), "the filtered quest, today");

    click(&mut rt, "home-filter-chip");
    assert_eq!(rt.state.quest_filter, None, "the chip clears the filter");
    assert!(shown(&rt, &format!("home-task-{design}")));
}

#[test]
fn add_quest_work_uses_the_chips_quest_and_add_a_task_none() {
    let (mut rt, _dir) = demo_app();
    let running = quest_id(&rt, "Run a half marathon");
    click(&mut rt, "home-add-quest");
    type_text(&mut rt, "half marathon");
    press(&mut rt, Key::Enter, Mods::default());
    click(&mut rt, "home-add");
    type_text(&mut rt, "Buy new shoes");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(task(&rt, "Buy new shoes").quest_id, Some(running));

    // OTHER sits below the quest work: scroll down to it.
    let (x, y) = rt.find("home-lists").expect("Home's lists");
    rt.wheel(x, y, -600.0);
    settle(&mut rt);
    click(&mut rt, "home-other-add");
    type_text(&mut rt, "Water the plants");
    press(&mut rt, Key::Enter, Mods::default());
    let other = task(&rt, "Water the plants");
    assert_eq!((other.quest_id, other.day), (None, Some(rt.state.today())), "OTHER's field adds without a quest");
}
