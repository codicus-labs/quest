use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::hlc::Hlc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Quest,
    Task,
    ChecklistItem,
    Postponement,
    TimeEntry,
    Settings,
    Workspace,
}

impl EntityKind {
    pub const ALL: [EntityKind; 7] = [
        EntityKind::Quest,
        EntityKind::Task,
        EntityKind::ChecklistItem,
        EntityKind::Postponement,
        EntityKind::TimeEntry,
        EntityKind::Settings,
        EntityKind::Workspace,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            EntityKind::Quest => "quest",
            EntityKind::Task => "task",
            EntityKind::ChecklistItem => "checklist_item",
            EntityKind::Postponement => "postponement",
            EntityKind::TimeEntry => "time_entry",
            EntityKind::Settings => "settings",
            EntityKind::Workspace => "workspace",
        }
    }
}

impl FromStr for EntityKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        EntityKind::ALL.into_iter().find(|kind| kind.as_str() == s).ok_or_else(|| format!("unknown entity kind {s:?}"))
    }
}

/// UUIDv7 for new records; UUIDv5 for records several replicas derive alike (a repeat's
/// occurrences and their checklists, postponements).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(pub Uuid);

impl EntityId {
    /// The default workspace's settings record. Every workspace keeps its settings under its own id.
    pub const SETTINGS: EntityId = EntityId::DEFAULT_WORKSPACE;

    /// The workspace every account and guest profile has, with the same id everywhere, so devices
    /// agree on it without syncing first. Records without a workspace and without a parent
    /// belong to it.
    pub const DEFAULT_WORKSPACE: EntityId = EntityId(Uuid::nil());

    pub fn new() -> Self {
        EntityId(Uuid::now_v7())
    }
}

impl Default for EntityId {
    fn default() -> Self {
        EntityId::new()
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for EntityId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(EntityId)
    }
}

/// A user's intent to set one field, before a clock stamps it.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldWrite {
    pub kind: EntityKind,
    pub id: EntityId,
    pub field: &'static str,
    pub value: Value,
}

impl FieldWrite {
    pub fn stamp(self, hlc: Hlc) -> FieldChange {
        FieldChange { kind: self.kind, id: self.id, field: self.field.to_owned(), value: self.value, hlc }
    }
}

/// The unit of sync: one field of one entity set to a value at a point in hybrid-logical time.
/// Replicas converge by keeping, per field, the change with the highest `hlc` (last writer wins).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldChange {
    pub kind: EntityKind,
    pub id: EntityId,
    pub field: String,
    pub value: Value,
    pub hlc: Hlc,
}

impl FieldChange {
    /// True if this change should replace a field currently holding `current` (value, clock).
    /// Equal clocks (only possible when a database was copied to a second device) are broken by
    /// the serialised value, so every replica still picks the same winner.
    pub fn supersedes(&self, current: Option<(&Value, Hlc)>) -> bool {
        let Some((value, hlc)) = current else { return true };
        match self.hlc.cmp(&hlc) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => serialized(&self.value) > serialized(value),
        }
    }
}

fn serialized(value: &Value) -> String {
    value.to_string()
}

pub type FieldValues = BTreeMap<String, (Value, Hlc)>;

/// In-memory last-writer-wins register map: the merged state of every field of every entity.
#[derive(Clone, Debug, Default)]
pub struct FieldStore {
    entities: BTreeMap<(EntityKind, EntityId), FieldValues>,
}

impl FieldStore {
    /// Merges one change. Returns true if it won and changed the state.
    pub fn apply(&mut self, change: &FieldChange) -> bool {
        let fields = self.entities.entry((change.kind, change.id)).or_default();
        let current = fields.get(&change.field).map(|(value, hlc)| (value, *hlc));
        if !change.supersedes(current) {
            return false;
        }
        fields.insert(change.field.clone(), (change.value.clone(), change.hlc));
        true
    }

    pub fn entities(&self, kind: EntityKind) -> impl Iterator<Item = (EntityId, &FieldValues)> {
        self.entities.iter().filter(move |((k, _), _)| *k == kind).map(|((_, id), fields)| (*id, fields))
    }

    pub fn get(&self, kind: EntityKind, id: EntityId) -> Option<&FieldValues> {
        self.entities.get(&(kind, id))
    }

    /// The highest timestamp in the store, to resume a clock after loading.
    pub fn highest_hlc(&self) -> Option<Hlc> {
        self.entities.values().flat_map(|fields| fields.values().map(|(_, hlc)| *hlc)).max()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::hlc::NodeId;

    fn change(value: Value, wall_ms: u64, node: u64) -> FieldChange {
        FieldChange {
            kind: EntityKind::Task,
            id: EntityId::SETTINGS,
            field: "title".into(),
            value,
            hlc: Hlc { wall_ms, counter: 0, node: NodeId(node) },
        }
    }

    #[test]
    fn replicas_converge_regardless_of_delivery_order() {
        let changes = [change(json!("a"), 1, 1), change(json!("b"), 3, 2), change(json!("c"), 2, 3)];
        let mut forward = FieldStore::default();
        let mut backward = FieldStore::default();
        changes.iter().for_each(|c| _ = forward.apply(c));
        changes.iter().rev().for_each(|c| _ = backward.apply(c));
        let value = |store: &FieldStore| store.get(EntityKind::Task, EntityId::SETTINGS).unwrap()["title"].0.clone();
        assert_eq!(value(&forward), json!("b"));
        assert_eq!(value(&backward), json!("b"));
    }

    #[test]
    fn equal_clocks_pick_the_same_winner_in_any_order() {
        let (a, b) = (change(json!("a"), 1, 1), change(json!("b"), 1, 1));
        let mut ab = FieldStore::default();
        let mut ba = FieldStore::default();
        _ = (ab.apply(&a), ab.apply(&b), ba.apply(&b), ba.apply(&a));
        assert_eq!(ab.get(EntityKind::Task, EntityId::SETTINGS), ba.get(EntityKind::Task, EntityId::SETTINGS));
    }

    #[test]
    fn reapplying_a_change_is_a_no_op() {
        let mut store = FieldStore::default();
        let c = change(json!("a"), 1, 1);
        assert!(store.apply(&c));
        assert!(!store.apply(&c));
    }
}
