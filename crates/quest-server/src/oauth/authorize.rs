//! `/authorize`: checks the request, signs the user in at the identity provider if the browser has
//! no session ([`super::login`]), then:
//!
//! - MCP clients: the consent page (GET) with "Read only" / "Read and write"; on approval (POST)
//!   the grant and a one-time code.
//! - The Quest apps (first party): a "Continue as …" page, so a stray request cannot get a code
//!   silently and the user can switch accounts. Right after a fresh sign-in the code comes without
//!   it for redirects no other app can claim (desktop loopback, Android's verified App Link);
//!   Android's private-use scheme can be registered by any app, so it always sees the page.
//!
//! Until the client and its redirect URI are known, errors show a page (never redirect to an
//! unchecked URI); after that they go back to the client as `error=…` with `state` and `iss`.

use axum::Form;
use axum::extract::{Query, RawQuery, State};
use axum::http::header::{CACHE_CONTROL, LOCATION};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use reqwest::Url;
use serde::Deserialize;
use uuid::Uuid;

use super::login::{self, Session};
use super::page::{self, Consent};
use super::policy::{self, Access, ClientInfo, Resource};
use super::{OAuthService, policy::metadata_document_url};
use crate::AppState;
use crate::auth::{hash_token, new_token};
use crate::repo::{NewCode, NewGrant};
use crate::unix_ms;

/// The authorization request's parameters (query on GET, hidden fields on POST).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct AuthorizeParams {
    response_type: Option<String>,
    client_id: Option<String>,
    redirect_uri: Option<String>,
    state: Option<String>,
    scope: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    resource: Option<String>,
}

impl AuthorizeParams {
    fn fields(&self) -> [(&'static str, Option<&str>); 8] {
        [
            ("response_type", self.response_type.as_deref()),
            ("client_id", self.client_id.as_deref()),
            ("redirect_uri", self.redirect_uri.as_deref()),
            ("state", self.state.as_deref()),
            ("scope", self.scope.as_deref()),
            ("code_challenge", self.code_challenge.as_deref()),
            ("code_challenge_method", self.code_challenge_method.as_deref()),
            ("resource", self.resource.as_deref()),
        ]
    }

    /// The request as a query string, to continue with after the sign-in.
    fn to_query(&self) -> String {
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        for (name, value) in self.fields() {
            if let Some(value) = value {
                query.append_pair(name, value);
            }
        }
        query.finish()
    }

    fn from_query(query: &str) -> Option<AuthorizeParams> {
        let uri: Uri = format!("/?{query}").parse().ok()?;
        Query::<AuthorizeParams>::try_from_uri(&uri).ok().map(|Query(params)| params)
    }
}

#[derive(Debug, Deserialize)]
pub struct ConsentForm {
    #[serde(flatten)]
    params: AuthorizeParams,
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    decision: String,
    /// `quest:read` or `quest:write` (MCP clients only).
    #[serde(default)]
    access: Option<String>,
}

/// A request that passed every check.
struct Checked {
    client: ClientInfo,
    redirect_uri: String,
    state: Option<String>,
    /// What the client asked for; `None` if it named no Quest scope.
    requested: Option<Access>,
    resource: Resource,
    code_challenge: String,
}

pub async fn authorize_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<AuthorizeParams>,
    RawQuery(query): RawQuery,
) -> Response {
    let checked = match check(&state, &params).await {
        Ok(checked) => checked,
        Err(response) => return *response,
    };
    let session = match login::current_session(&state, &headers).await {
        Ok(Some(session)) => session,
        Ok(None) => return login::start_login(&state, &query.unwrap_or_default(), false).await,
        Err(response) => return *response,
    };
    if checked.client.first_party {
        return page::continue_as(&display_name(&session), &session.csrf(), &hidden(&checked, &params));
    }
    consent_page(&checked, &params, &session)
}

pub async fn authorize_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<ConsentForm>,
) -> Response {
    let oauth = &state.oauth;
    let checked = match check(&state, &form.params).await {
        Ok(checked) => checked,
        Err(response) => return *response,
    };
    let session = match login::current_session(&state, &headers).await {
        Ok(Some(session)) => session,
        Ok(None) => return login::start_login(&state, &form.params.to_query(), false).await,
        Err(response) => return *response,
    };
    if form.csrf != session.csrf() {
        return page::message(
            StatusCode::FORBIDDEN,
            "This form cannot be used",
            "It belongs to another sign-in. Start connecting again from your client.",
        );
    }
    match form.decision.as_str() {
        "allow" => {}
        "switch" => {
            if let Err(response) = login::end_session(&state, &headers).await {
                return *response;
            }
            let mut response = login::start_login(&state, &form.params.to_query(), true).await;
            login::append_cookie(&mut response, &login::clear_session_cookie(oauth));
            return response;
        }
        _ => {
            return redirect_error(
                oauth,
                &checked.redirect_uri,
                checked.state.as_deref(),
                "access_denied",
                "The user declined",
            );
        }
    }
    let access = if checked.client.first_party {
        Access::Write
    } else {
        form.access.as_deref().and_then(Access::parse).unwrap_or(default_access(&checked))
    };
    issue_code(&state, &checked, &session, access).await
}

/// After the sign-in at the identity provider: continue the stored `/authorize` request.
pub(crate) async fn after_login(state: &AppState, authorize_query: &str, session: &Session) -> Response {
    let Some(params) = AuthorizeParams::from_query(authorize_query) else {
        return page::error(StatusCode::BAD_REQUEST, "The authorization request could not be restored.");
    };
    let checked = match check(state, &params).await {
        Ok(checked) => checked,
        Err(response) => return *response,
    };
    if checked.client.first_party {
        if policy::claimable_redirect(&checked.redirect_uri) {
            return page::continue_as(&display_name(session), &session.csrf(), &hidden(&checked, &params));
        }
        return issue_code(state, &checked, session, Access::Write).await;
    }
    consent_page(&checked, &params, session)
}

/// The identity provider refused or the user cancelled there: tell the client, if it is known.
pub(crate) async fn declined(state: &AppState, authorize_query: &str) -> Response {
    let Some(params) = AuthorizeParams::from_query(authorize_query) else {
        return page::message(StatusCode::UNAUTHORIZED, "Sign-in cancelled", "You can close this window.");
    };
    match check(state, &params).await {
        Ok(checked) => redirect_error(
            &state.oauth,
            &checked.redirect_uri,
            checked.state.as_deref(),
            "access_denied",
            "The sign-in at the identity provider was cancelled or refused",
        ),
        Err(response) => *response,
    }
}

async fn issue_code(state: &AppState, checked: &Checked, session: &Session, access: Access) -> Response {
    let oauth = &state.oauth;
    let user = session.account.user;
    let code = new_token();
    let code_hash = hash_token(&code);
    let grant = NewGrant {
        id: Uuid::now_v7(),
        user,
        client_id: &checked.client.client_id,
        client_name: &checked.client.client_name,
        redirect_uri: &checked.redirect_uri,
        scope: access.scope(),
        resource: oauth.resource_uri(checked.resource),
    };
    let now = unix_ms();
    let expires_at = now + i64::try_from(oauth.config.code_ttl.as_millis()).unwrap_or(i64::MAX);
    let new_code = NewCode { code_hash: &code_hash, code_challenge: &checked.code_challenge, expires_at };
    if let Err(error) = state.repo.create_grant_with_code(&grant, &new_code, now).await {
        return internal(error);
    }
    tracing::info!(
        user = %user.0, grant = %grant.id, client = %checked.client.client_id, scope = access.scope(),
        "oauth authorization given"
    );
    redirect(oauth, &checked.redirect_uri, checked.state.as_deref(), &[("code", &code)])
}

/// Preselected on the consent page and used if the form names none: what the client asked for,
/// else read only (least privilege).
fn default_access(checked: &Checked) -> Access {
    checked.requested.unwrap_or(Access::Read)
}

async fn check(state: &AppState, params: &AuthorizeParams) -> Result<Checked, Box<Response>> {
    let oauth = &state.oauth;
    let Some(client_id) = params.client_id.as_deref().filter(|id| !id.is_empty()) else {
        return Err(Box::new(page::error(
            StatusCode::BAD_REQUEST,
            "The request names no client (client_id is missing).",
        )));
    };
    let client = client(state, oauth, client_id).await?;
    let redirect_uri = match params.redirect_uri.as_deref() {
        Some(uri) if policy::redirect_allowed(&client.redirect_uris, uri) => uri.to_owned(),
        Some(_) => {
            return Err(Box::new(page::error(
                StatusCode::BAD_REQUEST,
                "The redirect URI is not registered for this client, so Quest will not send you there.",
            )));
        }
        None if client.redirect_uris.len() == 1 && !client.first_party => client.redirect_uris[0].clone(),
        None => return Err(Box::new(page::error(StatusCode::BAD_REQUEST, "The request names no redirect URI."))),
    };
    let state_param = params.state.clone();
    let fail = |error: &str, description: &str| {
        Box::new(redirect_error(oauth, &redirect_uri, state_param.as_deref(), error, description))
    };
    if params.response_type.as_deref() != Some("code") {
        return Err(fail("unsupported_response_type", "Only response_type=code is supported"));
    }
    let code_challenge = params.code_challenge.clone().unwrap_or_default();
    if params.code_challenge_method.as_deref() != Some("S256") || !policy::valid_challenge(&code_challenge) {
        return Err(fail("invalid_request", "PKCE is required: send code_challenge with code_challenge_method=S256"));
    }
    let requested =
        policy::requested_access(params.scope.as_deref()).map_err(|reason| fail("invalid_scope", &reason))?;
    let resource = policy::requested_resource(
        params.resource.as_deref(),
        client.first_party,
        &oauth.app_resource,
        &oauth.resource,
    )
    .map_err(|reason| fail("invalid_target", reason))?;
    Ok(Checked { client, redirect_uri, state: state_param, requested, resource, code_challenge })
}

async fn client(state: &AppState, oauth: &OAuthService, client_id: &str) -> Result<ClientInfo, Box<Response>> {
    if let Some(client) = policy::first_party_client(client_id, &oauth.issuer) {
        return Ok(client);
    }
    if let Some(url) = metadata_document_url(client_id) {
        return oauth
            .clients
            .client(&url)
            .await
            .map_err(|reason| Box::new(page::error(StatusCode::BAD_REQUEST, &reason)));
    }
    let stored = state.repo.find_client(client_id).await.map_err(|error| Box::new(internal(error)))?;
    let stored = stored.ok_or_else(|| {
        Box::new(page::error(
            StatusCode::BAD_REQUEST,
            "This client is not registered with Quest; reconnect it from the client.",
        ))
    })?;
    Ok(ClientInfo {
        client_id: stored.client_id,
        client_name: stored.client_name,
        redirect_uris: stored.redirect_uris,
        secret_hash: stored.secret_hash,
        first_party: false,
    })
}

/// The request's parameters as hidden form fields (client id and redirect URI as checked).
fn hidden<'a>(checked: &'a Checked, params: &'a AuthorizeParams) -> Vec<(&'static str, &'a str)> {
    params
        .fields()
        .into_iter()
        .map(|(name, value)| match name {
            "client_id" => (name, Some(checked.client.client_id.as_str())),
            "redirect_uri" => (name, Some(checked.redirect_uri.as_str())),
            _ => (name, value),
        })
        .filter_map(|(name, value)| value.map(|value| (name, value)))
        .collect()
}

fn display_name(session: &Session) -> String {
    let account = &session.account;
    match (&account.name, &account.email) {
        (Some(name), Some(email)) => format!("{name} ({email})"),
        (Some(name), None) => name.clone(),
        (None, Some(email)) => email.clone(),
        (None, None) => "your account".to_owned(),
    }
}

fn consent_page(checked: &Checked, params: &AuthorizeParams, session: &Session) -> Response {
    let redirect_host = Url::parse(&checked.redirect_uri).ok().and_then(|url| url.host_str().map(str::to_owned));
    page::consent(&Consent {
        client_name: &checked.client.client_name,
        redirect_host: redirect_host.as_deref().unwrap_or_default(),
        loopback_only: policy::loopback_only(&checked.client.redirect_uris),
        hidden: &hidden(checked, params),
        account: &display_name(session),
        csrf: &session.csrf(),
        access: default_access(checked),
    })
}

fn redirect_error(
    oauth: &OAuthService,
    redirect_uri: &str,
    state: Option<&str>,
    error: &str,
    description: &str,
) -> Response {
    redirect(oauth, redirect_uri, state, &[("error", error), ("error_description", description)])
}

/// 303 to the client's redirect URI with `pairs`, `state` and `iss` (RFC 9207) added to its query.
fn redirect(oauth: &OAuthService, redirect_uri: &str, state: Option<&str>, pairs: &[(&str, &str)]) -> Response {
    let Ok(mut url) = Url::parse(redirect_uri) else {
        return page::error(StatusCode::BAD_REQUEST, "The redirect URI is not a valid URL.");
    };
    {
        let mut query = url.query_pairs_mut();
        for (name, value) in pairs {
            query.append_pair(name, value);
        }
        if let Some(state) = state {
            query.append_pair("state", state);
        }
        query.append_pair("iss", &oauth.issuer);
    }
    let Ok(location) = HeaderValue::from_str(url.as_str()) else {
        return page::error(StatusCode::BAD_REQUEST, "The redirect URI is not a valid URL.");
    };
    (StatusCode::SEE_OTHER, [(LOCATION, location), (CACHE_CONTROL, HeaderValue::from_static("no-store"))])
        .into_response()
}

fn internal(error: impl std::fmt::Display) -> Response {
    tracing::error!(%error, "authorization request failed");
    page::error(StatusCode::INTERNAL_SERVER_ERROR, "Quest could not process this request; try again later.")
}
