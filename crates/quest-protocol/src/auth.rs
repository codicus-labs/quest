//! Sign-in and the account, for the apps.
//!
//! The apps sign in with OAuth 2.1 at the server's own authorization server (RFC 8252 native apps:
//! authorization code with PKCE S256), never with a password. The server in turn signs the user in
//! at the configured OpenID Connect provider; the account is identified by `(issuer, subject)`.
//!
//! 1. The app opens `{origin}`[`AUTHORIZE_PATH`] in the system browser with `client_id`
//!    ([`DESKTOP_CLIENT_ID`] or [`ANDROID_CLIENT_ID`]), `redirect_uri`, `state`, `code_challenge`
//!    (S256), `scope` [`APP_SCOPE`] and `resource` [`app_resource`].
//! 2. The redirect brings `code`, `state` and `iss` (RFC 9207; must equal the origin). The app
//!    posts `grant_type=authorization_code` with `code`, `redirect_uri`, `code_verifier`,
//!    `client_id` to [`TOKEN_PATH`] and gets a [`TokenResponse`] (errors: [`OAuthErrorBody`]).
//! 3. App routes ([`SYNC_PATH`](crate::SYNC_PATH), [`ACCOUNT_PATH`], `/v1/ai/*`) take
//!    `Authorization: Bearer <access_token>`. A 401 means: refresh with
//!    `grant_type=refresh_token` (the refresh token rotates; store the new pair before using it).
//!    `invalid_grant` on refresh means: sign in again.
//! 4. Signing out posts `token=<refresh_token>&client_id=…` to [`REVOKE_PATH`] (RFC 7009), then
//!    may open [`LOGOUT_PATH`] in the browser to end the server's browser session and the one at
//!    the identity provider.

use serde::{Deserialize, Serialize};

use crate::UserId;

pub const AUTHORIZE_PATH: &str = "/authorize";
pub const TOKEN_PATH: &str = "/token";
pub const REVOKE_PATH: &str = "/revoke";
/// The server's redirect URI at the identity provider.
pub const OIDC_CALLBACK_PATH: &str = "/auth/callback";
/// Ends the server's browser session and, if the provider supports it, the provider's session.
/// Optional `?return_to=<redirect URI of a first-party client>` to come back to the app.
pub const LOGOUT_PATH: &str = "/auth/logout";
/// "Signed out of Quest" page after [`LOGOUT_PATH`].
pub const LOGGED_OUT_PATH: &str = "/auth/logged-out";
/// `GET` returns [`AccountInfo`].
pub const ACCOUNT_PATH: &str = "/v1/account";
/// `PUT` with [`AccountSettingsUpdate`] returns the updated [`AccountInfo`].
pub const ACCOUNT_SETTINGS_PATH: &str = "/v1/account/settings";

/// Resource (RFC 8707 audience) path of the app routes; see [`app_resource`].
pub const APP_RESOURCE_PATH: &str = "/v1";
/// Resource path of the MCP server.
pub const MCP_RESOURCE_PATH: &str = "/mcp";

/// First-party clients: public, PKCE S256, no consent page. They are the only clients that get
/// tokens for [`APP_RESOURCE_PATH`].
pub const DESKTOP_CLIENT_ID: &str = "quest-desktop";
/// The desktop app's redirect: `http://127.0.0.1:<any port>` + this path (RFC 8252 loopback).
pub const DESKTOP_REDIRECT_PATH: &str = "/callback";
pub const ANDROID_CLIENT_ID: &str = "quest-android";
/// Private-use URI scheme redirect (RFC 8252 §7.1), reverse domain of the application id.
pub const ANDROID_REDIRECT_URI: &str = "app.quest.android:/oauth2redirect";
/// Verified App Link redirect (Android App Links): `{origin}` + this path, where origin is the
/// server's public `https` origin without a trailing slash. Allowed besides
/// [`ANDROID_REDIRECT_URI`] only for an `https` origin; see [`android_app_link_redirect`].
pub const ANDROID_APP_LINK_PATH: &str = "/auth/android";
/// Digital Asset Links statement list that verifies the App Link.
pub const ASSET_LINKS_PATH: &str = "/.well-known/assetlinks.json";
/// Application id of the Android app (package name in [`ASSET_LINKS_PATH`]).
pub const ANDROID_PACKAGE_NAME: &str = "app.quest.android";

/// Read the account's quests, tasks and time.
pub const SCOPE_READ: &str = "quest:read";
/// Read and change them; includes [`SCOPE_READ`].
pub const SCOPE_WRITE: &str = "quest:write";
/// Refresh tokens. The server issues them to every client anyway.
pub const SCOPE_OFFLINE_ACCESS: &str = "offline_access";
/// What the apps request.
pub const APP_SCOPE: &str = "quest:write offline_access";

/// The app resource of the server at `origin` (e.g. `https://quest.example.com`), sent as
/// `resource` in the authorization and token requests.
pub fn app_resource(origin: &str) -> String {
    format!("{}{APP_RESOURCE_PATH}", origin.trim_end_matches('/'))
}

/// The App Link redirect URI of the server at `origin`, e.g.
/// `https://quest.example.com/auth/android`.
pub fn android_app_link_redirect(origin: &str) -> String {
    format!("{}{ANDROID_APP_LINK_PATH}", origin.trim_end_matches('/'))
}

/// The signed-in account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountInfo {
    pub user_id: UserId,
    /// OpenID Connect issuer of the identity provider.
    pub issuer: String,
    /// The user's `sub` at that issuer. `(issuer, subject)` identifies the account.
    pub subject: String,
    /// For display only; may change at the provider.
    pub email: Option<String>,
    /// For display only.
    pub name: Option<String>,
    /// AI features opted in for this account (default off).
    pub ai_enabled: bool,
    /// The server has an AI provider configured.
    pub ai_available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSettingsUpdate {
    pub ai_enabled: bool,
}

/// A successful token response (RFC 6749 §5.1). `Debug` hides the tokens.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    /// Always `Bearer`.
    pub token_type: String,
    /// Lifetime of the access token in seconds.
    pub expires_in: u64,
    #[serde(default)]
    pub refresh_token: Option<String>,
    pub scope: String,
}

impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenResponse")
            .field("access_token", &"<redacted>")
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .field("refresh_token", &self.refresh_token.as_ref().map(|_| "<redacted>"))
            .field("scope", &self.scope)
            .finish()
    }
}

/// An OAuth error response (RFC 6749 §5.2), e.g. `invalid_grant`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthErrorBody {
    pub error: String,
    #[serde(default)]
    pub error_description: Option<String>,
}
