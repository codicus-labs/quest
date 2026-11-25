//! The account's state on the server, for MCP: read as a [`Snapshot`], changed with the same domain
//! commands the apps use. Changes are stamped by a server node of the account, stored like a push
//! and broadcast, so connected devices see them live.

use chrono::{DateTime, Utc};
use quest_domain::{Command, EntityId, FieldStore, HlcClock, NodeId, PlannedBatch, Snapshot, plan_all};
use quest_protocol::UserId;
use sha2::{Digest, Sha256};

use super::error::ToolError;
use super::view::resolve_workspace;
use crate::AppState;

/// Which workspace a tool works in.
#[derive(Clone, Copy)]
pub enum Scope<'a> {
    /// The one the `workspace` parameter names (see [`resolve_workspace`]).
    Named(Option<&'a str>),
    /// The one the running timer is in: there is one timer per account, across workspaces.
    Timer,
    /// Any live one, for tools that read only what spans workspaces.
    Account,
}

/// The account's state in the workspace `scope` picks.
pub async fn snapshot(state: &AppState, user: UserId, scope: Scope<'_>) -> Result<Snapshot, ToolError> {
    scoped(&load(state, user).await?, scope)
}

/// The read model of the workspace `scope` picks. A named one is resolved first:
/// `Snapshot::from_store` would fall back to another workspace for an unknown one.
fn scoped(store: &FieldStore, scope: Scope<'_>) -> Result<Snapshot, ToolError> {
    let any = Snapshot::from_store(store, EntityId::DEFAULT_WORKSPACE);
    let id = match scope {
        Scope::Named(workspace) => resolve_workspace(&any.workspaces, workspace)?,
        Scope::Timer => any.running_focus().map_or(any.workspace, |running| running.workspace),
        Scope::Account => any.workspace,
    };
    if id == any.workspace {
        return Ok(any);
    }
    Ok(Snapshot::from_store(store, id))
}

async fn load(state: &AppState, user: UserId) -> Result<FieldStore, ToolError> {
    let changes = state.repo.all_changes(user).await.map_err(|error| ToolError::internal("loading account", error))?;
    let mut store = FieldStore::default();
    for change in &changes {
        store.apply(change);
    }
    Ok(store)
}

/// Decides commands against the current state of the workspace `scope` picks and applies them all
/// or none, creating new records in that workspace. Pushes are
/// serialised per account (the same lock as sync pushes), so the decision sees the latest state.
/// Returns the snapshot after the change and what `decide` produced besides the commands.
pub async fn change<T>(
    state: &AppState,
    user: UserId,
    scope: Scope<'_>,
    now: DateTime<Utc>,
    decide: impl FnOnce(&Snapshot) -> Result<(Vec<Command>, T), ToolError>,
) -> Result<(Snapshot, T), ToolError> {
    let channel = state.hub.channel(user);
    let _write = channel.write_lock.lock().await;
    let store = load(state, user).await?;
    let before = scoped(&store, scope)?;
    let (commands, extra) = decide(&before)?;
    if commands.is_empty() {
        return Ok((before, extra));
    }
    let clock = HlcClock::resume(server_node(user), store.highest_hlc().unwrap_or(quest_domain::Hlc::ZERO));
    let now_ms = u64::try_from(now.timestamp_millis()).unwrap_or(0);
    let PlannedBatch { changes, store, .. } =
        plan_all(&commands, &store, before.workspace, &clock, now_ms).map_err(ToolError::from)?;
    let winners =
        state.repo.apply_changes(user, &changes).await.map_err(|error| ToolError::internal("saving changes", error))?;
    if !winners.is_empty() {
        // No receivers is fine: then no device is connected.
        let _ = channel.sender.send(winners.into());
    }
    Ok((Snapshot::from_store(&store, before.workspace), extra))
}

/// The node id MCP changes of `user` are stamped with: fixed per account, so its clock resumes
/// from the account's latest timestamp, and distinct from any device's (random) node.
pub fn server_node(user: UserId) -> NodeId {
    let digest = Sha256::digest(format!("quest-server/mcp/{}", user.0).as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    NodeId(u64::from_be_bytes(bytes))
}
