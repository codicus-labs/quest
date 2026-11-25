//! Sign-in at the identity provider (the fake) for the Quest apps: ID token checks, login CSRF,
//! app tokens for `/v1` and sync, refresh, revocation, sign-out and account switch.

mod common;

use common::oauth::{arrived_query, csrf, http, oauth_error, page_status};
use common::{Setup, TestServer};
use quest_oidc_fake::{AppAuthorization, AppClient, Browser, Fault, Step};
use quest_protocol::{DESKTOP_CLIENT_ID, ErrorCode};
use reqwest::StatusCode;

fn server_url(server: &TestServer) -> String {
    server.server.http_url()
}

/// Opens a desktop app authorization in `browser`; returns it and where navigation stopped.
async fn open_app(server: &TestServer, browser: &Browser) -> (AppAuthorization, Step) {
    let authorization = AppAuthorization::new(&server_url(server), AppClient::Desktop);
    let step = browser.navigate(&authorization.url, &authorization.redirect_uri).await.unwrap();
    (authorization, step)
}

async fn finish(server: &TestServer, browser: &Browser, authorization: &AppAuthorization, step: &Step) -> String {
    let Step::Arrived(callback) = step else { panic!("expected the redirect back to the app, got {step:?}") };
    let tokens = authorization.finish(browser.http(), callback).await.unwrap();
    let account = server.account(&tokens.access_token).await.unwrap();
    account.email.unwrap()
}

#[tokio::test]
async fn app_sign_in_creates_one_account_per_identity() {
    let server = TestServer::start().await;
    let first = server.sign_in("anna").await;
    let again = server.sign_in("anna").await;
    let other = server.sign_in("ben").await;
    let anna = server.account(&first.token).await.unwrap();
    assert_eq!(anna, server.account(&again.token).await.unwrap(), "the same identity is the same account");
    assert_eq!(anna.issuer, server.fake.issuer());
    assert_eq!(anna.subject, server.fake.subject("anna"));
    assert_eq!(anna.email.as_deref(), Some("anna@example.test"));
    assert!(!anna.ai_enabled && !anna.ai_available);
    assert_ne!(server.account(&other.token).await.unwrap().user_id, anna.user_id);
}

#[tokio::test]
async fn the_android_app_signs_in_with_its_private_use_redirect() {
    let server = TestServer::start().await;
    // Any app may claim the scheme: even right after a fresh sign-in the user confirms.
    let browser = Browser::new(&server_url(&server)).login_as("anna");
    let authorization = AppAuthorization::new(&server_url(&server), AppClient::Android);
    let page = browser.navigate(&authorization.url, &authorization.redirect_uri).await.unwrap();
    let Step::Page { status: 200, body, .. } = &page else { panic!("expected the continue page, got {page:?}") };
    assert!(body.contains("anna@example.test"), "{body}");
    let callback = browser.authorize(&authorization.url, &authorization.redirect_uri).await.unwrap();
    let tokens = authorization.finish(browser.http(), &callback).await.unwrap();
    assert!(server.account(&tokens.access_token).await.is_ok());
    let session = server.sign_in_as("anna", AppClient::Android).await;
    assert!(server.account(&session.token).await.is_ok());
}

#[tokio::test]
async fn invalid_id_tokens_are_refused() {
    let server = TestServer::start().await;
    for fault in [Fault::WrongNonce, Fault::WrongAudience, Fault::WrongIssuer, Fault::Expired, Fault::WrongKey] {
        let browser = Browser::new(&server_url(&server)).login_as("anna").with_idp_param("fault", fault.as_str());
        let (_, step) = open_app(&server, &browser).await;
        let Step::Page { status, body, .. } = &step else { panic!("{fault:?}: expected a page, got {step:?}") };
        assert_eq!(*status, 401, "{fault:?}");
        assert!(body.contains("Sign-in failed"), "{fault:?}: {body}");

        // No session came of it: the next attempt signs in at the provider again.
        let browser = Browser::new(&server_url(&server)).login_as("anna");
        let (authorization, step) = open_app(&server, &browser).await;
        assert_eq!(finish(&server, &browser, &authorization, &step).await, "anna@example.test", "{fault:?}");
    }
}

#[tokio::test]
async fn callbacks_with_an_unknown_or_used_state_are_refused() {
    let server = TestServer::start().await;
    let response = http().get(server.url("/auth/callback?code=abc&state=forged")).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = http().get(server.url("/auth/callback?code=abc")).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Stop at the provider's redirect back, use it once, then replay it.
    let browser = Browser::new(&server_url(&server)).login_as("anna");
    let authorization = AppAuthorization::new(&server_url(&server), AppClient::Desktop);
    let callback_prefix = server.url("/auth/callback");
    let Step::Arrived(callback) = browser.navigate(&authorization.url, &callback_prefix).await.unwrap() else {
        panic!("expected the provider's redirect to the callback");
    };
    let step = browser.navigate(callback.as_str(), &authorization.redirect_uri).await.unwrap();
    assert!(arrived_query(&step).contains_key("code"));
    let replayed = browser.navigate(callback.as_str(), &authorization.redirect_uri).await.unwrap();
    assert_eq!(page_status(&replayed), 400, "each sign-in is used once");
}

#[tokio::test]
async fn a_callback_in_another_browser_is_refused() {
    // Login CSRF: an attacker starts a sign-in and sends the victim the provider's answer.
    let server = TestServer::start().await;
    let attacker = Browser::new(&server_url(&server)).login_as("mallory");
    let authorization = AppAuthorization::new(&server_url(&server), AppClient::Desktop);
    let Step::Arrived(callback) = attacker.navigate(&authorization.url, &server.url("/auth/callback")).await.unwrap()
    else {
        panic!("expected the provider's redirect to the callback");
    };
    let victim = Browser::new(&server_url(&server));
    let step = victim.navigate(callback.as_str(), &authorization.redirect_uri).await.unwrap();
    let Step::Page { status, body, .. } = step else { panic!("expected a page") };
    assert_eq!(status, 400);
    assert!(body.contains("another browser"), "{body}");
}

#[tokio::test]
async fn app_tokens_refresh_with_rotation_and_revoke() {
    let server = TestServer::start().await;
    let session = server.sign_in("anna").await;
    let refresh = |token: String| {
        let form = [("grant_type", "refresh_token"), ("refresh_token", &token), ("client_id", DESKTOP_CLIENT_ID)];
        http().post(server.url("/token")).form(&form).send()
    };
    let response = refresh(session.refresh.clone()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let rotated: quest_protocol::TokenResponse = response.json().await.unwrap();
    assert_eq!(rotated.scope, "quest:write");
    let new_refresh = rotated.refresh_token.unwrap();
    assert_ne!(new_refresh, session.refresh);
    assert!(server.account(&rotated.access_token).await.is_ok());

    let form = [("token", new_refresh.as_str()), ("client_id", DESKTOP_CLIENT_ID)];
    let response = http().post(server.url("/revoke")).form(&form).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(server.account(&rotated.access_token).await, Err(401), "revoking ends the app's grant");
    let (_, error) = oauth_error(refresh(new_refresh).await.unwrap()).await;
    assert_eq!(error, "invalid_grant");
    assert!(server.connect_raw(Some(&rotated.access_token)).await.is_err(), "sync too");
}

#[tokio::test]
async fn sync_needs_a_valid_app_token() {
    let server = TestServer::start().await;
    assert!(server.connect_raw(None).await.is_err());
    assert!(server.connect_raw(Some("not-a-token")).await.is_err());
    let session = server.sign_in("anna").await;
    let (_, resumed) = server.connect(&session, None).await;
    assert!(resumed.changes.is_empty());
}

#[tokio::test]
async fn expired_app_tokens_are_refused() {
    let setup =
        Setup { auth: Some(Box::new(|auth| auth.access_token_ttl = std::time::Duration::ZERO)), ..Setup::default() };
    let server = TestServer::start_custom(setup).await;
    let session = server.sign_in("anna").await;
    let response = http().get(server.url("/v1/account")).bearer_auth(&session.token).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let challenge = response.headers()["www-authenticate"].to_str().unwrap().to_owned();
    assert!(challenge.contains("invalid_token"), "{challenge}");
    let error: quest_protocol::ApiError = response.json().await.unwrap();
    assert_eq!(error.code, ErrorCode::Unauthorized);
    assert!(server.connect_raw(Some(&session.token)).await.is_err());
}

#[tokio::test]
async fn an_existing_session_asks_to_continue_or_switch_accounts() {
    let server = TestServer::start().await;
    let mut browser = Browser::new(&server_url(&server)).login_as("anna");
    let (authorization, step) = open_app(&server, &browser).await;
    assert_eq!(finish(&server, &browser, &authorization, &step).await, "anna@example.test", "no page after sign-in");

    // With the session the app gets no code silently.
    let (authorization, page) = open_app(&server, &browser).await;
    let Step::Page { status: 200, body, .. } = &page else { panic!("expected the continue page, got {page:?}") };
    assert!(body.contains("anna@example.test"), "{body}");
    let form = continue_form(&authorization, &page, "allow");
    let step = browser.submit(&server.url("/authorize"), &pairs(&form), &authorization.redirect_uri).await.unwrap();
    assert_eq!(finish(&server, &browser, &authorization, &step).await, "anna@example.test");

    // "Use another account" ends the session and signs in again at the provider.
    let (authorization, page) = open_app(&server, &browser).await;
    browser.switch_user("ben");
    let form = continue_form(&authorization, &page, "switch");
    let step = browser.submit(&server.url("/authorize"), &pairs(&form), &authorization.redirect_uri).await.unwrap();
    assert_eq!(finish(&server, &browser, &authorization, &step).await, "ben@example.test");

    // Cancel goes back to the app with access_denied.
    let (authorization, page) = open_app(&server, &browser).await;
    let form = continue_form(&authorization, &page, "deny");
    let step = browser.submit(&server.url("/authorize"), &pairs(&form), &authorization.redirect_uri).await.unwrap();
    assert_eq!(arrived_query(&step).get("error").map(String::as_str), Some("access_denied"));
}

/// The fields of the "Continue as" form for `authorization`.
fn continue_form(authorization: &AppAuthorization, page: &Step, decision: &str) -> Vec<(String, String)> {
    let url = reqwest::Url::parse(&authorization.url).unwrap();
    let mut form: Vec<(String, String)> = url.query_pairs().into_owned().collect();
    form.push(("csrf".into(), csrf(page)));
    form.push(("decision".into(), decision.into()));
    form
}

fn pairs(form: &[(String, String)]) -> Vec<(&str, &str)> {
    form.iter().map(|(name, value)| (name.as_str(), value.as_str())).collect()
}

#[tokio::test]
async fn sign_out_ends_the_session_here_and_at_the_provider() {
    let server = TestServer::start().await;
    let browser = Browser::new(&server_url(&server)).login_as("anna");
    let (authorization, step) = open_app(&server, &browser).await;
    finish(&server, &browser, &authorization, &step).await;

    let return_to = &authorization.redirect_uri;
    let mut logout = reqwest::Url::parse(&server.url("/auth/logout")).unwrap();
    logout.query_pairs_mut().append_pair("return_to", return_to);
    let step = browser.navigate(logout.as_str(), return_to).await.unwrap();
    assert!(matches!(step, Step::Arrived(_)), "back in the app, got {step:?}");
    assert_eq!(server.fake.logouts(), 1, "signed out at the provider too");

    // No session any more: the next app sign-in goes to the provider, without the continue page.
    let (authorization, step) = open_app(&server, &browser).await;
    assert!(matches!(step, Step::Arrived(_)), "{step:?}");
    finish(&server, &browser, &authorization, &step).await;

    // Other return addresses are ignored: the browser ends on the "Signed out" page.
    let step = browser.navigate(&server.url("/auth/logout?return_to=https://evil.example.com/"), "https://evil").await;
    let Step::Page { status: 200, body, .. } = step.unwrap() else { panic!("expected the signed-out page") };
    assert!(body.contains("Signed out"), "{body}");
}

#[tokio::test]
async fn parallel_sign_ins_in_one_browser_do_not_overwrite_each_other() {
    let server = TestServer::start().await;
    let browser = Browser::new(&server_url(&server)).login_as("anna");
    let callback = server.url("/auth/callback");
    let first = AppAuthorization::new(&server_url(&server), AppClient::Desktop);
    let second = AppAuthorization::new(&server_url(&server), AppClient::Desktop);
    let Step::Arrived(first_callback) = browser.navigate(&first.url, &callback).await.unwrap() else {
        panic!("expected the provider's redirect to the callback");
    };
    let Step::Arrived(second_callback) = browser.navigate(&second.url, &callback).await.unwrap() else {
        panic!("expected the provider's redirect to the callback");
    };
    // The older sign-in finishes after the newer one started.
    let step = browser.navigate(first_callback.as_str(), &first.redirect_uri).await.unwrap();
    assert_eq!(finish(&server, &browser, &first, &step).await, "anna@example.test");
    let step = browser.navigate(second_callback.as_str(), &second.redirect_uri).await.unwrap();
    assert_eq!(finish(&server, &browser, &second, &step).await, "anna@example.test");
}
