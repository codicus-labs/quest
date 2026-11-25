//! The mobile core against a real quest-server whose identity provider is the in-process OIDC fake:
//! signing in through the (stand-in) browser, taking the guest data along, switching accounts,
//! signing out, and the AI opt-in.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use quest_mobile::{
    AddPosition, ChangeListener, HomeQuery, MobileError, Moment, QuestCore, SavedTokens, SignInResult, SyncOutcome,
    TaskSection, TokenStore, TokenStoreError, UrlPolicy,
};
use quest_oidc_fake::{Browser, FakeConfig, FakeProvider};
use quest_protocol::ANDROID_REDIRECT_URI;

#[derive(Default)]
struct MemoryTokens(Mutex<HashMap<String, SavedTokens>>);

impl TokenStore for MemoryTokens {
    fn load(&self, key: String) -> Result<Option<SavedTokens>, TokenStoreError> {
        Ok(self.0.lock().unwrap().get(&key).cloned())
    }

    fn save(&self, key: String, tokens: SavedTokens) -> Result<(), TokenStoreError> {
        self.0.lock().unwrap().insert(key, tokens);
        Ok(())
    }

    fn delete(&self, key: String) -> Result<(), TokenStoreError> {
        self.0.lock().unwrap().remove(&key);
        Ok(())
    }
}

struct Quiet;

impl ChangeListener for Quiet {
    fn on_change(&self) {}
}

struct Server {
    _provider: FakeProvider,
    running: quest_server::RunningServer,
    _dir: tempfile::TempDir,
}

impl Server {
    async fn start() -> Server {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let dir = tempfile::tempdir().unwrap();
        let provider = FakeProvider::start(FakeConfig::default()).await.expect("the OIDC fake starts");
        let oidc = quest_server::OidcSettings {
            issuer: provider.issuer().parse().unwrap(),
            client_id: provider.client_id().to_owned(),
            client_secret: provider.client_secret().to_owned(),
        };
        let config = quest_server::Config {
            bind: "127.0.0.1:0".parse().unwrap(),
            database_url: format!("sqlite://{}", dir.path().join("server.sqlite").display()),
            ai: None,
            auth: quest_server::AuthConfig::with_oidc(None, oidc).expect("valid OIDC settings"),
            mcp: None,
        };
        let running = quest_server::start(config).await.expect("the server starts");
        Server { _provider: provider, running, _dir: dir }
    }

    fn url(&self) -> String {
        format!("http://{}", self.running.addr())
    }
}

fn now() -> Moment {
    let epoch_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
    Moment { epoch_ms: epoch_ms as i64, time_zone: "Europe/Berlin".into() }
}

fn titles(core: &QuestCore) -> Vec<String> {
    core.home(now(), HomeQuery::default()).unwrap().other.into_iter().map(|t| t.title).collect()
}

fn add(core: &QuestCore, title: &str) {
    core.add_task(title.into(), TaskSection::Other, AddPosition::Bottom, HomeQuery::default(), now()).unwrap();
}

/// Runs a blocking core call off the async runtime, as the app runs it off the UI thread.
async fn blocking<T: Send + 'static>(core: &Arc<QuestCore>, call: impl FnOnce(&QuestCore) -> T + Send + 'static) -> T {
    let core = core.clone();
    tokio::task::spawn_blocking(move || call(&core)).await.unwrap()
}

/// Signs `user` in as the Android app does: the authorize page in the browser ("Continue as …"
/// confirmed), the redirect back.
async fn sign_in(core: &Arc<QuestCore>, server: &Server, user: &str) -> Result<SignInResult, MobileError> {
    let authorize = core.begin_sign_in(server.url(), UrlPolicy::Release, None)?;
    let browser = Browser::new(&server.url()).login_as(user);
    let redirect =
        browser.authorize(&authorize, ANDROID_REDIRECT_URI).await.expect("the sign-in comes back to the app");
    blocking(core, move |core| core.finish_sign_in(redirect.to_string())).await
}

async fn sync(core: &Arc<QuestCore>) {
    assert_eq!(blocking(core, |core| core.sync_until_idle(20_000)).await, SyncOutcome::Idle);
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_keep_their_own_data_and_the_guest_data_can_move_in() {
    let server = Server::start().await;
    let dir = tempfile::tempdir().unwrap();
    let core =
        QuestCore::open(dir.path().display().to_string(), Arc::new(MemoryTokens::default()), Arc::new(Quiet)).unwrap();
    add(&core, "Guest idea");

    let SignInResult::SignedIn { offer_guest_import, account } = sign_in(&core, &server, "anna").await.unwrap() else {
        panic!("anna should be signed in");
    };
    assert!(offer_guest_import, "the guest has data");
    assert!(!account.ai_features, "AI features are off by default");
    assert_eq!(account.server_url, server.url());
    assert!(titles(&core).is_empty(), "anna's store starts without the guest's data");
    assert_eq!(blocking(&core, |core| core.import_guest_data()).await, Ok(1));
    assert_eq!(titles(&core), ["Guest idea"]);
    add(&core, "Anna's task");
    sync(&core).await;
    assert_eq!(core.sign_out_warning(), None, "everything reached the server");

    let logout = blocking(&core, |core| core.sign_out()).await.unwrap();
    assert!(logout.starts_with(&format!("{}/auth/logout?return_to=", server.url())), "{logout}");
    assert_eq!(core.account(), None);
    assert!(titles(&core).is_empty(), "the guest data moved into anna's account");
    let back = blocking(&core, |core| core.finish_sign_in(ANDROID_REDIRECT_URI.into())).await;
    assert_eq!(back, Ok(SignInResult::NotASignIn), "returning from signing out");

    let SignInResult::SignedIn { offer_guest_import, .. } = sign_in(&core, &server, "ben").await.unwrap() else {
        panic!("ben should be signed in");
    };
    assert!(!offer_guest_import, "the guest is empty");
    sync(&core).await;
    assert!(titles(&core).is_empty(), "ben sees none of anna's tasks");
    blocking(&core, |core| core.sign_out()).await.unwrap();

    sign_in(&core, &server, "anna").await.unwrap();
    sync(&core).await;
    let mut anna = titles(&core);
    anna.sort();
    assert_eq!(anna, ["Anna's task", "Guest idea"], "anna's data comes back from the server");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_ai_opt_in_is_kept_by_the_server() {
    let server = Server::start().await;
    let dir = tempfile::tempdir().unwrap();
    let core =
        QuestCore::open(dir.path().display().to_string(), Arc::new(MemoryTokens::default()), Arc::new(Quiet)).unwrap();
    assert_eq!(blocking(&core, |core| core.set_ai_enabled(true)).await, Err(MobileError::NotSignedIn));
    sign_in(&core, &server, "anna").await.unwrap();

    let account = blocking(&core, |core| core.set_ai_enabled(true)).await.unwrap();
    assert!(account.ai_enabled);
    assert!(!account.ai_features, "this server has no AI provider");
    assert_eq!(core.account().map(|a| a.ai_enabled), Some(true));

    // An answer that is not the one awaited (another app can send this address too) leaves the
    // sign-in waiting; once answered, a further answer finds none.
    let authorize = core.begin_sign_in(server.url(), UrlPolicy::Release, None).unwrap();
    let forged = blocking(&core, |core| core.finish_sign_in(format!("{ANDROID_REDIRECT_URI}?code=x&state=y"))).await;
    assert!(matches!(forged, Err(MobileError::SignInFailed { .. })), "{forged:?}");
    let browser = Browser::new(&server.url()).login_as("anna");
    let redirect = browser.authorize(&authorize, ANDROID_REDIRECT_URI).await.unwrap();
    let renewed = blocking(&core, move |core| core.finish_sign_in(redirect.to_string())).await.unwrap();
    assert!(matches!(renewed, SignInResult::Renewed { .. }), "{renewed:?}");
    let again = blocking(&core, |core| core.finish_sign_in(format!("{ANDROID_REDIRECT_URI}?code=x&state=y"))).await;
    assert_eq!(again, Err(MobileError::SignInExpired));
}
