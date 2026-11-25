//! Test harness: an in-process server on an ephemeral port and a WebSocket sync client.

#![allow(dead_code)]

pub mod oauth;

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use quest_domain::{EntityId, EntityKind, FieldChange, Hlc, NodeId};
use quest_oidc_fake::{AppClient, FakeConfig, FakeProvider, app_login};
use quest_protocol::{
    ACCOUNT_PATH, ACCOUNT_SETTINGS_PATH, AccountInfo, AccountSettingsUpdate, ApiError, BatchId, ClientMessage, Cursor,
    Epoch, PROTOCOL_VERSION, ServerMessage,
};
use quest_server::{AiConfig, AuthConfig, Config, McpConfig, OidcSettings, RunningServer, SyncTimeouts, start_with};
use serde_json::Value;
use tempfile::TempDir;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

const TIMEOUT: Duration = Duration::from_secs(5);

/// Tokens of a Quest app signed in through the fake identity provider.
#[derive(Clone, Debug)]
pub struct Session {
    pub token: String,
    pub refresh: String,
}

/// Changes to the auth configuration before the server starts.
pub type AuthTweak = Box<dyn FnOnce(&mut AuthConfig) + Send>;

/// What a test server starts with; [`Setup::default`] is a plain server with a fresh database
/// and its own fake identity provider (any username signs in).
#[derive(Default)]
pub struct Setup {
    pub timeouts: SyncTimeouts,
    pub ai: Option<AiConfig>,
    pub mcp: Option<McpConfig>,
    pub auth: Option<AuthTweak>,
    /// Share a provider (e.g. across a restart), so accounts stay the same.
    pub fake: Option<Arc<FakeProvider>>,
    /// Use this database instead of a fresh one.
    pub database_url: Option<String>,
    pub bind: Option<std::net::SocketAddr>,
}

pub struct TestServer {
    pub server: RunningServer,
    pub fake: Arc<FakeProvider>,
    _dir: Option<TempDir>,
}

pub async fn start_fake() -> Arc<FakeProvider> {
    let config = FakeConfig { any_user: true, ..FakeConfig::default() };
    Arc::new(FakeProvider::start(config).await.unwrap())
}

/// The auth configuration for a server whose identity provider is `fake`.
pub fn auth_config(fake: &FakeProvider) -> AuthConfig {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let issuer = reqwest::Url::parse(fake.issuer()).unwrap();
    let settings =
        OidcSettings { issuer, client_id: fake.client_id().to_owned(), client_secret: fake.client_secret().to_owned() };
    AuthConfig::with_oidc(None, settings).unwrap()
}

impl TestServer {
    pub async fn start() -> TestServer {
        TestServer::start_custom(Setup::default()).await
    }

    pub async fn start_with(timeouts: SyncTimeouts) -> TestServer {
        TestServer::start_custom(Setup { timeouts, ..Setup::default() }).await
    }

    pub async fn start_with_ai(ai: AiConfig) -> TestServer {
        TestServer::start_custom(Setup { ai: Some(ai), ..Setup::default() }).await
    }

    /// With MCP (and AI capture if given).
    pub async fn start_with_mcp(mcp: McpConfig, ai: Option<AiConfig>) -> TestServer {
        TestServer::start_custom(Setup { ai, mcp: Some(mcp), ..Setup::default() }).await
    }

    pub async fn start_custom(setup: Setup) -> TestServer {
        let fake = match setup.fake {
            Some(fake) => fake,
            None => start_fake().await,
        };
        let (dir, database_url) = match setup.database_url {
            Some(url) => (None, url),
            None => {
                let dir = tempfile::tempdir().unwrap();
                let url = database_url(&dir);
                (Some(dir), url)
            }
        };
        let mut auth = auth_config(&fake);
        if let Some(tweak) = setup.auth {
            tweak(&mut auth);
        }
        let bind = setup.bind.unwrap_or_else(|| "127.0.0.1:0".parse().unwrap());
        let config = Config { bind, database_url, ai: setup.ai, auth, mcp: setup.mcp };
        let server = start_with(config, setup.timeouts).await.unwrap();
        TestServer { server, fake, _dir: dir }
    }

    /// POSTs `body` as JSON, with a bearer token if given.
    pub async fn post_json(&self, path: &str, token: Option<&str>, body: &impl serde::Serialize) -> reqwest::Response {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mut request = reqwest::Client::new().post(format!("{}{path}", self.server.http_url())).json(body);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        request.send().await.unwrap()
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.server.http_url())
    }

    /// Signs `username` in to the desktop app (account created on first sign-in).
    pub async fn sign_in(&self, username: &str) -> Session {
        self.sign_in_as(username, AppClient::Desktop).await
    }

    /// Signs `username` in and turns the AI opt-in on.
    pub async fn sign_in_with_ai(&self, username: &str) -> Session {
        let session = self.sign_in(username).await;
        self.set_ai_enabled(&session.token, true).await;
        session
    }

    /// `PUT /v1/account/settings`; returns the account as the server then shows it.
    pub async fn set_ai_enabled(&self, token: &str, enabled: bool) -> AccountInfo {
        let response = http_client()
            .put(self.url(ACCOUNT_SETTINGS_PATH))
            .bearer_auth(token)
            .json(&AccountSettingsUpdate { ai_enabled: enabled })
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 200);
        response.json().await.unwrap()
    }

    /// `GET /v1/account` with `token`: the account, or the status of the refusal.
    pub async fn account(&self, token: &str) -> Result<AccountInfo, u16> {
        let response = http_client().get(self.url(ACCOUNT_PATH)).bearer_auth(token).send().await.unwrap();
        match response.status().as_u16() {
            200 => Ok(response.json().await.unwrap()),
            status => Err(status),
        }
    }

    pub async fn sign_in_as(&self, username: &str, client: AppClient) -> Session {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let tokens = app_login(&self.server.http_url(), username, client).await.unwrap();
        assert_eq!(tokens.token_type, "Bearer");
        Session { token: tokens.access_token, refresh: tokens.refresh_token.expect("apps get a refresh token") }
    }

    pub async fn connect_raw(&self, token: Option<&str>) -> Result<SyncClient, tungstenite::Error> {
        let mut request = self.server.ws_url().into_client_request().unwrap();
        if let Some(token) = token {
            request.headers_mut().insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
        }
        let (socket, _) = tokio_tungstenite::connect_async(request).await?;
        Ok(SyncClient { socket, pending: VecDeque::new() })
    }

    /// Connects as node [`NODE`] and completes catch-up, resuming from `from` (epoch and cursor
    /// of an earlier sync) or from scratch.
    pub async fn connect(&self, session: &Session, from: Option<(Epoch, Cursor)>) -> (SyncClient, Resumed) {
        let mut client = self.connect_raw(Some(&session.token)).await.unwrap();
        let resumed = client.hello(from).await;
        (client, resumed)
    }
}

fn http_client() -> reqwest::Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::new()
}

pub fn database_url(dir: &TempDir) -> String {
    format!("sqlite://{}", dir.path().join("server.sqlite").display())
}

pub const NODE: NodeId = NodeId(1);

/// The outcome of a hello and catch-up.
#[derive(Debug)]
pub struct Resumed {
    pub epoch: Epoch,
    pub cursor_reset: bool,
    pub changes: Vec<FieldChange>,
    pub cursor: Cursor,
}

impl Resumed {
    pub fn position(&self) -> Option<(Epoch, Cursor)> {
        Some((self.epoch, self.cursor))
    }
}

pub struct SyncClient {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    /// Frames received while waiting for something else.
    pending: VecDeque<ServerMessage>,
}

impl SyncClient {
    pub async fn send(&mut self, message: &ClientMessage) {
        self.socket.send(Message::text(serde_json::to_string(message).unwrap())).await.unwrap();
    }

    pub async fn try_send_text(&mut self, text: &str) -> Result<(), tungstenite::Error> {
        self.socket.send(Message::text(text.to_owned())).await
    }

    pub async fn send_text(&mut self, text: &str) {
        self.socket.send(Message::text(text.to_owned())).await.unwrap();
    }

    async fn receive(&mut self) -> ServerMessage {
        if let Some(message) = self.pending.pop_front() {
            return message;
        }
        self.receive_from_socket(TIMEOUT).await.expect("timed out waiting for a server message")
    }

    async fn receive_from_socket(&mut self, wait: Duration) -> Option<ServerMessage> {
        loop {
            let frame = tokio::time::timeout(wait, self.socket.next()).await.ok()?;
            match frame.expect("socket closed").unwrap() {
                Message::Text(text) => return Some(serde_json::from_str(text.as_str()).unwrap()),
                Message::Ping(_) | Message::Pong(_) => {}
                other => panic!("unexpected frame {other:?}"),
            }
        }
    }

    pub async fn hello(&mut self, from: Option<(Epoch, Cursor)>) -> Resumed {
        let (epoch, cursor) = from.map_or((None, Cursor(0)), |(epoch, cursor)| (Some(epoch), cursor));
        self.send(&ClientMessage::Hello { protocol: PROTOCOL_VERSION, node: NODE, cursor, epoch }).await;
        let ServerMessage::Welcome { epoch, cursor_reset } = self.receive().await else {
            panic!("expected welcome first");
        };
        let mut changes = Vec::new();
        for _ in 0..1000 {
            let ServerMessage::Changes { changes: frame, cursor, caught_up } = self.receive().await else {
                panic!("expected catch-up changes");
            };
            changes.extend(frame);
            if caught_up {
                return Resumed { epoch, cursor_reset, changes, cursor };
            }
        }
        panic!("catch-up never finished");
    }

    /// Pushes and waits for the Ack, keeping other frames for later.
    pub async fn push(&mut self, changes: Vec<FieldChange>) {
        let reply = self.push_for_reply(changes).await;
        assert!(matches!(reply, ServerMessage::Ack { .. }), "expected ack, got {reply:?}");
    }

    /// Pushes and returns the Ack or Rejected for that batch, keeping other frames for later.
    pub async fn push_for_reply(&mut self, changes: Vec<FieldChange>) -> ServerMessage {
        let batch = BatchId(Uuid::now_v7());
        self.send(&ClientMessage::Push { batch, changes }).await;
        self.reply_to(batch).await
    }

    pub async fn reply_to(&mut self, batch: BatchId) -> ServerMessage {
        let mut others = Vec::new();
        let reply = loop {
            let message = self.receive().await;
            match &message {
                ServerMessage::Ack { batch: acked } | ServerMessage::Rejected { batch: acked, .. }
                    if *acked == batch =>
                {
                    break message;
                }
                _ => others.push(message),
            }
        };
        self.pending.extend(others);
        reply
    }

    pub async fn next_message(&mut self) -> ServerMessage {
        self.receive().await
    }

    pub async fn next_changes(&mut self) -> (Vec<FieldChange>, Cursor) {
        match self.receive().await {
            ServerMessage::Changes { changes, cursor, .. } => (changes, cursor),
            other => panic!("expected live changes, got {other:?}"),
        }
    }

    /// True once the server has closed the connection (pending live frames are skipped).
    pub async fn is_closed(&mut self) -> bool {
        self.closes_within(TIMEOUT).await
    }

    /// True if the server closes the connection within `wait`, reading (and so answering pings)
    /// meanwhile.
    pub async fn closes_within(&mut self, wait: Duration) -> bool {
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            match tokio::time::timeout_at(deadline, self.socket.next()).await {
                Err(_) => return false,
                Ok(None | Some(Err(_)) | Some(Ok(Message::Close(_)))) => return true,
                Ok(Some(Ok(_))) => {}
            }
        }
    }

    /// The server's error frame that precedes it closing the connection.
    pub async fn closing_error(&mut self) -> ApiError {
        let ServerMessage::Error { error } = self.receive().await else { panic!("expected an error frame") };
        assert!(self.is_closed().await, "server must close after an error");
        error
    }

    /// Asserts that no frame arrives for a short while.
    pub async fn expect_silence(&mut self) {
        assert!(self.pending.is_empty(), "unexpected frames: {:?}", self.pending);
        if let Some(message) = self.receive_from_socket(Duration::from_millis(300)).await {
            panic!("unexpected frame {message:?}");
        }
    }
}

pub fn change(id: EntityId, field: &str, value: Value, wall_ms: u64) -> FieldChange {
    FieldChange { kind: EntityKind::Task, id, field: field.into(), value, hlc: Hlc { wall_ms, counter: 0, node: NODE } }
}
