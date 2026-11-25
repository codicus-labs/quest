//! Network adapters: the HTTP client and the sync WebSocket. Translates transport and server
//! errors into `StoreError`s at this boundary.

use std::sync::Arc;
use std::time::Duration;

use quest_protocol::SYNC_PATH;
use rustls::ClientConfig;
use rustls_platform_verifier::BuilderVerifierExt;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderValue, StatusCode, header};
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream};
use url::Url;

use crate::error::StoreError;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Why opening the sync socket failed.
#[derive(Debug)]
pub(crate) enum ConnectFailure {
    /// Network trouble or a server hiccup: retry later.
    Transient,
    /// The server rejected the access token (401): refresh it, then try again.
    Rejected,
    /// Retrying cannot help, e.g. the address redirects elsewhere.
    Fatal(StoreError),
}

/// An HTTP client for `server_url` that follows no redirects: one could lead to a URL the app's URL
/// policy forbids (and resend the token), so the entered address must be the final one.
pub(crate) fn http_client(server_url: &Url, timeout: Duration) -> Result<reqwest::Client, StoreError> {
    reqwest::Client::builder()
        .tls_backend_preconfigured(tls_config(server_url)?)
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| unreachable(server_url, e))
}

pub(crate) fn unreachable(server_url: &Url, error: impl std::error::Error) -> StoreError {
    StoreError::Unreachable { server_url: server_url.to_string(), message: with_causes(&error) }
}

/// The error with its causes: `reqwest` alone says only "error sending request", the cause
/// (DNS, connection refused, certificate) is in the source chain.
pub(crate) fn with_causes(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut cause = error.source();
    while let Some(inner) = cause {
        message.push_str(": ");
        message.push_str(&inner.to_string());
        cause = inner.source();
    }
    message
}

pub(crate) fn redirected(server_url: &Url, response: &reqwest::Response) -> StoreError {
    let target = response.headers().get(reqwest::header::LOCATION).and_then(|value| value.to_str().ok());
    redirect_error(server_url, response.status().as_u16(), target)
}

/// A redirect the client refuses to follow. The message names only the target's origin: the
/// `Location` is the server's, of any length, and its path or query are no help in the URL field.
fn redirect_error(server_url: &Url, status: u16, location: Option<&str>) -> StoreError {
    let origin = location
        .and_then(|target| server_url.join(target).ok())
        .map(|target| target.origin().ascii_serialization())
        .filter(|origin| origin != "null");
    let message = match origin {
        Some(origin) => format!("the server redirects to {origin}; enter the final address instead"),
        None => format!("the server answered with redirect {status}; enter the final address instead"),
    };
    invalid_url(server_url, message)
}

fn sync_url(server_url: &Url) -> Result<Url, StoreError> {
    let scheme = match server_url.scheme() {
        "http" => "ws",
        "https" => "wss",
        other => return Err(invalid_url(server_url, format!("unsupported scheme {other:?}"))),
    };
    let mut url = server_url.join(SYNC_PATH).map_err(|e| invalid_url(server_url, e))?;
    url.set_scheme(scheme).map_err(|()| invalid_url(server_url, "cannot switch to a WebSocket scheme"))?;
    Ok(url)
}

pub(crate) fn invalid_url(url: &Url, message: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidServerUrl { url: url.to_string(), message: message.to_string() }
}

pub(crate) async fn connect(server_url: &Url, access_token: &str) -> Result<Socket, ConnectFailure> {
    let url = sync_url(server_url).map_err(ConnectFailure::Fatal)?;
    let mut request = url.as_str().into_client_request().map_err(|_| ConnectFailure::Transient)?;
    let bearer = HeaderValue::from_str(&format!("Bearer {access_token}")).map_err(|_| ConnectFailure::Rejected)?;
    request.headers_mut().insert(header::AUTHORIZATION, bearer);
    let connector = match url.scheme() {
        "wss" => Connector::Rustls(Arc::new(tls_config(server_url).map_err(ConnectFailure::Fatal)?)),
        _ => Connector::Plain,
    };
    let attempt = tokio_tungstenite::connect_async_tls_with_config(request, None, false, Some(connector));
    match tokio::time::timeout(CONNECT_TIMEOUT, attempt).await {
        Err(_) => Err(ConnectFailure::Transient),
        Ok(Ok((socket, _))) => Ok(socket),
        Ok(Err(tokio_tungstenite::tungstenite::Error::Http(response)))
            if response.status() == StatusCode::UNAUTHORIZED =>
        {
            Err(ConnectFailure::Rejected)
        }
        // tungstenite follows no redirects; retrying would fail the same way forever, unseen.
        Ok(Err(tokio_tungstenite::tungstenite::Error::Http(response))) if response.status().is_redirection() => {
            let location = response.headers().get(header::LOCATION).and_then(|value| value.to_str().ok());
            Err(ConnectFailure::Fatal(redirect_error(server_url, response.status().as_u16(), location)))
        }
        Ok(Err(_)) => Err(ConnectFailure::Transient),
    }
}

/// TLS for HTTP and the sync socket: ring for the crypto (smaller than aws-lc in the Android
/// library), certificates checked by the platform verifier, so user and company CAs trusted by the
/// OS work. On Android it needs the JNI init from `quest-mobile` first.
fn tls_config(server_url: &Url) -> Result<ClientConfig, StoreError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .and_then(|builder| builder.with_platform_verifier())
        .map_err(|e| StoreError::Unreachable {
            server_url: server_url.to_string(),
            message: format!("the system certificate check could not be set up ({e}); restart the app"),
        })?;
    Ok(config.with_no_client_auth())
}

#[cfg(test)]
#[path = "remote_tests.rs"]
pub(crate) mod tests;
