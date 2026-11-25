//! `/token` (code exchange, refresh with rotation) and `/revoke` (RFC 7009).

use axum::Form;
use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::json;

use super::policy::Access;
use super::policy::{self, metadata_document_url};
use super::{OAuthError, OAuthService, no_store};
use crate::AppState;
use crate::auth::{hash_token, new_token};
use crate::repo::{GrantRef, NewTokens, TokenKind};
use crate::unix_ms;

#[derive(Debug, Default, Deserialize)]
pub struct TokenRequest {
    grant_type: Option<String>,
    code: Option<String>,
    redirect_uri: Option<String>,
    code_verifier: Option<String>,
    refresh_token: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    resource: Option<String>,
    scope: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RevokeRequest {
    token: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
}

fn invalid_request(message: &str) -> OAuthError {
    OAuthError::new(StatusCode::BAD_REQUEST, "invalid_request", message)
}

fn invalid_grant(message: &str) -> OAuthError {
    OAuthError::new(StatusCode::BAD_REQUEST, "invalid_grant", message)
}

pub async fn token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(request): Form<TokenRequest>,
) -> Result<Response, OAuthError> {
    let oauth = &state.oauth;
    let client_id =
        authenticate_client(&state, &headers, request.client_id.as_deref(), request.client_secret.as_deref()).await?;
    match request.grant_type.as_deref() {
        Some("authorization_code") => exchange_code(&state, oauth, &client_id, &request).await,
        Some("refresh_token") => refresh(&state, oauth, &client_id, &request).await,
        _ => Err(OAuthError::new(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            "Use grant_type authorization_code or refresh_token",
        )),
    }
}

async fn exchange_code(
    state: &AppState,
    oauth: &OAuthService,
    client_id: &str,
    request: &TokenRequest,
) -> Result<Response, OAuthError> {
    let code = request.code.as_deref().ok_or_else(|| invalid_request("code is missing"))?;
    let code_hash = hash_token(code);
    let stored = state.repo.find_code(&code_hash).await.map_err(OAuthError::internal)?;
    let stored = stored.ok_or_else(|| invalid_grant("The authorization code is unknown"))?;
    if stored.used {
        // A replayed code may have been stolen: end everything issued for it.
        revoke_grant(state, &stored.grant, "authorization code reused").await?;
        return Err(invalid_grant("The authorization code was already used"));
    }
    if stored.grant.client_id != client_id {
        return Err(invalid_grant("The authorization code was issued to another client"));
    }
    if stored.expires_at <= unix_ms() {
        return Err(invalid_grant("The authorization code expired; authorize again"));
    }
    if request.redirect_uri.as_deref().is_some_and(|uri| uri != stored.redirect_uri) {
        return Err(invalid_grant("redirect_uri differs from the one in the authorization request"));
    }
    let verifier =
        request.code_verifier.as_deref().ok_or_else(|| invalid_request("code_verifier is missing (PKCE)"))?;
    if !policy::pkce_matches(verifier, &stored.code_challenge) {
        return Err(invalid_grant("code_verifier does not match the code_challenge"));
    }
    check_resource(request.resource.as_deref(), &stored.grant)?;
    if stored.grant.revoked {
        return Err(invalid_grant("This authorization was revoked"));
    }
    if !state.repo.use_code(&code_hash).await.map_err(OAuthError::internal)? {
        revoke_grant(state, &stored.grant, "authorization code raced").await?;
        return Err(invalid_grant("The authorization code was already used"));
    }
    issue(state, oauth, &stored.grant).await
}

async fn refresh(
    state: &AppState,
    oauth: &OAuthService,
    client_id: &str,
    request: &TokenRequest,
) -> Result<Response, OAuthError> {
    let token = request.refresh_token.as_deref().ok_or_else(|| invalid_request("refresh_token is missing"))?;
    let token_hash = hash_token(token);
    let stored = state.repo.find_token(&token_hash).await.map_err(OAuthError::internal)?;
    let stored = stored
        .filter(|stored| stored.kind == TokenKind::Refresh)
        .ok_or_else(|| invalid_grant("The refresh token is unknown or was revoked; authorize again"))?;
    if stored.grant.client_id != client_id {
        return Err(invalid_grant("The refresh token was issued to another client"));
    }
    if stored.grant.revoked {
        return Err(invalid_grant("This authorization was revoked; authorize again"));
    }
    if stored.used {
        // Rotation: an old refresh token coming back means one copy leaked.
        revoke_grant(state, &stored.grant, "refresh token reused").await?;
        return Err(invalid_grant("The refresh token was already used; authorize again"));
    }
    if stored.expires_at <= unix_ms() {
        return Err(invalid_grant("The refresh token expired; authorize again"));
    }
    check_resource(request.resource.as_deref(), &stored.grant)?;
    let asked = policy::requested_access(request.scope.as_deref())
        .map_err(|reason| OAuthError::new(StatusCode::BAD_REQUEST, "invalid_scope", reason))?;
    if asked.is_some_and(|asked| asked > Access::of_grant(&stored.grant.scope)) {
        return Err(OAuthError::new(
            StatusCode::BAD_REQUEST,
            "invalid_scope",
            "The refresh token cannot widen the granted scope; authorize again",
        ));
    }
    if !state.repo.use_refresh_token(&token_hash).await.map_err(OAuthError::internal)? {
        revoke_grant(state, &stored.grant, "refresh token raced").await?;
        return Err(invalid_grant("The refresh token was already used; authorize again"));
    }
    issue(state, oauth, &stored.grant).await
}

fn check_resource(requested: Option<&str>, grant: &GrantRef) -> Result<(), OAuthError> {
    match requested {
        Some(resource) if !policy::same_resource(resource, &grant.resource) => Err(OAuthError::new(
            StatusCode::BAD_REQUEST,
            "invalid_target",
            "This grant is only valid for the resource it was authorized for",
        )),
        _ => Ok(()),
    }
}

async fn issue(state: &AppState, oauth: &OAuthService, grant: &GrantRef) -> Result<Response, OAuthError> {
    let now = unix_ms();
    let ttl_ms = |ttl: std::time::Duration| i64::try_from(ttl.as_millis()).unwrap_or(i64::MAX);
    let (access, refresh) = (new_token(), new_token());
    let tokens = NewTokens {
        access_hash: &hash_token(&access),
        access_expires_at: now + ttl_ms(oauth.config.access_token_ttl),
        refresh_hash: &hash_token(&refresh),
        refresh_expires_at: now + ttl_ms(oauth.config.refresh_token_ttl),
    };
    state.repo.insert_tokens(grant.id, &tokens, now).await.map_err(OAuthError::internal)?;
    let body = json!({
        "access_token": access,
        "token_type": "Bearer",
        "expires_in": oauth.config.access_token_ttl.as_secs(),
        "refresh_token": refresh,
        "scope": grant.scope,
    });
    Ok((no_store(), axum::Json(body)).into_response())
}

async fn revoke_grant(state: &AppState, grant: &GrantRef, reason: &str) -> Result<(), OAuthError> {
    tracing::warn!(grant = %grant.id, reason, "oauth grant revoked");
    state.revoke_grant(grant.id, None).await.map_err(OAuthError::internal)?;
    Ok(())
}

/// The client a token request comes from. Public clients (the Quest apps, metadata-document
/// clients, public registrations) send only `client_id`; registered confidential ones
/// authenticate with their secret (HTTP Basic or form).
async fn authenticate_client(
    state: &AppState,
    headers: &HeaderMap,
    form_id: Option<&str>,
    form_secret: Option<&str>,
) -> Result<String, OAuthError> {
    let invalid_client = |message: &str| OAuthError::new(StatusCode::UNAUTHORIZED, "invalid_client", message);
    let basic = basic_credentials(headers);
    let (client_id, secret) = match (&basic, form_id) {
        (Some((id, secret)), _) => (id.as_str(), Some(secret.as_str())),
        (None, Some(id)) => (id, form_secret),
        (None, None) => return Err(invalid_client("client_id is missing")),
    };
    if policy::first_party_client(client_id, &state.oauth.issuer).is_some()
        || metadata_document_url(client_id).is_some()
    {
        return Ok(client_id.to_owned());
    }
    let client = state.repo.find_client(client_id).await.map_err(OAuthError::internal)?;
    let client = client.ok_or_else(|| invalid_client("The client is not registered; register it again"))?;
    if let Some(expected) = &client.secret_hash
        && secret.map(hash_token).as_ref() != Some(expected)
    {
        return Err(invalid_client("The client secret is missing or wrong"));
    }
    Ok(client.client_id)
}

fn basic_credentials(headers: &HeaderMap) -> Option<(String, String)> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, encoded) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("basic") {
        return None;
    }
    let decoded = String::from_utf8(STANDARD.decode(encoded.trim()).ok()?).ok()?;
    let (id, secret) = decoded.split_once(':')?;
    let decode =
        |part: &str| url::form_urlencoded::parse(format!("v={part}").as_bytes()).next().map(|(_, v)| v.into_owned());
    Some((decode(id)?, decode(secret)?))
}

/// RFC 7009: revoking either token of a grant revokes the grant (the apps' sign-out). Always 200,
/// also for unknown tokens.
pub async fn revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(request): Form<RevokeRequest>,
) -> Result<Response, OAuthError> {
    let client_id =
        authenticate_client(&state, &headers, request.client_id.as_deref(), request.client_secret.as_deref()).await?;
    let token = request.token.as_deref().ok_or_else(|| invalid_request("token is missing"))?;
    let stored = state.repo.find_token(&hash_token(token)).await.map_err(OAuthError::internal)?;
    if let Some(stored) = stored.filter(|stored| stored.grant.client_id == client_id) {
        state.revoke_grant(stored.grant.id, None).await.map_err(OAuthError::internal)?;
        tracing::info!(grant = %stored.grant.id, "oauth grant revoked by its client");
    }
    Ok((StatusCode::OK, no_store()).into_response())
}
