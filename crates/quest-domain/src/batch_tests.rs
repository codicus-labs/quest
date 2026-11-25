use super::*;
use crate::change::EntityId;
use crate::hlc::NodeId;
use crate::model::QuestColor;

#[test]
fn later_commands_see_what_earlier_ones_created() {
    let quest = EntityId::new();
    let commands = [
        Command::CreateQuest { id: quest, title: "Launch".into(), color: QuestColor::Blue },
        Command::RenameQuest { id: quest, title: "Notes".into() },
    ];
    let batch =
        plan_all(&commands, &FieldStore::default(), EntityId::DEFAULT_WORKSPACE, &HlcClock::new(NodeId(1)), 1_000)
            .unwrap();
    let snapshot = Snapshot::from_store(&batch.store, EntityId::DEFAULT_WORKSPACE);
    assert_eq!(snapshot.quest(quest).map(|q| q.title.as_str()), Some("Notes"));
    assert!(batch.changes.windows(2).all(|pair| pair[0].hlc < pair[1].hlc));
}

#[test]
fn one_refused_command_plans_nothing() {
    let commands = [
        Command::CreateQuest { id: EntityId::new(), title: "Launch".into(), color: QuestColor::Blue },
        Command::RenameQuest { id: EntityId::new(), title: "Unknown".into() },
    ];
    let refused =
        plan_all(&commands, &FieldStore::default(), EntityId::DEFAULT_WORKSPACE, &HlcClock::new(NodeId(1)), 1_000);
    assert!(matches!(refused, Err(CommandError::UnknownQuest(_))));
}
