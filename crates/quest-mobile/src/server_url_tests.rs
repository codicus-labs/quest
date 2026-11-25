use super::*;

#[test]
fn https_is_always_allowed() {
    for url in ["https://quest.example.com", "https://10.0.0.5:8443/api", "https://[2001:db8::1]"] {
        assert!(parse_server_url(url, UrlPolicy::Development).is_ok(), "{url}");
    }
}

#[test]
fn plain_http_is_allowed_for_loopback_and_the_emulator_host() {
    for url in [
        "http://localhost:8787",
        "http://LOCALHOST",
        "http://127.0.0.1:8787",
        "http://127.1.2.3",
        "http://[::1]:8787",
        "http://10.0.2.2:8787",
    ] {
        assert!(parse_server_url(url, UrlPolicy::Development).is_ok(), "{url}");
    }
}

#[test]
fn release_builds_allow_plain_http_only_to_loopback() {
    for url in ["http://localhost:8787", "http://127.0.0.1:8787", "http://[::1]:8787"] {
        assert!(parse_server_url(url, UrlPolicy::Release).is_ok(), "{url}");
    }
    let emulator_host = "http://10.0.2.2:8787";
    assert_eq!(
        parse_server_url(emulator_host, UrlPolicy::Release),
        Err(MobileError::InsecureServerUrl { value: emulator_host.into() })
    );
}

#[test]
fn plain_http_elsewhere_is_refused_as_insecure() {
    for url in [
        "http://quest.example.com",
        "http://10.0.2.3:8787",
        "http://192.168.1.10",
        "http://localhost.evil.com",
        "http://10.0.2.2.evil.com",
        "http://[::ffff:127.0.0.1]",
    ] {
        assert_eq!(
            parse_server_url(url, UrlPolicy::Development),
            Err(MobileError::InsecureServerUrl { value: url.into() }),
            "{url}"
        );
    }
}

#[test]
fn the_insecure_message_says_what_to_do() {
    let message = parse_server_url("http://quest.example.com", UrlPolicy::Release).unwrap_err().to_string();
    assert!(message.contains("Use an https:// address"), "{message}");
}

#[test]
fn malformed_urls_and_other_schemes_are_invalid() {
    assert!(matches!(parse_server_url("not a url", UrlPolicy::Release), Err(MobileError::InvalidServerUrl { .. })));
    let ftp = parse_server_url("ftp://quest.example.com", UrlPolicy::Release).unwrap_err();
    assert!(matches!(ftp, MobileError::InvalidServerUrl { reason, .. } if reason.contains("\"ftp\"")));
}

#[test]
fn urls_with_a_user_name_or_password_are_refused_without_echoing_them() {
    for url in ["https://ada:secret@quest.example.com", "https://ada@quest.example.com", "http://:secret@localhost"] {
        assert_eq!(parse_server_url(url, UrlPolicy::Development), Err(MobileError::ServerUrlWithCredentials), "{url}");
    }
    let message = MobileError::ServerUrlWithCredentials.to_string();
    assert!(message.contains("Remove the part before"), "{message}");
}
