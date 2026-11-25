//! Which workspace a record belongs to, and when a workspace may be deleted. Pure.

use crate::change::{EntityId, EntityKind, FieldStore, FieldValues};
use crate::command::CommandError;
use std::collections::HashMap;

use crate::model::{Workspace, fields, get, get_opt, workspace_id};
use crate::recurrence::Occurrence;
use crate::snapshot::Snapshot;

/// Parent links are at most session → task → series → quest deep; the limit also stops cycles
/// in corrupt data.
const MAX_DEPTH: u8 = 4;

/// The workspace a record belongs to: its own `workspace_id`, else its parent's (an occurrence's
/// series, a task's quest, a session's task or quest, a milestone's quest), else the default
/// workspace. Records from before workspaces and from older clients carry no workspace, so they
/// land with their parent, and parentless ones in the default workspace, on every device alike.
pub(crate) fn workspace_of(store: &FieldStore, kind: EntityKind, id: EntityId) -> EntityId {
    placed(store, kind, id, MAX_DEPTH).unwrap_or(EntityId::DEFAULT_WORKSPACE)
}

/// Where every quest, task, milestone and session is, worked out once per snapshot.
pub(crate) struct Placements(HashMap<(EntityKind, EntityId), EntityId>);

impl Placements {
    pub(crate) fn new(store: &FieldStore) -> Self {
        let placed = [EntityKind::Quest, EntityKind::Task, EntityKind::Milestone, EntityKind::TimeEntry]
            .into_iter()
            .flat_map(|kind| store.entities(kind).map(move |(id, _)| ((kind, id), workspace_of(store, kind, id))));
        Placements(placed.collect())
    }

    pub(crate) fn of(&self, kind: EntityKind, id: EntityId) -> EntityId {
        self.0.get(&(kind, id)).copied().unwrap_or(EntityId::DEFAULT_WORKSPACE)
    }
}

/// Every workspace, without content: the default one first, then by creation (ids are UUIDv7).
/// If devices deleted every workspace between them while offline, the default one counts as not
/// deleted, so there is always one to show (the same on every device); the second value says so.
pub(crate) fn workspace_records(store: &FieldStore) -> (Vec<Workspace>, bool) {
    let default = EntityId::DEFAULT_WORKSPACE;
    let others = store
        .entities(EntityKind::Workspace)
        .filter(|(id, _)| *id != default)
        .filter_map(|(id, values)| Workspace::from_fields(id, Some(values)));
    let mut workspaces: Vec<Workspace> =
        Workspace::from_fields(default, store.get(EntityKind::Workspace, default)).into_iter().chain(others).collect();
    let revived = workspaces.iter().all(Workspace::is_deleted);
    if revived {
        workspaces[0].deleted_at = None;
    }
    (workspaces, revived)
}

/// Whether `id` names a workspace that exists and is not deleted.
pub(crate) fn is_live(store: &FieldStore, id: EntityId) -> bool {
    workspace_records(store).0.iter().any(|w| w.id == id && !w.is_deleted())
}

fn placed(store: &FieldStore, kind: EntityKind, id: EntityId, depth: u8) -> Option<EntityId> {
    let values = store.get(kind, id)?;
    if let Some(workspace) = workspace_id(values) {
        return Some(workspace);
    }
    let depth = depth.checked_sub(1)?;
    parents(kind, values).into_iter().flatten().find_map(|(kind, id)| placed(store, kind, id, depth))
}

fn parents(kind: EntityKind, values: &FieldValues) -> [Option<(EntityKind, EntityId)>; 2] {
    let quest = |id: Option<EntityId>| id.map(|id| (EntityKind::Quest, id));
    match kind {
        EntityKind::Task => {
            let initial: Option<Occurrence> = get(values, fields::TASK_OCCURRENCE);
            let own_quest = get::<Option<EntityId>>(values, fields::QUEST_ID);
            let quest_id = own_quest.unwrap_or_else(|| initial.as_ref().and_then(|o| o.quest_id));
            [initial.map(|o| (EntityKind::Task, o.series)), quest(quest_id)]
        }
        EntityKind::TimeEntry => {
            let task = get_opt(values, fields::ENTRY_TASK_ID).map(|id| (EntityKind::Task, id));
            [task, quest(get_opt(values, fields::QUEST_ID))]
        }
        EntityKind::Milestone => [quest(get(values, fields::QUEST_ID)), None],
        EntityKind::Quest | EntityKind::Settings | EntityKind::Workspace => [None, None],
    }
}

impl Snapshot {
    /// Whether `id` names a workspace in `store` that exists and is not deleted, without building
    /// a snapshot.
    pub fn is_live_workspace(store: &FieldStore, id: EntityId) -> bool {
        is_live(store, id)
    }

    /// Whether the workspace may be deleted: any live one, the default one included, as long as
    /// another live one remains.
    pub fn check_deletable(&self, id: EntityId) -> Result<(), CommandError> {
        self.live_workspace(id).ok_or(CommandError::UnknownWorkspace(id))?;
        if self.live_workspaces().count() < 2 {
            return Err(CommandError::CannotDeleteLastWorkspace);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
