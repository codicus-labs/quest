use quest_domain::{EntityId, Hlc, Position};
use quest_protocol::MAX_CLOCK_DRIFT_MS;
use serde_json::json;

use super::*;

const NOW: u64 = 1_000_000;

struct FixedClock;

impl WallClock for FixedClock {
    fn now_ms(&self) -> u64 {
        NOW
    }
}

fn loaded(fields: FieldStore) -> Loaded {
    Loaded {
        node: NodeId(1),
        workspace: EntityId::DEFAULT_WORKSPACE,
        account_info: None,
        fields,
        pending: 0,
        rejected: 0,
    }
}

fn far_future_target(minutes: u32) -> FieldChange {
    FieldChange {
        kind: EntityKind::Settings,
        id: EntityId::SETTINGS,
        field: quest_domain::fields::SETTINGS_WEEKLY_TARGET_MINUTES.into(),
        value: json!(minutes),
        hlc: Hlc { wall_ms: NOW + MAX_CLOCK_DRIFT_MS + 1, counter: 0, node: NodeId(2) },
    }
}

#[test]
fn far_future_remote_changes_are_merged_but_do_not_move_the_clock() {
    let shared = Shared::new(&loaded(FieldStore::default()), None, Box::new(FixedClock), Box::new(|| {}));
    assert!(shared.apply_remote(&[far_future_target(5)]));
    shared.publish_latest();
    assert_eq!(shared.snapshot().settings.weekly_target_minutes, 5);

    let local = shared.apply_local(&Command::SetWeeklyTarget { minutes: 10 }).unwrap();
    assert_eq!(local[0].hlc.wall_ms, NOW);
}

#[test]
fn a_stored_far_future_timestamp_does_not_poison_the_clock_after_restart() {
    let mut fields = FieldStore::default();
    fields.apply(&far_future_target(5));
    let shared = Shared::new(&loaded(fields), None, Box::new(FixedClock), Box::new(|| {}));
    let local = shared.apply_local(&Command::SetWeeklyTarget { minutes: 10 }).unwrap();
    assert_eq!(local[0].hlc.wall_ms, NOW);
}

#[test]
fn an_older_snapshot_never_replaces_a_newer_one() {
    let shared = Shared::new(&loaded(FieldStore::default()), None, Box::new(FixedClock), Box::new(|| {}));
    shared.apply_local(&Command::SetWeeklyTarget { minutes: 10 }).unwrap();
    shared.publish(0, &FieldStore::default(), EntityId::DEFAULT_WORKSPACE);
    assert_eq!(shared.snapshot().settings.weekly_target_minutes, 10);
}

#[test]
fn remote_changes_become_visible_only_when_published() {
    let shared = Shared::new(&loaded(FieldStore::default()), None, Box::new(FixedClock), Box::new(|| {}));
    let mut change = far_future_target(7);
    change.hlc.wall_ms = NOW;
    shared.apply_remote(&[change]);
    assert_ne!(shared.snapshot().settings.weekly_target_minutes, 7);
    shared.publish_latest();
    assert_eq!(shared.snapshot().settings.weekly_target_minutes, 7);
}

#[test]
fn several_commands_apply_as_one_unit() {
    use quest_domain::{Position, QuestColor};
    let shared = Shared::new(&loaded(FieldStore::default()), None, Box::new(FixedClock), Box::new(|| {}));
    let (quest, task) = (EntityId::new(), EntityId::new());
    let create_quest = Command::CreateQuest { id: quest, title: "New".into(), color: QuestColor::Red };
    // Valid only after the quest before it.
    let create_task = Command::CreateTask {
        id: task,
        title: "T".into(),
        quest_id: Some(quest),
        planned_on: None,
        due_on: None,
        position: Position::Bottom,
    };
    let refused = Command::RenameTask { id: EntityId::new(), title: "missing".into() };

    let error = shared.apply_local_all(&[create_quest.clone(), create_task.clone(), refused]).unwrap_err();
    assert!(matches!(error, CommandError::UnknownTask(_)));
    assert!(shared.snapshot().quests.is_empty() && shared.snapshot().tasks.is_empty(), "nothing applied");

    let changes = shared.apply_local_all(&[create_quest, create_task]).unwrap();
    assert_eq!(shared.snapshot().task(task).unwrap().quest_id, Some(quest));
    let mut clocks: Vec<Hlc> = changes.iter().map(|c| c.hlc).collect();
    clocks.dedup();
    assert_eq!(clocks.len(), changes.len(), "every change gets its own timestamp");
    let next = shared.apply_local(&Command::SetWeeklyTarget { minutes: 10 }).unwrap();
    assert!(next[0].hlc > *clocks.last().unwrap(), "the clock moved on");
}

#[test]
fn an_import_skips_fields_the_account_got_after_planning() {
    let shared = Shared::new(&loaded(FieldStore::default()), None, Box::new(FixedClock), Box::new(|| {}));
    let target = |minutes: u32| ImportedField {
        kind: EntityKind::Settings,
        id: EntityId::SETTINGS,
        field: quest_domain::fields::SETTINGS_WEEKLY_TARGET_MINUTES.into(),
        value: json!(minutes),
    };
    // The server's value arrives between planning and applying the import.
    assert!(shared.apply_remote(&[far_future_target(90)]));

    assert!(shared.apply_imported(vec![target(60)]).is_empty());
    shared.publish_latest();
    assert_eq!(shared.snapshot().settings.weekly_target_minutes, 90, "the account wins");
}

#[test]
fn commands_and_snapshots_follow_the_active_workspace() {
    let shared = Shared::new(&loaded(FieldStore::default()), None, Box::new(FixedClock), Box::new(|| {}));
    let side = EntityId::new();
    shared.apply_local(&Command::CreateWorkspace { id: side, name: "Side".into() }).unwrap();
    assert_eq!(shared.switch_workspace(EntityId::new()).map_err(|_| ()), Err(()), "unknown workspace");
    shared.switch_workspace(side).unwrap();
    let task = EntityId::new();
    let create = |id| Command::CreateTask {
        id,
        title: "t".into(),
        quest_id: None,
        planned_on: None,
        due_on: None,
        position: Position::Bottom,
    };
    shared.apply_local_all(&[create(task)]).unwrap();
    shared.apply_local(&Command::SetWeeklyTarget { minutes: 90 }).unwrap();
    assert_eq!((shared.snapshot().workspace, shared.snapshot().tasks.len()), (side, 1));

    shared.switch_workspace(EntityId::DEFAULT_WORKSPACE).unwrap();
    let snapshot = shared.snapshot();
    assert!(snapshot.tasks.is_empty());
    assert_eq!(snapshot.settings.weekly_target_minutes, 20 * 60);
}

#[test]
fn a_workspace_deleted_on_another_device_falls_back_to_the_default_and_stays_there() {
    let shared = Shared::new(&loaded(FieldStore::default()), None, Box::new(FixedClock), Box::new(|| {}));
    let side = EntityId::new();
    let created = shared.apply_local(&Command::CreateWorkspace { id: side, name: "Side".into() }).unwrap();
    shared.switch_workspace(side).unwrap();
    let remote = |value| FieldChange {
        kind: EntityKind::Workspace,
        id: side,
        field: quest_domain::fields::DELETED_AT.into(),
        value,
        hlc: Hlc { wall_ms: created[0].hlc.wall_ms + 1, counter: 0, node: NodeId(2) },
    };
    shared.apply_remote(&[remote(json!("2026-11-24T12:00:00Z"))]);
    shared.publish_latest();
    assert_eq!(shared.snapshot().workspace, EntityId::DEFAULT_WORKSPACE);
    assert_eq!(shared.take_fallback(), Some(EntityId::DEFAULT_WORKSPACE), "remembered for the next start");
    assert_eq!(shared.take_fallback(), None);

    shared.apply_remote(&[FieldChange {
        hlc: Hlc { wall_ms: created[0].hlc.wall_ms + 2, ..remote(json!(null)).hlc },
        ..remote(json!(null))
    }]);
    shared.publish_latest();
    assert_eq!(shared.snapshot().workspace, EntityId::DEFAULT_WORKSPACE, "restoring it does not move the user back");
    assert!(shared.snapshot().live_workspace(side).is_some());
}
