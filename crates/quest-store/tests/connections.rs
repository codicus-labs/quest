//! Connected AI clients through the store's session against a real, in-process server.

#[allow(dead_code)] // shared harness; each test file uses part of it
mod common;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use common::{TestServer, TestStore};
use quest_oidc_fake::{Browser, Step};
use quest_store::{ConnectionsError, Url};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connections_need_a_session_and_unknown_ones_are_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    let connections = a.store().connections();
    let signed_out = tokio::task::spawn_blocking(move || connections.list());
    assert_eq!(signed_out.await.unwrap(), Err(ConnectionsError::SignedOut));

    a.sign_in(&server, "anna").await.unwrap();
    let connections = a.store().connections();
    let list = tokio::task::spawn_blocking({
        let connections = connections.clone();
        move || connections.list()
    });
    assert_eq!(list.await.unwrap(), Ok(vec![]), "a new account has none");
    let revoke = tokio::task::spawn_blocking(move || connections.revoke(Uuid::from_u128(1)));
    assert_eq!(revoke.await.unwrap(), Err(ConnectionsError::NotFound));
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unreachable_server_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::new(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    a.sign_in(&server, "anna").await.unwrap();
    server.stop().await;
    let connections = a.store().connections();
    let result = tokio::task::spawn_blocking(move || connections.list()).await.unwrap();
    assert!(matches!(result, Err(ConnectionsError::Unreachable { .. })), "{result:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_granted_client_is_listed_with_its_scope_until_it_is_revoked() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = TestServer::with_mcp(dir.path());
    server.start().await;
    let mut a = TestStore::open(&dir.path().join("a"));
    a.sign_in(&server, "anna").await.unwrap();
    grant_access(&server.url(), "anna", "quest:read").await;

    let connections = a.store().connections();
    let list = tokio::task::spawn_blocking({
        let connections = connections.clone();
        move || connections.list()
    });
    let list = list.await.unwrap().unwrap();
    assert_eq!(list.len(), 1, "{list:?}");
    assert_eq!((list[0].client_name.as_str(), list[0].redirect_host.as_str()), ("Test client", "127.0.0.1"));
    assert_eq!(list[0].scope, "quest:read");

    let id = list[0].id;
    let after = tokio::task::spawn_blocking(move || connections.revoke(id).and_then(|()| connections.list()));
    assert_eq!(after.await.unwrap(), Ok(vec![]));
    server.stop().await;
}

/// What an AI client does to connect: registers itself, has the user sign in and allow `access`
/// on the consent page, takes the tokens.
async fn grant_access(server: &Url, username: &str, access: &str) {
    const REDIRECT_URI: &str = "http://127.0.0.1/callback";
    let browser = Browser::new(server.as_str()).login_as(username);
    let http = browser.http();
    let url = |path: &str| server.join(path).unwrap();
    let registration = json!({
        "client_name": "Test client",
        "redirect_uris": [REDIRECT_URI],
        "token_endpoint_auth_method": "none",
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
    });
    let registered: Value = http.post(url("/register")).json(&registration).send().await.unwrap().json().await.unwrap();
    let client_id = registered["client_id"].as_str().expect("a client id").to_owned();
    let verifier = format!("verifier-{}-{}", Uuid::now_v7(), Uuid::now_v7());
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let resource = url("/mcp").to_string();

    let mut authorize = url("/authorize");
    authorize
        .query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &client_id)
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("state", "xyz")
        .append_pair("scope", "quest:write")
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("resource", &resource);
    let Step::Page { body, .. } = browser.navigate(authorize.as_str(), REDIRECT_URI).await.unwrap() else {
        panic!("expected the consent page");
    };
    let mut consent = hidden_fields(&body);
    consent.push(("access".into(), access.into()));
    consent.push(("decision".into(), "allow".into()));
    let consent: Vec<(&str, &str)> = consent.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let Step::Arrived(callback) = browser.submit(url("/authorize").as_str(), &consent, REDIRECT_URI).await.unwrap()
    else {
        panic!("expected the redirect back to the client");
    };
    let code = callback.query_pairs().find(|(k, _)| k == "code").expect("a code").1.into_owned();

    let exchange = [
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", REDIRECT_URI),
        ("code_verifier", &verifier),
        ("resource", &resource),
        ("client_id", &client_id),
    ];
    let tokens = http.post(url("/token")).form(&exchange).send().await.unwrap();
    assert!(tokens.status().is_success(), "{}", tokens.text().await.unwrap());
}

/// The hidden `<input>`s of a server page, unescaped.
fn hidden_fields(html: &str) -> Vec<(String, String)> {
    let attribute = |tag: &str, name: &str| {
        let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
        let end = tag[start..].find('"')? + start;
        Some(unescape(&tag[start..end]))
    };
    html.split("<input")
        .skip(1)
        .filter_map(|rest| rest.split('>').next())
        .filter(|tag| tag.contains("type=\"hidden\""))
        .filter_map(|tag| Some((attribute(tag, "name")?, attribute(tag, "value")?)))
        .collect()
}

fn unescape(text: &str) -> String {
    text.replace("&quot;", "\"").replace("&#39;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}
