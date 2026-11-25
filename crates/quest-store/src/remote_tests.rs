use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;

/// Reads one HTTP request completely (headers and `Content-Length` body).
pub(crate) async fn read_request(stream: &mut TcpStream) {
    let mut request = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = stream.read(&mut chunk).await.unwrap();
        assert!(read > 0, "client closed before sending the whole request");
        request.extend_from_slice(&chunk[..read]);
        let text = String::from_utf8_lossy(&request);
        let Some(header_end) = text.find("\r\n\r\n") else { continue };
        let body_length = text[..header_end]
            .lines()
            .find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().to_owned()))
            .map_or(0, |v| v.parse::<usize>().unwrap());
        if request.len() >= header_end + 4 + body_length {
            return;
        }
    }
}

#[tokio::test]
async fn token_requests_do_not_follow_redirects() {
    let elsewhere = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", elsewhere.local_addr().unwrap());
    let target = format!("{origin}/token?{}", "x".repeat(4000));
    let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_url: Url = format!("http://{}/", server.local_addr().unwrap()).parse().unwrap();
    let redirecting = tokio::spawn(async move {
        let (mut stream, _) = server.accept().await.unwrap();
        read_request(&mut stream).await;
        let response = format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();
    });

    let result = crate::oauth::refresh(&server_url, "quest-desktop", "refresh").await;
    redirecting.await.unwrap();

    match result {
        Err(crate::oauth::GrantFailure::Failed(StoreError::InvalidServerUrl { message, .. })) => {
            assert!(message.contains(&format!("redirects to {origin};")), "{message}");
            assert!(!message.contains("/token"), "only the origin shows: {message}");
        }
        other => panic!("expected a refused redirect, got {other:?}"),
    }
    let second_request = tokio::time::timeout(Duration::from_millis(200), elsewhere.accept()).await;
    assert!(second_request.is_err(), "the redirect target must not be contacted");
}

#[tokio::test]
async fn a_redirected_sync_socket_fails_for_good_instead_of_retrying() {
    let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_url: Url = format!("http://{}/", server.local_addr().unwrap()).parse().unwrap();
    let redirecting = tokio::spawn(async move {
        let (mut stream, _) = server.accept().await.unwrap();
        read_request(&mut stream).await;
        let response = "HTTP/1.1 301 Moved Permanently\r\nLocation: https://elsewhere.example/sync\r\n\
                        Content-Length: 0\r\nConnection: close\r\n\r\n";
        stream.write_all(response.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();
    });
    let result = connect(&server_url, "t").await;
    redirecting.await.unwrap();

    match result {
        Err(ConnectFailure::Fatal(StoreError::InvalidServerUrl { message, .. })) => {
            assert!(message.contains("redirects to https://elsewhere.example;"), "{message}");
        }
        other => panic!("expected a fatal redirect, got {:?}", other.map(|_| "socket")),
    }
}

/// Manual check (needs the internet; sends only a TLS handshake and a WebSocket upgrade to
/// github.com): wss with the platform verifier still trusts public certificates. Run with
/// `cargo test -p quest-store -- --ignored wss_trusts`.
#[tokio::test]
#[ignore = "contacts github.com"]
async fn wss_trusts_a_public_certificate() {
    let connector = Connector::Rustls(Arc::new(tls_config(&"https://github.com/".parse().unwrap()).unwrap()));
    let request = "wss://github.com/".into_client_request().unwrap();
    let outcome = tokio_tungstenite::connect_async_tls_with_config(request, None, false, Some(connector)).await;
    match outcome {
        // An HTTP answer means the TLS handshake (and thus the certificate check) succeeded.
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            assert!(!response.status().is_success(), "{response:?}");
        }
        Ok(_) => {}
        Err(error) => panic!("TLS to a public host failed: {error:?}"),
    }
}

#[tokio::test]
async fn an_unreachable_server_names_the_cause() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    drop(listener);
    let client = http_client(&server_url, Duration::from_secs(5)).unwrap();

    let error = client.get(server_url.clone()).send().await.unwrap_err();
    let StoreError::Unreachable { message, .. } = unreachable(&server_url, error) else { panic!("not unreachable") };

    assert!(message.starts_with("error sending request"), "{message}");
    assert!(message.contains("onnect"), "the cause is missing: {message}");
}
