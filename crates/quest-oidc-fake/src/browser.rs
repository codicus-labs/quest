//! A browser stand-in for tests: follows redirects, keeps cookies, and signs a Quest app in at a
//! quest-server whose identity provider is the fake, without any UI.
//!
//! ```no_run
//! # async fn demo(server_url: &str) -> Result<(), quest_oidc_fake::LoginFlowError> {
//! let tokens = quest_oidc_fake::app_login(server_url, "anna", quest_oidc_fake::AppClient::Desktop).await?;
//! # Ok(()) }
//! ```

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use quest_protocol::{
    ANDROID_CLIENT_ID, ANDROID_REDIRECT_URI, APP_SCOPE, AUTHORIZE_PATH, DESKTOP_CLIENT_ID, DESKTOP_REDIRECT_PATH,
    OAuthErrorBody, TOKEN_PATH, TokenResponse, app_resource,
};
use reqwest::header::LOCATION;
use sha2::{Digest, Sha256};
use url::Url;

use crate::random_token;

/// Redirects followed before giving up.
const MAX_REDIRECTS: usize = 12;

/// The first-party app that signs in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppClient {
    /// `http://127.0.0.1:<port>/callback`; nothing needs to listen there.
    Desktop,
    Android,
}

impl AppClient {
    pub fn client_id(self) -> &'static str {
        match self {
            AppClient::Desktop => DESKTOP_CLIENT_ID,
            AppClient::Android => ANDROID_CLIENT_ID,
        }
    }

    pub fn redirect_uri(self) -> String {
        match self {
            AppClient::Desktop => format!("http://127.0.0.1:49152{DESKTOP_REDIRECT_PATH}"),
            AppClient::Android => ANDROID_REDIRECT_URI.to_owned(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoginFlowError {
    #[error("HTTP request to {url} failed: {message}")]
    Http { url: String, message: String },
    #[error("ended on a page instead of the app's redirect: HTTP {status} at {url}")]
    Page { status: u16, url: String, body: String },
    #[error("the authorization server redirected back with error {error}")]
    Denied { error: String, description: Option<String> },
    #[error("the redirect back to the app is invalid: {0}")]
    InvalidCallback(String),
    #[error("the token request failed with HTTP {status}: {error:?}")]
    Token { status: u16, error: Option<OAuthErrorBody> },
    #[error("more than {MAX_REDIRECTS} redirects")]
    TooManyRedirects,
}

/// Where navigation stopped.
#[derive(Debug)]
pub enum Step {
    /// A redirect to a URL starting with the target (e.g. the app's redirect URI). Not followed.
    Arrived(Url),
    /// A response that is no redirect.
    Page { status: u16, url: Url, body: String },
}

/// Cookies plus manual redirects. Requests to `/authorize` on another origin than the server's
/// (the identity provider) get the extra query parameters, e.g. `login_hint=anna` or `fault=…`.
pub struct Browser {
    http: reqwest::Client,
    server_origin: String,
    idp_params: Vec<(String, String)>,
}

impl Browser {
    pub fn new(server_url: &str) -> Browser {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .cookie_store(true)
            .build()
            .expect("a plain-http client builds");
        Browser { http, server_origin: origin(server_url), idp_params: Vec::new() }
    }

    /// Picks the user at the fake provider.
    pub fn login_as(mut self, username: &str) -> Browser {
        self.switch_user(username);
        self
    }

    /// Picks another user for the next sign-in at the fake provider (cookies stay).
    pub fn switch_user(&mut self, username: &str) {
        self.idp_params.retain(|(name, _)| name != "login_hint");
        self.idp_params.push(("login_hint".to_owned(), username.to_owned()));
    }

    /// Adds a query parameter to the provider's authorization request, e.g. `("fault", "expired")`.
    pub fn with_idp_param(mut self, name: &str, value: &str) -> Browser {
        self.idp_params.push((name.to_owned(), value.to_owned()));
        self
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// GETs `url` and follows redirects until one points at `target` or a page answers.
    pub async fn navigate(&self, url: &str, target: &str) -> Result<Step, LoginFlowError> {
        let request = self.http.get(self.with_idp_params(url)?);
        self.follow(request, url, target).await
    }

    /// Runs an app's authorization request `authorize_url` (a quest-server `/authorize` URL) to
    /// the end: signs in at the provider, confirms the "Continue as …" page if the server shows
    /// one, and returns the redirect back to the app (the first URL starting with
    /// `redirect_target`, e.g. the app's redirect URI). The redirect may carry `error=…`.
    pub async fn authorize(&self, authorize_url: &str, redirect_target: &str) -> Result<Url, LoginFlowError> {
        let page = match self.navigate(authorize_url, redirect_target).await? {
            Step::Arrived(callback) => return Ok(callback),
            Step::Page { status, url, body } => (status, url, body),
        };
        let (status, url, body) = page;
        let Some(csrf) = (status == 200).then(|| form_csrf(&body)).flatten() else {
            return Err(LoginFlowError::Page { status, url: url.into(), body });
        };
        let request = Url::parse(authorize_url).map_err(|error| http_error(authorize_url, error))?;
        let mut form: Vec<(String, String)> = request.query_pairs().into_owned().collect();
        form.push(("csrf".to_owned(), csrf));
        form.push(("decision".to_owned(), "allow".to_owned()));
        let form: Vec<(&str, &str)> = form.iter().map(|(name, value)| (name.as_str(), value.as_str())).collect();
        let action = format!("{}{AUTHORIZE_PATH}", origin(authorize_url));
        match self.submit(&action, &form, redirect_target).await? {
            Step::Arrived(callback) => Ok(callback),
            Step::Page { status, url, body } => Err(LoginFlowError::Page { status, url: url.into(), body }),
        }
    }

    /// POSTs a form (as a browser submits one) and follows like [`Browser::navigate`].
    pub async fn submit(&self, url: &str, form: &[(&str, &str)], target: &str) -> Result<Step, LoginFlowError> {
        self.follow(self.http.post(url).form(form), url, target).await
    }

    async fn follow(&self, first: reqwest::RequestBuilder, url: &str, target: &str) -> Result<Step, LoginFlowError> {
        let mut request = first;
        let mut current = Url::parse(url).map_err(|error| http_error(url, error))?;
        for _ in 0..MAX_REDIRECTS {
            let response = request.send().await.map_err(|error| http_error(current.as_str(), error))?;
            let status = response.status();
            let location = response.headers().get(LOCATION).and_then(|value| value.to_str().ok()).map(str::to_owned);
            let Some(location) = location.filter(|_| status.is_redirection()) else {
                let body = response.text().await.unwrap_or_default();
                return Ok(Step::Page { status: status.as_u16(), url: current, body });
            };
            let next = current.join(&location).map_err(|error| http_error(&location, error))?;
            if next.as_str().starts_with(target) {
                return Ok(Step::Arrived(next));
            }
            let next = self.with_idp_params(next.as_str())?;
            request = self.http.get(next.clone());
            current = next;
        }
        Err(LoginFlowError::TooManyRedirects)
    }

    fn with_idp_params(&self, url: &str) -> Result<Url, LoginFlowError> {
        let mut url = Url::parse(url).map_err(|error| http_error(url, error))?;
        let at_provider = origin(url.as_str()) != self.server_origin && url.path().ends_with(AUTHORIZE_PATH);
        if at_provider && !self.idp_params.is_empty() {
            let mut query = url.query_pairs_mut();
            for (name, value) in &self.idp_params {
                query.append_pair(name, value);
            }
        }
        Ok(url)
    }
}

/// One app authorization request: PKCE verifier, `state` and the URL to open.
#[derive(Clone, Debug)]
pub struct AppAuthorization {
    pub server_url: String,
    pub client: AppClient,
    pub redirect_uri: String,
    pub state: String,
    pub verifier: String,
    pub url: String,
}

impl AppAuthorization {
    pub fn new(server_url: &str, client: AppClient) -> AppAuthorization {
        let server_url = origin(server_url);
        let verifier = random_token();
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let state = random_token();
        let redirect_uri = client.redirect_uri();
        let mut url = Url::parse(&format!("{server_url}{AUTHORIZE_PATH}")).expect("the server URL is an origin");
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", client.client_id())
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("state", &state)
            .append_pair("scope", APP_SCOPE)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("resource", &app_resource(&server_url));
        AppAuthorization { server_url, client, redirect_uri, state, verifier, url: url.into() }
    }

    /// Checks the redirect back to the app (`state`, `iss`) and exchanges its code.
    pub async fn finish(&self, http: &reqwest::Client, callback: &Url) -> Result<TokenResponse, LoginFlowError> {
        let query: std::collections::HashMap<String, String> = callback.query_pairs().into_owned().collect();
        if let Some(error) = query.get("error") {
            return Err(LoginFlowError::Denied {
                error: error.clone(),
                description: query.get("error_description").cloned(),
            });
        }
        if query.get("state") != Some(&self.state) {
            return Err(LoginFlowError::InvalidCallback("state does not match".to_owned()));
        }
        if query.get("iss") != Some(&self.server_url) {
            return Err(LoginFlowError::InvalidCallback(format!("iss is {:?}", query.get("iss"))));
        }
        let code = query.get("code").ok_or_else(|| LoginFlowError::InvalidCallback("no code".to_owned()))?;
        let token_url = format!("{}{TOKEN_PATH}", self.server_url);
        let resource = app_resource(&self.server_url);
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("code_verifier", self.verifier.as_str()),
            ("client_id", self.client.client_id()),
            ("resource", resource.as_str()),
        ];
        let response = http.post(&token_url).form(&form).send().await.map_err(|error| http_error(&token_url, error))?;
        let status = response.status();
        if !status.is_success() {
            return Err(LoginFlowError::Token { status: status.as_u16(), error: response.json().await.ok() });
        }
        response.json().await.map_err(|error| http_error(&token_url, error))
    }
}

/// Signs `username` in to the app `client` at the quest-server at `server_url`, whose identity
/// provider is a [`crate::FakeProvider`]: authorize, provider sign-in, callback, "Continue as …"
/// (Android always, desktop with an existing session), code exchange.
/// Each call starts without cookies, so it never reuses an earlier browser session.
pub async fn app_login(server_url: &str, username: &str, client: AppClient) -> Result<TokenResponse, LoginFlowError> {
    let browser = Browser::new(server_url).login_as(username);
    let authorization = AppAuthorization::new(server_url, client);
    let callback = browser.authorize(&authorization.url, &authorization.redirect_uri).await?;
    authorization.finish(browser.http(), &callback).await
}

/// The CSRF field of a server form page ("Continue as …" or consent), if it is one.
fn form_csrf(body: &str) -> Option<String> {
    let marker = "name=\"csrf\" value=\"";
    let start = body.find(marker)? + marker.len();
    body[start..].split('"').next().map(str::to_owned)
}

/// `http://host:port` of a URL, without a trailing slash.
fn origin(url: &str) -> String {
    Url::parse(url).map_or_else(|_| url.trim_end_matches('/').to_owned(), |url| url.origin().ascii_serialization())
}

fn http_error(url: &str, error: impl std::fmt::Display) -> LoginFlowError {
    LoginFlowError::Http { url: url.to_owned(), message: error.to_string() }
}
