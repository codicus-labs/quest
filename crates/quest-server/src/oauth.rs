//! OAuth 2.1 authorization server for the Quest apps and MCP clients (MCP authorization spec
//! 2026-07-28). Users sign in at the OpenID Connect provider ([`crate::oidc`]), never here.
//!
//! - Resources (RFC 8707): `{origin}/v1` for the apps (only the first-party clients
//!   `quest-desktop` and `quest-android`), `{origin}/mcp` for MCP clients. A token works only for
//!   its resource.
//! - `/.well-known/oauth-protected-resource[/mcp]` (RFC 9728) names the MCP resource and its
//!   issuer; `/mcp` answers 401 with `WWW-Authenticate: Bearer resource_metadata=…`.
//! - `/.well-known/oauth-authorization-server` (RFC 8414) lists the endpoints.
//! - MCP clients identify with a Client ID Metadata Document URL (preferred by the spec) or
//!   register at `/register` (RFC 7591).
//! - `/authorize`: authorization code with PKCE S256 (required). Without a browser session it sends
//!   the user to the identity provider ([`login`]); then the consent page with "Read only" or
//!   "Read and write" (MCP), or directly the code (apps). Redirect URIs match exactly (loopback:
//!   any port); `iss` in the answer.
//! - `/token`: code exchange and refresh-token rotation (reuse revokes the grant); short-lived access
//!   tokens bound to the resource. `/revoke` (RFC 7009).
//!
//! Codes, tokens and session cookies are random and stored only as SHA-256 hashes.

mod app_link;
mod authorize;
mod cimd;
pub(crate) mod login;
mod page;
pub(crate) mod policy;
mod rate_limit;
mod token;

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::header::{AUTHORIZATION, CACHE_CONTROL, RETRY_AFTER, WWW_AUTHENTICATE};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use quest_protocol::{ApiError, ErrorCode, SCOPE_OFFLINE_ACCESS, SCOPE_READ, SCOPE_WRITE, UserId};
use reqwest::Url;
use serde_json::json;
use uuid::Uuid;

pub use app_link::{CertFingerprint, asset_links_file, fallback_page as android_app_link_page};
pub use authorize::{authorize_form, authorize_page};
pub use cimd::{ClientMetadataSource, FetchError, Fetched, HttpsMetadataSource};
pub use login::{callback, logged_out, logout};
pub use policy::ClientMetadata;
pub use rate_limit::{ClientAddress, OAuthRateLimits, Window};
pub use token::{revoke, token};

use crate::AppState;
use crate::auth::{hash_token, new_token};
use crate::oidc::IdentityProvider;
use crate::repo::{StoredClient, TokenKind};
use crate::unix_ms;
use cimd::ClientDirectory;
use policy::{Access, ClientAuth, MCP_CHALLENGE_SCOPE, MetadataError, Resource};
use rate_limit::{Decision, Endpoint, RateLimiter};

pub const PROTECTED_RESOURCE_PATH: &str = "/.well-known/oauth-protected-resource";
pub const AUTHORIZATION_SERVER_PATH: &str = "/.well-known/oauth-authorization-server";
pub use quest_protocol::{AUTHORIZE_PATH, REVOKE_PATH, TOKEN_PATH};
pub const REGISTER_PATH: &str = "/register";

/// Sign-in and the authorization server. Always on: the apps need it.
#[derive(Clone)]
pub struct AuthConfig {
    /// The origin clients reach the server at, e.g. `https://quest.example.com` (no path). It is
    /// the OAuth issuer; `{public_url}/v1` and `{public_url}/mcp` are the resources, and
    /// `{public_url}/auth/callback` the redirect URI at the identity provider. HTTPS unless
    /// loopback. `None`: the bound address (`http://127.0.0.1:port`), for local use and tests.
    pub public_url: Option<Url>,
    /// Where users sign in, e.g. [`crate::OpenIdConnect`].
    pub identity: Arc<dyn IdentityProvider>,
    pub access_token_ttl: Duration,
    pub refresh_token_ttl: Duration,
    pub code_ttl: Duration,
    /// The server's browser session after signing in at the identity provider.
    pub session_ttl: Duration,
    /// How long a sign-in at the identity provider may take.
    pub login_ttl: Duration,
    pub client_metadata: Arc<dyn ClientMetadataSource>,
    /// Per client address on `/register`, `/authorize`, `/token` and `/auth/callback`.
    pub rate_limits: OAuthRateLimits,
    /// Where the rate limits read the client address (default: the TCP peer).
    pub client_address: ClientAddress,
    /// Signing certificates of the Android app, published in `/.well-known/assetlinks.json` so
    /// Android verifies the App Link redirect `{public_url}/auth/android`. Empty: no file (404).
    pub android_cert_fingerprints: Vec<CertFingerprint>,
}

impl AuthConfig {
    /// Defaults: access tokens 1 h, refresh tokens 30 days, codes 5 min, browser sessions 12 h,
    /// sign-ins 10 min, documents over HTTPS, [`OAuthRateLimits::default`], the TCP peer as
    /// the client address, no Android certificates.
    pub fn new(public_url: Option<Url>, identity: Arc<dyn IdentityProvider>) -> Result<Self, String> {
        if let Some(url) = &public_url {
            policy::check_public_url(url)?;
        }
        Ok(AuthConfig {
            public_url,
            identity,
            access_token_ttl: Duration::from_secs(60 * 60),
            refresh_token_ttl: Duration::from_secs(30 * 24 * 60 * 60),
            code_ttl: Duration::from_secs(5 * 60),
            session_ttl: Duration::from_secs(12 * 60 * 60),
            login_ttl: Duration::from_secs(10 * 60),
            client_metadata: Arc::new(HttpsMetadataSource::new()?),
            rate_limits: OAuthRateLimits::default(),
            client_address: ClientAddress::Peer,
            android_cert_fingerprints: Vec::new(),
        })
    }

    /// [`AuthConfig::new`] with the OpenID Connect provider of `settings`.
    pub fn with_oidc(public_url: Option<Url>, settings: crate::OidcSettings) -> Result<Self, String> {
        AuthConfig::new(public_url, Arc::new(crate::OpenIdConnect::new(settings)?))
    }
}

impl fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthConfig")
            .field("public_url", &self.public_url.as_ref().map(Url::as_str))
            .field("identity", &self.identity)
            .field("access_token_ttl", &self.access_token_ttl)
            .field("refresh_token_ttl", &self.refresh_token_ttl)
            .field("code_ttl", &self.code_ttl)
            .field("session_ttl", &self.session_ttl)
            .field("rate_limits", &self.rate_limits)
            .field("client_address", &self.client_address)
            .field("android_cert_fingerprints", &self.android_cert_fingerprints)
            .finish_non_exhaustive()
    }
}

pub(crate) struct OAuthService {
    pub config: AuthConfig,
    pub public_url: Url,
    pub issuer: String,
    /// The MCP resource.
    pub resource: String,
    /// The app resource.
    pub app_resource: String,
    pub resource_metadata_url: String,
    clients: ClientDirectory,
    limiter: Mutex<RateLimiter>,
}

impl OAuthService {
    /// `bound` is the listening address, the public URL when the config names none.
    pub fn new(config: AuthConfig, bound: std::net::SocketAddr) -> Result<Self, String> {
        let public_url = match &config.public_url {
            Some(url) => url.clone(),
            None => Url::parse(&format!("http://{bound}")).map_err(|error| error.to_string())?,
        };
        policy::check_public_url(&public_url)?;
        let issuer = policy::issuer(&public_url);
        let resource = policy::canonical_resource(&public_url);
        let app_resource = policy::app_resource(&public_url);
        let resource_metadata_url = format!("{issuer}{PROTECTED_RESOURCE_PATH}{}", policy::MCP_PATH);
        let clients = ClientDirectory::new(config.client_metadata.clone());
        let limiter = Mutex::new(RateLimiter::new(config.rate_limits));
        Ok(OAuthService { config, public_url, issuer, resource, app_resource, resource_metadata_url, clients, limiter })
    }

    pub fn resource_uri(&self, resource: Resource) -> &str {
        match resource {
            Resource::App => &self.app_resource,
            Resource::Mcp => &self.resource,
        }
    }

    /// Whether cookies get `Secure` (the public URL is https).
    pub fn secure_cookies(&self) -> bool {
        self.public_url.scheme() == "https"
    }

    /// The `WWW-Authenticate` challenge of a 401 from `/mcp`.
    fn challenge(&self, invalid_token: Option<&str>) -> String {
        let mut value =
            format!("Bearer resource_metadata=\"{}\", scope=\"{MCP_CHALLENGE_SCOPE}\"", self.resource_metadata_url);
        if let Some(description) = invalid_token {
            value.push_str(&format!(", error=\"invalid_token\", error_description=\"{description}\""));
        }
        value
    }
}

/// Who an `/mcp` request acts for, with what access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct McpPrincipal {
    pub user: UserId,
    pub grant: Uuid,
    pub access: Access,
    /// The account opted in to AI features (`capture_text`).
    pub ai_enabled: bool,
}

/// An access token that is known, unexpired, of an active grant, for `resource`.
pub(crate) struct ValidToken {
    pub user: UserId,
    pub grant: Uuid,
    pub access: Access,
    /// Unix ms.
    pub expires_at: i64,
}

/// Why a bearer token was refused.
pub(crate) enum TokenRejection {
    Missing,
    Invalid(&'static str),
    Internal,
}

/// Checks the bearer token of a request to `resource` and notes the grant's use.
pub(crate) async fn validate_bearer(
    state: &AppState,
    headers: &HeaderMap,
    resource: Resource,
) -> Result<ValidToken, TokenRejection> {
    let token = bearer(headers).ok_or(TokenRejection::Missing)?;
    let stored = state.repo.find_token(&hash_token(token)).await.map_err(|error| {
        tracing::error!(%error, "looking up access token failed");
        TokenRejection::Internal
    })?;
    let now = unix_ms();
    let Some(stored) = stored.filter(|t| t.kind == TokenKind::Access && !t.grant.revoked) else {
        return Err(TokenRejection::Invalid("The access token is unknown or was revoked"));
    };
    if stored.expires_at <= now {
        return Err(TokenRejection::Invalid("The access token expired; refresh it"));
    }
    if !policy::same_resource(&stored.grant.resource, state.oauth.resource_uri(resource)) {
        return Err(TokenRejection::Invalid("The access token was issued for another resource"));
    }
    if let Err(error) = state.repo.touch_grant(stored.grant.id, now).await {
        tracing::warn!(%error, "noting grant use failed");
    }
    Ok(ValidToken {
        user: stored.grant.user,
        grant: stored.grant.id,
        access: Access::of_grant(&stored.grant.scope),
        expires_at: stored.expires_at,
    })
}

/// Checks the access token of an `/mcp` request. Otherwise the 401 with the discovery challenge.
pub(crate) async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<McpPrincipal, Box<Response>> {
    let oauth = &state.oauth;
    let unauthorized = |reason: Option<&str>| {
        let message = reason.unwrap_or("Authorization required: connect this client to Quest with OAuth");
        let mut response =
            (StatusCode::UNAUTHORIZED, Json(ApiError { code: ErrorCode::Unauthorized, message: message.to_owned() }))
                .into_response();
        if let Ok(value) = HeaderValue::from_str(&oauth.challenge(reason)) {
            response.headers_mut().insert(WWW_AUTHENTICATE, value);
        }
        Box::new(response)
    };
    let token = match validate_bearer(state, headers, Resource::Mcp).await {
        Ok(token) => token,
        Err(TokenRejection::Missing) => return Err(unauthorized(None)),
        Err(TokenRejection::Invalid(reason)) => return Err(unauthorized(Some(reason))),
        Err(TokenRejection::Internal) => return Err(Box::new(StatusCode::INTERNAL_SERVER_ERROR.into_response())),
    };
    let ai_enabled = state.ai.is_some()
        && state.repo.ai_enabled(token.user).await.map_err(|error| {
            tracing::error!(%error, "reading the AI setting failed");
            Box::new(StatusCode::INTERNAL_SERVER_ERROR.into_response())
        })?;
    Ok(McpPrincipal { user: token.user, grant: token.grant, access: token.access, ai_enabled })
}

/// The 403 of an `/mcp` call that needs `quest:write` with a read-only token (RFC 6750 §3.1).
pub(crate) fn insufficient_scope(state: &AppState) -> Response {
    let message = "This connection is read-only; reconnect it with \"Read and write\" to change things";
    let mut response =
        (StatusCode::FORBIDDEN, Json(ApiError { code: ErrorCode::InsufficientScope, message: message.to_owned() }))
            .into_response();
    let challenge = format!(
        "Bearer error=\"insufficient_scope\", scope=\"{SCOPE_WRITE}\", resource_metadata=\"{}\", \
         error_description=\"The connection is read-only\"",
        state.oauth.resource_metadata_url
    );
    if let Ok(value) = HeaderValue::from_str(&challenge) {
        response.headers_mut().insert(WWW_AUTHENTICATE, value);
    }
    response
}

/// Middleware of the public OAuth endpoints: refuses a client over its limit with 429 and
/// `Retry-After`, before the handler fetches documents or exchanges codes.
const X_FORWARDED_FOR: HeaderName = HeaderName::from_static("x-forwarded-for");

/// Which line of a header the rate limits read.
enum Line {
    /// The last: a proxy appends its own line or entry.
    Last,
    /// The only one: more than one line means the header is not the edge's alone, so it is ignored.
    Only,
}

fn header_line<'r>(request: &'r Request, name: &HeaderName, line: Line) -> Option<&'r str> {
    let mut lines = request.headers().get_all(name).iter();
    let value = match line {
        Line::Last => lines.next_back()?,
        Line::Only => {
            let first = lines.next()?;
            if lines.next().is_some() {
                return None;
            }
            first
        }
    };
    value.to_str().ok()
}

pub(crate) async fn limit_requests(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let oauth = &state.oauth;
    let Some(endpoint) = Endpoint::of(request.method(), request.uri().path()) else {
        return next.run(request).await;
    };
    // Without connection info (a router served without it) all requests share one budget.
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |ConnectInfo(addr)| addr.ip());
    let claimed = match &oauth.config.client_address {
        ClientAddress::Peer => None,
        ClientAddress::LastForwardedFor => {
            header_line(&request, &X_FORWARDED_FOR, Line::Last).and_then(rate_limit::last_forwarded_for)
        }
        ClientAddress::Header(name) => header_line(&request, name, Line::Only).and_then(rate_limit::single_address),
    };
    let client = rate_limit::client_key(peer, claimed);
    let decision = oauth.limiter.lock().unwrap_or_else(PoisonError::into_inner).check(endpoint, client, Instant::now());
    let Decision::Limited { retry_after } = decision else {
        return next.run(request).await;
    };
    tracing::warn!(?endpoint, "oauth request rate-limited");
    let seconds = retry_after.as_secs().saturating_add(u64::from(retry_after.subsec_nanos() > 0)).max(1);
    let wait = match seconds.div_ceil(60) {
        1 => "a minute".to_owned(),
        minutes => format!("{minutes} minutes"),
    };
    let mut response = match endpoint {
        Endpoint::Register => OAuthError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            format!("Too many client registrations from this address; try again in {wait}"),
        )
        .into_response(),
        Endpoint::Token => OAuthError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            format!("Too many token requests from this address; try again in {wait}"),
        )
        .into_response(),
        Endpoint::AuthorizePage => page::rate_limited("Too many requests", &wait),
        Endpoint::Consent => page::rate_limited("Too many attempts", &wait),
        Endpoint::Callback => page::rate_limited("Too many sign-in attempts", &wait),
    };
    response.headers_mut().insert(RETRY_AFTER, HeaderValue::from(seconds));
    response
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then_some(token.trim()).filter(|token| !token.is_empty())
}

pub async fn protected_resource(State(state): State<AppState>) -> Result<Response, OAuthError> {
    let oauth = &state.oauth;
    Ok(Json(json!({
        "resource": oauth.resource,
        "authorization_servers": [oauth.issuer],
        "scopes_supported": [SCOPE_READ, SCOPE_WRITE],
        "bearer_methods_supported": ["header"],
        "resource_name": "Quest",
    }))
    .into_response())
}

pub async fn authorization_server(State(state): State<AppState>) -> Result<Response, OAuthError> {
    let oauth = &state.oauth;
    let issuer = &oauth.issuer;
    Ok(Json(json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}{AUTHORIZE_PATH}"),
        "token_endpoint": format!("{issuer}{TOKEN_PATH}"),
        "registration_endpoint": format!("{issuer}{REGISTER_PATH}"),
        "revocation_endpoint": format!("{issuer}{REVOKE_PATH}"),
        "response_types_supported": ["code"],
        "response_modes_supported": ["query"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none", "client_secret_basic", "client_secret_post"],
        "revocation_endpoint_auth_methods_supported": ["none", "client_secret_basic", "client_secret_post"],
        "scopes_supported": [SCOPE_READ, SCOPE_WRITE, SCOPE_OFFLINE_ACCESS],
        "client_id_metadata_document_supported": true,
        "authorization_response_iss_parameter_supported": true,
    }))
    .into_response())
}

/// Dynamic Client Registration (RFC 7591). Open, like the account registration.
pub async fn register(
    State(state): State<AppState>,
    body: Result<Json<ClientMetadata>, JsonRejection>,
) -> Result<Response, OAuthError> {
    let invalid = |message: String| OAuthError::new(StatusCode::BAD_REQUEST, "invalid_client_metadata", message);
    let Json(metadata) = body.map_err(|rejection| invalid(rejection.body_text()))?;
    if metadata.client_id.is_some() {
        return Err(invalid("client_id is assigned by the server; leave it out".to_owned()));
    }
    let checked = policy::check_registration(&metadata).map_err(|error| match error {
        MetadataError::RedirectUri(reason) => OAuthError::new(StatusCode::BAD_REQUEST, "invalid_redirect_uri", reason),
        MetadataError::Metadata(reason) => invalid(reason),
    })?;
    let client_id = Uuid::new_v4().simple().to_string();
    let secret = (checked.auth != ClientAuth::None).then(new_token);
    let client_name = checked.client_name.unwrap_or_else(|| "Unnamed MCP client".to_owned());
    let now = unix_ms();
    let client = StoredClient {
        client_id: client_id.clone(),
        client_name: client_name.clone(),
        redirect_uris: checked.redirect_uris.clone(),
        secret_hash: secret.as_deref().map(hash_token),
    };
    state.repo.insert_client(&client, now).await.map_err(OAuthError::internal)?;
    tracing::info!(client = %client_id, auth = checked.auth.as_str(), "oauth client registered");
    let mut body = json!({
        "client_id": client_id,
        "client_id_issued_at": now / 1000,
        "client_name": client_name,
        "redirect_uris": checked.redirect_uris,
        "token_endpoint_auth_method": checked.auth.as_str(),
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
    });
    if let Some(secret) = secret {
        body["client_secret"] = json!(secret);
        body["client_secret_expires_at"] = json!(0);
    }
    Ok((StatusCode::CREATED, no_store(), Json(body)).into_response())
}

fn no_store() -> [(axum::http::HeaderName, HeaderValue); 2] {
    [
        (CACHE_CONTROL, HeaderValue::from_static("no-store")),
        (axum::http::header::PRAGMA, HeaderValue::from_static("no-cache")),
    ]
}

/// An OAuth error response: `{"error": …, "error_description": …}`.
#[derive(Debug)]
pub struct OAuthError {
    status: StatusCode,
    error: &'static str,
    description: String,
}

impl OAuthError {
    fn new(status: StatusCode, error: &'static str, description: impl Into<String>) -> Self {
        OAuthError { status, error, description: description.into() }
    }

    fn internal(error: impl fmt::Display) -> Self {
        tracing::error!(%error, "oauth request failed");
        OAuthError::new(StatusCode::INTERNAL_SERVER_ERROR, "server_error", "Internal server error; try again later")
    }
}

impl IntoResponse for OAuthError {
    fn into_response(self) -> Response {
        let body = Json(json!({ "error": self.error, "error_description": self.description }));
        (self.status, no_store(), body).into_response()
    }
}
