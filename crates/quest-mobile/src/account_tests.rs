use quest_protocol::AccountInfo;
use uuid::Uuid;

use super::*;

fn info(email: Option<&str>, name: Option<&str>, ai_enabled: bool, ai_available: bool) -> quest_store::Account {
    quest_store::Account {
        server_url: "https://quest.example.com/".parse().unwrap(),
        info: AccountInfo {
            user_id: quest_protocol::UserId(Uuid::from_u128(1)),
            issuer: "https://id.example.com".into(),
            subject: "abc".into(),
            email: email.map(Into::into),
            name: name.map(Into::into),
            ai_enabled,
            ai_available,
        },
    }
}

#[test]
fn the_account_is_named_by_email_then_name() {
    let by_email = account(&info(Some("anna@example.com"), Some("Anna"), false, true));
    assert_eq!(
        (by_email.display_name.as_str(), by_email.server_url.as_str()),
        ("anna@example.com", "https://quest.example.com")
    );
    assert_eq!(account(&info(None, Some("Anna"), false, true)).display_name, "Anna");
    assert_eq!(account(&info(None, Some(" "), false, true)).display_name, "Signed in");
}

#[test]
fn ai_features_need_the_opt_in_and_a_configured_server() {
    assert!(!account(&info(None, None, false, true)).ai_features, "off by default");
    assert!(!account(&info(None, None, true, false)).ai_features, "the server has no AI provider");
    assert!(account(&info(None, None, true, true)).ai_features);
}

#[test]
fn only_a_code_or_an_error_is_a_sign_in_answer() {
    let answer = |url: &str| is_sign_in_answer(&url.parse().unwrap());
    assert!(answer("app.quest.android:/oauth2redirect?code=abc&state=s&iss=x"));
    assert!(answer("app.quest.android:/oauth2redirect?error=access_denied&state=s"));
    assert!(!answer("app.quest.android:/oauth2redirect"), "the return from signing out");
}

#[test]
fn only_an_answer_with_the_sign_ins_state_belongs_to_it() {
    let authorize: Url =
        "https://quest.example.com/authorize?client_id=quest-android&state=s1&code_challenge=c".parse().unwrap();
    let answers = |url: &str| answers(&authorize, &url.parse().unwrap());
    assert!(answers("app.quest.android:/oauth2redirect?code=abc&state=s1&iss=x"));
    assert!(answers("app.quest.android:/oauth2redirect?error=access_denied&state=s1"));
    assert!(!answers("app.quest.android:/oauth2redirect?code=abc&state=s2"), "another sign-in's answer");
    assert!(!answers("app.quest.android:/oauth2redirect?code=abc"), "no state");
}

#[test]
fn signing_out_warns_only_about_unsent_changes() {
    assert_eq!(sign_out_warning(0), None);
    assert!(sign_out_warning(1).unwrap().starts_with("1 change has not reached the server yet."));
    assert!(sign_out_warning(3).unwrap().starts_with("3 changes have not reached the server yet."));
}

#[test]
fn store_errors_become_mobile_errors() {
    assert_eq!(sign_in_error(StoreError::SignInCancelled), MobileError::SignInCancelled);
    let other = sign_in_error(StoreError::AnotherAccountActive { active: "anna".into(), signing_in: "ben".into() });
    assert!(matches!(other, MobileError::SignInFailed { reason } if reason.contains("Sign out first")));
    assert!(matches!(sign_in_error(StoreError::TokenStore { message: "gone".into() }), MobileError::Storage { .. }));
    assert_eq!(account_error(AccountError::SessionExpired), MobileError::SessionExpired);
    assert_eq!(account_error(AccountError::SignedOut), MobileError::NotSignedIn);
}
