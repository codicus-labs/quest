//! Pure OAuth decisions: identifiers, redirect URIs, PKCE, scopes, resources, client metadata. No IO.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use quest_protocol::{
    ANDROID_CLIENT_ID, ANDROID_REDIRECT_URI, APP_RESOURCE_PATH, DESKTOP_CLIENT_ID, DESKTOP_REDIRECT_PATH,
    MCP_RESOURCE_PATH, SCOPE_OFFLINE_ACCESS, SCOPE_READ, SCOPE_WRITE, android_app_link_redirect,
};
use reqwest::Url;
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Path of the MCP endpoint; with the public URL it is the protected resource.
pub const MCP_PATH: &str = MCP_RESOURCE_PATH;
/// The scope of the protected resource metadata and of the 401 challenge at `/mcp`.
pub const MCP_CHALLENGE_SCOPE: &str = SCOPE_WRITE;
/// The scope before read-only access existed; still accepted, as `quest:write`.
const LEGACY_SCOPE: &str = "quest";
const MAX_REDIRECT_URIS: usize = 10;
const MAX_URI_LEN: usize = 2000;
const MAX_CLIENT_NAME_CHARS: usize = 100;

/// The issuer: the public origin without a trailing slash, e.g. `https://quest.example.com`.
pub fn issuer(public_url: &Url) -> String {
    public_url.as_str().trim_end_matches('/').to_owned()
}

/// The canonical URI of the MCP server (RFC 8707 resource), e.g. `https://quest.example.com/mcp`.
pub fn canonical_resource(public_url: &Url) -> String {
    format!("{}{MCP_PATH}", issuer(public_url))
}

/// The resource of the app routes, e.g. `https://quest.example.com/v1`.
pub fn app_resource(public_url: &Url) -> String {
    format!("{}{APP_RESOURCE_PATH}", issuer(public_url))
}

/// What a token is for (RFC 8707 audience).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    /// The apps' routes under `/v1` (sync, account, AI capture): only first-party clients.
    App,
    /// The MCP server.
    Mcp,
}

/// The resource an authorization request is for. First-party apps get the app resource, every
/// other client the MCP resource; asking for the other one is `invalid_target`.
pub fn requested_resource(
    requested: Option<&str>,
    first_party: bool,
    app: &str,
    mcp: &str,
) -> Result<Resource, &'static str> {
    let resource = match requested {
        None if first_party => Resource::App,
        None => Resource::Mcp,
        Some(raw) if same_resource(raw, app) => Resource::App,
        Some(raw) if same_resource(raw, mcp) => Resource::Mcp,
        Some(_) => return Err("This server only issues tokens for its own /v1 and /mcp endpoints"),
    };
    match (resource, first_party) {
        (Resource::App, true) | (Resource::Mcp, false) => Ok(resource),
        (Resource::App, false) => Err("Only the Quest apps get tokens for the app routes; MCP clients use /mcp"),
        (Resource::Mcp, true) => Err("The Quest apps get tokens for the app routes (/v1) only"),
    }
}

/// Read-only or read and write access to the account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Access {
    Read,
    Write,
}

impl Access {
    pub fn scope(self) -> &'static str {
        match self {
            Access::Read => SCOPE_READ,
            Access::Write => SCOPE_WRITE,
        }
    }

    /// The access a stored grant scope stands for; anything without `quest:write` reads only.
    pub fn of_grant(scope: &str) -> Access {
        if scope.split_whitespace().any(|scope| scope == SCOPE_WRITE) { Access::Write } else { Access::Read }
    }

    pub fn parse(raw: &str) -> Option<Access> {
        match raw {
            SCOPE_READ => Some(Access::Read),
            SCOPE_WRITE | LEGACY_SCOPE => Some(Access::Write),
            _ => None,
        }
    }
}

/// Whether a `resource` parameter names this server: scheme and host compare case-insensitively
/// (the URL parser lowercases them), a trailing slash is ignored.
pub fn same_resource(requested: &str, canonical: &str) -> bool {
    let normalized = |raw: &str| Url::parse(raw).ok().map(|url| url.as_str().trim_end_matches('/').to_owned());
    match (normalized(requested), normalized(canonical)) {
        (Some(requested), Some(canonical)) => requested == canonical,
        _ => false,
    }
}

/// Whether the public URL may serve OAuth: HTTPS, or plain HTTP on a loopback host.
pub fn check_public_url(public_url: &Url) -> Result<(), String> {
    if public_url.path() != "/" || public_url.query().is_some() || public_url.fragment().is_some() {
        return Err(format!("public URL {public_url} must be an origin like https://quest.example.com, without path"));
    }
    match public_url.scheme() {
        "https" => Ok(()),
        "http" if is_loopback(public_url) => Ok(()),
        _ => Err(format!(
            "public URL {public_url} must use https (plain http is only allowed for localhost); \
             put a TLS reverse proxy in front of the server"
        )),
    }
}

pub fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

/// A redirect URI a client may register: HTTPS, or HTTP to a loopback host (native apps), absolute,
/// without fragment.
pub fn check_redirect_uri(raw: &str) -> Result<Url, String> {
    if raw.len() > MAX_URI_LEN {
        return Err(format!("redirect URI is longer than {MAX_URI_LEN} characters"));
    }
    let url = Url::parse(raw).map_err(|_| format!("redirect URI {raw:?} is not an absolute URL"))?;
    if url.fragment().is_some() {
        return Err(format!("redirect URI {raw} must not have a fragment"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(format!("redirect URI {raw} must not contain credentials"));
    }
    match url.scheme() {
        "https" if url.host().is_some() => Ok(url),
        "http" if is_loopback(&url) => Ok(url),
        _ => Err(format!("redirect URI {raw} must use https, or http on localhost/127.0.0.1/[::1]")),
    }
}

/// Whether `requested` is one of the client's registered redirect URIs: exactly, except that a
/// loopback URI may use any port (RFC 8252 7.3, OAuth 2.1: native apps get an ephemeral port).
pub fn redirect_allowed(registered: &[String], requested: &str) -> bool {
    registered.iter().any(|candidate| redirect_matches(candidate, requested))
}

fn redirect_matches(registered: &str, requested: &str) -> bool {
    if registered == requested {
        return true;
    }
    let (Ok(mut registered), Ok(mut requested)) = (Url::parse(registered), Url::parse(requested)) else {
        return false;
    };
    if registered.scheme() != "http" || !is_loopback(&registered) || !is_loopback(&requested) {
        return false;
    }
    let ports_cleared = registered.set_port(None).is_ok() && requested.set_port(None).is_ok();
    ports_cleared && registered == requested
}

/// A PKCE S256 challenge as the client sends it: 43 base64url characters (a SHA-256 digest).
pub fn valid_challenge(challenge: &str) -> bool {
    challenge.len() == 43 && URL_SAFE_NO_PAD.decode(challenge).is_ok_and(|digest| digest.len() == 32)
}

/// RFC 7636: the verifier has 43 to 128 unreserved characters and hashes to the challenge.
pub fn pkce_matches(verifier: &str, challenge: &str) -> bool {
    let well_formed = (43..=128).contains(&verifier.len())
        && verifier.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'));
    well_formed && URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) == challenge
}

/// The access a `scope` parameter asks for: the widest of `quest:read`, `quest:write` (or the old
/// `quest`), `None` if it names neither. `offline_access` is accepted (refresh tokens are always
/// issued); unknown scopes are refused so a client does not believe it got them.
pub fn requested_access(requested: Option<&str>) -> Result<Option<Access>, String> {
    let mut access = None;
    let mut unknown = Vec::new();
    for scope in requested.unwrap_or_default().split_whitespace() {
        match Access::parse(scope) {
            Some(asked) => access = access.max(Some(asked)),
            None if scope == SCOPE_OFFLINE_ACCESS => {}
            None => unknown.push(scope),
        }
    }
    if !unknown.is_empty() {
        return Err(format!(
            "unknown scope {}; this server offers {SCOPE_READ}, {SCOPE_WRITE} and {SCOPE_OFFLINE_ACCESS}",
            unknown.join(" ")
        ));
    }
    Ok(access)
}

/// The Quest apps: built in, public (PKCE only), no consent page (at most "Continue as …"). `None` for other client ids.
/// `issuer` is this server's origin: Android may also use its App Link there, if it is https.
pub fn first_party_client(client_id: &str, issuer: &str) -> Option<ClientInfo> {
    let (name, redirect_uris) = match client_id {
        DESKTOP_CLIENT_ID => ("Quest for Mac", vec![format!("http://127.0.0.1{DESKTOP_REDIRECT_PATH}")]),
        ANDROID_CLIENT_ID => ("Quest for Android", android_redirect_uris(issuer)),
        _ => return None,
    };
    Some(ClientInfo {
        client_id: client_id.to_owned(),
        client_name: name.to_owned(),
        redirect_uris,
        secret_hash: None,
        first_party: true,
    })
}

/// The private-use scheme always (development, other servers); the verified App Link
/// `{issuer}/auth/android` only on an https origin, where Android can verify it.
fn android_redirect_uris(issuer: &str) -> Vec<String> {
    let mut uris = vec![ANDROID_REDIRECT_URI.to_owned()];
    if Url::parse(issuer).is_ok_and(|url| url.scheme() == "https") {
        uris.push(android_app_link_redirect(issuer));
    }
    uris
}

/// Whether `uri` is where a first-party app receives its redirect (e.g. `return_to` at logout).
pub fn first_party_redirect(uri: &str, issuer: &str) -> bool {
    [DESKTOP_CLIENT_ID, ANDROID_CLIENT_ID]
        .into_iter()
        .filter_map(|client_id| first_party_client(client_id, issuer))
        .any(|client| redirect_allowed(&client.redirect_uris, uri))
}

/// Whether another app could receive redirects to `uri`: a private-use scheme (RFC 8252 §8.6),
/// unlike loopback http or https. Codes for such URIs need the user's confirmation every time.
pub fn claimable_redirect(uri: &str) -> bool {
    Url::parse(uri).map_or(true, |url| !matches!(url.scheme(), "http" | "https"))
}

/// A client id that is a Client ID Metadata Document URL: https with a path.
pub fn metadata_document_url(client_id: &str) -> Option<Url> {
    let url = Url::parse(client_id).ok()?;
    let usable = url.scheme() == "https"
        && url.host().is_some()
        && url.path() != "/"
        && url.fragment().is_none()
        && url.username().is_empty()
        && url.password().is_none();
    usable.then_some(url)
}

/// A client as the authorization endpoint needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct ClientInfo {
    pub client_id: String,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    /// Hash of the client secret; `None` for public clients.
    pub secret_hash: Option<String>,
    /// One of the Quest apps ([`first_party_client`]).
    pub first_party: bool,
}

/// What a client sends to register (RFC 7591) or publishes as its metadata document.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ClientMetadata {
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_name: Option<String>,
    #[serde(default)]
    pub redirect_uris: Vec<String>,
    #[serde(default)]
    pub token_endpoint_auth_method: Option<String>,
    #[serde(default)]
    pub grant_types: Option<Vec<String>>,
    #[serde(default)]
    pub response_types: Option<Vec<String>>,
}

/// How a client authenticates at the token endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientAuth {
    None,
    SecretBasic,
    SecretPost,
}

impl ClientAuth {
    pub fn as_str(self) -> &'static str {
        match self {
            ClientAuth::None => "none",
            ClientAuth::SecretBasic => "client_secret_basic",
            ClientAuth::SecretPost => "client_secret_post",
        }
    }
}

/// Checked client metadata: redirect URIs valid, grant and response types supported.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckedMetadata {
    pub client_name: Option<String>,
    pub redirect_uris: Vec<String>,
    pub auth: ClientAuth,
}

/// Why client metadata was refused; registration answers with the matching RFC 7591 error code.
#[derive(Clone, Debug, PartialEq)]
pub enum MetadataError {
    /// `invalid_redirect_uri`
    RedirectUri(String),
    /// `invalid_client_metadata`
    Metadata(String),
}

impl MetadataError {
    pub fn reason(self) -> String {
        match self {
            MetadataError::RedirectUri(reason) | MetadataError::Metadata(reason) => reason,
        }
    }
}

/// Checks metadata for registration. An absent auth method means `client_secret_basic` (RFC 7591).
pub fn check_registration(metadata: &ClientMetadata) -> Result<CheckedMetadata, MetadataError> {
    let auth = match metadata.token_endpoint_auth_method.as_deref() {
        Some("none") => ClientAuth::None,
        None | Some("client_secret_basic") => ClientAuth::SecretBasic,
        Some("client_secret_post") => ClientAuth::SecretPost,
        Some(other) => {
            return Err(MetadataError::Metadata(format!(
                "token_endpoint_auth_method {other:?} is not supported; use none, client_secret_basic or client_secret_post"
            )));
        }
    };
    check_common(metadata, auth)
}

/// Checks a fetched Client ID Metadata Document: its `client_id` must equal the URL it came from,
/// and such clients are public (`none`).
pub fn check_metadata_document(url: &Url, metadata: &ClientMetadata) -> Result<ClientInfo, String> {
    if metadata.client_id.as_deref() != Some(url.as_str()) {
        return Err("the document's client_id does not match its URL".to_owned());
    }
    if !matches!(metadata.token_endpoint_auth_method.as_deref(), None | Some("none")) {
        return Err("only public clients (token_endpoint_auth_method none) are supported".to_owned());
    }
    let checked = check_common(metadata, ClientAuth::None).map_err(MetadataError::reason)?;
    let client_name = checked.client_name.unwrap_or_else(|| url.host_str().unwrap_or_default().to_owned());
    Ok(ClientInfo {
        client_id: url.as_str().to_owned(),
        client_name,
        redirect_uris: checked.redirect_uris,
        secret_hash: None,
        first_party: false,
    })
}

fn check_common(metadata: &ClientMetadata, auth: ClientAuth) -> Result<CheckedMetadata, MetadataError> {
    if metadata.redirect_uris.is_empty() || metadata.redirect_uris.len() > MAX_REDIRECT_URIS {
        return Err(MetadataError::RedirectUri(format!("register 1 to {MAX_REDIRECT_URIS} redirect_uris")));
    }
    for uri in &metadata.redirect_uris {
        check_redirect_uri(uri).map_err(MetadataError::RedirectUri)?;
    }
    let supported = |values: &Option<Vec<String>>, allowed: &[&str], name: &str| match values {
        Some(values) if values.iter().any(|value| !allowed.contains(&value.as_str())) => {
            Err(MetadataError::Metadata(format!("{name} may only contain {}", allowed.join(", "))))
        }
        _ => Ok(()),
    };
    supported(&metadata.grant_types, &["authorization_code", "refresh_token"], "grant_types")?;
    supported(&metadata.response_types, &["code"], "response_types")?;
    let client_name = metadata
        .client_name
        .as_deref()
        .map(|name| name.trim().chars().filter(|c| !c.is_control()).take(MAX_CLIENT_NAME_CHARS).collect::<String>())
        .filter(|name| !name.is_empty());
    Ok(CheckedMetadata { client_name, redirect_uris: metadata.redirect_uris.clone(), auth })
}

/// Whether every redirect URI is a loopback one: anyone can claim such a client, so the consent
/// page warns.
pub fn loopback_only(redirect_uris: &[String]) -> bool {
    redirect_uris.iter().all(|uri| Url::parse(uri).is_ok_and(|url| is_loopback(&url)))
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
