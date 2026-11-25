//! Client ID Metadata Documents: a client id that is an HTTPS URL names a JSON document with the
//! client's metadata. The server fetches it (guarded against SSRF: public addresses only, no
//! redirects, small, fast) and caches it as its HTTP headers allow.

use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use futures_util::future::BoxFuture;
use reqwest::Url;
use reqwest::header::{ACCEPT, CACHE_CONTROL};
use rustls::ClientConfig;
use rustls_platform_verifier::BuilderVerifierExt;

use super::policy::{ClientInfo, ClientMetadata, check_metadata_document};

const FETCH_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_DOCUMENT_BYTES: usize = 16 * 1024;
const DEFAULT_MAX_AGE: Duration = Duration::from_secs(60 * 60);
const MIN_MAX_AGE: Duration = Duration::from_secs(60);
const MAX_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_CACHED: usize = 1000;

/// A fetched document and how long it may be cached (`max-age`), if the server said.
#[derive(Clone, Debug)]
pub struct Fetched {
    pub metadata: ClientMetadata,
    pub max_age: Option<Duration>,
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("its host resolves to a private or local address")]
    NotPublic,
    #[error("it could not be fetched")]
    Unreachable,
    #[error("it answered with HTTP {0}")]
    Status(u16),
    #[error("it is larger than {MAX_DOCUMENT_BYTES} bytes")]
    TooLarge,
    #[error("it is not a JSON client metadata document")]
    NotJson,
}

/// Where client metadata documents come from; tests plug in a fake.
pub trait ClientMetadataSource: Send + Sync + fmt::Debug {
    fn fetch<'a>(&'a self, url: &'a Url) -> BoxFuture<'a, Result<Fetched, FetchError>>;
}

/// Fetches over HTTPS from public addresses only.
pub struct HttpsMetadataSource {
    tls: ClientConfig,
}

impl fmt::Debug for HttpsMetadataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HttpsMetadataSource")
    }
}

impl HttpsMetadataSource {
    pub fn new() -> Result<Self, String> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let tls = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .and_then(|builder| builder.with_platform_verifier())
            .map(|builder| builder.with_no_client_auth())
            .map_err(|error| format!("TLS setup for client metadata documents failed: {error}"))?;
        Ok(HttpsMetadataSource { tls })
    }

    async fn fetch_public(&self, url: &Url) -> Result<Fetched, FetchError> {
        let host = url.host_str().ok_or(FetchError::Unreachable)?;
        let port = url.port_or_known_default().unwrap_or(443);
        let address = public_address(host, port).await?;
        // Pin the connection to the checked address so DNS cannot change it in between.
        let client = reqwest::Client::builder()
            .tls_backend_preconfigured(self.tls.clone())
            .redirect(reqwest::redirect::Policy::none())
            // A proxy would resolve the host itself, past the pinned address.
            .no_proxy()
            .timeout(FETCH_TIMEOUT)
            .resolve(host, address)
            .build()
            .map_err(|_| FetchError::Unreachable)?;
        let response = client
            .get(url.clone())
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|_| FetchError::Unreachable)?;
        if !response.status().is_success() {
            return Err(FetchError::Status(response.status().as_u16()));
        }
        let max_age = response.headers().get(CACHE_CONTROL).and_then(|value| value.to_str().ok()).and_then(max_age);
        let mut body = Vec::new();
        let mut response = response;
        while let Some(chunk) = response.chunk().await.map_err(|_| FetchError::Unreachable)? {
            if body.len() + chunk.len() > MAX_DOCUMENT_BYTES {
                return Err(FetchError::TooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        let metadata = serde_json::from_slice(&body).map_err(|_| FetchError::NotJson)?;
        Ok(Fetched { metadata, max_age })
    }
}

impl ClientMetadataSource for HttpsMetadataSource {
    fn fetch<'a>(&'a self, url: &'a Url) -> BoxFuture<'a, Result<Fetched, FetchError>> {
        Box::pin(self.fetch_public(url))
    }
}

async fn public_address(host: &str, port: u16) -> Result<SocketAddr, FetchError> {
    let lookup = tokio::time::timeout(FETCH_TIMEOUT, tokio::net::lookup_host((host, port))).await;
    let addresses: Vec<SocketAddr> =
        lookup.map_err(|_| FetchError::Unreachable)?.map_err(|_| FetchError::Unreachable)?.collect();
    if addresses.is_empty() {
        return Err(FetchError::Unreachable);
    }
    if !addresses.iter().all(|address| is_public(address.ip())) {
        return Err(FetchError::NotPublic);
    }
    Ok(addresses[0])
}

/// Globally routable unicast: not loopback, private, link-local, shared, documentation, reserved
/// or multicast.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            !(v4.is_unspecified()
                || v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                || a == 0
                || (a == 100 && (64..128).contains(&b))
                || (a == 192 && b == 0 && c == 0)
                || (a == 198 && (18..20).contains(&b))
                || a >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public(IpAddr::V4(v4));
            }
            // Global unicast (2000::/3) only, without ranges that embed or route to IPv4
            // (6to4 2002::/16, Teredo 2001::/32) and documentation (2001:db8::/32).
            let [first, second, ..] = v6.segments();
            (first & 0xe000) == 0x2000 && first != 0x2002 && !(first == 0x2001 && (second == 0 || second == 0x0db8))
        }
    }
}

/// `max-age` of a Cache-Control header; `no-store`/`no-cache` mean zero.
fn max_age(header: &str) -> Option<Duration> {
    let directives: Vec<String> = header.split(',').map(|part| part.trim().to_ascii_lowercase()).collect();
    if directives.iter().any(|d| d == "no-store" || d == "no-cache") {
        return Some(Duration::ZERO);
    }
    directives.iter().find_map(|d| d.strip_prefix("max-age=")?.parse().ok().map(Duration::from_secs))
}

/// A checked document (or why it is unusable) and until when it counts.
type CachedClient = (Result<ClientInfo, String>, Instant);

/// Fetches and checks documents, with a cache bounded in size and lifetime.
#[derive(Debug)]
pub struct ClientDirectory {
    source: Arc<dyn ClientMetadataSource>,
    cache: Mutex<HashMap<String, CachedClient>>,
}

impl ClientDirectory {
    pub fn new(source: Arc<dyn ClientMetadataSource>) -> Self {
        ClientDirectory { source, cache: Mutex::new(HashMap::new()) }
    }

    /// The client behind a metadata document URL, or why it cannot be used.
    pub async fn client(&self, url: &Url) -> Result<ClientInfo, String> {
        let now = Instant::now();
        if let Some((client, _)) = self.cache().get(url.as_str()).filter(|(_, until)| *until > now) {
            return client.clone();
        }
        // Failures are cached too (for the minimum lifetime), so /authorize cannot be used to
        // make this server fetch a URL over and over.
        let (client, lifetime) = match self.source.fetch(url).await {
            Err(error) => (Err(format!("The client's metadata document {error}")), MIN_MAX_AGE),
            Ok(fetched) => (
                check_metadata_document(url, &fetched.metadata)
                    .map_err(|reason| format!("The client's metadata document is not usable: {reason}")),
                fetched.max_age.map_or(DEFAULT_MAX_AGE, |age| age.clamp(MIN_MAX_AGE, MAX_MAX_AGE)),
            ),
        };
        let mut cache = self.cache();
        if cache.len() >= MAX_CACHED {
            cache.retain(|_, (_, until)| *until > now);
            if cache.len() >= MAX_CACHED {
                cache.clear();
            }
        }
        cache.insert(url.as_str().to_owned(), (client.clone(), now + lifetime));
        client
    }

    fn cache(&self) -> std::sync::MutexGuard<'_, HashMap<String, CachedClient>> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
#[path = "cimd_tests.rs"]
mod tests;
