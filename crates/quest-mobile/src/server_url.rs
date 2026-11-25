//! Which server URLs the app may talk to. Pure.
//!
//! The Android network security config does not cover sockets opened by Rust, so the policy is
//! enforced here: https always, plain http only to this device or (debug builds) the emulator's host.

use std::net::{IpAddr, Ipv4Addr};

use quest_store::Url;

use crate::error::MobileError;
use crate::types::UrlPolicy;

/// The emulator's alias for the development machine's loopback.
const EMULATOR_HOST: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 2);

pub(crate) fn parse_server_url(value: &str, policy: UrlPolicy) -> Result<Url, MobileError> {
    let invalid = |reason: String| MobileError::InvalidServerUrl { value: value.to_owned(), reason };
    let url = Url::parse(value).map_err(|e| invalid(e.to_string()))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(MobileError::ServerUrlWithCredentials);
    }
    match url.scheme() {
        "https" => Ok(url),
        "http" if url.host_str().is_some_and(|host| allows_plain_http(host, policy)) => Ok(url),
        "http" => Err(MobileError::InsecureServerUrl { value: value.to_owned() }),
        other => Err(invalid(format!("the scheme {other:?} is not supported"))),
    }
}

fn allows_plain_http(host: &str, policy: UrlPolicy) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    match host.trim_start_matches('[').trim_end_matches(']').parse::<IpAddr>() {
        Ok(ip) => ip.is_loopback() || (policy == UrlPolicy::Development && ip == IpAddr::V4(EMULATOR_HOST)),
        Err(_) => false,
    }
}

#[cfg(test)]
#[path = "server_url_tests.rs"]
mod tests;
