//! `/v1/account`: who is signed in, and the account settings (the AI opt-in).

use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use quest_protocol::{AccountInfo, AccountSettingsUpdate, ErrorCode, UserId};

use crate::AppState;
use crate::auth::AuthenticatedUser;
use crate::error::HttpFailure;

pub async fn get(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Json<AccountInfo>, HttpFailure> {
    account_info(&state, user).await.map(Json)
}

pub async fn update_settings(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    body: Result<Json<AccountSettingsUpdate>, JsonRejection>,
) -> Result<Json<AccountInfo>, HttpFailure> {
    let Json(update) = body.map_err(|rejection| HttpFailure::invalid_request(rejection.body_text()))?;
    state
        .repo
        .set_ai_enabled(user, update.ai_enabled)
        .await
        .map_err(|error| HttpFailure::internal("saving account settings failed", error))?;
    tracing::info!(user = %user.0, ai_enabled = update.ai_enabled, "account settings changed");
    account_info(&state, user).await.map(Json)
}

async fn account_info(state: &AppState, user: UserId) -> Result<AccountInfo, HttpFailure> {
    let account =
        state.repo.account(user).await.map_err(|error| HttpFailure::internal("reading the account failed", error))?;
    let account = account.ok_or_else(|| {
        HttpFailure::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::Unauthorized,
            "The account no longer exists; sign in again",
        )
    })?;
    Ok(AccountInfo {
        user_id: account.id,
        issuer: account.issuer.unwrap_or_default(),
        subject: account.subject.unwrap_or_default(),
        email: account.email,
        name: account.name,
        ai_enabled: account.ai_enabled,
        ai_available: state.ai.is_some(),
    })
}
