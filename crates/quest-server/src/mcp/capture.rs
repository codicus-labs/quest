//! `capture_text`: the apps' AI capture for MCP. The same request (context from the account), rate
//! limit, provider call and validation as `POST /v1/ai/capture`; applying runs the same review
//! check (`Draft::can_apply`) and commands (`quest_capture::plan`) as the apps, all or nothing.
//! Context and changes stay in the call's workspace.

use quest_capture::review::{created_message, dropped_message};
use quest_capture::{Created, Draft, RequestError};
use quest_domain::EntityId;
use quest_protocol::UserId;

use super::account;
use super::error::{ToolCode, ToolError};
use super::types::{CaptureInput, CaptureOut, CreatedOut, ItemKind};
use super::view::Clock;
use crate::AppState;
use crate::ai::{CaptureFailure, ProviderError, capitalized};

const DEFAULT_LANGUAGE: &str = "en";

pub async fn capture_text(
    state: &AppState,
    user: UserId,
    input: &CaptureInput,
    clock: &Clock,
) -> Result<CaptureOut, ToolError> {
    let Some(ai) = state.ai.as_deref() else {
        return Err(ToolError::new(
            ToolCode::AiNotConfigured,
            "AI capture is not set up on this server; use create_task and the other tools",
        ));
    };
    let workspace = input.context.workspace.as_deref();
    let snapshot = account::snapshot(state, user, account::Scope::Named(workspace)).await?;
    let language = input.language.as_deref().map(str::trim).filter(|tag| !tag.is_empty()).unwrap_or(DEFAULT_LANGUAGE);
    let request = quest_capture::request(&input.text, clock.now, clock.zone, language, &snapshot);
    // Not logged here: per MCP call only the tool line (name, duration, code) is written.
    let outcome = ai.capture_request(user, &request).await;
    let proposal = outcome.result.map_err(failure)?;
    let dropped = proposal.dropped.iter().map(dropped_message).collect();
    let proposal_json =
        serde_json::to_value(&proposal).map_err(|error| ToolError::internal("encoding proposal", error))?;
    if !input.apply {
        return Ok(CaptureOut {
            applied: false,
            proposal: proposal_json,
            dropped,
            message: String::new(),
            created: Vec::new(),
        });
    }
    let (_, plan) = account::change(state, user, account::Scope::Named(workspace), clock.now, |snapshot| {
        let draft = Draft::new(proposal.clone());
        if !draft.can_apply(snapshot) {
            return Err(ToolError::new(
                ToolCode::ProposalNotApplicable,
                "Nothing was created: the proposal is empty or an item cannot be created as proposed (for example \
                 its task or quest no longer exists). Call again with apply false to see the proposal, then use \
                 create_task, log_work and the other tools for the items you want",
            ));
        }
        let plan = quest_capture::plan(&draft.confirmed(), snapshot, clock.now, clock.zone, EntityId::new)
            .map_err(|error| ToolError::new(ToolCode::ProposalNotApplicable, capitalized(&error)))?;
        Ok((plan.commands.clone(), plan))
    })
    .await?;
    let count = |wanted: fn(&Created) -> bool| plan.created.iter().filter(|created| wanted(created)).count();
    let message = created_message(
        count(|c| matches!(c, Created::Quest(_))),
        count(|c| matches!(c, Created::Task(_))),
        count(|c| matches!(c, Created::Entry(_))),
    );
    let created = plan.created.iter().map(created_out).collect();
    Ok(CaptureOut { applied: true, proposal: proposal_json, dropped, message, created })
}

fn created_out(created: &Created) -> CreatedOut {
    let (kind, id) = match created {
        Created::Quest(id) => (ItemKind::Quest, id),
        Created::Task(id) => (ItemKind::Task, id),
        Created::ChecklistItem(id) => (ItemKind::ChecklistItem, id),
        Created::Entry(id) => (ItemKind::TimeEntry, id),
    };
    CreatedOut { kind, id: id.to_string() }
}

fn failure(failure: CaptureFailure) -> ToolError {
    let unavailable = || {
        ToolError::new(
            ToolCode::AiUnavailable,
            "The AI provider is not available right now; try again in a few minutes",
        )
    };
    match failure {
        CaptureFailure::RateLimited => {
            ToolError::new(ToolCode::RateLimited, "Too many AI requests from this account; try again in a minute")
        }
        CaptureFailure::BodyTooLarge => ToolError::new(ToolCode::TextTooLong, "The text is too long; shorten it"),
        CaptureFailure::Request(error @ RequestError::TextTooLong { .. }) => {
            ToolError::new(ToolCode::TextTooLong, capitalized(&error))
        }
        CaptureFailure::Request(error) => ToolError::invalid(capitalized(&error)),
        CaptureFailure::MalformedBody(_) => ToolError::invalid("The capture request is malformed"),
        CaptureFailure::Provider(ProviderError::InvalidResponse) | CaptureFailure::Invalid => ToolError::new(
            ToolCode::AiInvalidResponse,
            "The AI answer could not be used; try again, maybe with other words",
        ),
        CaptureFailure::Provider(_) => unavailable(),
    }
}
