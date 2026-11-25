//! A small OpenID Connect provider for tests and local development; never for production.
//!
//! Discovery, JWKS, authorization (no passwords: the user is picked with `login_hint`, the
//! configured `auto_login`, or a button on a page), token (code with PKCE → RS256-signed ID token)
//! and end-session. A `fault` query parameter at `/authorize` makes the ID token invalid on purpose
//! ([`Fault`]), so tests can check that the relying party refuses it.
//!
//! [`browser`] drives a complete sign-in of a Quest app against a quest-server without a browser.

pub mod browser;
mod jwt;
mod routes;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;
use tokio::task::JoinHandle;

pub use browser::{AppAuthorization, AppClient, Browser, LoginFlowError, Step, app_login};

/// The client `quest-server` is registered as, as in `keycloak/quest-realm.json`.
pub const DEV_CLIENT_ID: &str = "quest-server";
pub const DEV_CLIENT_SECRET: &str = "quest-dev-secret";

/// A user of the fake. `username` is what `login_hint` names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeUser {
    pub username: String,
    pub subject: String,
    pub email: String,
    pub name: String,
}

impl FakeUser {
    /// A username with `@` is also the email; otherwise the email is `<username>@example.test`.
    pub fn new(username: &str, name: &str) -> Self {
        let email = if username.contains('@') { username.to_owned() } else { format!("{username}@example.test") };
        FakeUser { username: username.to_owned(), subject: format!("fake-{username}"), email, name: name.to_owned() }
    }
}

/// Ways the fake breaks its ID token, chosen with `fault=<name>` at `/authorize`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// `nonce` differs from the one in the request.
    WrongNonce,
    /// `aud` is another client.
    WrongAudience,
    /// `iss` is another issuer.
    WrongIssuer,
    /// `exp` lies in the past.
    Expired,
    /// Signed with a key that is not in the JWKS (under the published key id).
    WrongKey,
}

impl Fault {
    pub fn as_str(self) -> &'static str {
        match self {
            Fault::WrongNonce => "wrong_nonce",
            Fault::WrongAudience => "wrong_aud",
            Fault::WrongIssuer => "wrong_issuer",
            Fault::Expired => "expired",
            Fault::WrongKey => "wrong_key",
        }
    }

    pub fn parse(raw: &str) -> Option<Fault> {
        [Fault::WrongNonce, Fault::WrongAudience, Fault::WrongIssuer, Fault::Expired, Fault::WrongKey]
            .into_iter()
            .find(|fault| fault.as_str() == raw)
    }
}

#[derive(Clone, Debug)]
pub struct FakeConfig {
    pub bind: SocketAddr,
    pub client_id: String,
    pub client_secret: String,
    pub users: Vec<FakeUser>,
    /// Sign this user in without showing the picker when `/authorize` names none.
    pub auto_login: Option<String>,
    /// Accept any `login_hint` as a user of that name (tests that need many accounts).
    pub any_user: bool,
    pub id_token_ttl: Duration,
}

impl Default for FakeConfig {
    /// An ephemeral loopback port, the dev client and the users anna and ben.
    fn default() -> Self {
        FakeConfig {
            bind: SocketAddr::from(([127, 0, 0, 1], 0)),
            client_id: DEV_CLIENT_ID.to_owned(),
            client_secret: DEV_CLIENT_SECRET.to_owned(),
            users: vec![FakeUser::new("anna", "Anna Example"), FakeUser::new("ben", "Ben Example")],
            auto_login: None,
            any_user: false,
            id_token_ttl: Duration::from_secs(5 * 60),
        }
    }
}

/// A started fake. Dropping it stops the server.
pub struct FakeProvider {
    addr: SocketAddr,
    state: Arc<routes::Shared>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl FakeProvider {
    pub async fn start(config: FakeConfig) -> std::io::Result<FakeProvider> {
        let listener = tokio::net::TcpListener::bind(config.bind).await?;
        let addr = listener.local_addr()?;
        let state = Arc::new(routes::Shared {
            issuer: format!("http://{addr}"),
            config,
            codes: Mutex::new(HashMap::new()),
            logouts: AtomicUsize::new(0),
        });
        // Generate the keys before the first request needs them.
        let _ = jwt::keys();
        let router = routes::router(state.clone());
        let (stop, stopped) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            let served = axum::serve(listener, router).with_graceful_shutdown(async {
                let _ = stopped.await;
            });
            if let Err(error) = served.await {
                tracing::error!(%error, "oidc fake stopped with an error");
            }
        });
        Ok(FakeProvider { addr, state, stop: Some(stop), task: Some(task) })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The issuer, e.g. `http://127.0.0.1:8792` (no trailing slash): `QUEST_OIDC_ISSUER`.
    pub fn issuer(&self) -> &str {
        &self.state.issuer
    }

    pub fn client_id(&self) -> &str {
        &self.state.config.client_id
    }

    pub fn client_secret(&self) -> &str {
        &self.state.config.client_secret
    }

    /// The subject (`sub`) the fake gives `username`.
    pub fn subject(&self, username: &str) -> String {
        FakeUser::new(username, username).subject
    }

    /// How often the end-session endpoint was called.
    pub fn logouts(&self) -> usize {
        self.state.logouts.load(Ordering::SeqCst)
    }

    pub async fn shutdown(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

/// 32 random bytes, base64url: codes, tokens, PKCE verifiers.
pub(crate) fn random_token() -> String {
    use base64::Engine;
    use rsa::rand_core::{OsRng, RngCore};
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

impl Drop for FakeProvider {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
