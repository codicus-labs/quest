//! Which guest data moves into an account when the user accepts the import. Pure.

use quest_domain::{EntityId, EntityKind, FieldChange, FieldStore};

use crate::shared::ImportedField;

/// Every guest field the account does not have yet. Ids are random, so this is all of the
/// guest's workspaces, quests, tasks and time; guest records in the default workspace land in the
/// account's default workspace, which has the same id. Only the default workspace's own records
/// (its name and settings) can overlap, and there the account's values win: they may already be
/// in use on other devices. So those only come along if the account is known to be caught up
/// (`account_caught_up`); otherwise a value the account has on the server but not yet here would
/// be overwritten.
pub(crate) fn plan(guest: &FieldStore, account: &FieldStore, account_caught_up: bool) -> Vec<ImportedField> {
    let shared_by_both = |kind: EntityKind, id: EntityId| {
        matches!(kind, EntityKind::Settings | EntityKind::Workspace) && id == EntityId::DEFAULT_WORKSPACE
    };
    // Workspaces first: an import may reach other devices in several pushes, and records in a
    // workspace those devices do not know yet would be hidden until it arrives.
    let kinds = std::iter::once(EntityKind::Workspace)
        .chain(EntityKind::ALL.into_iter().filter(|k| *k != EntityKind::Workspace));
    let mut writes = Vec::new();
    for kind in kinds {
        for (id, fields) in guest.entities(kind) {
            if shared_by_both(kind, id) && !account_caught_up {
                continue;
            }
            let existing = account.get(kind, id);
            for (field, (value, _)) in fields {
                if existing.is_some_and(|existing| existing.contains_key(field)) {
                    continue;
                }
                writes.push(ImportedField { kind, id, field: field.clone(), value: value.clone() });
            }
        }
    }
    writes
}

/// How many entities `changes` touch, for telling the user.
pub(crate) fn entity_count(changes: &[FieldChange]) -> usize {
    let mut entities: Vec<_> = changes.iter().map(|change| (change.kind, change.id)).collect();
    entities.sort();
    entities.dedup();
    entities.len()
}

#[cfg(test)]
mod tests {
    use quest_domain::{Hlc, NodeId};
    use serde_json::{Value, json};

    use super::*;

    fn store(fields: &[(EntityKind, EntityId, &str, Value)]) -> FieldStore {
        let mut store = FieldStore::default();
        for (kind, id, field, value) in fields {
            store.apply(&FieldChange {
                kind: *kind,
                id: *id,
                field: (*field).into(),
                value: value.clone(),
                hlc: Hlc { wall_ms: 1, counter: 0, node: NodeId(7) },
            });
        }
        store
    }

    #[test]
    fn all_guest_entities_move_and_account_settings_win() {
        let task = EntityId::new();
        let guest = store(&[
            (EntityKind::Task, task, "title", json!("Guest task")),
            (EntityKind::Task, task, "day", json!("2026-11-24")),
            (EntityKind::Settings, EntityId::SETTINGS, "weekly_target_minutes", json!(60)),
            (EntityKind::Settings, EntityId::SETTINGS, "other", json!(1)),
        ]);
        let account = store(&[(EntityKind::Settings, EntityId::SETTINGS, "weekly_target_minutes", json!(90))]);

        let fields = |writes: Vec<ImportedField>| -> Vec<(String, Value)> {
            writes.into_iter().map(|w| (w.field, w.value)).collect()
        };
        let task_fields = [("day".to_owned(), json!("2026-11-24")), ("title".to_owned(), json!("Guest task"))];

        let caught_up = fields(plan(&guest, &account, true));
        assert_eq!(caught_up[..2], task_fields);
        assert_eq!(caught_up[2..], [("other".to_owned(), json!(1))]);
        assert_eq!(fields(plan(&guest, &account, false)), task_fields, "settings only if caught up");
    }

    #[test]
    fn workspaces_come_first_so_their_records_are_never_shown_without_them() {
        let (workspace, task) = (EntityId::new(), EntityId::new());
        let guest = store(&[
            (EntityKind::Task, task, "workspace_id", json!(workspace)),
            (EntityKind::Workspace, workspace, "name", json!("Side")),
        ]);
        let kinds: Vec<EntityKind> = plan(&guest, &FieldStore::default(), true).iter().map(|w| w.kind).collect();
        assert_eq!(kinds, [EntityKind::Workspace, EntityKind::Task]);
    }

    #[test]
    fn entities_are_counted_once() {
        let task = EntityId::new();
        let change = |id, field: &str| FieldChange {
            kind: EntityKind::Task,
            id,
            field: field.into(),
            value: json!(1),
            hlc: Hlc { wall_ms: 1, counter: 0, node: NodeId(7) },
        };
        assert_eq!(entity_count(&[change(task, "a"), change(task, "b"), change(EntityId::new(), "a")]), 2);
    }

    #[test]
    fn an_empty_guest_imports_nothing() {
        assert!(plan(&FieldStore::default(), &FieldStore::default(), true).is_empty());
    }
}
