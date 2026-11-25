use quest_domain::{EntityKind, Hlc};
use quest_protocol::{Cursor, Epoch};
use serde_json::{Value, json};

use super::*;

fn change(value: Value, wall_ms: u64) -> FieldChange {
    FieldChange {
        kind: EntityKind::Task,
        id: EntityId::SETTINGS,
        field: "title".into(),
        value,
        hlc: Hlc { wall_ms, counter: 0, node: NodeId(1) },
    }
}

fn title(fields: &FieldStore) -> Value {
    fields.get(EntityKind::Task, EntityId::SETTINGS).unwrap()["title"].0.clone()
}

#[test]
fn local_changes_survive_reopen_and_wait_in_the_outbox() {
    let dir = tempfile::tempdir().unwrap();
    let node = {
        let mut db = Db::open(dir.path()).unwrap();
        db.persist_local(&[change(json!("a"), 1), change(json!("b"), 2)], true).unwrap();
        db.load().unwrap().node
    };
    let mut db = Db::open(dir.path()).unwrap();
    let loaded = db.load().unwrap();
    assert_eq!(loaded.node, node);
    assert_eq!(loaded.pending, 2);
    assert_eq!(title(&loaded.fields), json!("b"));
}

#[test]
fn an_older_change_never_overwrites_a_newer_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Db::open(dir.path()).unwrap();
    db.persist_local(&[change(json!("new"), 5)], true).unwrap();
    db.apply_remote(&[change(json!("old"), 3)], Cursor(7)).unwrap();
    let loaded = db.load().unwrap();
    assert_eq!(title(&loaded.fields), json!("new"));
    assert_eq!(db.sync_position().unwrap().0, Cursor(7));
}

#[test]
fn acknowledged_batches_leave_the_outbox_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Db::open(dir.path()).unwrap();
    db.persist_local(&[change(json!("a"), 1), change(json!("b"), 2), change(json!("c"), 3)], true).unwrap();
    let first: Vec<i64> = db.outbox_batch(2).unwrap().into_iter().map(|(seq, _)| seq).collect();
    db.remove_outbox(&first).unwrap();
    let rest = db.outbox_batch(10).unwrap();
    assert_eq!(rest.len(), 1);
    assert_eq!(rest[0].1.value, json!("c"));
}

fn account_info() -> AccountInfo {
    AccountInfo {
        user_id: quest_protocol::UserId(uuid::Uuid::from_u128(1)),
        issuer: "https://id.example".into(),
        subject: "anna".into(),
        email: Some("anna@example.com".into()),
        name: None,
        ai_enabled: false,
        ai_available: true,
    }
}

fn epoch(n: u128) -> Epoch {
    Epoch(uuid::Uuid::from_u128(n))
}

#[test]
fn the_account_info_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    Db::open(dir.path()).unwrap().set_account_info(&account_info()).unwrap();
    assert_eq!(Db::open(dir.path()).unwrap().load().unwrap().account_info, Some(account_info()));
}

#[test]
fn the_guest_queues_nothing_for_the_server() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Db::open(dir.path()).unwrap();
    db.persist_local(&[change(json!("a"), 1)], false).unwrap();
    let loaded = db.load().unwrap();
    assert_eq!(loaded.pending, 0);
    assert_eq!(title(&loaded.fields), json!("a"));
}

#[test]
fn equal_timestamps_pick_the_same_winner_in_any_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut forward = Db::open(&dir.path().join("f")).unwrap();
    let mut backward = Db::open(&dir.path().join("b")).unwrap();
    forward.apply_remote(&[change(json!("apple"), 1)], Cursor(1)).unwrap();
    forward.apply_remote(&[change(json!("banana"), 1)], Cursor(2)).unwrap();
    backward.apply_remote(&[change(json!("banana"), 1)], Cursor(1)).unwrap();
    backward.apply_remote(&[change(json!("apple"), 1)], Cursor(2)).unwrap();
    assert_eq!(title(&forward.load().unwrap().fields), json!("banana"));
    assert_eq!(title(&backward.load().unwrap().fields), json!("banana"));
}

#[test]
fn the_cursor_never_moves_backwards_while_syncing() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Db::open(dir.path()).unwrap();
    db.apply_remote(&[], Cursor(9)).unwrap();
    db.apply_remote(&[], Cursor(4)).unwrap();
    assert_eq!(db.sync_position().unwrap().0, Cursor(9));
}

#[test]
fn a_cursor_reset_requeues_this_nodes_fields_only() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Db::open(dir.path()).unwrap();
    let own = change(json!("mine"), 5);
    let mut foreign = change(json!("theirs"), 6);
    foreign.field = "notes".into();
    foreign.hlc.node = NodeId(2);
    db.apply_remote(&[own.clone(), foreign], Cursor(9)).unwrap();
    assert_eq!(db.welcome(epoch(2), true, NodeId(1)).unwrap(), 1);
    assert_eq!(db.sync_position().unwrap(), (Cursor(0), Some(epoch(2))));
    let queued: Vec<FieldChange> = db.outbox_batch(10).unwrap().into_iter().map(|(_, c)| c).collect();
    assert_eq!(queued, [own]);
}

#[test]
fn rejected_rows_leave_the_outbox_but_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Db::open(dir.path()).unwrap();
    db.persist_local(&[change(json!("a"), 1), change(json!("b"), 2)], true).unwrap();
    let first = db.outbox_batch(1).unwrap()[0].0;
    db.reject_outbox(&[first], "too large").unwrap();
    let loaded = db.load().unwrap();
    assert_eq!((loaded.pending, loaded.rejected), (1, 1));
}

#[test]
fn resetting_local_data_keeps_only_the_node_id() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Db::open(dir.path()).unwrap();
    let node = db.load().unwrap().node;
    db.persist_local(&[change(json!("a"), 1)], true).unwrap();
    db.set_account_info(&account_info()).unwrap();
    db.reset_local_data().unwrap();
    let loaded = db.load().unwrap();
    assert_eq!(loaded.node, node);
    assert_eq!((loaded.pending, loaded.account_info), (0, None));
    assert!(loaded.fields.highest_hlc().is_none());
}

#[test]
fn the_chosen_workspace_is_remembered_until_the_local_data_is_reset() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Db::open(dir.path()).unwrap();
    assert_eq!(db.load().unwrap().workspace, EntityId::DEFAULT_WORKSPACE);
    let side = EntityId::new();
    db.set_workspace(side).unwrap();
    assert_eq!(Db::open(dir.path()).unwrap().load().unwrap().workspace, side);
    db.reset_local_data().unwrap();
    assert_eq!(db.load().unwrap().workspace, EntityId::DEFAULT_WORKSPACE);
}

#[test]
fn a_store_of_the_previous_model_opens_empty_and_keeps_its_profile() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Db::open(dir.path()).unwrap();
    let node = db.load().unwrap().node;
    db.persist_local(&[change(json!("a"), 1)], true).unwrap();
    db.welcome(epoch(1), false, node).unwrap();
    db.apply_remote(&[], Cursor(5)).unwrap();
    db.set_workspace(EntityId::new()).unwrap();
    let info = account_info();
    db.set_account_info(&info).unwrap();
    let db = Db::open(dir.path()).unwrap();
    assert_eq!(db.sync_position().unwrap(), (Cursor(5), Some(epoch(1))), "the current model keeps its data");

    // A store written by the previous model, with the fields of its kinds.
    db.conn.execute("DELETE FROM meta WHERE key = ?1", [MODEL_KEY]).unwrap();
    db.conn
        .execute(
            "INSERT INTO fields VALUES ('task', ?1, 'day', '\"2026-11-23\"', ?2)",
            [EntityId::new().to_string(), "0".repeat(32)],
        )
        .unwrap();
    let mut db = Db::open(dir.path()).unwrap();
    let loaded = db.load().unwrap();
    assert_eq!((loaded.node, loaded.account_info), (node, Some(info)));
    assert_eq!((loaded.fields.highest_hlc(), loaded.pending, loaded.workspace), (None, 0, EntityId::DEFAULT_WORKSPACE));
    assert_eq!(db.sync_position().unwrap(), (Cursor(0), None));
}
