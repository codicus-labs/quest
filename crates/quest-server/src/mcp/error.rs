//! Tool failures as models see them: a stable `code` and a message that says what to do next.

use std::fmt;

use quest_domain::{CommandError, EntityKind};
use rmcp::handler::server::tool::IntoCallToolResult;
use rmcp::model::{CallToolResponse, CallToolResult, ContentBlock};
use serde::Serialize;

/// Stable error codes of the tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCode {
    InvalidInput,
    TaskNotFound,
    QuestNotFound,
    WorkspaceNotFound,
    ChecklistItemNotFound,
    EntryNotFound,
    NotInArchive,
    InvalidTimeRange,
    InvalidRepeatRule,
    EmptyTitle,
    AiNotConfigured,
    AiDisabled,
    /// The token is read-only (`quest:read`) and the tool writes.
    InsufficientScope,
    AiUnavailable,
    AiInvalidResponse,
    RateLimited,
    TextTooLong,
    ProposalNotApplicable,
    Internal,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ToolError {
    pub code: ToolCode,
    pub message: String,
}

impl ToolError {
    pub fn new(code: ToolCode, message: impl Into<String>) -> Self {
        ToolError { code, message: message.into() }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        ToolError::new(ToolCode::InvalidInput, message)
    }

    /// Logs the cause (never tool content) and returns a neutral error.
    pub fn internal(context: &str, error: impl fmt::Display) -> Self {
        tracing::error!(%error, "mcp: {context} failed");
        ToolError::new(ToolCode::Internal, "The server could not do this right now; try again later")
    }

    pub fn task_not_found(id: impl fmt::Display) -> Self {
        ToolError::new(ToolCode::TaskNotFound, format!("No task with id {id}; find ids with list_tasks or search"))
    }

    pub fn workspace_not_found(id: impl fmt::Display) -> Self {
        ToolError::new(
            ToolCode::WorkspaceNotFound,
            format!("No workspace {id}; find ids and names with list_workspaces"),
        )
    }

    pub fn quest_not_found(id: impl fmt::Display) -> Self {
        ToolError::new(ToolCode::QuestNotFound, format!("No quest with id {id}; find ids with list_quests or search"))
    }
}

impl From<CommandError> for ToolError {
    fn from(error: CommandError) -> Self {
        match error {
            CommandError::UnknownQuest(id) => ToolError::quest_not_found(id),
            CommandError::UnknownTask(id) => ToolError::task_not_found(id),
            CommandError::UnknownChecklistItem(id) => ToolError::new(
                ToolCode::ChecklistItemNotFound,
                format!("No checklist item with id {id}; see get_checklist"),
            ),
            CommandError::UnknownPostponement(id) => {
                ToolError::new(ToolCode::InvalidInput, format!("No postponement with id {id}"))
            }
            CommandError::UnknownEntry(id) => {
                ToolError::new(ToolCode::EntryNotFound, format!("No time entry with id {id}; see list_time_entries"))
            }
            CommandError::InvalidTimeRange => {
                ToolError::new(ToolCode::InvalidTimeRange, "The end must be after the start")
            }
            CommandError::CannotResumeEntry => ToolError::new(
                ToolCode::InvalidTimeRange,
                "A finished session cannot be resumed; start the timer again instead",
            ),
            CommandError::NothingToRebook | CommandError::RebookOutsideEntry => {
                ToolError::new(ToolCode::InvalidTimeRange, error.to_string())
            }
            CommandError::NotInTrash { kind, id } => ToolError::new(
                ToolCode::NotInArchive,
                format!("No archived {} with id {id}; search with include_archived to find it", kind_name(kind)),
            ),
            CommandError::EmptyTitle => ToolError::new(ToolCode::EmptyTitle, "The title cannot be empty"),
            CommandError::InvalidRepeatRule => ToolError::new(
                ToolCode::InvalidRepeatRule,
                "Use an interval from 1 to 999 and an end date on or after the start",
            ),
            CommandError::UnknownWorkspace(id) => ToolError::workspace_not_found(id),
            CommandError::WorkspaceExists(id) => {
                ToolError::new(ToolCode::InvalidInput, format!("Workspace {id} already exists"))
            }
            CommandError::WorkspaceNotDeleted(id) => {
                ToolError::new(ToolCode::InvalidInput, format!("Workspace {id} is not deleted"))
            }
            CommandError::CannotDeleteLastWorkspace => {
                ToolError::new(ToolCode::InvalidInput, "The last workspace cannot be deleted")
            }
        }
    }
}

pub fn kind_name(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::TimeEntry => "time entry",
        other => other.as_str(),
    }
}

/// Errors go back as a tool result with `isError: true` and the JSON `{"code", "message"}` as
/// text, so the model can read and react to them.
impl IntoCallToolResult for ToolError {
    fn into_call_tool_result(self) -> Result<CallToolResponse, rmcp::ErrorData> {
        let json = serde_json::to_string(&self).unwrap_or_else(|_| self.message.clone());
        Ok(CallToolResult::error(vec![ContentBlock::text(json)]).into())
    }
}

/// The code of an error result, for the log line.
pub fn result_code(result: &CallToolResult) -> Option<String> {
    if result.is_error != Some(true) {
        return None;
    }
    let text = result.content.first().and_then(|content| content.as_text()).map(|text| text.text.as_str())?;
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    value.get("code").and_then(|code| code.as_str()).map(str::to_owned)
}
