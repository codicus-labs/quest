//! Profiles: the guest store and one store per account, and switching between them on signing in
//! and out. Every method does disk IO, and some network IO: never call them on the UI thread. They
//! return the store to use next; the app replaces its current store with it.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use quest_protocol::AccountInfo;
use url::Url;

use crate::db::{self, Db};
use crate::error::StoreError;
use crate::layout::{AccountKey, ActiveProfile, Layout};
use crate::session::AccountSession;
use crate::vault::{StoredTokens, TokenVault};
use crate::worker::Timing;
use crate::{Store, SyncOutcome, guest_import, oauth};

/// How long importing guest data waits for the account to catch up first, so the account's own
/// settings are known and kept.
const IMPORT_CATCH_UP: Duration = Duration::from_secs(10);
const IMPORT_SAVE_TIMEOUT: Duration = Duration::from_secs(10);

pub struct ProfilesConfig {
    /// Folder for all profiles; created if missing.
    pub data_dir: PathBuf,
    /// Where the accounts' tokens are kept.
    pub vault: Arc<dyn TokenVault>,
}

/// The result of a completed sign-in (`LoginRequest::finish`), for `Profiles::sign_in`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedIn {
    /// The server's origin.
    pub server_url: Url,
    pub client_id: String,
    pub tokens: StoredTokens,
    pub account: AccountInfo,
}

pub enum SignInOutcome {
    /// Switched from the guest to the account: use `store` from now on. If `offer_guest_import`,
    /// ask once whether to move the guest data into the account (`Profiles::import_guest`).
    Switched { store: Store, offer_guest_import: bool },
    /// The active account signed in again (its session had expired); the current store
    /// reconnects with the new tokens.
    Renewed,
}

/// Opens and switches profiles. Cheap to clone; safe from any thread. Open it once per data folder
/// and process: token refreshes are only coordinated within one `Profiles` and its stores.
#[derive(Clone)]
pub struct Profiles {
    layout: Layout,
    vault: Arc<dyn TokenVault>,
    on_change: Arc<dyn Fn() + Send + Sync>,
    timing: Timing,
}

impl Profiles {
    /// Opens the active profile (the guest if none is signed in). Adopts the database of earlier
    /// versions as the guest store on first use. `on_change` is passed to every store opened.
    pub fn open(
        config: ProfilesConfig,
        on_change: impl Fn() + Send + Sync + 'static,
    ) -> Result<(Profiles, Store), StoreError> {
        Profiles::open_with(config, Timing::PRODUCTION, Arc::new(on_change))
    }

    pub(crate) fn open_with(
        config: ProfilesConfig,
        timing: Timing,
        on_change: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<(Profiles, Store), StoreError> {
        let profiles = Profiles { layout: Layout::new(config.data_dir), vault: config.vault, on_change, timing };
        profiles.layout.adopt_legacy_db()?;
        if let Err(error) = profiles.layout.remove_leftovers() {
            tracing::warn!(%error, "could not delete the data of a signed-out account; retrying on the next start");
        }
        let store = match profiles.layout.read_active()? {
            ActiveProfile::Guest => profiles.open_guest()?,
            ActiveProfile::Account { server_url, client_id, key } => {
                profiles.open_account(server_url, client_id, key)?
            }
        };
        Ok((profiles, store))
    }

    /// Switches from the guest store `current` to the signed-in account, or renews the session
    /// of the active account. Another account than the active one is refused: sign out first.
    pub fn sign_in(&self, current: &Store, signed_in: SignedIn) -> Result<SignInOutcome, StoreError> {
        let SignedIn { server_url, client_id, tokens, account } = signed_in;
        let key = AccountKey::new(&server_url, &account.issuer, &account.subject);
        if let Some(active) = current.session() {
            if active.key != key {
                let active = current.account().map_or_else(|| "another account".to_owned(), |a| display(&a.info));
                return Err(StoreError::AnotherAccountActive { active, signing_in: display(&account) });
            }
            self.save_tokens(&key, &tokens)?;
            current.update_account_info(account);
            current.resume();
            return Ok(SignInOutcome::Renewed);
        }
        let dir = self.layout.account_dir(&key);
        let first_on_device = !dir.join(db::FILE_NAME).exists();
        self.save_tokens(&key, &tokens)?;
        Db::open(&dir)?.set_account_info(&account)?;
        let store = self.open_account(server_url.clone(), client_id.clone(), key.clone())?;
        self.layout.write_active(&ActiveProfile::Account { server_url, client_id, key })?;
        Ok(SignInOutcome::Switched { store, offer_guest_import: first_on_device && current.has_data() })
    }

    /// Moves the guest data into the account store `account` as new local changes (fresh
    /// timestamps, synced from the outbox), then empties the guest store. Fields the account
    /// already has keep its values; the guest's settings only come along if the account caught up
    /// with the server first (within 10 s), else they are dropped with the guest data. Call it after replacing the guest store. Returns how many
    /// items (quests, tasks, …) were imported.
    pub fn import_guest(&self, account: &Store) -> Result<usize, StoreError> {
        if account.session().is_none() {
            return Err(StoreError::NotSignedIn);
        }
        // Offline, the import goes ahead with what the account has locally, but without settings.
        let caught_up = account.sync_until_idle(IMPORT_CATCH_UP) == SyncOutcome::Idle;
        let mut guest = Db::open(&self.layout.guest_dir())?;
        let writes = guest_import::plan(&guest.load()?.fields, &account.shared.fields(), caught_up);
        let changes = account.shared.apply_imported(writes);
        let imported = guest_import::entity_count(&changes);
        account.queue(changes);
        if !account.flush(IMPORT_SAVE_TIMEOUT) {
            return Err(StoreError::Storage {
                path: self.layout.guest_dir().display().to_string(),
                message: "the imported data could not be saved yet; the guest data stays until it is".into(),
            });
        }
        guest.reset_local_data()?;
        Ok(imported)
    }

    /// Signs the account of `current` out: stops syncing, revokes the sign-in at the server
    /// (best effort), deletes its tokens and its local data (the server keeps everything that was
    /// synced; `Store::pending` changes are lost, so warn first). Returns the guest store.
    pub fn sign_out(&self, current: &Store) -> Result<Store, StoreError> {
        let session = current.session().ok_or(StoreError::NotSignedIn)?;
        let refresh_token = session.tokens().ok().flatten().and_then(|tokens| tokens.refresh_token);
        current.close_session();
        // Also if the worker did not answer in time: no refresh may store tokens after the delete.
        session.close();
        self.layout.write_active(&ActiveProfile::Guest)?;
        if let Some(refresh_token) = refresh_token {
            revoke(&session, &refresh_token);
        }
        if let Err(error) = session.delete_tokens() {
            tracing::warn!(%error, "could not delete the signed-out account's tokens; they were revoked");
        }
        if let Err(error) = self.layout.remove_account_dir(&session.key) {
            tracing::warn!(%error, "could not delete the signed-out account's local data");
        }
        self.open_guest()
    }

    fn open_guest(&self) -> Result<Store, StoreError> {
        Store::open_with(&self.layout.guest_dir(), None, self.timing, self.change_callback())
    }

    fn open_account(&self, server_url: Url, client_id: String, key: AccountKey) -> Result<Store, StoreError> {
        let dir = self.layout.account_dir(&key);
        let session = AccountSession::new(server_url, client_id, key, self.vault.clone());
        Store::open_with(&dir, Some(Arc::new(session)), self.timing, self.change_callback())
    }

    fn save_tokens(&self, key: &AccountKey, tokens: &StoredTokens) -> Result<(), StoreError> {
        self.vault.save(key, tokens).map_err(|e| StoreError::TokenStore { message: e.message })
    }

    fn change_callback(&self) -> Box<dyn Fn() + Send + Sync> {
        let on_change = self.on_change.clone();
        Box::new(move || on_change())
    }
}

/// Revokes the refresh token; failures only log, signing out goes ahead.
fn revoke(session: &AccountSession, refresh_token: &str) {
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => return tracing::warn!(%error, "could not revoke the sign-in"),
    };
    if let Err(error) = runtime.block_on(oauth::revoke(&session.server_url, &session.client_id, refresh_token)) {
        tracing::warn!(%error, "could not revoke the sign-in at the server");
    }
}

/// How to name an account to the user.
fn display(info: &AccountInfo) -> String {
    info.email.clone().or_else(|| info.name.clone()).unwrap_or_else(|| info.subject.clone())
}

#[cfg(test)]
#[path = "profiles_tests.rs"]
mod tests;
