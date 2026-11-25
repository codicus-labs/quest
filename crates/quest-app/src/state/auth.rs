//! Signing in and out, importing guest data and the account's AI setting. Each runs on its own
//! thread (they wait on the browser, the network and the disk); results come back as `AuthEvent`s
//! that `refresh` takes in. A sign-in or sign-out hands over the next store, which replaces the
//! current one.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use quest_store::{
    AccountError, AccountInfo, AppClient, LoginRequest, Profiles, SignInOutcome, Store, StoreError, SyncStatus, Url,
};

use super::{Overlay, State, View};
use crate::browser::Browser;
use crate::loopback::{Canceller, Loopback, Reply};

/// What the account flows are busy with. One flow at a time: a new one starts only from `Idle`,
/// so two sign-in threads never switch to the same account.
pub enum AuthStep {
    Idle,
    /// Waiting for the browser's redirect.
    SigningIn {
        canceller: Canceller,
    },
    /// "Cancel" was pressed; waits for the sign-in thread to stop.
    CancellingSignIn,
    /// Signing out would lose `pending` changes that haven't reached the server: asks first.
    ConfirmSignOut {
        pending: usize,
    },
    /// Edits are locked meanwhile: the account's data is about to be deleted.
    SigningOut,
    Importing,
    SavingAi,
}

enum AuthEvent {
    SignedIn(Result<SignInOutcome, StoreError>),
    SignedOut(Result<Store, StoreError>),
    Imported(Result<usize, StoreError>),
    AiSaved(Result<AccountInfo, AccountError>),
}

pub struct AuthState {
    pub profiles: Profiles,
    pub step: AuthStep,
    pub browser: Browser,
    /// Why importing the guest data failed; the dialog offers to try again.
    pub import_error: Option<String>,
    sender: Sender<AuthEvent>,
    events: Receiver<AuthEvent>,
    waker: Option<rune::Waker>,
}

impl AuthState {
    pub fn new(profiles: Profiles) -> AuthState {
        let (sender, events) = mpsc::channel();
        AuthState {
            profiles,
            step: AuthStep::Idle,
            browser: Browser::default(),
            import_error: None,
            sender,
            events,
            waker: None,
        }
    }

    /// Wakes the window when a result lands.
    pub fn set_waker(&mut self, waker: rune::Waker) {
        self.waker = Some(waker);
    }

    /// While signing out, changes would be deleted with the account's data unseen.
    pub fn locks_edits(&self) -> bool {
        matches!(self.step, AuthStep::SigningOut)
    }

    /// Runs `work` on its own thread and sends its event back.
    fn spawn(&self, work: impl FnOnce() -> AuthEvent + Send + 'static) {
        let sender = self.sender.clone();
        let waker = self.waker.clone();
        std::thread::spawn(move || {
            // The window may be gone; nobody is left to tell.
            let _ = sender.send(work());
            if let Some(wake) = waker {
                wake();
            }
        });
    }
}

/// Whether the store's sign-in has expired (the account's data stays; signing in again renews it).
pub fn session_expired(status: &SyncStatus) -> bool {
    matches!(status, SyncStatus::Failed { error: StoreError::SessionExpired })
}

impl State {
    /// "Sign in": opens the browser at the server's sign-in and waits for its redirect. Signing
    /// in again after the session expired uses the account's server.
    pub fn start_sign_in(&mut self) {
        if !matches!(self.auth.step, AuthStep::Idle) {
            return;
        }
        let server = match &self.account {
            Some(account) => account.server_url.to_string(),
            None => self.account_form.server.text.trim().to_owned(),
        };
        if server.is_empty() {
            self.notice = Some("Enter your Quest server’s address, for example https://quest.example.com.".into());
            return;
        }
        let server_url = match Url::parse(&server) {
            Ok(url) => url,
            Err(error) => {
                self.notice = Some(format!(
                    "“{server}” is not a server URL ({error}). Use something like https://quest.example.com."
                ));
                return;
            }
        };
        self.notice = None;
        let canceller = Canceller::default();
        let (profiles, store, browser) = (self.auth.profiles.clone(), self.store.clone(), self.auth.browser.clone());
        let thread_canceller = canceller.clone();
        self.auth
            .spawn(move || AuthEvent::SignedIn(sign_in(&profiles, &store, &server_url, &browser, thread_canceller)));
        self.auth.step = AuthStep::SigningIn { canceller };
    }

    pub fn cancel_sign_in(&mut self) {
        if let AuthStep::SigningIn { canceller } = &self.auth.step {
            canceller.cancel();
            self.auth.step = AuthStep::CancellingSignIn;
        }
    }

    /// "Sign out": asks first when changes would be lost, else signs out.
    pub fn ask_sign_out(&mut self) {
        if !matches!(self.auth.step, AuthStep::Idle) {
            return;
        }
        match self.store.pending() {
            0 => self.sign_out(),
            pending => self.auth.step = AuthStep::ConfirmSignOut { pending },
        }
    }

    pub fn keep_signed_in(&mut self) {
        self.auth.step = AuthStep::Idle;
    }

    /// Signs out (revokes the sign-in, deletes the account's local data), then shows the guest
    /// data and ends the browser session at the server.
    pub fn sign_out(&mut self) {
        if !matches!(self.auth.step, AuthStep::Idle | AuthStep::ConfirmSignOut { .. }) {
            return;
        }
        let Some(account) = &self.account else { return };
        let logout = AppClient::Desktop { redirect_port: 0 }.logout_url(&account.server_url).ok();
        let (profiles, store, browser) = (self.auth.profiles.clone(), self.store.clone(), self.auth.browser.clone());
        self.auth.spawn(move || {
            let result = profiles.sign_out(&store);
            if let (Ok(_), Some(logout)) = (&result, logout) {
                // Best effort: the app is signed out either way.
                let _ = browser.open(&logout);
            }
            AuthEvent::SignedOut(result)
        });
        self.auth.step = AuthStep::SigningOut;
    }

    /// "Yes, import" in the guest data dialog.
    pub fn import_guest(&mut self) {
        if !matches!(self.auth.step, AuthStep::Idle) {
            return;
        }
        self.auth.import_error = None;
        let (profiles, store) = (self.auth.profiles.clone(), self.store.clone());
        self.auth.spawn(move || AuthEvent::Imported(profiles.import_guest(&store)));
        self.auth.step = AuthStep::Importing;
        self.overlay = Overlay::Account;
    }

    /// "No": the guest data stays in the guest store.
    pub fn keep_guest_data(&mut self) {
        self.auth.import_error = None;
        self.overlay = Overlay::Account;
    }

    /// The account's "AI features" switch: turning on shows the privacy notice first.
    pub fn toggle_ai_features(&mut self) {
        let Some(account) = &self.account else { return };
        if account.info.ai_enabled {
            self.save_ai_enabled(false);
        } else {
            self.overlay = Overlay::AiNotice;
        }
    }

    /// "Turn on" in the privacy notice.
    pub fn accept_ai_notice(&mut self) {
        self.overlay = Overlay::Account;
        self.save_ai_enabled(true);
    }

    pub fn decline_ai_notice(&mut self) {
        self.overlay = Overlay::Account;
    }

    fn save_ai_enabled(&mut self, enabled: bool) {
        if !matches!(self.auth.step, AuthStep::Idle) {
            return;
        }
        let store = self.store.clone();
        self.auth.spawn(move || AuthEvent::AiSaved(store.set_ai_enabled(enabled)));
        self.auth.step = AuthStep::SavingAi;
    }

    /// Takes in results of the account flows; true when anything visible changed.
    pub(super) fn take_auth_events(&mut self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.auth.events.try_recv() {
            changed = true;
            self.on_auth_event(event);
        }
        changed
    }

    fn on_auth_event(&mut self, event: AuthEvent) {
        match event {
            AuthEvent::SignedIn(result) => {
                let cancelled = matches!(self.auth.step, AuthStep::CancellingSignIn);
                self.auth.step = AuthStep::Idle;
                match result {
                    // A cancel that came after the account was opened can't take it back: the
                    // profile switched on disk already, so the app follows.
                    Ok(SignInOutcome::Switched { store, offer_guest_import }) => {
                        self.use_store(store);
                        self.overlay = if offer_guest_import { Overlay::GuestImport } else { Overlay::Account };
                    }
                    Ok(SignInOutcome::Renewed) => self.notice = None,
                    Err(StoreError::SignInCancelled) => {}
                    Err(_) if cancelled => {}
                    Err(error) => self.notice = Some(sentence(&error.to_string())),
                }
            }
            AuthEvent::SignedOut(result) => {
                self.auth.step = AuthStep::Idle;
                match result {
                    Ok(guest) => self.use_store(guest),
                    Err(error) => self.notice = Some(format!("Signing out didn’t work: {error}")),
                }
            }
            AuthEvent::Imported(result) => {
                self.auth.step = AuthStep::Idle;
                match result {
                    Ok(0) => self.notice = None,
                    Ok(count) => self.notice = Some(format!("Imported {count} item(s) from the guest data.")),
                    // The offer stays: the guest data is untouched until an import succeeds.
                    Err(error) => {
                        self.auth.import_error = Some(sentence(&error.to_string()));
                        self.overlay = Overlay::GuestImport;
                    }
                }
            }
            AuthEvent::AiSaved(result) => {
                self.auth.step = AuthStep::Idle;
                match result {
                    Ok(_) => self.notice = None,
                    Err(error) => self.notice = Some(sentence(&error.to_string())),
                }
            }
        }
    }

    /// Replaces the current store with `store` (another profile). The old one is dropped on a
    /// thread: dropping waits for its worker to finish saving.
    pub(crate) fn use_store(&mut self, store: Store) {
        // What the UI holds refers to the old profile's data.
        let overlay = self.overlay;
        self.go(View::Home);
        self.overlay = overlay;
        let old = std::mem::replace(&mut self.store, Arc::new(store));
        if let Some(windows) = &self.desktop {
            windows.publish(&self.store, self.dark, self.auth.locks_edits());
        }
        std::thread::spawn(move || drop(old));
        self.undo = crate::undo::UndoState::default();
        self.capture_undo = None;
        self.capture.step = super::CaptureStep::Compose { error: None };
        self.focus_task = None;
        self.reset_quick_add();
        self.lingering_tasks.clear();
        self.snap = self.store.snapshot();
        self.sync = self.store.status();
        self.account = self.store.account();
        self.rejected = self.store.rejected_count();
        self.sync_focus_draft();
        self.reload_ai_connections();
    }
}

/// How long a sign-in waits for the browser to come back (a closed tab never does).
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// The blocking sign-in: the browser at the server's sign-in, the redirect to the loopback
/// listener, the code exchange, then the switch to the account's store.
fn sign_in(
    profiles: &Profiles,
    current: &Store,
    server_url: &Url,
    browser: &Browser,
    canceller: Canceller,
) -> Result<SignInOutcome, StoreError> {
    let failed = |reason: String| StoreError::SignInFailed { reason };
    let loopback =
        Loopback::bind(canceller.clone()).map_err(|e| failed(format!("no local port for the redirect: {e}")))?;
    let request = LoginRequest::new(server_url, AppClient::Desktop { redirect_port: loopback.port() })?;
    let state = request.authorize_url().query_pairs().find(|(name, _)| name == "state").map(|(_, value)| value);
    let state = state.ok_or_else(|| failed("the sign-in request has no state".to_owned()))?;
    browser.open(request.authorize_url()).map_err(failed)?;
    let callback = match loopback.next_callback(&state, SIGN_IN_TIMEOUT) {
        Ok(Some(callback)) => callback,
        Ok(None) => return Err(StoreError::SignInCancelled),
        Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
            return Err(failed("the browser did not come back within 10 minutes".to_owned()));
        }
        Err(error) => return Err(failed(format!("the redirect did not arrive: {error}"))),
    };
    let finished = request.finish(&callback.url);
    callback.reply(if finished.is_ok() { Reply::SignedIn } else { Reply::Failed });
    let signed_in = finished?;
    // Cancelled while the code was exchanged: stop before switching profiles.
    if canceller.is_cancelled() {
        return Err(StoreError::SignInCancelled);
    }
    profiles.sign_in(current, signed_in)
}

/// Store errors read as the end of a sentence; a notice shows them alone.
fn sentence(message: &str) -> String {
    super::capture::capitalized(message)
}
