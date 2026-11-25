//! `QuestCore`'s workspaces: the switcher, "Manage workspaces" and what a switch resets.
//!
//! As on the desktop, a switch (by the user, or because another device deleted the shown
//! workspace) drops the core's memory of the old workspace: the undo snackbar, the AI capture's
//! undo, the plan's previews and the tick memory. The app closes its sheets and pages that hold
//! the old workspace's data and clears its filters (`docs/mobile.md`, Workspaces). The focus timer
//! is one per person and keeps running.

use std::sync::Arc;

use quest_domain::{Command, EntityId, Snapshot, Task};

use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::ids::parse_id;
use crate::time::Now;
use crate::types::Moment;
use crate::workspaces::{self, WorkspacesData};

#[uniffi::export]
impl QuestCore {
    /// The switcher and "Manage workspaces". Also notices when the shown workspace changed without
    /// a switch here (another device deleted it) and forgets what belonged to the old one.
    pub fn workspaces(&self, now: Moment) -> Result<WorkspacesData, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let snapshot = self.store().snapshot();
            self.follow_workspace(snapshot.workspace);
            Ok(workspaces::workspaces(&snapshot, now))
        })
    }

    /// Shows `workspace_id` from now on (remembered for the next start on this device). Nothing
    /// happens if it is the shown one; a deleted or unknown one is refused.
    pub fn switch_workspace(&self, workspace_id: String) -> Result<(), MobileError> {
        guard(|| {
            let id = parse_id(&workspace_id)?;
            self.switch_to(id)
        })
    }

    /// Creates a workspace from "New workspace" and shows it. The name is trimmed, at most 60
    /// characters and unique among the live workspaces (`InvalidForm` says what to fix). Returns its id.
    pub fn create_workspace(&self, name: String) -> Result<String, MobileError> {
        guard(|| {
            let name = workspaces::name(&self.store().snapshot(), &name, None)?;
            let id = EntityId::new();
            self.dispatch(Command::CreateWorkspace { id, name })?;
            self.switch_to(id)?;
            Ok(id.to_string())
        })
    }

    /// Saves a live workspace's new name; an unchanged one saves nothing.
    pub fn rename_workspace(&self, workspace_id: String, name: String) -> Result<(), MobileError> {
        guard(|| {
            let id = parse_id(&workspace_id)?;
            let snapshot = self.store().snapshot();
            let current = snapshot.live_workspace(id).ok_or_else(|| workspaces::unknown(id))?;
            let name = workspaces::name(&snapshot, &name, Some(id))?;
            if name == current.name {
                return Ok(());
            }
            self.dispatch(Command::RenameWorkspace { id, name })
        })
    }

    /// What the delete confirmation says ("Delete “Work”? It holds 1 quest, …"), or why the
    /// workspace can't be deleted (the last one stays).
    pub fn delete_workspace_question(&self, workspace_id: String) -> Result<String, MobileError> {
        guard(|| {
            let id = parse_id(&workspace_id)?;
            let snapshot = self.store().snapshot();
            snapshot.check_deletable(id).map_err(|e| MobileError::Rejected { reason: e.to_string() })?;
            let workspace = snapshot.live_workspace(id).ok_or_else(|| workspaces::unknown(id))?;
            Ok(quest_view::workspaces::delete_question(workspace))
        })
    }

    /// Deletes a workspace (restorable under "Deleted"); its running timer stops. Deleting the
    /// shown one shows a remaining one (the default workspace while it is live).
    pub fn delete_workspace(&self, workspace_id: String, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let (id, now) = (parse_id(&workspace_id)?, Now::parse(&now)?);
            self.follow_workspace(self.store().snapshot().workspace);
            self.dispatch(Command::DeleteWorkspace { id, at: now.at })?;
            self.follow_workspace(self.store().snapshot().workspace);
            Ok(())
        })
    }

    /// Restores a deleted workspace, unless a live one took its name meanwhile (`InvalidForm`).
    pub fn restore_workspace(&self, workspace_id: String) -> Result<(), MobileError> {
        guard(|| {
            let id = parse_id(&workspace_id)?;
            workspaces::check_restorable(&self.store().snapshot(), id)?;
            self.dispatch(Command::RestoreWorkspace { id })
        })
    }
}

impl QuestCore {
    /// A task as its own workspace holds it, the shown one first: the focus notification acts on
    /// the timer's task, which may be in another workspace than the shown one.
    pub(crate) fn task_anywhere(&self, id: EntityId) -> Option<(Arc<Snapshot>, Task)> {
        let store = self.store();
        let shown = store.snapshot();
        if let Some(task) = shown.task(id).cloned() {
            return Some((shown, task));
        }
        let others: Vec<EntityId> = shown.live_workspaces().map(|w| w.id).filter(|&w| w != shown.workspace).collect();
        others.into_iter().filter_map(|w| store.snapshot_in(w)).find_map(|there| {
            let task = there.task(id).cloned()?;
            Some((there, task))
        })
    }

    /// Dispatches a command on a task of `snapshot`'s workspace, shown or not.
    pub(crate) fn dispatch_there(&self, snapshot: &Snapshot, command: Command) -> Result<(), MobileError> {
        if snapshot.workspace == self.store().snapshot().workspace {
            return self.dispatch(command);
        }
        self.edit(|store| {
            store.dispatch_in(snapshot.workspace, command).map_err(|e| MobileError::Rejected { reason: e.to_string() })
        })
    }

    fn switch_to(&self, id: EntityId) -> Result<(), MobileError> {
        let store = self.store();
        let before = store.snapshot().workspace;
        if before == id {
            return Ok(());
        }
        self.follow_workspace(before);
        self.edit(|store| store.switch_workspace(id).map_err(|e| MobileError::Rejected { reason: e.to_string() }))?;
        self.follow_workspace(store.snapshot().workspace);
        self.listener.on_change();
        Ok(())
    }
}

#[cfg(test)]
#[path = "core_workspaces_tests.rs"]
mod tests;
