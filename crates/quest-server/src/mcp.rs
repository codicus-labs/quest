//! The MCP server at `/mcp` (Streamable HTTP, stateless): the account's quests, tasks and time as
//! tools for AI clients. It is an adapter only: inputs become the same domain commands the apps
//! send, stored and broadcast like a sync push, so devices see changes live.
//!
//! Requests need an OAuth access token for this resource (see [`crate::oauth`]); the auth layer
//! puts the [`McpPrincipal`] on the request. A read-only token (`quest:read`) sees only the read
//! tools, and calling another one is answered with HTTP 403 `insufficient_scope`. `capture_text`
//! exists only when the server has AI configured and the account opted in. Logs name only the
//! tool, its duration and the error code, never arguments or results.

mod account;
mod capture;
mod error;
mod read;
mod tools;
mod types;
mod view;
mod write;

use std::sync::Arc;
use std::time::Instant;

use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chrono_tz::Tz;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{
    CacheScope, CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, ResultType, Tool,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, RoleServer, ServerHandler, tool_handler};

use crate::AppState;
use crate::oauth::policy::Access;
use crate::oauth::{self, McpPrincipal, OAuthService};
use error::{ToolCode, ToolError, result_code};

/// Largest JSON-RPC request body.
const MAX_BODY_BYTES: usize = 256 * 1024;

/// Tools that only read; every other tool needs `quest:write`.
pub(crate) const READ_TOOLS: &[&str] = &[
    "list_tasks",
    "get_task",
    "get_checklist",
    "list_quests",
    "get_quest",
    "search",
    "get_week_review",
    "get_running_timer",
    "list_time_entries",
    "list_workspaces",
];
const CAPTURE_TOOL: &str = "capture_text";

/// The MCP server at `/mcp`; `None` in [`crate::Config`] leaves it out.
#[derive(Clone, Debug)]
pub struct McpConfig {
    /// Time zone for "today" and local times when a tool call does not name one.
    pub default_time_zone: Tz,
}

impl Default for McpConfig {
    fn default() -> Self {
        McpConfig { default_time_zone: Tz::UTC }
    }
}

/// Whether `principal` may see and call `tool`.
pub(crate) fn tool_allowed(principal: &McpPrincipal, tool: &str) -> bool {
    tool_refusal(principal, tool).is_none()
}

/// Why `principal` may not call `tool`, if it may not: a tool error with what to do.
pub(crate) fn tool_refusal(principal: &McpPrincipal, tool: &str) -> Option<ToolError> {
    if principal.access != Access::Write && !READ_TOOLS.contains(&tool) {
        return Some(insufficient_scope_error());
    }
    if tool == CAPTURE_TOOL && !principal.ai_enabled {
        return Some(ToolError::new(
            ToolCode::AiDisabled,
            "AI features are off for this account; turn them on in the Quest app under Settings → AI features",
        ));
    }
    None
}

fn insufficient_scope_error() -> ToolError {
    ToolError::new(
        ToolCode::InsufficientScope,
        "This connection may only read; reconnect it with \"Read and write\" access to change anything",
    )
}

#[derive(Clone)]
pub(crate) struct QuestMcp {
    state: AppState,
    default_time_zone: Tz,
    tool_router: ToolRouter<QuestMcp>,
}

impl QuestMcp {
    fn new(state: AppState, default_time_zone: Tz) -> Self {
        let mut tool_router = Self::tool_router();
        if state.ai.is_none() {
            tool_router.remove_route(CAPTURE_TOOL);
        }
        QuestMcp { state, default_time_zone, tool_router }
    }
}

fn principal(context: &RequestContext<RoleServer>) -> Option<McpPrincipal> {
    context.extensions.get::<Parts>().and_then(|parts| parts.extensions.get::<McpPrincipal>()).copied()
}

#[tool_handler(
    router = self.tool_router,
    name = "quest",
    instructions = "Quest plans the user's days: quests (long-term goals), tasks (with an optional plan \
        date, deadline and checklist) and tracked time (on tasks, or organizing). Always pass the user's IANA time_zone (for \
        example Europe/Berlin) so that today, days and clock times mean what the user means; dates are ISO \
        (2026-11-24), clock times HH:MM in that zone, instants RFC 3339 with offset. The user may keep \
        separate workspaces (e.g. work and private); every tool works in the default workspace unless you pass \
        workspace (an id or exact name from list_workspaces), and ids from one workspace are not found in \
        another. Find ids with list_tasks, \
        list_quests or search before changing things. Archived items can be brought back with restore. Errors \
        come back as JSON {\"code\", \"message\"}; the message says what to do next."
)]
impl ServerHandler for QuestMcp {
    /// The tools this caller may use: read-only tokens see only [`READ_TOOLS`]; `capture_text` only
    /// with the account's AI opt-in.
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let principal = principal(&context);
        let tools: Vec<Tool> = self
            .tool_router
            .list_all()
            .into_iter()
            .filter(|tool| principal.as_ref().is_some_and(|principal| tool_allowed(principal, &tool.name)))
            .collect();
        let supports_cache_hints =
            context.protocol_version().is_some_and(|version| version >= ProtocolVersion::V_2026_07_28);
        Ok(ListToolsResult {
            result_type: Some(ResultType::COMPLETE),
            tools,
            meta: None,
            next_cursor: None,
            ttl_ms: supports_cache_hints.then_some(0),
            cache_scope: supports_cache_hints.then_some(CacheScope::Private),
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let tool = request.name.to_string();
        let started = Instant::now();
        // Write tools of read-only tokens rarely get here (403 in `require_token`); this mainly
        // catches `capture_text` without the AI opt-in.
        let refusal = match principal(&context) {
            Some(principal) => tool_refusal(&principal, &tool),
            None => Some(insufficient_scope_error()),
        };
        if let Some(error) = refusal {
            tracing::info!(tool, code = ?error.code, "mcp tool call");
            let json = serde_json::to_string(&error).unwrap_or_default();
            return Ok(CallToolResponse::Complete(CallToolResult::error(vec![ContentBlock::text(json)])));
        }
        let response = self.tool_router.call(ToolCallContext::new(self, request, context)).await.map(coded);
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let code = match &response {
            Ok(CallToolResponse::Complete(result)) => result_code(result),
            Ok(_) => None,
            Err(error) => Some(format!("protocol_{}", error.code.0)),
        };
        tracing::info!(tool, duration_ms, code = code.as_deref().unwrap_or("ok"), "mcp tool call");
        response
    }
}

/// Argument errors rmcp reports itself (wrong JSON types, missing fields) get the same
/// `{"code", "message"}` shape as the tools' own errors.
fn coded(response: CallToolResponse) -> CallToolResponse {
    let CallToolResponse::Complete(result) = &response else { return response };
    if result.is_error != Some(true) || result_code(result).is_some() {
        return response;
    }
    let text = result.content.first().and_then(|content| content.as_text()).map(|text| text.text.clone());
    let error =
        ToolError::invalid(text.unwrap_or_else(|| "The arguments do not match the tool's input schema".to_owned()));
    let json = serde_json::to_string(&error).unwrap_or_default();
    CallToolResponse::Complete(CallToolResult::error(vec![ContentBlock::text(json)]))
}

/// The `/mcp` endpoint: stateless Streamable HTTP with JSON responses. `Host` must be the public
/// host (or loopback) against DNS rebinding.
pub(crate) fn service(
    state: AppState,
    oauth: &OAuthService,
    mcp_config: &McpConfig,
) -> StreamableHttpService<QuestMcp, NeverSessionManager> {
    let mut allowed_hosts = vec!["localhost".to_owned(), "127.0.0.1".to_owned(), "::1".to_owned()];
    if let Some(host) = oauth.public_url.host_str() {
        allowed_hosts.push(host.trim_start_matches('[').trim_end_matches(']').to_owned());
        if let Some(port) = oauth.public_url.port() {
            allowed_hosts.push(format!("{host}:{port}"));
        }
    }
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None)
        .with_allowed_hosts(allowed_hosts)
        .with_max_request_body_bytes(MAX_BODY_BYTES);
    let default_time_zone = mcp_config.default_time_zone;
    StreamableHttpService::new(
        move || Ok(QuestMcp::new(state.clone(), default_time_zone)),
        Arc::new(NeverSessionManager::default()),
        config,
    )
}

/// Lets only requests with a valid access token through, with their [`McpPrincipal`]. A
/// read-only token calling a write tool gets 403 `insufficient_scope` (RFC 6750), so the client
/// can ask the user for more access.
pub(crate) async fn require_token(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let principal = match oauth::authenticate(&state, request.headers()).await {
        Ok(principal) => principal,
        Err(response) => return *response,
    };
    let (mut parts, body) = request.into_parts();
    parts.extensions.insert(principal);
    if principal.access == Access::Write {
        return next.run(Request::from_parts(parts, body)).await;
    }
    let Ok(bytes) = to_bytes(body, MAX_BODY_BYTES).await else {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    };
    if calls_write_tool(&bytes) {
        tracing::info!("mcp write tool refused: read-only token");
        return oauth::insufficient_scope(&state);
    }
    next.run(Request::from_parts(parts, Body::from(bytes))).await
}

/// Whether a JSON-RPC body (one message or a batch) calls a tool that is not read-only.
fn calls_write_tool(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else { return false };
    let messages = match value {
        serde_json::Value::Array(messages) => messages,
        message => vec![message],
    };
    messages.iter().any(|message| {
        message["method"] == "tools/call"
            && message["params"]["name"].as_str().is_some_and(|tool| !READ_TOOLS.contains(&tool))
    })
}

#[cfg(test)]
mod tests;
