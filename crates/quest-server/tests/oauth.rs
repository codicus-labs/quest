//! OAuth 2.1 for MCP against the in-process server: discovery, registration (DCR and client
//! metadata documents), PKCE, sign-in at the identity provider and consent with access choice,
//! code and refresh rules, revocation, rate limits and the 401/403 challenges of `/mcp`.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::oauth::{OAuthClient, REDIRECT_URI, arrived_query, csrf, http, oauth_error, page_status, pkce, tokens};
use common::{Setup, TestServer};
use futures_util::future::BoxFuture;
use quest_oidc_fake::{FakeConfig, FakeProvider, Step};
use quest_protocol::{AI_CONNECTIONS_PATH, AiConnections};
use quest_server::{
    AuthConfig, ClientAddress, ClientMetadataSource, FetchError, Fetched, McpConfig, OAuthRateLimits, OidcSettings,
    Window,
};
use reqwest::{StatusCode, Url};
use serde_json::{Value, json};

const USER: &str = "ada";
const CIMD_URL: &str = "https://client.example.com/oauth/metadata.json";

/// Serves one client metadata document without any network.
#[derive(Debug)]
struct FakeDocuments;

impl ClientMetadataSource for FakeDocuments {
    fn fetch<'a>(&'a self, url: &'a Url) -> BoxFuture<'a, Result<Fetched, FetchError>> {
        Box::pin(async move {
            if url.as_str() != CIMD_URL {
                return Err(FetchError::Status(404));
            }
            let document = json!({
                "client_id": CIMD_URL,
                "client_name": "Document client",
                "redirect_uris": ["http://localhost/cb"],
                "token_endpoint_auth_method": "none",
            });
            Ok(Fetched { metadata: serde_json::from_value(document).unwrap(), max_age: None })
        })
    }
}

/// A server with MCP and [`FakeDocuments`], after `tweak` changed its auth configuration.
fn setup(tweak: impl FnOnce(&mut AuthConfig) + Send + 'static) -> Setup {
    let auth = move |auth: &mut AuthConfig| {
        auth.client_metadata = Arc::new(FakeDocuments);
        tweak(auth);
    };
    Setup { mcp: Some(McpConfig::default()), auth: Some(Box::new(auth)), ..Setup::default() }
}

async fn server() -> TestServer {
    TestServer::start_custom(setup(|_| {})).await
}

async fn get_json(url: &str) -> Value {
    let response = http().get(url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{url}");
    response.json().await.unwrap()
}

async fn mcp_post(server: &TestServer, token: Option<&str>) -> reqwest::Response {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} });
    let mut request = http()
        .post(server.url("/mcp"))
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-11-25")
        .json(&body);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    request.send().await.unwrap()
}

fn challenge(response: &reqwest::Response) -> String {
    response.headers().get("www-authenticate").expect("WWW-Authenticate").to_str().unwrap().to_owned()
}

async fn mcp_call(server: &TestServer, token: &str, tool: &str) -> reqwest::Response {
    let body = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": tool, "arguments": { "title": "x", "time_zone": "Europe/Berlin" } } });
    http()
        .post(server.url("/mcp"))
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-11-25")
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn discovery_starts_from_the_401_of_mcp() {
    let server = server().await;
    let base = server.server.http_url();
    let response = mcp_post(&server, None).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let header = challenge(&response);
    let metadata_url = format!("{base}/.well-known/oauth-protected-resource/mcp");
    assert!(header.starts_with("Bearer "), "{header}");
    assert!(header.contains(&format!("resource_metadata=\"{metadata_url}\"")), "{header}");
    assert!(header.contains("scope=\"quest:write\""), "{header}");

    let resource = get_json(&metadata_url).await;
    assert_eq!(resource["resource"], format!("{base}/mcp"));
    assert_eq!(resource["authorization_servers"], json!([base]));
    assert_eq!(get_json(&format!("{base}/.well-known/oauth-protected-resource")).await, resource);

    let auth = get_json(&format!("{base}/.well-known/oauth-authorization-server")).await;
    assert_eq!(auth["issuer"], base);
    assert_eq!(auth["code_challenge_methods_supported"], json!(["S256"]));
    assert_eq!(auth["client_id_metadata_document_supported"], true);
    assert_eq!(auth["authorization_response_iss_parameter_supported"], true);
    for endpoint in ["authorization_endpoint", "token_endpoint", "registration_endpoint", "revocation_endpoint"] {
        assert!(auth[endpoint].as_str().unwrap().starts_with(&base), "{endpoint}");
    }
}

#[tokio::test]
async fn full_flow_signs_in_at_the_provider_and_gives_tokens_that_open_mcp() {
    let server = server().await;
    let client = OAuthClient::register(&server).await;
    let params = client.authorize_params(&pkce().1);
    let browser = client.browser(USER);
    let page = client.open(&browser, &params).await;
    let Step::Page { status: 200, body, .. } = &page else { panic!("expected the consent page, got {page:?}") };
    assert!(body.contains("Test client") && body.contains("127.0.0.1"), "consent names client and redirect host");
    assert!(body.contains("ada@example.test"), "consent names the account");
    assert!(body.contains("value=\"quest:write\" checked"), "preselects what the client asked for");

    // The browser now has a session: the page comes without another sign-in, framing denied.
    let again = browser.http().get(client.authorize_url(&params)).send().await.unwrap();
    assert_eq!(again.status(), StatusCode::OK);
    assert_eq!(again.headers()["x-frame-options"], "DENY");

    let tokens = client.tokens(USER).await;
    assert_eq!(tokens.scope, "quest:write");
    let response = mcp_post(&server, Some(&tokens.access)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert!(body["result"]["tools"].as_array().unwrap().len() > 20, "{body}");
}

#[tokio::test]
async fn read_only_access_is_chosen_on_the_consent_page_and_enforced() {
    let server = server().await;
    let client = OAuthClient::register(&server).await;
    let mut params = client.authorize_params(&pkce().1);
    params.retain(|(name, _)| *name != "scope");
    params.push(("scope", "quest:read".into()));
    let browser = client.browser(USER);
    let Step::Page { body, .. } = client.open(&browser, &params).await else { panic!("expected the consent page") };
    assert!(body.contains("value=\"quest:read\" checked"), "a read-only request is preselected as such");

    let tokens = client.tokens_with(USER, Some("quest:read")).await;
    assert_eq!(tokens.scope, "quest:read");
    let response = mcp_post(&server, Some(&tokens.access)).await;
    let body: Value = response.json().await.unwrap();
    let names: Vec<&str> =
        body["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"list_tasks") && !names.contains(&"create_task"), "{names:?}");

    let response = mcp_call(&server, &tokens.access, "create_task").await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let header = challenge(&response);
    assert!(header.contains("error=\"insufficient_scope\""), "{header}");
    assert!(header.contains("scope=\"quest:write\""), "{header}");
    assert!(header.contains("resource_metadata="), "{header}");
    assert_eq!(mcp_call(&server, &tokens.access, "list_tasks").await.status(), StatusCode::OK);

    // A refresh keeps the access and cannot widen it.
    let refreshed = tokens_after_refresh(&client, &tokens.refresh).await;
    assert_eq!(refreshed.scope, "quest:read");
    let form =
        [("grant_type", "refresh_token"), ("refresh_token", refreshed.refresh.as_str()), ("scope", "quest:write")];
    assert_eq!(oauth_error(client.token(&form).await).await, (StatusCode::BAD_REQUEST, "invalid_scope".to_owned()));
}

async fn tokens_after_refresh(client: &OAuthClient<'_>, refresh: &str) -> common::oauth::Tokens {
    tokens(client.refresh(refresh).await).await
}

#[tokio::test]
async fn without_a_quest_scope_read_only_is_preselected_and_granted() {
    let server = server().await;
    let client = OAuthClient::register(&server).await;
    let (verifier, challenge) = pkce();
    let mut params = client.authorize_params(&challenge);
    params.retain(|(name, _)| *name != "scope");
    let browser = client.browser(USER);
    let page = client.open(&browser, &params).await;
    let Step::Page { body, .. } = &page else { panic!("expected the consent page") };
    assert!(body.contains("value=\"quest:read\" checked"), "least privilege");
    let step = client.decide(&browser, &page, &params, "allow", None).await;
    let code = arrived_query(&step).get("code").cloned().unwrap();
    assert_eq!(tokens(client.exchange(&code, &verifier).await).await.scope, "quest:read");
}

#[tokio::test]
async fn unknown_scopes_are_refused() {
    let server = server().await;
    let client = OAuthClient::register(&server).await;
    let mut params = client.authorize_params(&pkce().1);
    params.retain(|(name, _)| *name != "scope");
    params.push(("scope", "admin".into()));
    let step = client.open(&client.browser(USER), &params).await;
    assert_eq!(arrived_query(&step).get("error").map(String::as_str), Some("invalid_scope"));
}

#[tokio::test]
async fn registration_checks_redirect_uris() {
    let server = server().await;
    for uris in [json!([]), json!(["http://example.com/cb"]), json!(["https://example.com/cb#frag"])] {
        let body = json!({ "client_name": "x", "redirect_uris": uris, "token_endpoint_auth_method": "none" });
        let response = http().post(server.url("/register")).json(&body).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uris}");
    }
    let body = json!({ "client_name": "Confidential", "redirect_uris": ["https://app.example.com/cb"] });
    let response = http().post(server.url("/register")).json(&body).send().await.unwrap();
    let registered: Value = response.json().await.unwrap();
    assert_eq!(registered["token_endpoint_auth_method"], "client_secret_basic");
    assert!(registered["client_secret"].is_string());
}

#[tokio::test]
async fn confidential_clients_need_their_secret() {
    let server = server().await;
    let body = json!({ "client_name": "Confidential", "redirect_uris": [REDIRECT_URI] });
    let registered: Value =
        http().post(server.url("/register")).json(&body).send().await.unwrap().json().await.unwrap();
    let client = OAuthClient {
        server: &server,
        client_id: registered["client_id"].as_str().unwrap().into(),
        redirect_uri: REDIRECT_URI.into(),
    };
    let (verifier, challenge) = pkce();
    let code = client.code(USER, &challenge).await;
    let (status, error) = oauth_error(client.exchange(&code, &verifier).await).await;
    assert_eq!((status, error.as_str()), (StatusCode::UNAUTHORIZED, "invalid_client"));
    let resource = client.resource();
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("code_verifier", verifier.as_str()),
        ("resource", resource.as_str()),
    ];
    let response = http()
        .post(server.url("/token"))
        .basic_auth(&client.client_id, registered["client_secret"].as_str())
        .form(&form)
        .send()
        .await
        .unwrap();
    tokens(response).await;
}

#[tokio::test]
async fn metadata_document_clients_need_no_registration() {
    let server = server().await;
    let client =
        OAuthClient { server: &server, client_id: CIMD_URL.into(), redirect_uri: "http://localhost:4711/cb".into() };
    let browser = client.browser(USER);
    let Step::Page { body, .. } = client.open(&browser, &client.authorize_params(&pkce().1)).await else {
        panic!("expected the consent page");
    };
    assert!(body.contains("Document client"));
    let tokens = client.tokens(USER).await;
    assert_eq!(mcp_post(&server, Some(&tokens.access)).await.status(), StatusCode::OK);

    let unknown = OAuthClient {
        server: &server,
        client_id: "https://client.example.com/other.json".into(),
        redirect_uri: REDIRECT_URI.into(),
    };
    let step = unknown.consent(&unknown.authorize_params(&pkce().1), USER, "allow").await;
    assert_eq!(page_status(&step), 400);
}

#[tokio::test]
async fn pkce_is_required_and_checked() {
    let server = server().await;
    let client = OAuthClient::register(&server).await;
    let mut params = client.authorize_params(&pkce().1);
    params.retain(|(name, _)| *name != "code_challenge_method");
    let step = client.consent(&params, USER, "allow").await;
    assert_eq!(arrived_query(&step).get("error").map(String::as_str), Some("invalid_request"));

    let (_, challenge) = pkce();
    let code = client.code(USER, &challenge).await;
    let (wrong_verifier, _) = pkce();
    let (status, error) = oauth_error(client.exchange(&code, &wrong_verifier).await).await;
    assert_eq!((status, error.as_str()), (StatusCode::BAD_REQUEST, "invalid_grant"));
}

#[tokio::test]
async fn unregistered_redirect_uris_get_an_error_page_not_a_redirect() {
    let server = server().await;
    let mut client = OAuthClient::register(&server).await;
    client.redirect_uri = "https://evil.example.com/cb".into();
    let step = client.consent(&client.authorize_params(&pkce().1), USER, "allow").await;
    assert_eq!(page_status(&step), 400);

    // Loopback: any port matches.
    client.redirect_uri = "http://127.0.0.1:53682/callback".into();
    let step = client.consent(&client.authorize_params(&pkce().1), USER, "allow").await;
    assert!(arrived_query(&step).contains_key("code"));

    // The token request must name the same redirect URI as the authorization request.
    client.redirect_uri = REDIRECT_URI.into();
    let (verifier, challenge) = pkce();
    let code = client.code(USER, &challenge).await;
    client.redirect_uri = "http://127.0.0.1:9/other".into();
    let (_, error) = oauth_error(client.exchange(&code, &verifier).await).await;
    assert_eq!(error, "invalid_grant");
}

#[tokio::test]
async fn denial_and_a_foreign_form_give_no_code() {
    let server = server().await;
    let client = OAuthClient::register(&server).await;
    let params = client.authorize_params(&pkce().1);
    let step = client.consent(&params, USER, "deny").await;
    assert_eq!(arrived_query(&step).get("error").map(String::as_str), Some("access_denied"));

    // A form whose CSRF token belongs to another browser session is refused.
    let browser = client.browser(USER);
    let page = client.open(&browser, &params).await;
    let other = client.browser(USER);
    let other_page = client.open(&other, &params).await;
    assert_ne!(csrf(&page), csrf(&other_page));
    let step = client.decide(&browser, &other_page, &params, "allow", None).await;
    assert_eq!(page_status(&step), 403);
    let step = client.decide(&browser, &page, &params, "allow", None).await;
    assert!(arrived_query(&step).contains_key("code"));
}

#[tokio::test]
async fn an_identity_provider_that_refuses_gives_access_denied() {
    // This provider knows only anna and ben and refuses everyone else.
    let fake = Arc::new(FakeProvider::start(FakeConfig::default()).await.unwrap());
    let server = TestServer::start_custom(Setup { fake: Some(fake), ..setup(|_| {}) }).await;
    let client = OAuthClient::register(&server).await;
    let browser = client.browser("mallory");
    let step = client.open(&browser, &client.authorize_params(&pkce().1)).await;
    let query = arrived_query(&step);
    assert_eq!(query.get("error").map(String::as_str), Some("access_denied"));
    assert_eq!(query.get("state").map(String::as_str), Some("xyz"));
}

#[tokio::test]
async fn codes_expire_and_work_once() {
    let server = TestServer::start_custom(setup(|auth| auth.code_ttl = Duration::ZERO)).await;
    let client = OAuthClient::register(&server).await;
    let (verifier, challenge) = pkce();
    let code = client.code(USER, &challenge).await;
    let (_, error) = oauth_error(client.exchange(&code, &verifier).await).await;
    assert_eq!(error, "invalid_grant", "expired code");

    let server = self::server().await;
    let client = OAuthClient::register(&server).await;
    let (verifier, challenge) = pkce();
    let code = client.code(USER, &challenge).await;
    let first = tokens(client.exchange(&code, &verifier).await).await;
    let (_, error) = oauth_error(client.exchange(&code, &verifier).await).await;
    assert_eq!(error, "invalid_grant", "replayed code");
    // A replayed code revokes what it gave.
    assert_eq!(mcp_post(&server, Some(&first.access)).await.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn refresh_tokens_rotate_and_reuse_revokes_the_grant() {
    let server = server().await;
    let client = OAuthClient::register(&server).await;
    let first = client.tokens(USER).await;
    let second = tokens(client.refresh(&first.refresh).await).await;
    assert_ne!(second.refresh, first.refresh);
    assert_eq!(mcp_post(&server, Some(&second.access)).await.status(), StatusCode::OK);

    let (_, error) = oauth_error(client.refresh(&first.refresh).await).await;
    assert_eq!(error, "invalid_grant");
    assert_eq!(mcp_post(&server, Some(&second.access)).await.status(), StatusCode::UNAUTHORIZED);
    let (_, error) = oauth_error(client.refresh(&second.refresh).await).await;
    assert_eq!(error, "invalid_grant", "the whole grant is gone");
}

#[tokio::test]
async fn short_lived_access_tokens_expire() {
    let server = TestServer::start_custom(setup(|auth| auth.access_token_ttl = Duration::ZERO)).await;
    let tokens = OAuthClient::register(&server).await.tokens(USER).await;
    let response = mcp_post(&server, Some(&tokens.access)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(challenge(&response).contains("error=\"invalid_token\""));
}

#[tokio::test]
async fn revocation_by_client_and_from_the_apps() {
    let server = server().await;
    let session = server.sign_in(USER).await;
    let client = OAuthClient::register(&server).await;
    // A consent whose code is never exchanged is no connection.
    let (_, challenge) = pkce();
    client.code(USER, &challenge).await;
    let tokens = client.tokens(USER).await;

    // The apps list the connection and can revoke it.
    let list = || async {
        let response = http().get(server.url(AI_CONNECTIONS_PATH)).bearer_auth(&session.token).send().await.unwrap();
        response.json::<AiConnections>().await.unwrap().connections
    };
    let connections = list().await;
    assert_eq!(connections.len(), 1);
    assert_eq!(
        (connections[0].client_name.as_str(), connections[0].redirect_host.as_str()),
        ("Test client", "127.0.0.1")
    );
    let url = server.url(&format!("{AI_CONNECTIONS_PATH}/{}", connections[0].id));
    let response = http().delete(&url).bearer_auth(&session.token).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(list().await.is_empty());
    assert_eq!(mcp_post(&server, Some(&tokens.access)).await.status(), StatusCode::UNAUTHORIZED);
    let response = http().delete(&url).bearer_auth(&session.token).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // RFC 7009 at /revoke.
    let tokens = client.tokens(USER).await;
    let form = [("token", tokens.refresh.as_str()), ("client_id", client.client_id.as_str())];
    let response = http().post(server.url("/revoke")).form(&form).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(mcp_post(&server, Some(&tokens.access)).await.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn tokens_are_not_interchangeable_with_app_tokens() {
    let server = server().await;
    let session = server.sign_in(USER).await;
    let response = mcp_post(&server, Some(&session.token)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "an app token is for /v1, not /mcp");
    assert!(challenge(&response).contains("resource_metadata="));

    let tokens = OAuthClient::register(&server).await.tokens(USER).await;
    let response = http().get(server.url(AI_CONNECTIONS_PATH)).bearer_auth(&tokens.access).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "an MCP token is for /mcp, not /v1");
    assert!(server.connect_raw(Some(&tokens.access)).await.is_err(), "nor for sync");
}

#[tokio::test]
async fn other_clients_cannot_ask_for_the_app_resource() {
    let server = server().await;
    let client = OAuthClient::register(&server).await;
    let mut params = client.authorize_params(&pkce().1);
    params.retain(|(name, _)| *name != "resource");
    params.push(("resource", server.url("/v1")));
    let step = client.consent(&params, USER, "allow").await;
    assert_eq!(arrived_query(&step).get("error").map(String::as_str), Some("invalid_target"));
}

#[tokio::test]
async fn tokens_for_another_resource_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let database_url = common::database_url(&dir);
    let first = TestServer::start_custom(Setup { database_url: Some(database_url.clone()), ..setup(|_| {}) }).await;
    let client = OAuthClient::register(&first).await;
    let tokens = client.tokens(USER).await;
    // A resource other than this server's /mcp is refused at /authorize too.
    let mut params = client.authorize_params(&pkce().1);
    params.retain(|(name, _)| *name != "resource");
    params.push(("resource", "https://other.example.com/mcp".into()));
    let step = client.consent(&params, USER, "allow").await;
    assert_eq!(arrived_query(&step).get("error").map(String::as_str), Some("invalid_target"));
    let fake = first.fake.clone();
    first.server.shutdown().await;

    // Same database, now published under another origin: the old audience no longer fits.
    let moved = setup(|auth| auth.public_url = Some(Url::parse("http://localhost:9").unwrap()));
    let second = TestServer::start_custom(Setup { database_url: Some(database_url), fake: Some(fake), ..moved }).await;
    let response = mcp_post(&second, Some(&tokens.access)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let header = challenge(&response);
    assert!(
        header.contains("resource_metadata=\"http://localhost:9/.well-known/oauth-protected-resource/mcp\""),
        "{header}"
    );
    assert!(header.contains("another resource"), "{header}");
}

#[tokio::test]
async fn public_url_must_be_https_unless_loopback() {
    let settings = || OidcSettings {
        issuer: Url::parse("https://id.example.com").unwrap(),
        client_id: "quest-server".into(),
        client_secret: "secret".into(),
    };
    let config = |url: &str| AuthConfig::with_oidc(Some(Url::parse(url).unwrap()), settings());
    assert!(config("http://quest.example.com").is_err());
    assert!(config("https://quest.example.com/sub").is_err());
    assert!(config("https://quest.example.com").is_ok());
    assert!(config("http://127.0.0.1:8787").is_ok());
}

/// A server where `limit` sets one endpoint to `max` requests per hour from one address, behind a
/// proxy that sets `X-Forwarded-For` ([`ClientAddress::LastForwardedFor`]).
async fn limited_server(max: usize, limit: fn(&mut OAuthRateLimits, Window)) -> TestServer {
    limited_server_with(max, ClientAddress::LastForwardedFor, limit).await
}

async fn limited_server_with(
    max: usize,
    client_address: ClientAddress,
    limit: fn(&mut OAuthRateLimits, Window),
) -> TestServer {
    let window = Window { max, per: Duration::from_secs(60 * 60) };
    TestServer::start_custom(setup(move |auth| {
        limit(&mut auth.rate_limits, window);
        auth.client_address = client_address.clone();
    }))
    .await
}

fn retry_after(response: &reqwest::Response) -> u64 {
    response.headers().get("retry-after").expect("Retry-After").to_str().unwrap().parse().unwrap()
}

#[tokio::test]
async fn registrations_are_rate_limited_per_address() {
    let server = limited_server(1, |limits, window| limits.register = window).await;
    OAuthClient::register(&server).await;
    let body = json!({ "client_name": "Flood", "redirect_uris": [REDIRECT_URI], "token_endpoint_auth_method": "none" });
    let response = http().post(server.url("/register")).json(&body).send().await.unwrap();
    let wait = retry_after(&response);
    assert!((3500..=3600).contains(&wait), "{wait}");
    assert_eq!(oauth_error(response).await, (StatusCode::TOO_MANY_REQUESTS, "rate_limited".to_owned()));

    let other = http().post(server.url("/register")).header("x-forwarded-for", "198.51.100.4").json(&body);
    assert_eq!(other.send().await.unwrap().status(), StatusCode::CREATED, "another client behind the proxy");
}

#[tokio::test]
async fn consent_answers_are_rate_limited() {
    let server = limited_server(1, |limits, window| limits.consent = window).await;
    let client = OAuthClient::register(&server).await;
    let params = client.authorize_params(&pkce().1);
    let browser = client.browser(USER);
    let page = client.open(&browser, &params).await;
    let step = client.decide(&browser, &page, &params, "allow", None).await;
    assert!(arrived_query(&step).contains_key("code"));

    let page = client.open(&browser, &params).await;
    let token = csrf(&page);
    let mut form: Vec<(&str, &str)> = params.iter().map(|(name, value)| (*name, value.as_str())).collect();
    form.extend([("csrf", token.as_str()), ("decision", "allow")]);
    let response = browser.http().post(server.url("/authorize")).form(&form).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(retry_after(&response) > 0);
    assert!(response.text().await.unwrap().contains("Too many attempts"));

    let response = browser
        .http()
        .post(server.url("/authorize"))
        .header("x-forwarded-for", "198.51.100.4")
        .form(&form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER, "another client is not limited");
}

#[tokio::test]
async fn the_authorize_page_is_rate_limited() {
    let server = limited_server(1, |limits, window| limits.authorize_page = window).await;
    let client = OAuthClient::register(&server).await;
    let page = || http().get(server.url("/authorize")).query(&client.authorize_params(&pkce().1));
    assert_eq!(page().send().await.unwrap().status(), StatusCode::SEE_OTHER, "off to the identity provider");
    let response = page().send().await.unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(retry_after(&response) > 0);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(response.text().await.unwrap().contains("Too many requests"));
}

#[tokio::test]
async fn sign_in_callbacks_and_token_requests_are_rate_limited() {
    let server = limited_server(1, |limits, window| limits.callback = window).await;
    let callback = || http().get(server.url("/auth/callback?code=x&state=y"));
    assert_eq!(callback().send().await.unwrap().status(), StatusCode::BAD_REQUEST);
    assert_eq!(callback().send().await.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);

    let server = limited_server(1, |limits, window| limits.token = window).await;
    let form = [("grant_type", "refresh_token"), ("refresh_token", "nope"), ("client_id", "x")];
    let token = || http().post(server.url("/token")).form(&form);
    assert_ne!(token().send().await.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);
    let response = token().send().await.unwrap();
    assert!(retry_after(&response) > 0);
    assert_eq!(oauth_error(response).await, (StatusCode::TOO_MANY_REQUESTS, "rate_limited".to_owned()));
}

#[tokio::test]
async fn the_proxy_added_forwarded_line_counts_not_the_clients() {
    let server = limited_server(1, |limits, window| limits.authorize_page = window).await;
    let client = OAuthClient::register(&server).await;
    let page = |spoofed: &str| {
        http()
            .get(server.url("/authorize"))
            .query(&client.authorize_params(&pkce().1))
            .header("x-forwarded-for", spoofed)
            .header("x-forwarded-for", "198.51.100.4")
    };
    assert_eq!(page("203.0.113.1").send().await.unwrap().status(), StatusCode::SEE_OTHER);
    assert_eq!(page("203.0.113.2").send().await.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn forwarded_for_is_ignored_unless_the_operator_trusts_it() {
    let server = limited_server_with(1, ClientAddress::Peer, |limits, window| limits.register = window).await;
    let body = json!({ "client_name": "Flood", "redirect_uris": [REDIRECT_URI], "token_endpoint_auth_method": "none" });
    let register = |claimed: &str| http().post(server.url("/register")).header("x-forwarded-for", claimed).json(&body);
    assert_eq!(register("198.51.100.4").send().await.unwrap().status(), StatusCode::CREATED);
    assert_eq!(register("198.51.100.5").send().await.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);
}

/// Behind an edge proxy every request comes from the proxy's address and carries the edge's
/// `X-Real-IP`: clients count apart by it, and without it the proxy's address counts.
#[tokio::test]
async fn an_edge_header_tells_clients_apart_behind_one_tunnel() {
    let edge = ClientAddress::Header(axum::http::HeaderName::from_static("x-real-ip"));
    let server = limited_server_with(1, edge, |limits, window| limits.register = window).await;
    let body = json!({ "client_name": "Flood", "redirect_uris": [REDIRECT_URI], "token_endpoint_auth_method": "none" });
    let register = |client: Option<&str>| {
        let request = http().post(server.url("/register")).json(&body);
        match client {
            Some(client) => request.header("x-real-ip", client),
            None => request,
        }
    };
    assert_eq!(register(Some("198.51.100.4")).send().await.unwrap().status(), StatusCode::CREATED);
    assert_eq!(register(Some("198.51.100.5")).send().await.unwrap().status(), StatusCode::CREATED);
    assert_eq!(register(Some("198.51.100.4")).send().await.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(register(None).send().await.unwrap().status(), StatusCode::CREATED, "the tunnel's own budget");
    assert_eq!(register(Some("garbage")).send().await.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);
}
