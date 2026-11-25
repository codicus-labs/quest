//! Workspaces as the switcher and the "Manage workspaces" sheet show them, and their checks.
//! The words and rules are the desktop's (`quest_view::workspaces`); this only shapes records.

use quest_domain::{EntityId, Snapshot, Workspace};
use quest_view::format;
use quest_view::workspaces::{check_name, content_summary, timer_elsewhere};

use crate::error::MobileError;
use crate::time::Now;

/// The switcher and "Manage workspaces".
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct WorkspacesData {
    /// The shown workspace: the switcher's label.
    pub active: WorkspaceRow,
    /// The live workspaces in the switcher's order; the shown one has `current`.
    pub live: Vec<WorkspaceRow>,
    /// Deleted workspaces, to restore ("Deleted" in the sheet).
    pub deleted: Vec<WorkspaceRow>,
    /// The focus timer when it runs in another workspace than the shown one.
    pub timer_elsewhere: Option<TimerElsewhere>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct WorkspaceRow {
    pub id: String,
    pub name: String,
    /// What it holds, as on the desktop: "Current · 2 quests · 5 tasks · 3.5h tracked", "Empty".
    pub summary: String,
    pub current: bool,
    /// False for the last live workspace (it stays) and for deleted ones.
    pub can_delete: bool,
}

/// "● 36:00 in Default": the running timer's session time, task and workspace.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct TimerElsewhere {
    pub workspace_id: String,
    pub workspace_name: String,
    /// The task's title, "Other work" without a task.
    pub task_title: String,
    /// The session's time so far ("36:00"); counts live with the ticks.
    pub time: String,
}

pub(crate) fn workspaces(snapshot: &Snapshot, now: Now) -> WorkspacesData {
    let can_delete = snapshot.live_workspaces().count() > 1;
    let live: Vec<WorkspaceRow> =
        snapshot.live_workspaces().map(|w| row(w, w.id == snapshot.workspace, can_delete)).collect();
    let active =
        live.iter().find(|w| w.current).cloned().unwrap_or_else(|| row(snapshot.active_workspace(), true, false));
    WorkspacesData {
        active,
        live,
        deleted: snapshot.deleted_workspaces().into_iter().map(|w| row(w, false, false)).collect(),
        timer_elsewhere: timer_note(snapshot, now),
    }
}

fn row(workspace: &Workspace, current: bool, can_delete: bool) -> WorkspaceRow {
    let summary = content_summary(&workspace.content);
    WorkspaceRow {
        id: workspace.id.to_string(),
        name: workspace.name.clone(),
        summary: if current { format!("Current · {summary}") } else { summary },
        current,
        can_delete,
    }
}

fn timer_note(snapshot: &Snapshot, now: Now) -> Option<TimerElsewhere> {
    let (workspace, workspace_name, task_title) = timer_elsewhere(snapshot)?;
    let running = snapshot.running_focus()?;
    Some(TimerElsewhere {
        workspace_id: workspace.to_string(),
        workspace_name,
        task_title,
        time: format::timer(running.entry.tracked_ms(now.at)),
    })
}

/// The trimmed name, or the desktop's message for the field ("Give the workspace a name.").
pub(crate) fn name(snapshot: &Snapshot, typed: &str, renaming: Option<EntityId>) -> Result<String, MobileError> {
    check_name(snapshot, typed, renaming).map_err(|e| MobileError::InvalidForm { reason: e.message().to_owned() })
}

/// Whether `id` may be restored: not while a live workspace has its name (names stay unique).
pub(crate) fn check_restorable(snapshot: &Snapshot, id: EntityId) -> Result<(), MobileError> {
    let workspace = snapshot.workspace(id).ok_or_else(|| unknown(id))?;
    check_name(snapshot, &workspace.name, None).map(drop).map_err(|e| MobileError::InvalidForm {
        reason: format!("Rename the other “{}” first. {}", workspace.name, e.message()),
    })
}

pub(crate) fn unknown(id: EntityId) -> MobileError {
    MobileError::Rejected { reason: format!("the workspace {id} no longer exists. Pick one from the current list.") }
}
