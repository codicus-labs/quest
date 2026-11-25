//! Workspaces as the switcher and "Manage workspaces" show them, pure: what a workspace holds in
//! words, whether a name can be used, and what a delete asks before it goes ahead.

#[cfg(test)]
mod tests;

use quest_domain::{EntityId, Snapshot, Workspace, WorkspaceContent};

use crate::format;

/// Longest workspace name, as in the prototype.
pub const MAX_NAME_CHARS: usize = 60;

/// "2 quests · 5 tasks · 3.5h tracked", or "Empty".
pub fn content_summary(content: &WorkspaceContent) -> String {
    if *content == WorkspaceContent::default() {
        return "Empty".to_owned();
    }
    let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    [
        plural(content.quests, "quest", "quests"),
        plural(content.tasks, "task", "tasks"),
        format!("{} tracked", format::hours(content.tracked_ms)),
    ]
    .join(" · ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
    /// Another live workspace has this name (names pick workspaces in MCP, so they stay unique).
    Taken,
}

impl NameError {
    pub fn message(self) -> &'static str {
        match self {
            NameError::Empty => "Give the workspace a name.",
            NameError::TooLong => "Use at most 60 characters.",
            NameError::Taken => "Another workspace already has this name.",
        }
    }
}

/// The trimmed name, if a live workspace other than `renaming` may take it.
pub fn check_name(snapshot: &Snapshot, name: &str, renaming: Option<EntityId>) -> Result<String, NameError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(NameError::TooLong);
    }
    let wanted = name.to_lowercase();
    let taken = snapshot.live_workspaces().any(|w| Some(w.id) != renaming && w.name.trim().to_lowercase() == wanted);
    if taken {
        return Err(NameError::Taken);
    }
    Ok(name.to_owned())
}

/// What the delete confirmation says about `workspace`.
pub fn delete_question(workspace: &Workspace) -> String {
    let holds = if workspace.content == WorkspaceContent::default() {
        "It is empty.".to_owned()
    } else {
        format!("It holds {}.", content_summary(&workspace.content).replace(" · ", ", "))
    };
    format!("Delete “{}”? {holds} You can restore it under Deleted.", workspace.name)
}

/// Where the running focus timer is when it is not in the shown workspace: its workspace's name
/// and the task's title ("Other work" without a task).
pub fn timer_elsewhere(snapshot: &Snapshot) -> Option<(EntityId, String, String)> {
    let running = snapshot.running_focus()?;
    if running.workspace == snapshot.workspace {
        return None;
    }
    let workspace = snapshot.workspace(running.workspace).map_or_else(String::new, |w| w.name.clone());
    let title = running.task_title.clone().unwrap_or_else(|| "Other work".to_owned());
    Some((running.workspace, workspace, title))
}
