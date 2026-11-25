//! Where an account's OAuth tokens live: never in the database, always behind `TokenVault`. The
//! apps pick the platform's secure store (macOS keychain, Android Keystore); tests use memory.

#[cfg(all(feature = "keychain", target_os = "macos"))]
mod keychain;

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};

#[cfg(all(feature = "keychain", target_os = "macos"))]
pub use keychain::KeychainVault;

use crate::layout::AccountKey;

/// An account's tokens. `Debug` hides them.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// When the access token expires, as Unix epoch milliseconds.
    pub access_expires_at_ms: u64,
}

impl fmt::Debug for StoredTokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredTokens")
            .field("access_token", &"<redacted>")
            .field("refresh_token", &self.refresh_token.as_ref().map(|_| "<redacted>"))
            .field("access_expires_at_ms", &self.access_expires_at_ms)
            .finish()
    }
}

/// The token store could not be used. The message never contains a token.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct VaultError {
    pub message: String,
}

impl VaultError {
    pub fn new(message: impl Into<String>) -> VaultError {
        VaultError { message: message.into() }
    }
}

/// Secure storage for tokens, one entry per account. Calls may block (keychain, file): the store
/// only calls it from its worker and from blocking calls that are documented as not for the UI
/// thread.
pub trait TokenVault: Send + Sync {
    fn load(&self, key: &AccountKey) -> Result<Option<StoredTokens>, VaultError>;
    fn save(&self, key: &AccountKey, tokens: &StoredTokens) -> Result<(), VaultError>;
    /// Deleting a missing entry is not an error.
    fn delete(&self, key: &AccountKey) -> Result<(), VaultError>;
}

/// Keeps tokens in memory only: for tests.
#[derive(Default)]
pub struct MemoryVault {
    entries: Mutex<HashMap<AccountKey, StoredTokens>>,
}

impl TokenVault for MemoryVault {
    fn load(&self, key: &AccountKey) -> Result<Option<StoredTokens>, VaultError> {
        Ok(self.entries.lock().unwrap_or_else(PoisonError::into_inner).get(key).cloned())
    }

    fn save(&self, key: &AccountKey, tokens: &StoredTokens) -> Result<(), VaultError> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner).insert(key.clone(), tokens.clone());
        Ok(())
    }

    fn delete(&self, key: &AccountKey) -> Result<(), VaultError> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner).remove(key);
        Ok(())
    }
}

/// Keeps tokens in a plain JSON file readable only by the user (0600). For development and the
/// end-to-end tests only: the tokens are not encrypted.
pub struct FileVault {
    path: PathBuf,
    /// Serialises read-modify-write cycles within this process.
    lock: Mutex<()>,
}

impl FileVault {
    pub fn new(path: impl Into<PathBuf>) -> FileVault {
        FileVault { path: path.into(), lock: Mutex::new(()) }
    }

    fn read(&self) -> Result<HashMap<AccountKey, StoredTokens>, VaultError> {
        let raw = match std::fs::read(&self.path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
            Err(e) => return Err(self.error("read", e)),
        };
        serde_json::from_slice(&raw).map_err(|e| self.error("parse", e))
    }

    fn write(&self, entries: &HashMap<AccountKey, StoredTokens>) -> Result<(), VaultError> {
        let json = serde_json::to_vec(entries).map_err(|e| self.error("encode", e))?;
        write_private(&self.path, &json).map_err(|e| self.error("write", e))
    }

    fn update(&self, change: impl FnOnce(&mut HashMap<AccountKey, StoredTokens>)) -> Result<(), VaultError> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut entries = self.read()?;
        change(&mut entries);
        self.write(&entries)
    }

    fn error(&self, action: &str, error: impl fmt::Display) -> VaultError {
        VaultError::new(format!("could not {action} the token file {}: {error}", self.path.display()))
    }
}

impl TokenVault for FileVault {
    fn load(&self, key: &AccountKey) -> Result<Option<StoredTokens>, VaultError> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(self.read()?.remove(key))
    }

    fn save(&self, key: &AccountKey, tokens: &StoredTokens) -> Result<(), VaultError> {
        self.update(|entries| {
            entries.insert(key.clone(), tokens.clone());
        })
    }

    fn delete(&self, key: &AccountKey) -> Result<(), VaultError> {
        self.update(|entries| {
            entries.remove(key);
        })
    }
}

/// Writes `contents` atomically (temporary file, then rename), readable only by the user.
fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&temporary)?;
    file.write_all(contents)?;
    file.sync_all()?;
    std::fs::rename(&temporary, path)
}

#[cfg(test)]
#[path = "vault_tests.rs"]
mod tests;
