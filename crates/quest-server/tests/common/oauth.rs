//! An OAuth client for tests: registration, PKCE, the browser part (sign-in at the fake identity
//! provider and the consent form, as a browser would post it) and the token endpoint.

use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use quest_oidc_fake::{Browser, Step};
use reqwest::redirect::Policy;
use reqwest::{StatusCode, Url};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::TestServer;

pub const REDIRECT_URI: &str = "http://127.0.0.1/callback";

pub fn http() -> reqwest::Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder().redirect(Policy::none()).build().unwrap()
}

/// A verifier and its S256 challenge.
pub fn pkce() -> (String, String) {
    let verifier = format!("verifier-{}-{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

#[derive(Clone, Debug)]
pub struct Tokens {
    pub access: String,
    pub refresh: String,
    pub scope: String,
}

pub struct OAuthClient<'a> {
    pub server: &'a TestServer,
    pub client_id: String,
    pub redirect_uri: String,
}

impl<'a> OAuthClient<'a> {
    /// Registers a public client (DCR) with [`REDIRECT_URI`].
    pub async fn register(server: &'a TestServer) -> OAuthClient<'a> {
        let body = json!({
            "client_name": "Test client",
            "redirect_uris": [REDIRECT_URI],
            "token_endpoint_auth_method": "none",
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
        });
        let response = http().post(server.url("/register")).json(&body).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let registered: Value = response.json().await.unwrap();
        OAuthClient {
            server,
            client_id: registered["client_id"].as_str().unwrap().to_owned(),
            redirect_uri: REDIRECT_URI.to_owned(),
        }
    }

    pub fn resource(&self) -> String {
        self.server.url("/mcp")
    }

    pub fn authorize_params(&self, challenge: &str) -> Vec<(&'static str, String)> {
        vec![
            ("response_type", "code".into()),
            ("client_id", self.client_id.clone()),
            ("redirect_uri", self.redirect_uri.clone()),
            ("state", "xyz".into()),
            ("scope", "quest:write".into()),
            ("code_challenge", challenge.into()),
            ("code_challenge_method", "S256".into()),
            ("resource", self.resource()),
        ]
    }

    pub fn authorize_url(&self, params: &[(&'static str, String)]) -> String {
        let mut url = Url::parse(&self.server.url("/authorize")).unwrap();
        url.query_pairs_mut().extend_pairs(params);
        url.into()
    }

    /// A fresh browser that signs in as `user` at the identity provider.
    pub fn browser(&self, user: &str) -> Browser {
        Browser::new(&self.server.server.http_url()).login_as(user)
    }

    /// Opens `/authorize` in `browser`: signs in if needed and stops at the consent page or at
    /// the redirect back to the client.
    pub async fn open(&self, browser: &Browser, params: &[(&'static str, String)]) -> Step {
        browser.navigate(&self.authorize_url(params), &self.redirect_uri).await.unwrap()
    }

    /// Submits the consent (or "Continue as") form shown in `page` with `decision` and `access`.
    pub async fn decide(
        &self,
        browser: &Browser,
        page: &Step,
        params: &[(&'static str, String)],
        decision: &str,
        access: Option<&str>,
    ) -> Step {
        let csrf = csrf(page);
        let mut form: Vec<(&str, &str)> = params.iter().map(|(name, value)| (*name, value.as_str())).collect();
        form.extend([("csrf", csrf.as_str()), ("decision", decision)]);
        if let Some(access) = access {
            form.push(("access", access));
        }
        browser.submit(&self.server.url("/authorize"), &form, &self.redirect_uri).await.unwrap()
    }

    /// Signs in as `user` in a fresh browser and answers the consent with `decision`.
    pub async fn consent(&self, params: &[(&'static str, String)], user: &str, decision: &str) -> Step {
        let browser = self.browser(user);
        let page = self.open(&browser, params).await;
        if !matches!(page, Step::Page { status: 200, .. }) {
            return page;
        }
        self.decide(&browser, &page, params, decision, None).await
    }

    /// Signs in as `user`, allows with `access` (or the preselected one) and returns the code.
    pub async fn code_with(&self, user: &str, challenge: &str, access: Option<&str>) -> String {
        let params = self.authorize_params(challenge);
        let browser = self.browser(user);
        let page = self.open(&browser, &params).await;
        let step = self.decide(&browser, &page, &params, "allow", access).await;
        let query = arrived_query(&step);
        assert_eq!(query.get("state").map(String::as_str), Some("xyz"));
        assert_eq!(query.get("iss"), Some(&self.server.server.http_url()));
        query.get("code").cloned().unwrap_or_else(|| panic!("no code in {query:?}"))
    }

    pub async fn code(&self, user: &str, challenge: &str) -> String {
        self.code_with(user, challenge, None).await
    }

    pub async fn token(&self, form: &[(&str, &str)]) -> reqwest::Response {
        let mut form: Vec<(&str, &str)> = form.to_vec();
        form.push(("client_id", &self.client_id));
        http().post(self.server.url("/token")).form(&form).send().await.unwrap()
    }

    pub async fn exchange(&self, code: &str, verifier: &str) -> reqwest::Response {
        let resource = self.resource();
        self.token(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &self.redirect_uri),
            ("code_verifier", verifier),
            ("resource", &resource),
        ])
        .await
    }

    pub async fn refresh(&self, refresh_token: &str) -> reqwest::Response {
        self.token(&[("grant_type", "refresh_token"), ("refresh_token", refresh_token)]).await
    }

    /// The whole flow for `user` with read and write access: code, exchange, tokens.
    pub async fn tokens(&self, user: &str) -> Tokens {
        self.tokens_with(user, None).await
    }

    pub async fn tokens_with(&self, user: &str, access: Option<&str>) -> Tokens {
        let (verifier, challenge) = pkce();
        let code = self.code_with(user, &challenge, access).await;
        tokens(self.exchange(&code, &verifier).await).await
    }
}

/// The CSRF field of a consent or "Continue as" page.
pub fn csrf(page: &Step) -> String {
    let Step::Page { status: 200, body, .. } = page else { panic!("expected a form page, got {page:?}") };
    let marker = "name=\"csrf\" value=\"";
    let start = body.find(marker).unwrap_or_else(|| panic!("no csrf field in {body}")) + marker.len();
    body[start..].split('"').next().unwrap().to_owned()
}

pub async fn tokens(response: reqwest::Response) -> Tokens {
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["token_type"], "Bearer");
    Tokens {
        access: body["access_token"].as_str().unwrap().to_owned(),
        refresh: body["refresh_token"].as_str().unwrap().to_owned(),
        scope: body["scope"].as_str().unwrap_or_default().to_owned(),
    }
}

/// The query of a 303 redirect's `Location`.
pub fn redirect_query(response: &reqwest::Response) -> HashMap<String, String> {
    assert_eq!(response.status(), StatusCode::SEE_OTHER, "expected a redirect");
    let location = response.headers()["location"].to_str().unwrap();
    Url::parse(location).unwrap().query_pairs().into_owned().collect()
}

/// The query of the redirect back to the client where navigation stopped.
pub fn arrived_query(step: &Step) -> HashMap<String, String> {
    let Step::Arrived(url) = step else { panic!("expected the redirect back to the client, got {step:?}") };
    url.query_pairs().into_owned().collect()
}

/// The status of the page where navigation stopped.
pub fn page_status(step: &Step) -> u16 {
    let Step::Page { status, .. } = step else { panic!("expected a page, got {step:?}") };
    *status
}

/// The `error` of an OAuth error body.
pub async fn oauth_error(response: reqwest::Response) -> (StatusCode, String) {
    let status = response.status();
    let body: Value = response.json().await.unwrap();
    (status, body["error"].as_str().unwrap_or_default().to_owned())
}
