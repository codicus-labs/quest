use super::*;

fn url(raw: &str) -> Url {
    Url::parse(raw).unwrap()
}

#[test]
fn resource_and_issuer_come_from_the_public_origin() {
    let public = url("https://Quest.Example.com/");
    assert_eq!(issuer(&public), "https://quest.example.com");
    assert_eq!(canonical_resource(&public), "https://quest.example.com/mcp");
    assert!(same_resource("HTTPS://quest.example.com/mcp/", "https://quest.example.com/mcp"));
    assert!(!same_resource("https://other.example.com/mcp", "https://quest.example.com/mcp"));
    assert!(!same_resource("not a url", "https://quest.example.com/mcp"));
}

#[test]
fn public_url_needs_https_unless_loopback() {
    assert!(check_public_url(&url("https://quest.example.com")).is_ok());
    assert!(check_public_url(&url("http://127.0.0.1:8787")).is_ok());
    assert!(check_public_url(&url("http://localhost:8787")).is_ok());
    assert!(check_public_url(&url("http://quest.example.com")).is_err());
    assert!(check_public_url(&url("https://quest.example.com/base")).is_err());
}

#[test]
fn redirect_uris_must_be_https_or_loopback() {
    assert!(check_redirect_uri("https://claude.ai/api/mcp/auth_callback").is_ok());
    assert!(check_redirect_uri("http://localhost:33418/callback").is_ok());
    assert!(check_redirect_uri("http://127.0.0.1/cb").is_ok());
    assert!(check_redirect_uri("http://[::1]:9/cb").is_ok());
    assert!(check_redirect_uri("http://evil.example/cb").is_err());
    assert!(check_redirect_uri("https://claude.ai/cb#frag").is_err());
    assert!(check_redirect_uri("myapp://callback").is_err());
    assert!(check_redirect_uri("https://user:pw@claude.ai/cb").is_err());
}

#[test]
fn redirects_match_exactly_except_loopback_ports() {
    let registered = vec!["https://claude.ai/cb".to_owned(), "http://127.0.0.1:1234/callback".to_owned()];
    assert!(redirect_allowed(&registered, "https://claude.ai/cb"));
    assert!(!redirect_allowed(&registered, "https://claude.ai/cb/"));
    assert!(!redirect_allowed(&registered, "https://claude.ai/cb?x=1"));
    assert!(redirect_allowed(&registered, "http://127.0.0.1:50123/callback"));
    assert!(!redirect_allowed(&registered, "http://127.0.0.1:50123/other"));
    assert!(!redirect_allowed(&registered, "http://localhost:50123/callback"));
}

#[test]
fn pkce_s256_round_trip() {
    // RFC 7636 appendix B.
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
    assert!(valid_challenge(challenge));
    assert!(pkce_matches(verifier, challenge));
    assert!(!pkce_matches("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXK", challenge));
    assert!(!pkce_matches("short", challenge));
    assert!(!valid_challenge("plain-text-challenge"));
}

#[test]
fn scopes_are_quest_only() {
    assert_eq!(requested_access(None).unwrap(), None);
    assert_eq!(requested_access(Some("offline_access")).unwrap(), None);
    assert_eq!(requested_access(Some("quest:read offline_access")).unwrap(), Some(Access::Read));
    assert_eq!(requested_access(Some("quest:read quest:write")).unwrap(), Some(Access::Write));
    assert_eq!(requested_access(Some("quest")).unwrap(), Some(Access::Write));
    assert!(requested_access(Some("admin")).is_err());
}

#[test]
fn metadata_document_urls_need_https_and_a_path() {
    assert!(metadata_document_url("https://claude.ai/oauth/claude-code-client-metadata").is_some());
    assert!(metadata_document_url("https://claude.ai/").is_none());
    assert!(metadata_document_url("http://claude.ai/client.json").is_none());
    assert!(metadata_document_url("3f2b9c").is_none());
}

#[test]
fn registration_defaults_to_client_secret_basic() {
    let metadata = ClientMetadata { redirect_uris: vec!["https://claude.ai/cb".into()], ..Default::default() };
    assert_eq!(check_registration(&metadata).unwrap().auth, ClientAuth::SecretBasic);
    let public = ClientMetadata { token_endpoint_auth_method: Some("none".into()), ..metadata.clone() };
    assert_eq!(check_registration(&public).unwrap().auth, ClientAuth::None);
    let jwt = ClientMetadata { token_endpoint_auth_method: Some("private_key_jwt".into()), ..metadata.clone() };
    assert!(check_registration(&jwt).is_err());
    let implicit = ClientMetadata { response_types: Some(vec!["token".into()]), ..metadata };
    assert!(check_registration(&implicit).is_err());
    assert!(check_registration(&ClientMetadata::default()).is_err());
}

#[test]
fn metadata_document_must_name_its_own_url() {
    let doc_url = url("https://client.example/meta.json");
    let metadata = ClientMetadata {
        client_id: Some("https://client.example/meta.json".into()),
        client_name: Some("  Example\u{7} Client ".into()),
        redirect_uris: vec!["http://localhost:3000/cb".into()],
        ..Default::default()
    };
    let client = check_metadata_document(&doc_url, &metadata).unwrap();
    assert_eq!(client.client_name, "Example Client");
    assert!(loopback_only(&client.redirect_uris));
    let other = ClientMetadata { client_id: Some("https://evil.example/meta.json".into()), ..metadata };
    assert!(check_metadata_document(&doc_url, &other).is_err());
}

#[test]
fn write_includes_read_and_grants_parse_back() {
    assert!(Access::Write > Access::Read);
    assert_eq!(Access::of_grant("quest:write"), Access::Write);
    assert_eq!(Access::of_grant("quest:read"), Access::Read);
    assert_eq!(Access::of_grant(""), Access::Read);
}

#[test]
fn apps_get_the_app_resource_and_mcp_clients_the_mcp_resource() {
    let (app, mcp) = ("http://127.0.0.1:8787/v1", "http://127.0.0.1:8787/mcp");
    assert_eq!(requested_resource(None, true, app, mcp), Ok(Resource::App));
    assert_eq!(requested_resource(Some(app), true, app, mcp), Ok(Resource::App));
    assert!(requested_resource(Some(mcp), true, app, mcp).is_err());
    assert_eq!(requested_resource(None, false, app, mcp), Ok(Resource::Mcp));
    assert_eq!(requested_resource(Some("http://127.0.0.1:8787/mcp/"), false, app, mcp), Ok(Resource::Mcp));
    assert!(requested_resource(Some(app), false, app, mcp).is_err());
    assert!(requested_resource(Some("https://elsewhere.example/v1"), true, app, mcp).is_err());
}

#[test]
fn first_party_clients_are_built_in_with_their_redirects() {
    let issuer = "http://127.0.0.1:8787";
    let desktop = first_party_client("quest-desktop", issuer).unwrap();
    assert!(desktop.first_party && desktop.secret_hash.is_none());
    assert!(redirect_allowed(&desktop.redirect_uris, "http://127.0.0.1:53111/callback"));
    assert!(!redirect_allowed(&desktop.redirect_uris, "http://127.0.0.1:53111/other"));
    assert!(!redirect_allowed(&desktop.redirect_uris, "https://evil.example/callback"));
    let android = first_party_client("quest-android", issuer).unwrap();
    assert!(redirect_allowed(&android.redirect_uris, "app.quest.android:/oauth2redirect"));
    assert!(first_party_redirect("app.quest.android:/oauth2redirect", issuer));
    assert!(!first_party_redirect("https://evil.example/", issuer));
    assert!(first_party_client("claude", issuer).is_none());
}

#[test]
fn android_may_use_its_app_link_only_on_an_https_origin() {
    let https = "https://quest.example.com";
    let android = first_party_client("quest-android", https).unwrap();
    assert!(redirect_allowed(&android.redirect_uris, "app.quest.android:/oauth2redirect"));
    assert!(redirect_allowed(&android.redirect_uris, "https://quest.example.com/auth/android"));
    assert!(!redirect_allowed(&android.redirect_uris, "https://other.example.com/auth/android"));
    assert!(!redirect_allowed(&android.redirect_uris, "https://quest.example.com/auth/android/x"));
    assert!(first_party_redirect("https://quest.example.com/auth/android", https));
    let desktop = first_party_client("quest-desktop", https).unwrap();
    assert!(!redirect_allowed(&desktop.redirect_uris, "https://quest.example.com/auth/android"));

    let loopback = "http://127.0.0.1:8787";
    let android = first_party_client("quest-android", loopback).unwrap();
    assert!(!redirect_allowed(&android.redirect_uris, "http://127.0.0.1:8787/auth/android"));
    assert!(!first_party_redirect("http://127.0.0.1:8787/auth/android", loopback));
}

#[test]
fn private_use_redirects_are_claimable() {
    assert!(claimable_redirect(quest_protocol::ANDROID_REDIRECT_URI));
    assert!(!claimable_redirect("http://127.0.0.1:49152/callback"));
    assert!(!claimable_redirect("https://app.example.com/cb"));
    assert!(!claimable_redirect("https://quest.example.com/auth/android"));
    assert!(claimable_redirect("not a uri"));
}
