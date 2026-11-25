//! `/v1/ai/connections`: the account's MCP clients (OAuth grants for `/mcp`), listed and revoked by
//! the apps with their access token. The apps' own grants are not listed. See [`quest_protocol::AiConnections`].

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use quest_protocol::{AiConnection, AiConnections, ErrorCode};
use reqwest::Url;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthenticatedUser;
use crate::error::HttpFailure;
use crate::oauth::policy::Access;
use crate::repo::GrantSummary;

pub async fn list(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Json<AiConnections>, HttpFailure> {
    let grants = state
        .repo
        .active_grants(user, &state.oauth.resource)
        .await
        .map_err(|error| HttpFailure::internal("listing grants failed", error))?;
    Ok(Json(AiConnections { connections: grants.into_iter().map(connection).collect() }))
}

pub async fn revoke(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<StatusCode, HttpFailure> {
    let not_found = || {
        HttpFailure::new(
            StatusCode::NOT_FOUND,
            ErrorCode::NotFound,
            "This AI connection does not exist or was already revoked; reload the list",
        )
    };
    let id = Uuid::parse_str(&id).map_err(|_| not_found())?;
    let revoked = state
        .revoke_grant(id, Some(user))
        .await
        .map_err(|error| HttpFailure::internal("revoking grant failed", error))?;
    if !revoked {
        return Err(not_found());
    }
    tracing::info!(user = %user.0, grant = %id, "ai connection revoked");
    Ok(StatusCode::NO_CONTENT)
}

fn connection(grant: GrantSummary) -> AiConnection {
    let redirect_host =
        Url::parse(&grant.redirect_uri).ok().and_then(|url| url.host_str().map(str::to_owned)).unwrap_or_default();
    AiConnection {
        id: grant.id,
        client_name: grant.client_name,
        redirect_host,
        scope: Access::of_grant(&grant.scope).scope().to_owned(),
        created_at: from_ms(grant.created_at),
        last_used_at: grant.last_used_at.map(from_ms),
    }
}

fn from_ms(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap_or_default()
}
