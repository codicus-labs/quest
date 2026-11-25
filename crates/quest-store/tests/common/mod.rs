//! Test harness: profiles whose stores report changes on a channel, a restartable server whose
//! identity provider is the in-process OIDC fake, and a browserless sign-in.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use quest_domain::{Command, EntityId, Position, QuestColor};
use quest_oidc_fake::{Browser, FakeConfig, FakeProvider};
use quest_store::{
    AppClient, LoginRequest, MemoryVault, Profiles, ProfilesConfig, SignInOutcome, Store, StoreError, SyncStatus,
    TokenVault, Url,
};

const TIMEOUT: Duration = Duration::from_secs(20);
/// Nothing needs to listen there: the browser stand-in stops at the redirect.
const REDIRECT_PORT: u16 = 49152;

/// A device: its profiles and the store it shows right now.
pub struct TestStore {
    pub profiles: Profiles,
    /// `None` only while a blocking call has it.
    store: Option<Store>,
    pub vault: Arc<MemoryVault>,
    changes: Mutex<Receiver<()>>,
}

impl TestStore {
    pub fn open(data_dir: &Path) -> TestStore {
        TestStore::open_with_vault(data_dir, Arc::new(MemoryVault::default()))
    }

    /// Reopens a device after a restart: the tokens live in the vault, not in the data folder.
    pub fn open_with_vault(data_dir: &Path, vault: Arc<MemoryVault>) -> TestStore {
        let (tx, rx) = channel();
        let tx = Arc::new(Mutex::new(tx));
        let config = ProfilesConfig { data_dir: data_dir.to_path_buf(), vault: vault.clone() };
        let (profiles, store) = Profiles::open(config, move || {
            let _ = tx.lock().unwrap().send(());
        })
        .expect("profiles open");
        TestStore { profiles, store: Some(store), vault, changes: Mutex::new(rx) }
    }

    pub fn store(&self) -> &Store {
        self.store.as_ref().expect("a store")
    }

    /// Runs `work` with the store on a blocking thread (the store's network calls build their
    /// own runtime) and keeps the store it returns, if any.
    pub async fn blocking<T: Send + 'static>(
        &mut self,
        work: impl FnOnce(&Profiles, &Store) -> (Option<Store>, T) + Send + 'static,
    ) -> T {
        let store = self.store.take().expect("a store");
        let profiles = self.profiles.clone();
        let (old, new, output) = tokio::task::spawn_blocking(move || {
            let (new, output) = work(&profiles, &store);
            (store, new, output)
        })
        .await
        .expect("blocking work");
        self.store = Some(new.unwrap_or(old));
        output
    }

    /// Signs `username` in through the server's login page and the fake provider. Returns whether
    /// the guest data is offered for import.
    pub async fn sign_in(&mut self, server: &TestServer, username: &str) -> Result<bool, StoreError> {
        let server_url = server.url();
        let login = LoginRequest::new(&server_url, AppClient::Desktop { redirect_port: REDIRECT_PORT })?;
        let browser = Browser::new(server_url.as_str()).login_as(username);
        let redirect_uri = AppClient::Desktop { redirect_port: REDIRECT_PORT }.redirect_uri(&server_url);
        let callback = match browser.authorize(login.authorize_url().as_str(), &redirect_uri).await {
            Ok(callback) => callback,
            Err(error) => panic!("the sign-in did not come back to the app: {error:?}"),
        };
        self.blocking(move |profiles, store| {
            let signed_in = match login.finish(&callback) {
                Ok(signed_in) => signed_in,
                Err(error) => return (None, Err(error)),
            };
            match profiles.sign_in(store, signed_in) {
                Ok(SignInOutcome::Switched { store, offer_guest_import }) => (Some(store), Ok(offer_guest_import)),
                Ok(SignInOutcome::Renewed) => (None, Ok(false)),
                Err(error) => (None, Err(error)),
            }
        })
        .await
    }

    pub async fn sign_out(&mut self) -> Result<(), StoreError> {
        self.blocking(|profiles, store| match profiles.sign_out(store) {
            Ok(guest) => (Some(guest), Ok(())),
            Err(error) => (None, Err(error)),
        })
        .await
    }

    pub async fn import_guest(&mut self) -> Result<usize, StoreError> {
        self.blocking(|profiles, store| (None, profiles.import_guest(store))).await
    }

    /// The access token the device holds now.
    pub fn access_token(&self) -> Option<String> {
        let account = self.store().account()?;
        let key = quest_store::AccountKey::new(&account.server_url, &account.info.issuer, &account.info.subject);
        self.vault.load(&key).unwrap().map(|tokens| tokens.access_token)
    }

    /// Blocks until `condition` holds, re-checking whenever the store reports a change.
    pub fn wait_for(&self, what: &str, condition: impl Fn(&Store) -> bool) {
        let deadline = Instant::now() + TIMEOUT;
        let changes = self.changes.lock().unwrap();
        while !condition(self.store()) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            // A short tick also catches conditions that change without a notification.
            match changes.recv_timeout(remaining.min(Duration::from_millis(200))) {
                Ok(()) | Err(RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
                _ => panic!("timed out waiting for {what}; status is {:?}", self.store().status()),
            }
        }
    }

    pub fn wait_live_and_drained(&self) {
        self.wait_for("live with an empty outbox", |s| s.status() == SyncStatus::Live { pending: 0 });
    }

    pub fn wait_offline(&self) {
        self.wait_for("offline", |s| matches!(s.status(), SyncStatus::Offline { .. }));
    }

    pub fn create_task(&self, title: &str) -> EntityId {
        let id = EntityId::new();
        let command = Command::CreateTask {
            id,
            title: title.into(),
            quest_id: None,
            planned_on: None,
            due_on: None,
            position: Position::Bottom,
        };
        self.store().dispatch(command).expect("task is valid");
        id
    }

    pub fn create_quest(&self, title: &str) -> EntityId {
        let id = EntityId::new();
        self.store()
            .dispatch(Command::CreateQuest { id, title: title.into(), color: QuestColor::Red })
            .expect("quest is valid");
        id
    }

    pub fn task_title(&self, id: EntityId) -> Option<String> {
        self.store().snapshot().task(id).map(|t| t.title.clone())
    }
}

/// A server whose database outlives restarts, listening on a fixed address after the first start,
/// with the OIDC fake (users anna and ben) as identity provider.
pub struct TestServer {
    database_url: String,
    addr: Option<SocketAddr>,
    running: Option<quest_server::RunningServer>,
    provider: Option<FakeProvider>,
    ai: Option<quest_server::AiConfig>,
    mcp: Option<quest_server::McpConfig>,
}

impl TestServer {
    pub fn new(dir: &Path) -> TestServer {
        let database_url = format!("sqlite://{}", dir.join("server.sqlite").display());
        TestServer { database_url, addr: None, running: None, provider: None, ai: None, mcp: None }
    }

    /// A server with AI capture through `ai`.
    #[allow(dead_code)] // only the `ai` test uses it
    pub fn with_ai(dir: &Path, ai: quest_server::AiConfig) -> TestServer {
        TestServer { ai: Some(ai), ..TestServer::new(dir) }
    }

    /// A server that AI clients can connect to over OAuth and MCP.
    #[allow(dead_code)] // only the `connections` test uses it
    pub fn with_mcp(dir: &Path) -> TestServer {
        let mcp = quest_server::McpConfig::default();
        TestServer { mcp: Some(mcp), ..TestServer::new(dir) }
    }

    pub async fn start(&mut self) {
        // The store's reqwest brings no TLS provider of its own (the apps pick ring).
        let _ = rustls::crypto::ring::default_provider().install_default();
        if self.provider.is_none() {
            self.provider = Some(FakeProvider::start(FakeConfig::default()).await.expect("the OIDC fake starts"));
        }
        let provider = self.provider.as_ref().unwrap();
        let bind = self.addr.unwrap_or_else(|| "127.0.0.1:0".parse().unwrap());
        let oidc = quest_server::OidcSettings {
            issuer: provider.issuer().parse().unwrap(),
            client_id: provider.client_id().to_owned(),
            client_secret: provider.client_secret().to_owned(),
        };
        let config = quest_server::Config {
            bind,
            database_url: self.database_url.clone(),
            ai: self.ai.clone(),
            auth: quest_server::AuthConfig::with_oidc(None, oidc).expect("valid OIDC settings"),
            mcp: self.mcp.clone(),
        };
        let running = quest_server::start(config).await.expect("server starts");
        self.addr = Some(running.addr());
        self.running = Some(running);
    }

    pub async fn stop(&mut self) {
        if let Some(running) = self.running.take() {
            running.shutdown().await;
        }
    }

    pub fn url(&self) -> Url {
        format!("http://{}", self.addr.expect("server was started")).parse().unwrap()
    }
}
