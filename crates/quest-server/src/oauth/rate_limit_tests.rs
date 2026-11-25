use super::*;

const SECOND: Duration = Duration::from_secs(1);

fn limits() -> OAuthRateLimits {
    OAuthRateLimits {
        register: Window { max: 2, per: Duration::from_secs(60) },
        authorize_page: Window { max: 3, per: Duration::from_secs(60) },
        consent: Window { max: 1, per: Duration::from_secs(600) },
        token: Window { max: 5, per: Duration::from_secs(60) },
        callback: Window { max: 5, per: Duration::from_secs(60) },
    }
}

fn ip(raw: &str) -> IpAddr {
    raw.parse().unwrap()
}

#[test]
fn allows_the_window_then_limits_until_the_oldest_request_leaves_it() {
    let mut limiter = RateLimiter::new(limits());
    let start = Instant::now();
    let client = ip("203.0.113.7");
    assert_eq!(limiter.check(Endpoint::Register, client, start), Decision::Allowed);
    assert_eq!(limiter.check(Endpoint::Register, client, start + 10 * SECOND), Decision::Allowed);
    assert_eq!(
        limiter.check(Endpoint::Register, client, start + 20 * SECOND),
        Decision::Limited { retry_after: 40 * SECOND }
    );
    assert_eq!(limiter.check(Endpoint::Register, client, start + 60 * SECOND), Decision::Allowed);
    assert!(matches!(limiter.check(Endpoint::Register, client, start + 61 * SECOND), Decision::Limited { .. }));
}

#[test]
fn refused_requests_do_not_count() {
    let mut limiter = RateLimiter::new(limits());
    let start = Instant::now();
    let client = ip("203.0.113.7");
    assert_eq!(limiter.check(Endpoint::Consent, client, start), Decision::Allowed);
    for minute in 1..10 {
        assert!(matches!(
            limiter.check(Endpoint::Consent, client, start + minute * 60 * SECOND),
            Decision::Limited { .. }
        ));
    }
    assert_eq!(limiter.check(Endpoint::Consent, client, start + 600 * SECOND), Decision::Allowed);
}

#[test]
fn counts_each_endpoint_and_address_on_its_own() {
    let mut limiter = RateLimiter::new(limits());
    let now = Instant::now();
    let client = ip("203.0.113.7");
    assert_eq!(limiter.check(Endpoint::Consent, client, now), Decision::Allowed);
    assert!(matches!(limiter.check(Endpoint::Consent, client, now), Decision::Limited { .. }));
    assert_eq!(limiter.check(Endpoint::Consent, ip("203.0.113.8"), now), Decision::Allowed);
    assert_eq!(limiter.check(Endpoint::AuthorizePage, client, now), Decision::Allowed);
    assert_eq!(limiter.check(Endpoint::Register, client, now), Decision::Allowed);
}

#[test]
fn forgets_quiet_addresses() {
    let mut limiter = RateLimiter::new(limits());
    let start = Instant::now();
    limiter.check(Endpoint::Register, ip("203.0.113.7"), start);
    limiter.check(Endpoint::Consent, ip("203.0.113.8"), start);
    limiter.check(Endpoint::Register, ip("203.0.113.9"), start + 120 * SECOND);
    assert_eq!(limiter.accepted.len(), 2, "the register entry outlived its window, the sign-in one did not");
}

#[test]
fn only_the_public_endpoints_are_limited() {
    assert_eq!(Endpoint::of(&Method::POST, "/register"), Some(Endpoint::Register));
    assert_eq!(Endpoint::of(&Method::GET, "/authorize"), Some(Endpoint::AuthorizePage));
    assert_eq!(Endpoint::of(&Method::HEAD, "/authorize"), Some(Endpoint::AuthorizePage));
    assert_eq!(Endpoint::of(&Method::POST, "/authorize"), Some(Endpoint::Consent));
    assert_eq!(Endpoint::of(&Method::POST, "/token"), Some(Endpoint::Token));
    assert_eq!(Endpoint::of(&Method::GET, "/auth/callback"), Some(Endpoint::Callback));
    assert_eq!(Endpoint::of(&Method::POST, "/revoke"), None);
    assert_eq!(Endpoint::of(&Method::OPTIONS, "/register"), None);
}

#[test]
fn a_trusted_proxy_names_the_client_in_its_last_forwarded_entry() {
    assert_eq!(last_forwarded_for("10.0.0.1, 198.51.100.4"), Some(ip("198.51.100.4")));
    assert_eq!(last_forwarded_for("garbage"), None);
    let proxy = ip("10.0.0.2");
    assert_eq!(client_key(proxy, last_forwarded_for("::ffff:198.51.100.4")), ip("198.51.100.4"));
    assert_eq!(client_key(proxy, last_forwarded_for("garbage")), proxy);
}

#[test]
fn an_edge_header_names_one_client() {
    assert_eq!(single_address(" 198.51.100.4 "), Some(ip("198.51.100.4")));
    assert_eq!(single_address("2001:db8::7"), Some(ip("2001:db8::7")));
    assert_eq!(single_address("198.51.100.4, 10.0.0.1"), None, "a list is not one address");
    assert_eq!(single_address(""), None);
    let tunnel = ip("10.42.0.17");
    assert_eq!(client_key(tunnel, single_address("garbage")), tunnel, "unreadable: the peer counts");
}

#[test]
fn without_a_trusted_header_the_peer_counts() {
    assert_eq!(client_key(ip("127.0.0.1"), None), ip("127.0.0.1"));
    assert_eq!(client_key(ip("::ffff:127.0.0.1"), None), ip("127.0.0.1"));
}

#[test]
fn ipv6_counts_per_64_and_mapped_ipv4_as_ipv4() {
    assert_eq!(client_key(ip("2001:db8:1:2:aaaa::1"), None), ip("2001:db8:1:2::"));
    assert_eq!(client_key(ip("2001:db8:1:2:bbbb::9"), None), ip("2001:db8:1:2::"));
    assert_eq!(client_key(ip("::ffff:203.0.113.7"), None), ip("203.0.113.7"));
}
