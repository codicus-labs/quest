//! Sliding-window limits of the OAuth endpoints anyone can call (`/register`, `/authorize`,
//! `/token`, `/auth/callback`), per client address. Pure: the time is passed in. In memory, so a
//! restart forgets them, which is fine against guessing codes and flooding registrations on a
//! personal server.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv6Addr};
use std::time::{Duration, Instant};

use axum::http::{HeaderName, Method};

use quest_protocol::OIDC_CALLBACK_PATH;

use super::{AUTHORIZE_PATH, REGISTER_PATH, TOKEN_PATH};

/// At most `max` requests per `per`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub max: usize,
    pub per: Duration,
}

/// Limits per client address of the public OAuth endpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OAuthRateLimits {
    /// `POST /register` (Dynamic Client Registration).
    pub register: Window,
    /// `GET /authorize` (starts a sign-in or shows the consent page; may fetch a client metadata
    /// document).
    pub authorize_page: Window,
    /// `POST /authorize` (the consent decision).
    pub consent: Window,
    /// `POST /token` (code exchange and refresh).
    pub token: Window,
    /// `GET /auth/callback` (each one exchanges a code at the identity provider).
    pub callback: Window,
}

impl Default for OAuthRateLimits {
    /// Per minute: 30 authorization pages, 60 token requests, 20 sign-in callbacks. Per 15 minutes:
    /// 30 consent decisions. Per hour: 20 registrations.
    fn default() -> Self {
        OAuthRateLimits {
            register: Window { max: 20, per: Duration::from_secs(60 * 60) },
            authorize_page: Window { max: 30, per: Duration::from_secs(60) },
            consent: Window { max: 30, per: Duration::from_secs(15 * 60) },
            token: Window { max: 60, per: Duration::from_secs(60) },
            callback: Window { max: 20, per: Duration::from_secs(60) },
        }
    }
}

impl OAuthRateLimits {
    fn window(&self, endpoint: Endpoint) -> Window {
        match endpoint {
            Endpoint::Register => self.register,
            Endpoint::AuthorizePage => self.authorize_page,
            Endpoint::Consent => self.consent,
            Endpoint::Token => self.token,
            Endpoint::Callback => self.callback,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Endpoint {
    Register,
    AuthorizePage,
    Consent,
    Token,
    Callback,
}

impl Endpoint {
    /// The limited endpoint a request goes to, if any.
    pub fn of(method: &Method, path: &str) -> Option<Endpoint> {
        match (method, path) {
            (&Method::POST, REGISTER_PATH) => Some(Endpoint::Register),
            (&Method::GET | &Method::HEAD, AUTHORIZE_PATH) => Some(Endpoint::AuthorizePage),
            (&Method::POST, AUTHORIZE_PATH) => Some(Endpoint::Consent),
            (&Method::POST, TOKEN_PATH) => Some(Endpoint::Token),
            (&Method::GET | &Method::HEAD, OIDC_CALLBACK_PATH) => Some(Endpoint::Callback),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Allowed,
    Limited { retry_after: Duration },
}

/// Accepted requests per endpoint and client address.
#[derive(Debug)]
pub struct RateLimiter {
    limits: OAuthRateLimits,
    /// Times of the accepted requests still inside their window, oldest first.
    accepted: HashMap<(Endpoint, IpAddr), VecDeque<Instant>>,
    last_sweep: Option<Instant>,
}

/// How often addresses that went quiet are forgotten.
const SWEEP_EVERY: Duration = Duration::from_secs(60);

impl RateLimiter {
    pub fn new(limits: OAuthRateLimits) -> Self {
        RateLimiter { limits, accepted: HashMap::new(), last_sweep: None }
    }

    /// Counts a request of `client` at `now` if it is below the endpoint's limit.
    pub fn check(&mut self, endpoint: Endpoint, client: IpAddr, now: Instant) -> Decision {
        self.sweep(now);
        let window = self.limits.window(endpoint);
        let times = self.accepted.entry((endpoint, client)).or_default();
        forget_older(times, window.per, now);
        if times.len() < window.max {
            times.push_back(now);
            return Decision::Allowed;
        }
        let oldest = times.front().copied().unwrap_or(now);
        Decision::Limited { retry_after: window.per.saturating_sub(now.duration_since(oldest)) }
    }

    fn sweep(&mut self, now: Instant) {
        if self.last_sweep.is_some_and(|at| now.duration_since(at) < SWEEP_EVERY) {
            return;
        }
        self.last_sweep = Some(now);
        let limits = self.limits;
        self.accepted.retain(|(endpoint, _), times| {
            forget_older(times, limits.window(*endpoint).per, now);
            !times.is_empty()
        });
    }
}

fn forget_older(times: &mut VecDeque<Instant>, per: Duration, now: Instant) {
    while times.front().is_some_and(|at| now.duration_since(*at) >= per) {
        times.pop_front();
    }
}

/// Where the limits read a request's client address. Anything but [`ClientAddress::Peer`] trusts a
/// header, so only when every request reaches the server through the operator's proxy, which sets
/// it; otherwise clients could pick their own address and evade the limits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ClientAddress {
    /// The TCP peer.
    #[default]
    Peer,
    /// The last entry of the last `X-Forwarded-For` line: the one the proxy in front added.
    LastForwardedFor,
    /// The one address in this header, set by the edge for every request (an edge
    /// that overwrites what a client sends, e.g. `X-Real-IP`).
    Header(HeaderName),
}

/// The client address `X-Forwarded-For` names: its last entry (`value` is the last header line).
pub fn last_forwarded_for(value: &str) -> Option<IpAddr> {
    value.rsplit(',').next().and_then(|last| last.trim().parse().ok())
}

/// The address of a header that holds exactly one, like `X-Real-IP`.
pub fn single_address(value: &str) -> Option<IpAddr> {
    value.trim().parse().ok()
}

/// The key a request is counted under: the address a trusted header names (`claimed`), else the TCP
/// peer. IPv6 clients count per /64, the block one subscriber usually gets.
pub fn client_key(peer: IpAddr, claimed: Option<IpAddr>) -> IpAddr {
    match claimed.unwrap_or(peer).to_canonical() {
        IpAddr::V6(v6) => IpAddr::V6(Ipv6Addr::from(v6.to_bits() & !((1u128 << 64) - 1))),
        v4 => v4,
    }
}

#[cfg(test)]
#[path = "rate_limit_tests.rs"]
mod tests;
