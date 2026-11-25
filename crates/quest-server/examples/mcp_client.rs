//! A headless MCP client for trying a local server's MCP end to end: discovery from the 401,
//! dynamic registration, PKCE, sign-in at the identity provider + consent (as a browser would,
//! with `quest_oidc_fake::Browser`), token exchange, then a few tool calls with rmcp. Only for
//! loopback servers whose identity provider is `quest-oidc-fake` (`mise run oidc-fake`).
//!
//! ```sh
//! cargo run -p quest-server --example mcp_client                       # http://127.0.0.1:8799
//! QUEST_MCP_SERVER=http://127.0.0.1:8787 QUEST_MCP_USER=ben QUEST_MCP_ACCESS=quest:read \
//!   cargo run -p quest-server --example mcp_client
//! ```

use std::env;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use quest_oidc_fake::{Browser, Step};
use reqwest::Url;
use reqwest::redirect::Policy;
use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const REDIRECT_URI: &str = "http://127.0.0.1/callback";

#[tokio::main]
async fn main() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let server = env::var("QUEST_MCP_SERVER").unwrap_or_else(|_| "http://127.0.0.1:8799".into());
    let user = env::var("QUEST_MCP_USER").unwrap_or_else(|_| "anna".into());
    let access = env::var("QUEST_MCP_ACCESS").unwrap_or_else(|_| "quest:write".into());
    let time_zone = env::var("QUEST_MCP_TIME_ZONE").unwrap_or_else(|_| "Europe/Berlin".into());
    assert!(
        Url::parse(&server).unwrap().host_str().is_some_and(|h| h == "127.0.0.1" || h == "localhost"),
        "loopback only"
    );
    let http = reqwest::Client::builder().redirect(Policy::none()).build().unwrap();

    // 1. Discovery.
    let unauthorized = http.post(format!("{server}/mcp")).json(&json!({})).send().await.unwrap();
    let challenge = unauthorized.headers()["www-authenticate"].to_str().unwrap().to_owned();
    println!("401 challenge: {challenge}");
    let metadata_url = challenge.split('"').nth(1).unwrap();
    let resource: Value = http.get(metadata_url).send().await.unwrap().json().await.unwrap();
    let issuer = resource["authorization_servers"][0].as_str().unwrap();
    let auth: Value = http
        .get(format!("{issuer}/.well-known/oauth-authorization-server"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    println!("resource {} · issuer {issuer}", resource["resource"]);

    // 2. Registration.
    let registration = json!({ "client_name": "mcp_client example", "redirect_uris": [REDIRECT_URI], "token_endpoint_auth_method": "none" });
    let registered: Value = http
        .post(auth["registration_endpoint"].as_str().unwrap())
        .json(&registration)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let client_id = registered["client_id"].as_str().unwrap().to_owned();
    println!("registered client {client_id}");

    // 3. Authorization with PKCE: sign-in at the fake provider, then the consent form.
    let verifier = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let resource_url = resource["resource"].as_str().unwrap().to_owned();
    let params = [
        ("response_type", "code"),
        ("client_id", &client_id),
        ("redirect_uri", REDIRECT_URI),
        ("state", "demo"),
        ("scope", "quest:write"),
        ("code_challenge", &code_challenge),
        ("code_challenge_method", "S256"),
        ("resource", &resource_url),
    ];
    let authorization_endpoint = auth["authorization_endpoint"].as_str().unwrap();
    let mut authorize_url = Url::parse(authorization_endpoint).unwrap();
    authorize_url.query_pairs_mut().extend_pairs(params);
    let browser = Browser::new(&server).login_as(&user);
    let page = browser.navigate(authorize_url.as_str(), REDIRECT_URI).await.unwrap();
    let Step::Page { status: 200, body, .. } = page else { panic!("expected the consent page, got {page:?}") };
    let csrf = body.split("name=\"csrf\" value=\"").nth(1).and_then(|rest| rest.split('"').next()).unwrap();
    let mut form = params.to_vec();
    form.extend([("csrf", csrf), ("decision", "allow"), ("access", &access)]);
    let Step::Arrived(location) = browser.submit(authorization_endpoint, &form, REDIRECT_URI).await.unwrap() else {
        panic!("the consent did not redirect back");
    };
    let query: std::collections::HashMap<_, _> = location.query_pairs().into_owned().collect();
    println!("signed in as {user}; redirect: state={} iss={}", query["state"], query["iss"]);

    // 4. Tokens.
    let token_form = [
        ("grant_type", "authorization_code"),
        ("code", query["code"].as_str()),
        ("redirect_uri", REDIRECT_URI),
        ("code_verifier", &verifier),
        ("client_id", &client_id),
        ("resource", &resource_url),
    ];
    let tokens: Value = http
        .post(auth["token_endpoint"].as_str().unwrap())
        .form(&token_form)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    println!("tokens: Bearer, expires_in {}s, scope {}", tokens["expires_in"], tokens["scope"]);

    // 5. MCP.
    let config = StreamableHttpClientTransportConfig::with_uri(resource_url)
        .auth_header(tokens["access_token"].as_str().unwrap().to_owned());
    let client = ().serve(StreamableHttpClientTransport::from_config(config)).await.expect("MCP handshake");
    let tools = client.list_all_tools().await.unwrap();
    println!("{} tools: {}", tools.len(), tools.iter().map(|t| t.name.as_ref()).collect::<Vec<_>>().join(", "));
    let call = |name: &'static str, mut arguments: Value| {
        arguments["time_zone"] = json!(time_zone);
        let params = CallToolRequestParams::new(name).with_arguments(arguments.as_object().unwrap().clone());
        let client = &client;
        async move {
            let result = client.call_tool(params).await.unwrap();
            let shown =
                result.structured_content.clone().unwrap_or_else(|| json!(result.content[0].as_text().unwrap().text));
            println!("{name}{}: {shown}", if result.is_error == Some(true) { " (error)" } else { "" });
            shown
        }
    };
    let quest = call("create_quest", json!({ "title": "MCP demo" })).await;
    let task = call("create_task", json!({ "title": "Try Quest over MCP", "quest_id": quest["quest"]["id"], "block": { "start": "16:00", "minutes": 30 } })).await;
    call("list_tasks", json!({})).await;
    call("complete_task", json!({ "task_id": task["task"]["id"] })).await;
    call("get_task", json!({ "task_id": "not-an-id" })).await;
    client.cancel().await.unwrap();
}
