//! `QuestCore` for the account: signing in through the browser (RFC 8252), taking the guest data
//! along, signing out, and the account's AI setting. Switching profiles replaces the core's store;
//! everything bound to the core (widget, worker, tile, share, focus service) follows it.

use std::sync::{Arc, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use quest_store::{AppClient, LoginRequest, SignInOutcome, Store, Url};

use crate::account::{self, account_error, sign_in_error};
use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::server_url;
use crate::types::{Account, SignInResult, UrlPolicy};

/// How long a switch waits for other calls to let go of the replaced store, so its worker is
/// stopped here and not on whichever thread drops it last (possibly the UI thread).
const RELEASE_WAIT: Duration = Duration::from_secs(1);

#[uniffi::export]
impl QuestCore {
    /// The signed-in account; None for the guest.
    pub fn account(&self) -> Option<Account> {
        self.store().account().as_ref().map(account::account)
    }

    /// Starts signing in at `server_url`: returns the page to open in the browser (Custom Tab).
    /// It comes back to `{origin}/auth/android` (App Link) if the server is `app_link_origin`,
    /// the https origin a build's App Link is verified for (the app passes None), else to
    /// `app.quest.android:/oauth2redirect`; hand that to `finish_sign_in`. A new
    /// call replaces an unfinished sign-in. Refuses unencrypted URLs like `check_server_url`.
    /// Memory-only.
    pub fn begin_sign_in(
        &self,
        server_url: String,
        policy: UrlPolicy,
        app_link_origin: Option<String>,
    ) -> Result<String, MobileError> {
        guard(|| {
            let url = server_url::parse_server_url(&server_url, policy)?;
            let client = AppClient::android(&url, app_link_origin.as_deref());
            let login = LoginRequest::new(&url, client).map_err(sign_in_error)?;
            let authorize_url = login.authorize_url().to_string();
            *self.login.lock().unwrap_or_else(PoisonError::into_inner) = Some(login);
            Ok(authorize_url)
        })
    }

    /// The browser came back to the app with `redirect_url`. Finishes the sign-in started with
    /// `begin_sign_in`: checks the answer, gets the tokens and switches to the account (or renews
    /// its expired session). A return from signing out is `NotASignIn`. Blocks on the network:
    /// never on the UI thread.
    pub fn finish_sign_in(&self, redirect_url: String) -> Result<SignInResult, MobileError> {
        guard(|| {
            let redirect = Url::parse(&redirect_url).map_err(|e| MobileError::SignInFailed {
                reason: format!("the browser returned an invalid address ({e})"),
            })?;
            if !account::is_sign_in_answer(&redirect) {
                return Ok(SignInResult::NotASignIn);
            }
            let login = {
                let mut waiting = self.login.lock().unwrap_or_else(PoisonError::into_inner);
                let Some(login) = waiting.as_ref() else { return Err(MobileError::SignInExpired) };
                // Another app can send this address too: an answer to some other sign-in must not
                // cancel the one waiting.
                if !account::answers(login.authorize_url(), &redirect) {
                    return Err(MobileError::SignInFailed {
                        reason: "the answer does not belong to this sign-in".into(),
                    });
                }
                waiting.take().ok_or(MobileError::SignInExpired)?
            };
            let signed_in = login.finish(&redirect).map_err(sign_in_error)?;
            let _switching = self.switching.lock().unwrap_or_else(PoisonError::into_inner);
            let current = self.store();
            let outcome = self.profiles.sign_in(&current, signed_in).map_err(sign_in_error)?;
            drop(current);
            match outcome {
                SignInOutcome::Switched { store, offer_guest_import } => {
                    release(self.replace_store(store));
                    Ok(SignInResult::SignedIn { offer_guest_import, account: self.signed_in_account()? })
                }
                SignInOutcome::Renewed => {
                    self.listener.on_change();
                    Ok(SignInResult::Renewed { account: self.signed_in_account()? })
                }
            }
        })
    }

    /// "Add guest data to this account?" answered with yes: moves the guest data into the account
    /// (synced as new changes) and empties the guest store. Returns how many items moved. Blocks
    /// (up to about 20 s while the account catches up and saves): never on the UI thread.
    pub fn import_guest_data(&self) -> Result<u32, MobileError> {
        guard(|| {
            let _switching = self.switching.lock().unwrap_or_else(PoisonError::into_inner);
            let store = self.store();
            let imported = self.profiles.import_guest(&store).map_err(sign_in_error)?;
            self.listener.on_change();
            Ok(u32::try_from(imported).unwrap_or(u32::MAX))
        })
    }

    /// What to warn about before signing out (changes the server has not got yet); None: nothing
    /// is lost. Memory-only.
    pub fn sign_out_warning(&self) -> Option<String> {
        let store = self.store();
        store.account()?;
        account::sign_out_warning(store.pending())
    }

    /// Signs out: revokes the sign-in at the server (best effort), deletes the account's tokens and
    /// its data on this device (the server keeps what was synced) and switches to the guest.
    /// Returns the page that ends the browser's sign-in too; open it in the browser, it comes back
    /// to the app. Blocks (up to about 10 s): never on the UI thread.
    /// Edits made meanwhile are refused (`SigningOut`) rather than lost with the account's data.
    pub fn sign_out(&self) -> Result<String, MobileError> {
        guard(|| {
            let _switching = self.switching.lock().unwrap_or_else(PoisonError::into_inner);
            let current = self.store();
            let signed_in = current.account().ok_or(MobileError::NotSignedIn)?;
            let logout_url = AppClient::Android.logout_url(&signed_in.server_url).map_err(sign_in_error)?;
            self.set_signing_out(true);
            let signed_out = self.profiles.sign_out(&current).map_err(sign_in_error);
            drop(current);
            let replaced = signed_out.map(|guest| self.replace_store(guest));
            self.set_signing_out(false);
            release(replaced?);
            *self.login.lock().unwrap_or_else(PoisonError::into_inner) = None;
            Ok(logout_url.to_string())
        })
    }

    /// Turns the account's AI features on or off on the server (all devices). Show the privacy
    /// note before turning them on. Blocks on the network (up to about 20 s): never on the UI thread.
    pub fn set_ai_enabled(&self, enabled: bool) -> Result<Account, MobileError> {
        guard(|| {
            let store = self.store();
            store.set_ai_enabled(enabled).map_err(account_error)?;
            let signed_in = store.account().ok_or(MobileError::NotSignedIn)?;
            self.listener.on_change();
            Ok(account::account(&signed_in))
        })
    }
}

impl QuestCore {
    fn signed_in_account(&self) -> Result<Account, MobileError> {
        self.account().ok_or(MobileError::NotSignedIn)
    }

    /// Waits for edits under way when turning on.
    fn set_signing_out(&self, signing_out: bool) {
        *self.signing_out.write().unwrap_or_else(PoisonError::into_inner) = signing_out;
    }
}

/// Drops a replaced store (stopping its worker) on a thread of its own, once no other call holds
/// it any more (or after `RELEASE_WAIT`): never on the caller's, which may be the UI thread.
fn release(previous: Arc<Store>) {
    let released = thread::Builder::new().name("quest-release-store".into()).spawn(move || {
        let started = Instant::now();
        while Arc::strong_count(&previous) > 1 && started.elapsed() < RELEASE_WAIT {
            thread::sleep(Duration::from_millis(10));
        }
        drop(previous);
    });
    // Without a thread the store is dropped here instead, which is still correct.
    drop(released);
}
