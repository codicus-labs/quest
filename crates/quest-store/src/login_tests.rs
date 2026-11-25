use std::collections::HashMap;

use super::*;

fn request(client: AppClient) -> LoginRequest {
    let server: Url = "https://quest.example.com/".parse().unwrap();
    LoginRequest::with_secrets(
        &server,
        client,
        "the-state".into(),
        "dBjftJeZ4CVP-mJ92K9a1qQ6ccB2xmbJ5Zt9Zq0yD9U".into(),
    )
    .unwrap()
}

fn query(url: &Url) -> HashMap<String, String> {
    url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect()
}

fn callback(query: &str) -> Url {
    format!("http://127.0.0.1:4711/callback?{query}").parse().unwrap()
}

#[test]
fn the_authorize_url_asks_for_the_app_resource_with_pkce() {
    let login = request(AppClient::Desktop { redirect_port: 4711 });
    let url = login.authorize_url();
    assert_eq!(url.as_str().split('?').next(), Some("https://quest.example.com/authorize"));
    let query = query(url);
    let expected = [
        ("response_type", "code"),
        ("client_id", "quest-desktop"),
        ("redirect_uri", "http://127.0.0.1:4711/callback"),
        ("scope", "quest:write offline_access"),
        ("state", "the-state"),
        // base64url(sha256(verifier)), computed independently with Python's hashlib.
        ("code_challenge", "NBraieBwvjmPESGopaNqTB2Ebqdrw2gZo7be5Jc0TB8"),
        ("code_challenge_method", "S256"),
        ("resource", "https://quest.example.com/v1"),
    ];
    for (key, value) in expected {
        assert_eq!(query.get(key).map(String::as_str), Some(value), "{key}");
    }
}

#[test]
fn android_uses_its_private_scheme_redirect() {
    let login = request(AppClient::Android);
    let query = query(login.authorize_url());
    assert_eq!(query["client_id"], "quest-android");
    assert_eq!(query["redirect_uri"], "app.quest.android:/oauth2redirect");
    let code = login.authorization_code(
        &"app.quest.android:/oauth2redirect?code=c1&state=the-state&iss=https%3A%2F%2Fquest.example.com"
            .parse()
            .unwrap(),
    );
    assert_eq!(code, Ok("c1".into()));
}

#[test]
fn every_request_gets_fresh_secrets() {
    let server: Url = "http://127.0.0.1:8787".parse().unwrap();
    let a = LoginRequest::new(&server, AppClient::Android).unwrap();
    let b = LoginRequest::new(&server, AppClient::Android).unwrap();
    assert_ne!(a.state, b.state);
    assert_ne!(a.code_verifier, b.code_verifier);
    assert_eq!(a.code_verifier.len(), 43);
}

#[test]
fn a_path_in_the_server_url_is_ignored() {
    let server: Url = "http://127.0.0.1:8787/some/path".parse().unwrap();
    let login = LoginRequest::new(&server, AppClient::Android).unwrap();
    assert_eq!(login.server_url().as_str(), "http://127.0.0.1:8787/");
    assert_eq!(query(login.authorize_url())["resource"], "http://127.0.0.1:8787/v1");
}

#[test]
fn other_schemes_are_refused() {
    let server: Url = "ftp://quest.example.com".parse().unwrap();
    assert!(matches!(LoginRequest::new(&server, AppClient::Android), Err(StoreError::InvalidServerUrl { .. })));
}

#[test]
fn the_matching_redirect_yields_the_code() {
    let login = request(AppClient::Desktop { redirect_port: 4711 });
    let answer = callback("code=abc&state=the-state&iss=https%3A%2F%2Fquest.example.com");
    assert_eq!(login.authorization_code(&answer), Ok("abc".into()));
}

#[test]
fn a_foreign_state_issuer_or_address_is_refused() {
    let login = request(AppClient::Desktop { redirect_port: 4711 });
    let refused = [
        callback("code=abc&state=other"),
        callback("code=abc&state=the-state"),
        callback("code=abc"),
        callback("code=abc&state=the-state&iss=https%3A%2F%2Fevil.example.com"),
        "http://127.0.0.1:9999/callback?code=abc&state=the-state".parse().unwrap(),
        "http://127.0.0.1:4711/elsewhere?code=abc&state=the-state".parse().unwrap(),
        callback("state=the-state"),
    ];
    for answer in refused {
        assert!(
            matches!(login.authorization_code(&answer), Err(StoreError::SignInFailed { .. })),
            "{answer} must be refused"
        );
    }
}

#[test]
fn an_error_redirect_reports_cancel_or_reason() {
    let login = request(AppClient::Desktop { redirect_port: 4711 });
    assert_eq!(
        login.authorization_code(&callback("error=access_denied&state=the-state&iss=https%3A%2F%2Fquest.example.com")),
        Err(StoreError::SignInCancelled)
    );
    let Err(StoreError::SignInFailed { reason }) = login.authorization_code(&callback(
        "error=server_error&error_description=boom&state=the-state&iss=https%3A%2F%2Fquest.example.com",
    )) else {
        panic!("expected a failure");
    };
    assert!(reason.contains("server_error") && reason.contains("boom"), "{reason}");
}

#[test]
fn android_uses_the_app_link_at_the_server_it_is_verified_for() {
    let app_link = Some("https://quest.example.com");
    let server = |url: &str| url.parse::<Url>().unwrap();
    assert_eq!(AppClient::android(&server("https://quest.example.com/"), app_link), AppClient::AndroidAppLink);
    assert_eq!(AppClient::android(&server("https://QUEST.example.com:443/x"), app_link), AppClient::AndroidAppLink);
    assert_eq!(
        AppClient::android(&server("https://quest.example.com"), Some("https://quest.example.com/")),
        AppClient::AndroidAppLink
    );
    for other in ["https://other.example.com", "https://quest.example.com:8443", "http://quest.example.com"] {
        assert_eq!(AppClient::android(&server(other), app_link), AppClient::Android, "{other}");
    }
    assert_eq!(AppClient::android(&server("https://quest.example.com"), None), AppClient::Android, "debug builds");
    assert_eq!(
        AppClient::android(&server("http://quest.example.com"), Some("http://quest.example.com")),
        AppClient::Android
    );
    assert_eq!(AppClient::android(&server("https://quest.example.com"), Some("not a url")), AppClient::Android);
}

#[test]
fn the_app_link_redirect_is_the_servers_own_origin() {
    let login = request(AppClient::AndroidAppLink);
    let query = query(login.authorize_url());
    assert_eq!(query["client_id"], "quest-android");
    assert_eq!(query["redirect_uri"], "https://quest.example.com/auth/android");
    let answer = "code=c1&state=the-state&iss=https%3A%2F%2Fquest.example.com";
    let code = login.authorization_code(&format!("https://quest.example.com/auth/android?{answer}").parse().unwrap());
    assert_eq!(code, Ok("c1".into()));
    for elsewhere in [
        format!("app.quest.android:/oauth2redirect?{answer}"),
        format!("https://evil.example.com/auth/android?{answer}"),
        format!("http://quest.example.com/auth/android?{answer}"),
        format!("https://quest.example.com/auth/other?{answer}"),
    ] {
        assert!(login.authorization_code(&elsewhere.parse().unwrap()).is_err(), "{elsewhere}");
    }
}

#[test]
fn the_logout_page_returns_to_the_android_app_only() {
    let server: Url = "https://quest.example.com".parse().unwrap();
    let android = AppClient::Android.logout_url(&server).unwrap();
    assert_eq!(
        android.as_str(),
        "https://quest.example.com/auth/logout?return_to=app.quest.android%3A%2Foauth2redirect"
    );
    assert_eq!(AppClient::AndroidAppLink.logout_url(&server).unwrap(), android, "no secret: the private scheme");
    let desktop = AppClient::Desktop { redirect_port: 1 }.logout_url(&server).unwrap();
    assert_eq!(desktop.as_str(), "https://quest.example.com/auth/logout");
}
