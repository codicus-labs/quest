//! The loopback redirect of signing in (RFC 8252 §7.3): a listener on `127.0.0.1` with a port
//! the system picks, which takes the browser's redirect to `/callback` and answers with a small
//! page. Only a redirect with the sign-in's `state` counts; other local requests get an error page
//! and the wait goes on. Blocking: runs on the sign-in thread, never on the UI thread.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use quest_protocol::DESKTOP_REDIRECT_PATH;
use quest_store::Url;

/// How long a connected browser may take to send its request.
const READ_TIMEOUT: Duration = Duration::from_secs(5);
/// Requests are a request line and a few headers; anything longer is not a redirect.
const MAX_REQUEST: usize = 16 * 1024;

/// Stops a waiting listener from another thread; also works before it is bound.
#[derive(Clone, Default)]
pub struct Canceller {
    cancelled: Arc<AtomicBool>,
    port: Arc<OnceLock<u16>>,
}

impl Canceller {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Some(port) = self.port.get() {
            // Wakes the blocking accept; the listener sees the flag and stops.
            let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, *port));
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

pub struct Loopback {
    listener: TcpListener,
    port: u16,
    canceller: Canceller,
}

/// The browser's redirect, waiting for its answer page.
pub struct Callback {
    pub url: Url,
    stream: TcpStream,
}

/// What the answer page says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reply {
    SignedIn,
    Failed,
}

impl Loopback {
    /// Binds `127.0.0.1:0`; `canceller` can stop the wait from then on.
    pub fn bind(canceller: Canceller) -> io::Result<Loopback> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let _ = canceller.port.set(port);
        Ok(Loopback { listener, port, canceller })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Waits up to `timeout` for the redirect to `/callback` carrying `state`. Other requests (a
    /// favicon, another local program) get an error page and the wait goes on. `None` once
    /// cancelled; `ErrorKind::TimedOut` after `timeout`.
    pub fn next_callback(&self, state: &str, timeout: Duration) -> io::Result<Option<Callback>> {
        let expired = Arc::new(AtomicBool::new(false));
        // Dropped when this returns, which ends the watchdog early.
        let (_waiting, done) = mpsc::channel::<()>();
        let (watch_expired, port) = (expired.clone(), self.port);
        std::thread::spawn(move || {
            if done.recv_timeout(timeout) == Err(RecvTimeoutError::Timeout) {
                watch_expired.store(true, Ordering::SeqCst);
                let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, port));
            }
        });
        loop {
            if self.canceller.is_cancelled() {
                return Ok(None);
            }
            let (mut stream, _) = self.listener.accept()?;
            if self.canceller.is_cancelled() {
                return Ok(None);
            }
            if expired.load(Ordering::SeqCst) {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "no redirect in time"));
            }
            let Some(target) = read_target(&mut stream) else {
                respond(&mut stream, "400 Bad Request", "Not a sign-in redirect.");
                continue;
            };
            let base = format!("http://127.0.0.1:{}", self.port);
            let url = match Url::parse(&base).and_then(|base| base.join(&target)) {
                Ok(url) if url.path() == DESKTOP_REDIRECT_PATH => url,
                _ => {
                    respond(&mut stream, "404 Not Found", "Not found.");
                    continue;
                }
            };
            if url.query_pairs().any(|(name, value)| name == "state" && value == state) {
                return Ok(Some(Callback { url, stream }));
            }
            respond(&mut stream, "400 Bad Request", "This is not the sign-in Quest is waiting for.");
        }
    }
}

impl Callback {
    pub fn reply(mut self, reply: Reply) {
        let message = match reply {
            Reply::SignedIn => "Signed in to Quest. You can close this window.",
            Reply::Failed => "Signing in did not complete. Go back to Quest to try again. You can close this window.",
        };
        respond(&mut self.stream, "200 OK", message);
    }
}

/// The request target of a `GET` (`/callback?code=…`), from the request line.
fn read_target(stream: &mut TcpStream) -> Option<String> {
    stream.set_read_timeout(Some(READ_TIMEOUT)).ok()?;
    let mut request = Vec::new();
    let mut chunk = [0u8; 2048];
    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 || request.len() + read > MAX_REQUEST {
            break;
        }
        request.extend_from_slice(&chunk[..read]);
    }
    let line = String::from_utf8_lossy(&request).lines().next()?.to_owned();
    let mut parts = line.split(' ');
    match (parts.next(), parts.next()) {
        (Some("GET"), Some(target)) if target.starts_with('/') => Some(target.to_owned()),
        _ => None,
    }
}

fn respond(stream: &mut TcpStream, status: &str, message: &str) {
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Quest</title></head>\
         <body style=\"font-family:-apple-system,sans-serif;margin:4em;text-align:center\">\
         <p>{message}</p></body></html>"
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    // The browser may already be gone; the sign-in goes on either way.
    let _ = stream.write_all(response.as_bytes());
}

#[cfg(test)]
#[path = "loopback_tests.rs"]
mod tests;
