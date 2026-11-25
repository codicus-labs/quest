//! The server's browser session: sign-in at the identity provider (`/authorize` → provider →
//! `/auth/callback`), the `quest_session` cookie, and sign-out (`/auth/logout`).
//!
//! A sign-in in progress is stored by its hashed `state` with nonce, PKCE verifier and the
//! `/authorize` request to continue with, and is bound to the browser that started it by a
//! `quest_login_<id>` cookie (against login CSRF; one per sign-in, so parallel sign-ins in one
//! browser do not overwrite each other). Each one is used once and expires.

use axum::extract::{Query, State};
use axum::http::header::{CACHE_CONTROL, COOKIE, LOCATION, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use quest_protocol::{LOGGED_OUT_PATH, OIDC_CALLBACK_PATH};
use serde::Deserialize;

use super::{OAuthService, authorize, page, policy};
use crate::AppState;
use crate::auth::{hash_token, new_token};
use crate::oidc::PendingLogin;
use crate::repo::{BrowserSession, NewBrowserSession, NewLogin};
use crate::unix_ms;

const SESSION_COOKIE: &str = "quest_session";
const LOGIN_COOKIE_PREFIX: &str = "quest_login_";

/// The binding cookie of the sign-in whose `state` hashes to `state_hash`.
fn login_cookie(state_hash: &str) -> String {
    format!("{LOGIN_COOKIE_PREFIX}{}", &state_hash[..16.min(state_hash.len())])
}

/// A valid browser session and its secret (the cookie value).
pub(crate) struct Session {
    pub token: String,
    pub account: BrowserSession,
}

impl Session {
    /// The consent form's CSRF token: derived from the session secret, so another site cannot
    /// know it (the cookie is also `SameSite=Lax`).
    pub fn csrf(&self) -> String {
        csrf_token(&self.token)
    }
}

pub(crate) fn csrf_token(session_token: &str) -> String {
    hash_token(&format!("csrf:{session_token}"))
}

/// The browser session of the request, if its cookie names a valid one.
pub(crate) async fn current_session(state: &AppState, headers: &HeaderMap) -> Result<Option<Session>, Box<Response>> {
    let Some(token) = cookie(headers, SESSION_COOKIE) else { return Ok(None) };
    let account =
        state.repo.browser_session(&hash_token(&token), unix_ms()).await.map_err(|error| Box::new(internal(error)))?;
    Ok(account.map(|account| Session { token, account }))
}

/// Ends the request's browser session; returns its ID token (logout hint) and the cookie that
/// clears it.
pub(crate) async fn end_session(state: &AppState, headers: &HeaderMap) -> Result<Option<String>, Box<Response>> {
    let Some(token) = cookie(headers, SESSION_COOKIE) else { return Ok(None) };
    state.repo.delete_browser_session(&hash_token(&token)).await.map_err(|error| Box::new(internal(error)))
}

/// Sends the browser to the identity provider; the callback continues with `authorize_query`.
/// `force_login` asks the provider to sign in again (another account).
pub(crate) async fn start_login(state: &AppState, authorize_query: &str, force_login: bool) -> Response {
    let oauth = &state.oauth;
    let redirect_uri = callback_uri(oauth);
    let started = match oauth.config.identity.start_login(&redirect_uri, force_login).await {
        Ok(started) => started,
        Err(error) => {
            tracing::error!(%error, "starting the sign-in at the identity provider failed");
            return page::message(
                StatusCode::BAD_GATEWAY,
                "Sign-in is unavailable",
                "Quest cannot reach its sign-in provider right now. Try again in a minute.",
            );
        }
    };
    let binding = new_token();
    let now = unix_ms();
    let ttl = oauth.config.login_ttl;
    let state_hash = hash_token(&started.pending.state);
    let login = NewLogin {
        state_hash: &state_hash,
        browser_hash: &hash_token(&binding),
        nonce: &started.pending.nonce,
        pkce_verifier: &started.pending.pkce_verifier,
        authorize_query,
        expires_at: now + millis(ttl),
    };
    if let Err(error) = state.repo.insert_login(&login, now).await {
        return internal(error);
    }
    let cookie = set_cookie(oauth, &login_cookie(&state_hash), &binding, OIDC_CALLBACK_PATH, ttl.as_secs());
    let mut response = see_other(started.url.as_str());
    append_cookie(&mut response, &cookie);
    response
}

#[derive(Debug, Default, Deserialize)]
pub struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// `GET /auth/callback`: the identity provider's redirect after sign-in.
pub async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CallbackParams>,
) -> Response {
    let oauth = &state.oauth;
    let expired = || {
        page::message(
            StatusCode::BAD_REQUEST,
            "This sign-in cannot be used",
            "It expired, was already used, or was started in another browser. Start signing in again from the \
             Quest app or your AI client.",
        )
    };
    let Some(login_state) = params.state.as_deref() else { return expired() };
    let state_hash = hash_token(login_state);
    let login_cookie = login_cookie(&state_hash);
    let started = match state.repo.take_login(&state_hash).await {
        Ok(Some(started)) if started.expires_at > unix_ms() => started,
        Ok(_) => return expired(),
        Err(error) => return internal(error),
    };
    let bound = cookie(&headers, &login_cookie).is_some_and(|binding| hash_token(&binding) == started.browser_hash);
    if !bound {
        tracing::warn!("sign-in callback from another browser refused");
        return expired();
    }
    let clear_login = set_cookie(oauth, &login_cookie, "", OIDC_CALLBACK_PATH, 0);
    let mut response = match (params.error.as_deref(), params.code.as_deref()) {
        (Some(error), _) => {
            tracing::info!(error, "the identity provider refused the sign-in");
            authorize::declined(&state, &started.authorize_query).await
        }
        (None, Some(code)) => finish(&state, code, login_state, &started).await,
        (None, None) => expired(),
    };
    append_cookie(&mut response, &clear_login);
    response
}

async fn finish(state: &AppState, code: &str, login_state: &str, started: &crate::repo::StartedLogin) -> Response {
    let oauth = &state.oauth;
    let pending = PendingLogin {
        state: login_state.to_owned(),
        nonce: started.nonce.clone(),
        pkce_verifier: started.pkce_verifier.clone(),
    };
    let identity = match oauth.config.identity.finish_login(&callback_uri(oauth), code, &pending).await {
        Ok(identity) => identity,
        Err(error) => {
            tracing::warn!(%error, "sign-in at the identity provider failed");
            return page::message(
                StatusCode::UNAUTHORIZED,
                "Sign-in failed",
                "Quest could not verify the answer of its sign-in provider. Start again; if it keeps failing, the \
                 server's OIDC settings may be wrong.",
            );
        }
    };
    let user = match state
        .repo
        .sign_in_account(&identity.issuer, &identity.subject, identity.email.as_deref(), identity.name.as_deref())
        .await
    {
        Ok(user) => user,
        Err(error) => return internal(error),
    };
    let token = new_token();
    let now = unix_ms();
    let session = NewBrowserSession {
        token_hash: &hash_token(&token),
        user,
        id_token: &identity.id_token,
        expires_at: now + millis(oauth.config.session_ttl),
    };
    if let Err(error) = state.repo.insert_browser_session(&session, now).await {
        return internal(error);
    }
    tracing::info!(user = %user.0, "signed in at the identity provider");
    let account = BrowserSession { user, email: identity.email, name: identity.name };
    let session = Session { token, account };
    let mut response = authorize::after_login(state, &started.authorize_query, &session).await;
    let cookie = set_cookie(oauth, SESSION_COOKIE, &session.token, "/", oauth.config.session_ttl.as_secs());
    append_cookie(&mut response, &cookie);
    response
}

#[derive(Debug, Default, Deserialize)]
pub struct LogoutParams {
    return_to: Option<String>,
}

/// `GET /auth/logout`: ends the browser session here and, if the provider supports it, there.
/// `return_to` must be a first-party app's redirect URI; anything else is ignored.
pub async fn logout(State(state): State<AppState>, headers: HeaderMap, Query(params): Query<LogoutParams>) -> Response {
    let oauth = &state.oauth;
    let return_to = params.return_to.filter(|uri| policy::first_party_redirect(uri, &oauth.issuer));
    let id_token = match end_session(&state, &headers).await {
        Ok(id_token) => id_token,
        Err(response) => return *response,
    };
    let logged_out = format!("{}{LOGGED_OUT_PATH}", oauth.issuer);
    let provider_logout = match &id_token {
        Some(hint) => oauth.config.identity.logout_url(hint, &logged_out, return_to.as_deref()).await,
        None => None,
    };
    let target = provider_logout.map(String::from).unwrap_or_else(|| {
        let mut url = url::Url::parse(&logged_out).expect("the issuer is a valid origin");
        if let Some(return_to) = &return_to {
            url.query_pairs_mut().append_pair("state", return_to);
        }
        url.into()
    });
    tracing::info!(had_session = id_token.is_some(), "signed out of the browser session");
    let mut response = see_other(&target);
    append_cookie(&mut response, &set_cookie(oauth, SESSION_COOKIE, "", "/", 0));
    response
}

#[derive(Debug, Default, Deserialize)]
pub struct LoggedOutParams {
    state: Option<String>,
}

/// `GET /auth/logged-out`: back in the app (`state` is a first-party redirect URI) or a page.
pub async fn logged_out(State(state): State<AppState>, Query(params): Query<LoggedOutParams>) -> Response {
    match params.state.filter(|uri| policy::first_party_redirect(uri, &state.oauth.issuer)) {
        Some(return_to) => see_other(&return_to),
        None => page::message(StatusCode::OK, "Signed out of Quest", "You can close this window."),
    }
}

fn callback_uri(oauth: &OAuthService) -> String {
    format!("{}{OIDC_CALLBACK_PATH}", oauth.issuer)
}

fn millis(duration: std::time::Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

fn see_other(location: &str) -> Response {
    let Ok(location) = HeaderValue::from_str(location) else {
        return page::message(StatusCode::BAD_GATEWAY, "Sign-in is unavailable", "The redirect is not a valid URL.");
    };
    (StatusCode::SEE_OTHER, [(LOCATION, location), (CACHE_CONTROL, HeaderValue::from_static("no-store"))])
        .into_response()
}

/// A `Set-Cookie` value: HttpOnly, SameSite=Lax, Secure on https; `max_age` 0 deletes it.
fn set_cookie(oauth: &OAuthService, name: &str, value: &str, path: &str, max_age: u64) -> String {
    let secure = if oauth.secure_cookies() { "; Secure" } else { "" };
    format!("{name}={value}; Path={path}; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}")
}

pub(crate) fn clear_session_cookie(oauth: &OAuthService) -> String {
    set_cookie(oauth, SESSION_COOKIE, "", "/", 0)
}

pub(crate) fn append_cookie(response: &mut Response, cookie: &str) {
    if let Ok(value) = HeaderValue::from_str(cookie) {
        response.headers_mut().append(SET_COOKIE, value);
    }
}

fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, value)| *key == name && !value.is_empty())
        .map(|(_, value)| value.to_owned())
}

fn internal(error: impl std::fmt::Display) -> Response {
    tracing::error!(%error, "sign-in request failed");
    page::message(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Something went wrong",
        "Quest could not process this request; try again later.",
    )
}
