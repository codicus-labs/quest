use quest_domain::{Command, EntityId};

use crate::test_support::{World, day};

#[test]
fn the_context_holds_only_the_active_workspace() {
    let mut world = World::new();
    let launch = world.quest("Launch");
    world.task("Write launch post", Some(launch), Some(day("2026-11-24")));
    let private = world.id();
    world.run(&[Command::CreateWorkspace { id: private, name: "Private".into() }]).unwrap();
    world.workspace = private;
    let garden = world.quest("Garden");
    world.task("Buy seeds", None, None);

    let context = world.request("Seeds bought").context;
    assert_eq!(context.quests.iter().map(|q| q.id).collect::<Vec<EntityId>>(), [garden]);
    assert_eq!(context.tasks.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["Buy seeds"]);
}
