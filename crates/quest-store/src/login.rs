//! Signing in as a native app (RFC 8252): the authorization request with `state` and PKCE S256, and
//! checking the redirect it comes back with. Pure; the code exchange is in `oauth`.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use quest_protocol::{
    ANDROID_CLIENT_ID, ANDROID_REDIRECT_URI, APP_SCOPE, AUTHORIZE_PATH, DESKTOP_CLIENT_ID, DESKTOP_REDIRECT_PATH,
    LOGOUT_PATH, android_app_link_redirect, app_resource,
};
use sha2::{Digest, Sha256};
use url::Url;

use crate::error::StoreError;
use crate::layout::origin;
use crate::remote::invalid_url;

/// Which first-party app signs in; decides the client id and the redirect URI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppClient {
    /// The redirect goes to a listener on `http://127.0.0.1:<redirect_port>` (loopback).
    Desktop { redirect_port: u16 },
    /// The redirect goes to the app's private-use URI scheme.
    Android,
    /// The redirect goes to the server's own `https` origin + `ANDROID_APP_LINK_PATH`, a verified
    /// App Link that only the app signed with the release key receives (RFC 8252 §7.2).
    AndroidAppLink,
}

impl AppClient {
    /// The Android client for a sign-in at `server_url`: the App Link when the server is the one
    /// the build's App Link is verified for (`app_link_origin`, e.g. `https://quest.example.com`;
    /// `https` only), else the private-use scheme. An unparsable `app_link_origin` counts as none.
    pub fn android(server_url: &Url, app_link_origin: Option<&str>) -> AppClient {
        let app_link_origin = app_link_origin.and_then(|o| Url::parse(o).ok()).map(|o| origin(&o));
        let is_app_link_server = server_url.scheme() == "https" && app_link_origin == Some(origin(server_url));
        if is_app_link_server { AppClient::AndroidAppLink } else { AppClient::Android }
    }

    pub fn client_id(self) -> &'static str {
        match self {
            AppClient::Desktop { .. } => DESKTOP_CLIENT_ID,
            AppClient::Android | AppClient::AndroidAppLink => ANDROID_CLIENT_ID,
        }
    }

    /// Where the browser returns with the answer to a sign-in at `server_url`.
    pub fn redirect_uri(self, server_url: &Url) -> String {
        match self {
            AppClient::Desktop { redirect_port } => format!("http://127.0.0.1:{redirect_port}{DESKTOP_REDIRECT_PATH}"),
            AppClient::Android => ANDROID_REDIRECT_URI.to_owned(),
            AppClient::AndroidAppLink => android_app_link_redirect(&origin(server_url)),
        }
    }

    /// The page to open in the browser after `Profiles::sign_out`, to end the browser session
    /// at the server and the identity provider. Android comes back to the app through its
    /// private-use redirect (closing the Custom Tab), also with an App Link: that return carries
    /// no secret, and a browser may not leave for an App Link after redirects without a tap.
    /// The desktop leaves the browser on "Signed out".
    pub fn logout_url(self, server_url: &Url) -> Result<Url, StoreError> {
        let mut url = server_url.join(LOGOUT_PATH).map_err(|e| invalid_url(server_url, e))?;
        if matches!(self, AppClient::Android | AppClient::AndroidAppLink) {
            url.query_pairs_mut().append_pair("return_to", ANDROID_REDIRECT_URI);
        }
        Ok(url)
    }
}

/// One sign-in attempt: open `authorize_url` in the system browser, then hand the redirect to
/// `LoginRequest::finish`. Holds the secrets that bind the answer to this attempt; keep it in
/// memory only.
pub struct LoginRequest {
    server_url: Url,
    client: AppClient,
    state: String,
    code_verifier: String,
    authorize_url: Url,
}

impl LoginRequest {
    /// Fresh `state` and PKCE verifier (32 random bytes each). Fails on URLs the app cannot use.
    pub fn new(server_url: &Url, client: AppClient) -> Result<LoginRequest, StoreError> {
        LoginRequest::with_secrets(server_url, client, random_secret(), random_secret())
    }

    pub(crate) fn with_secrets(
        server_url: &Url,
        client: AppClient,
        state: String,
        code_verifier: String,
    ) -> Result<LoginRequest, StoreError> {
        if !matches!(server_url.scheme(), "http" | "https") || server_url.host().is_none() {
            return Err(invalid_url(server_url, "not an http:// or https:// address"));
        }
        let server_url: Url = origin(server_url).parse().map_err(|e| invalid_url(server_url, e))?;
        let mut authorize_url = server_url.join(AUTHORIZE_PATH).map_err(|e| invalid_url(&server_url, e))?;
        authorize_url
            .query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", client.client_id())
            .append_pair("redirect_uri", &client.redirect_uri(&server_url))
            .append_pair("scope", APP_SCOPE)
            .append_pair("state", &state)
            .append_pair("code_challenge", &code_challenge(&code_verifier))
            .append_pair("code_challenge_method", "S256")
            .append_pair("resource", &app_resource(&origin(&server_url)));
        Ok(LoginRequest { server_url, client, state, code_verifier, authorize_url })
    }

    /// The page to open in the system browser.
    pub fn authorize_url(&self) -> &Url {
        &self.authorize_url
    }

    /// The server's origin.
    pub fn server_url(&self) -> &Url {
        &self.server_url
    }

    pub fn client(&self) -> AppClient {
        self.client
    }

    pub(crate) fn code_verifier(&self) -> &str {
        &self.code_verifier
    }

    /// The authorization code from the redirect, after checking that it answers this attempt
    /// (`state`) and comes from this server (`iss`, RFC 9207).
    pub(crate) fn authorization_code(&self, callback: &Url) -> Result<String, StoreError> {
        let expected: Url = self.client.redirect_uri(&self.server_url).parse().map_err(|e| failed(format!("{e}")))?;
        let same_target = callback.scheme() == expected.scheme()
            && callback.host_str() == expected.host_str()
            && callback.port() == expected.port()
            && callback.path() == expected.path();
        if !same_target {
            return Err(failed("the browser returned to an unexpected address"));
        }
        let param =
            |name: &str| callback.query_pairs().find(|(key, _)| key == name).map(|(_, value)| value.into_owned());
        if param("state").as_deref() != Some(self.state.as_str()) {
            return Err(failed("the answer does not belong to this sign-in"));
        }
        // The server announces `iss` (RFC 9207 §2.4), so an answer without it is refused too.
        if param("iss").is_none_or(|iss| iss.trim_end_matches('/') != origin(&self.server_url)) {
            return Err(failed("the answer comes from another server"));
        }
        if let Some(error) = param("error") {
            if error == "access_denied" {
                return Err(StoreError::SignInCancelled);
            }
            let description = param("error_description").map(|d| format!(" ({d})")).unwrap_or_default();
            return Err(failed(format!("the server refused: {error}{description}")));
        }
        param("code").filter(|code| !code.is_empty()).ok_or_else(|| failed("the answer has no authorization code"))
    }
}

/// PKCE S256 (RFC 7636): base64url of the verifier's SHA-256.
fn code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// 32 random bytes as base64url: 43 characters, valid as PKCE verifier and as `state`.
fn random_secret() -> String {
    URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>())
}

fn failed(reason: impl Into<String>) -> StoreError {
    StoreError::SignInFailed { reason: reason.into() }
}

#[cfg(test)]
#[path = "login_tests.rs"]
mod tests;
