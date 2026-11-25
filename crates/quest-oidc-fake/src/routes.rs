//! The provider's HTTP endpoints.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::header::{AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, LOCATION};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Form, Json, Router};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{FakeConfig, FakeUser, Fault, jwt, random_token};

const CODE_TTL: Duration = Duration::from_secs(60);
pub const AUTHORIZE_PATH: &str = "/authorize";
const TOKEN_PATH: &str = "/token";
const JWKS_PATH: &str = "/jwks";
const LOGOUT_PATH: &str = "/logout";

pub struct Shared {
    pub issuer: String,
    pub config: FakeConfig,
    pub codes: Mutex<HashMap<String, IssuedCode>>,
    pub logouts: AtomicUsize,
}

pub struct IssuedCode {
    redirect_uri: String,
    nonce: Option<String>,
    challenge: String,
    user: FakeUser,
    fault: Option<Fault>,
    expires: Instant,
}

pub fn router(state: Arc<Shared>) -> Router {
    Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route(JWKS_PATH, get(jwks))
        .route(AUTHORIZE_PATH, get(authorize))
        .route(TOKEN_PATH, axum::routing::post(token))
        .route(LOGOUT_PATH, get(logout))
        .with_state(state)
}

async fn discovery(State(state): State<Arc<Shared>>) -> Json<Value> {
    let issuer = &state.issuer;
    Json(json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}{AUTHORIZE_PATH}"),
        "token_endpoint": format!("{issuer}{TOKEN_PATH}"),
        "jwks_uri": format!("{issuer}{JWKS_PATH}"),
        "end_session_endpoint": format!("{issuer}{LOGOUT_PATH}"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "scopes_supported": ["openid", "profile", "email"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"],
        "code_challenge_methods_supported": ["S256"],
        "claims_supported": ["sub", "iss", "aud", "exp", "iat", "nonce", "email", "name", "preferred_username"],
    }))
}

async fn jwks() -> Json<Value> {
    Json(jwt::jwks())
}

#[derive(Debug, Default, Deserialize)]
struct AuthorizeParams {
    client_id: Option<String>,
    redirect_uri: Option<String>,
    response_type: Option<String>,
    scope: Option<String>,
    state: Option<String>,
    nonce: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    login_hint: Option<String>,
    fault: Option<String>,
}

async fn authorize(State(state): State<Arc<Shared>>, Query(params): Query<AuthorizeParams>) -> Response {
    let config = &state.config;
    if params.client_id.as_deref() != Some(config.client_id.as_str()) {
        return page(StatusCode::BAD_REQUEST, "<h1>Unknown client</h1>");
    }
    let Some(redirect_uri) = params.redirect_uri.clone().filter(|uri| Url::parse(uri).is_ok()) else {
        return page(StatusCode::BAD_REQUEST, "<h1>redirect_uri is missing or invalid</h1>");
    };
    let fail = |error: &str| redirect(&redirect_uri, &[("error", error)], params.state.as_deref());
    if params.response_type.as_deref() != Some("code") {
        return fail("unsupported_response_type");
    }
    if !params.scope.as_deref().unwrap_or_default().split_whitespace().any(|scope| scope == "openid") {
        return fail("invalid_scope");
    }
    let challenge = params.code_challenge.clone().unwrap_or_default();
    if params.code_challenge_method.as_deref() != Some("S256") || challenge.is_empty() {
        return fail("invalid_request");
    }
    let username = params.login_hint.clone().or_else(|| config.auto_login.clone());
    let Some(username) = username else { return picker(&params) };
    let known = config.users.iter().find(|user| user.username == username).cloned();
    let Some(user) = known.or_else(|| config.any_user.then(|| FakeUser::new(&username, &username))) else {
        return fail("access_denied");
    };
    let code = random_token();
    let issued = IssuedCode {
        redirect_uri: redirect_uri.clone(),
        nonce: params.nonce.clone(),
        challenge,
        user,
        fault: params.fault.as_deref().and_then(Fault::parse),
        expires: Instant::now() + CODE_TTL,
    };
    state.codes.lock().unwrap_or_else(PoisonError::into_inner).insert(code.clone(), issued);
    redirect(&redirect_uri, &[("code", &code), ("iss", &state.issuer)], params.state.as_deref())
}

/// The sign-in page: one button per user, and the faults for manual checks.
fn picker(params: &AuthorizeParams) -> Response {
    let hidden: String = [
        ("client_id", &params.client_id),
        ("redirect_uri", &params.redirect_uri),
        ("response_type", &params.response_type),
        ("scope", &params.scope),
        ("state", &params.state),
        ("nonce", &params.nonce),
        ("code_challenge", &params.code_challenge),
        ("code_challenge_method", &params.code_challenge_method),
    ]
    .into_iter()
    .filter_map(|(name, value)| value.as_ref().map(|value| (name, value)))
    .map(|(name, value)| format!("<input type=\"hidden\" name=\"{name}\" value=\"{}\">", escape(value)))
    .collect();
    let body = format!(
        "<h1>Quest OIDC fake</h1><p>Development identity provider. Pick a user.</p>\
         <form method=\"get\" action=\"{AUTHORIZE_PATH}\">{hidden}\
         <p><button name=\"login_hint\" value=\"anna\">Sign in as anna</button> \
         <button name=\"login_hint\" value=\"ben\">Sign in as ben</button></p>\
         <p><label>Fault for testing: <select name=\"fault\"><option value=\"\">none</option>\
         <option>wrong_nonce</option><option>wrong_aud</option><option>wrong_issuer</option>\
         <option>expired</option><option>wrong_key</option></select></label></p></form>"
    );
    page(StatusCode::OK, &body)
}

#[derive(Debug, Default, Deserialize)]
struct TokenRequest {
    grant_type: Option<String>,
    code: Option<String>,
    redirect_uri: Option<String>,
    code_verifier: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
}

async fn token(State(state): State<Arc<Shared>>, headers: HeaderMap, Form(request): Form<TokenRequest>) -> Response {
    let config = &state.config;
    let (client_id, secret) = match basic_credentials(&headers) {
        Some((id, secret)) => (Some(id), Some(secret)),
        None => (request.client_id.clone(), request.client_secret.clone()),
    };
    if client_id.as_deref() != Some(config.client_id.as_str()) || secret.as_deref() != Some(&config.client_secret) {
        return oauth_error(StatusCode::UNAUTHORIZED, "invalid_client");
    }
    if request.grant_type.as_deref() != Some("authorization_code") {
        return oauth_error(StatusCode::BAD_REQUEST, "unsupported_grant_type");
    }
    let code = request.code.unwrap_or_default();
    let Some(issued) = state.codes.lock().unwrap_or_else(PoisonError::into_inner).remove(&code) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    let verifier = request.code_verifier.unwrap_or_default();
    let pkce_ok = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) == issued.challenge;
    if issued.expires < Instant::now() || request.redirect_uri.as_deref() != Some(&issued.redirect_uri) || !pkce_ok {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    }
    let id_token = id_token(&state, &issued);
    let body = json!({
        "access_token": random_token(),
        "token_type": "Bearer",
        "expires_in": config.id_token_ttl.as_secs(),
        "id_token": id_token,
        "scope": "openid profile email",
    });
    (StatusCode::OK, [(CACHE_CONTROL, HeaderValue::from_static("no-store"))], Json(body)).into_response()
}

fn id_token(state: &Shared, issued: &IssuedCode) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs());
    let fault = issued.fault;
    let ttl = state.config.id_token_ttl.as_secs();
    let exp = if fault == Some(Fault::Expired) { now.saturating_sub(600) } else { now + ttl };
    let iat = if fault == Some(Fault::Expired) { exp.saturating_sub(ttl) } else { now };
    let issuer = if fault == Some(Fault::WrongIssuer) {
        "http://127.0.0.1:1/not-the-issuer".to_owned()
    } else {
        state.issuer.clone()
    };
    let audience =
        if fault == Some(Fault::WrongAudience) { "another-client".to_owned() } else { state.config.client_id.clone() };
    let nonce = if fault == Some(Fault::WrongNonce) { Some("not-the-nonce".to_owned()) } else { issued.nonce.clone() };
    let user = &issued.user;
    let mut claims = json!({
        "iss": issuer,
        "sub": user.subject,
        "aud": audience,
        "exp": exp,
        "iat": iat,
        "email": user.email,
        "email_verified": true,
        "name": user.name,
        "preferred_username": user.username,
    });
    if let Some(nonce) = nonce {
        claims["nonce"] = json!(nonce);
    }
    let keys = jwt::keys();
    let key = if fault == Some(Fault::WrongKey) { &keys.unpublished } else { &keys.published };
    jwt::sign(&claims, key)
}

#[derive(Debug, Default, Deserialize)]
struct LogoutParams {
    post_logout_redirect_uri: Option<String>,
    state: Option<String>,
}

async fn logout(State(state): State<Arc<Shared>>, Query(params): Query<LogoutParams>) -> Response {
    state.logouts.fetch_add(1, Ordering::SeqCst);
    match params.post_logout_redirect_uri {
        Some(uri) if Url::parse(&uri).is_ok() => redirect(&uri, &[], params.state.as_deref()),
        _ => page(StatusCode::OK, "<h1>Signed out of the OIDC fake</h1>"),
    }
}

fn basic_credentials(headers: &HeaderMap) -> Option<(String, String)> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let encoded = value.strip_prefix("Basic ")?;
    let decoded = String::from_utf8(STANDARD.decode(encoded.trim()).ok()?).ok()?;
    let (id, secret) = decoded.split_once(':')?;
    let decode =
        |part: &str| url::form_urlencoded::parse(format!("v={part}").as_bytes()).next().map(|(_, v)| v.into_owned());
    Some((decode(id)?, decode(secret)?))
}

fn redirect(uri: &str, pairs: &[(&str, &str)], state: Option<&str>) -> Response {
    let Ok(mut url) = Url::parse(uri) else { return page(StatusCode::BAD_REQUEST, "<h1>Invalid redirect URI</h1>") };
    {
        let mut query = url.query_pairs_mut();
        for (name, value) in pairs {
            query.append_pair(name, value);
        }
        if let Some(state) = state {
            query.append_pair("state", state);
        }
    }
    let location = HeaderValue::from_str(url.as_str()).unwrap_or_else(|_| HeaderValue::from_static("/"));
    (StatusCode::FOUND, [(LOCATION, location)]).into_response()
}

fn oauth_error(status: StatusCode, error: &str) -> Response {
    (status, Json(json!({ "error": error }))).into_response()
}

fn page(status: StatusCode, body: &str) -> Response {
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>OIDC fake</title></head><body>{body}</body></html>"
    );
    (status, [(CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))], html).into_response()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}
