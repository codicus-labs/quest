use std::io::{Read, Write};
use std::net::TcpStream;

use super::*;

const WAIT: Duration = Duration::from_secs(10);

/// Sends a GET like a browser and returns the whole answer.
fn get(port: u16, target: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(stream, "GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    answer
}

#[test]
fn takes_the_callback_and_answers_with_a_page() {
    let loopback = Loopback::bind(Canceller::default()).unwrap();
    let port = loopback.port();
    let browser = std::thread::spawn(move || {
        (get(port, "/favicon.ico"), get(port, "/callback?code=x&state=other"), get(port, "/callback?code=c&state=s"))
    });
    let callback = loopback.next_callback("s", WAIT).unwrap().expect("a callback");
    assert_eq!(callback.url.path(), "/callback");
    assert_eq!(callback.url.query(), Some("code=c&state=s"));
    callback.reply(Reply::SignedIn);
    let (favicon, foreign, page) = browser.join().unwrap();
    assert!(favicon.starts_with("HTTP/1.1 404"), "other paths are not the callback");
    assert!(foreign.starts_with("HTTP/1.1 400"), "a redirect with another state is not this sign-in");
    assert!(page.starts_with("HTTP/1.1 200"));
    assert!(page.contains("You can close this window."));
}

#[test]
fn cancelling_stops_the_wait() {
    let canceller = Canceller::default();
    let loopback = Loopback::bind(canceller.clone()).unwrap();
    let waiting = std::thread::spawn(move || loopback.next_callback("s", WAIT).unwrap().is_none());
    canceller.cancel();
    assert!(waiting.join().unwrap(), "a cancelled wait gives no callback");
}

#[test]
fn cancelled_before_waiting_gives_nothing() {
    let canceller = Canceller::default();
    canceller.cancel();
    let loopback = Loopback::bind(canceller).unwrap();
    assert!(loopback.next_callback("s", WAIT).unwrap().is_none());
}

#[test]
fn gives_up_after_the_timeout() {
    let loopback = Loopback::bind(Canceller::default()).unwrap();
    let error = loopback.next_callback("s", Duration::from_millis(50)).err().expect("times out");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}
