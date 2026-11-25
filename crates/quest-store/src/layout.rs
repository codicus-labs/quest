//! Where profiles live in the data folder:
//!
//! - `guest/quest.sqlite`: the guest store (no account, never syncs).
//! - `accounts/<key>/quest.sqlite`: one store per account, see `AccountKey`.
//! - `active.json`: which profile is active (no tokens; missing means guest).
//! - `removed/`: account folders being deleted after signing out; emptied on the next start.
//! - `quest.sqlite`: the single database of earlier versions, adopted as the guest store once.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::db;
use crate::error::StoreError;

const GUEST_DIR: &str = "guest";
const ACCOUNTS_DIR: &str = "accounts";
const REMOVED_DIR: &str = "removed";
const ACTIVE_FILE: &str = "active.json";
const KEY_HEX_LEN: usize = 32;

/// Names an account's local data and its tokens: a hash of the server origin, the identity
/// provider's issuer and the user's subject there (as a JSON array, so no part can run into the
/// next). The server is part of it so the same identity on two servers never shares a database.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AccountKey(String);

impl AccountKey {
    pub fn new(server_url: &Url, issuer: &str, subject: &str) -> AccountKey {
        let input = serde_json::json!([origin(server_url), issuer, subject]).to_string();
        let digest = Sha256::digest(input.as_bytes());
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        AccountKey(hex[..KEY_HEX_LEN].to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AccountKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AccountKey({})", self.0)
    }
}

/// Only keys this module could have made: they become folder names.
impl TryFrom<String> for AccountKey {
    type Error = String;

    fn try_from(raw: String) -> Result<AccountKey, String> {
        let valid = raw.len() == KEY_HEX_LEN && raw.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if valid { Ok(AccountKey(raw)) } else { Err(format!("invalid account key {raw:?}")) }
    }
}

impl From<AccountKey> for String {
    fn from(key: AccountKey) -> String {
        key.0
    }
}

/// The server's origin without a trailing slash, e.g. `https://quest.example.com`: the OAuth
/// issuer the server reports, and the part of a server URL that names the server.
pub(crate) fn origin(server_url: &Url) -> String {
    server_url.origin().ascii_serialization()
}

/// The profile `active.json` names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "profile", rename_all = "snake_case")]
pub(crate) enum ActiveProfile {
    Guest,
    Account { server_url: Url, client_id: String, key: AccountKey },
}

/// The data folder's layout.
#[derive(Clone, Debug)]
pub(crate) struct Layout {
    data_dir: PathBuf,
}

impl Layout {
    pub fn new(data_dir: PathBuf) -> Layout {
        Layout { data_dir }
    }

    pub fn guest_dir(&self) -> PathBuf {
        self.data_dir.join(GUEST_DIR)
    }

    pub fn account_dir(&self, key: &AccountKey) -> PathBuf {
        self.data_dir.join(ACCOUNTS_DIR).join(key.as_str())
    }

    /// Missing means guest. An unreadable file is an error rather than a silent guest: the
    /// account's data and tokens would otherwise look abandoned.
    pub fn read_active(&self) -> Result<ActiveProfile, StoreError> {
        let path = self.data_dir.join(ACTIVE_FILE);
        let raw = match std::fs::read(&path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ActiveProfile::Guest),
            Err(e) => return Err(storage(&path, e)),
        };
        serde_json::from_slice(&raw).map_err(|e| storage(&path, format!("unreadable: {e}")))
    }

    /// Replaces `active.json` atomically.
    pub fn write_active(&self, active: &ActiveProfile) -> Result<(), StoreError> {
        let path = self.data_dir.join(ACTIVE_FILE);
        let temporary = self.data_dir.join(format!("{ACTIVE_FILE}.tmp"));
        let json = serde_json::to_vec_pretty(active).map_err(|e| storage(&path, e))?;
        std::fs::create_dir_all(&self.data_dir).map_err(|e| storage(&self.data_dir, e))?;
        std::fs::write(&temporary, json).map_err(|e| storage(&temporary, e))?;
        std::fs::rename(&temporary, &path).map_err(|e| storage(&path, e))
    }

    /// Moves an account's folder out of the way (atomic), then deletes it. A failed delete is
    /// finished by `remove_leftovers` on the next start.
    pub fn remove_account_dir(&self, key: &AccountKey) -> Result<(), StoreError> {
        let dir = self.account_dir(key);
        if !dir.exists() {
            return Ok(());
        }
        let removed = self.data_dir.join(REMOVED_DIR);
        std::fs::create_dir_all(&removed).map_err(|e| storage(&removed, e))?;
        let target = removed.join(format!("{}-{:016x}", key.as_str(), rand::random::<u64>()));
        std::fs::rename(&dir, &target).map_err(|e| storage(&dir, e))?;
        std::fs::remove_dir_all(&target).map_err(|e| storage(&target, e))
    }

    /// Deletes what an interrupted sign-out left behind.
    pub fn remove_leftovers(&self) -> Result<(), StoreError> {
        let removed = self.data_dir.join(REMOVED_DIR);
        match std::fs::remove_dir_all(&removed) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(storage(&removed, e)),
        }
    }

    /// Adopts the single database of earlier versions as the guest store, once: its data stays
    /// on this device, its old session and account binding are dropped. Does nothing if there is
    /// none or a guest store exists already.
    pub fn adopt_legacy_db(&self) -> Result<(), StoreError> {
        let legacy = self.data_dir.join(db::FILE_NAME);
        if !legacy.exists() {
            return Ok(());
        }
        let guest_dir = self.guest_dir();
        let guest = guest_dir.join(db::FILE_NAME);
        if !guest.exists() {
            std::fs::create_dir_all(&guest_dir).map_err(|e| storage(&guest_dir, e))?;
            let copy = guest_dir.join(format!("{}.adopting", db::FILE_NAME));
            let _ = std::fs::remove_file(&copy);
            db::copy_as_guest(&legacy, &copy)?;
            std::fs::rename(&copy, &guest).map_err(|e| storage(&guest, e))?;
        }
        for suffix in ["", "-wal", "-shm"] {
            let file = self.data_dir.join(format!("{}{suffix}", db::FILE_NAME));
            match std::fs::remove_file(&file) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(storage(&file, e)),
            }
        }
        Ok(())
    }
}

fn storage(path: &Path, error: impl fmt::Display) -> StoreError {
    StoreError::Storage { path: path.display().to_string(), message: error.to_string() }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
