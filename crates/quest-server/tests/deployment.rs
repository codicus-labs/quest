//! What the deployment relies on: health probes, the Android App Link (assetlinks.json, the
//! fallback page, the redirect URI on an https origin).

mod common;

use common::oauth::{http, pkce};
use common::{Setup, TestServer};
use quest_protocol::{ANDROID_APP_LINK_PATH, ANDROID_CLIENT_ID, ANDROID_REDIRECT_URI, APP_SCOPE, ASSET_LINKS_PATH};
use reqwest::header::{CACHE_CONTROL, CONTENT_TYPE, LOCATION, REFERRER_POLICY};
use reqwest::{StatusCode, Url};
use serde_json::{Value, json};

const RELEASE_CERT: &str =
    "1F:2E:3D:4C:5B:6A:79:88:97:A6:B5:C4:D3:E2:F1:00:0F:1E:2D:3C:4B:5A:69:78:87:96:A5:B4:C3:D2:E1:F0";
const PUBLIC_URL: &str = "https://quest.example.com";

/// A server published at [`PUBLIC_URL`] (behind a TLS proxy), reached directly in the test.
async fn published() -> TestServer {
    TestServer::start_custom(Setup {
        auth: Some(Box::new(|auth| {
            auth.public_url = Some(Url::parse(PUBLIC_URL).unwrap());
            auth.android_cert_fingerprints = vec![RELEASE_CERT.parse().unwrap()];
        })),
        ..Setup::default()
    })
    .await
}

#[tokio::test]
async fn health_probes_answer_while_the_database_does() {
    let server = TestServer::start().await;
    let get = |path: &str| http().get(server.url(path)).send();
    assert_eq!(get("/health/live").await.unwrap().status(), StatusCode::OK);
    assert_eq!(get("/health/ready").await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn asset_links_list_the_configured_certificates() {
    let server = published().await;
    let response = http().get(server.url(ASSET_LINKS_PATH)).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
    let body: Value = response.json().await.unwrap();
    assert_eq!(body[0]["relation"], json!(["delegate_permission/common.handle_all_urls"]));
    assert_eq!(body[0]["target"]["package_name"], "app.quest.android");
    assert_eq!(body[0]["target"]["sha256_cert_fingerprints"], json!([RELEASE_CERT]));
}

#[tokio::test]
async fn without_certificates_there_are_no_asset_links() {
    let server = TestServer::start().await;
    let response = http().get(server.url(ASSET_LINKS_PATH)).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_app_link_fallback_page_keeps_the_code_to_itself() {
    let server = published().await;
    let url = server.url(&format!("{ANDROID_APP_LINK_PATH}?code=secret-code&state=s&iss={PUBLIC_URL}"));
    let response = http().get(url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[REFERRER_POLICY], "no-referrer");
    let body = response.text().await.unwrap();
    assert!(body.contains("Open Quest to finish signing in"), "{body}");
    assert!(body.contains("href=\"\""), "a tap reopens the same URL: {body}");
    assert!(!body.contains("secret-code"), "{body}");
}

/// Status of `/authorize` for the Android app with `redirect_uri`: 303 to the identity provider
/// if the redirect is allowed, 400 otherwise.
async fn authorize_android(server: &TestServer, redirect_uri: &str) -> StatusCode {
    let (_, challenge) = pkce();
    let resource = format!("{}/v1", server.server.issuer());
    let params = [
        ("response_type", "code"),
        ("client_id", ANDROID_CLIENT_ID),
        ("redirect_uri", redirect_uri),
        ("state", "state-1"),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
        ("scope", APP_SCOPE),
        ("resource", &resource),
    ];
    http().get(server.url("/authorize")).query(&params).send().await.unwrap().status()
}

#[tokio::test]
async fn android_signs_in_through_its_app_link_only_on_an_https_origin() {
    let published = published().await;
    let app_link = format!("{PUBLIC_URL}{ANDROID_APP_LINK_PATH}");
    assert_eq!(authorize_android(&published, &app_link).await, StatusCode::SEE_OTHER);
    assert_eq!(authorize_android(&published, ANDROID_REDIRECT_URI).await, StatusCode::SEE_OTHER);
    let elsewhere = format!("https://other.example.com{ANDROID_APP_LINK_PATH}");
    assert_eq!(authorize_android(&published, &elsewhere).await, StatusCode::BAD_REQUEST);

    let local = TestServer::start().await;
    let local_link = local.url(ANDROID_APP_LINK_PATH);
    assert_eq!(authorize_android(&local, &local_link).await, StatusCode::BAD_REQUEST);
    assert_eq!(authorize_android(&local, ANDROID_REDIRECT_URI).await, StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn sign_out_returns_to_the_app_link() {
    let server = published().await;
    let app_link = format!("{PUBLIC_URL}{ANDROID_APP_LINK_PATH}");
    let mut logout = Url::parse(&server.url("/auth/logout")).unwrap();
    logout.query_pairs_mut().append_pair("return_to", &app_link);
    let response = http().get(logout).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let logged_out = Url::parse(response.headers()[LOCATION].to_str().unwrap()).unwrap();
    assert_eq!(logged_out.query_pairs().find(|(name, _)| name == "state").unwrap().1, app_link);

    // The logged-out page lives at the public origin; reach it directly here.
    let local = server.url(&format!("{}?{}", logged_out.path(), logged_out.query().unwrap()));
    let response = http().get(local).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[LOCATION], app_link.as_str());
}
