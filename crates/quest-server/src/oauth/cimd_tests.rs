use super::*;

#[test]
fn only_public_addresses_are_fetched() {
    for blocked in [
        "127.0.0.1",
        "10.1.2.3",
        "192.168.0.1",
        "169.254.169.254",
        "100.64.0.1",
        "::1",
        "fd00::1",
        "fe80::1",
        "::ffff:127.0.0.1",
        "0.0.0.0",
    ] {
        assert!(!is_public(blocked.parse().unwrap()), "{blocked}");
    }
    for allowed in ["1.1.1.1", "160.79.104.10", "2606:4700::1111"] {
        assert!(is_public(allowed.parse().unwrap()), "{allowed}");
    }
}

#[test]
fn cache_control_sets_the_lifetime() {
    assert_eq!(max_age("public, max-age=600"), Some(Duration::from_secs(600)));
    assert_eq!(max_age("no-store"), Some(Duration::ZERO));
    assert_eq!(max_age("public"), None);
}
