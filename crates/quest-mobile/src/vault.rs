//! The accounts' tokens in the platform's secure storage (Android: Keystore-encrypted files): the
//! foreign `TokenStore` behind `quest_store::TokenVault`.

use std::sync::Arc;

use quest_store::{AccountKey, StoredTokens, TokenVault, VaultError};

/// An account's tokens as the platform keeps them. Secret: never log them.
#[derive(Clone, PartialEq, Eq, uniffi::Record)]
pub struct SavedTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// When the access token expires, as Unix epoch milliseconds.
    pub access_expires_at_ms: u64,
}

/// The platform's storage failed (e.g. the Keystore key is gone). `detail` never contains a token.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum TokenStoreError {
    #[error("{detail}")]
    Failed { detail: String },
}

impl From<uniffi::UnexpectedUniFFICallbackError> for TokenStoreError {
    fn from(error: uniffi::UnexpectedUniFFICallbackError) -> Self {
        TokenStoreError::Failed { detail: error.reason }
    }
}

/// Secure storage for tokens, one entry per account `key` (32 hex characters). Called from the
/// core's background threads, never from the UI thread; calls may block on disk.
#[uniffi::export(with_foreign)]
pub trait TokenStore: Send + Sync {
    fn load(&self, key: String) -> Result<Option<SavedTokens>, TokenStoreError>;
    fn save(&self, key: String, tokens: SavedTokens) -> Result<(), TokenStoreError>;
    /// Deleting a missing entry is not an error.
    fn delete(&self, key: String) -> Result<(), TokenStoreError>;
}

/// Adapts the foreign store to the store crate's vault.
pub(crate) struct ForeignVault(Arc<dyn TokenStore>);

impl ForeignVault {
    pub(crate) fn new(store: Arc<dyn TokenStore>) -> ForeignVault {
        ForeignVault(store)
    }
}

impl TokenVault for ForeignVault {
    fn load(&self, key: &AccountKey) -> Result<Option<StoredTokens>, VaultError> {
        let saved = self.0.load(key.as_str().to_owned()).map_err(vault_error)?;
        Ok(saved.map(|s| StoredTokens {
            access_token: s.access_token,
            refresh_token: s.refresh_token,
            access_expires_at_ms: s.access_expires_at_ms,
        }))
    }

    fn save(&self, key: &AccountKey, tokens: &StoredTokens) -> Result<(), VaultError> {
        let saved = SavedTokens {
            access_token: tokens.access_token.clone(),
            refresh_token: tokens.refresh_token.clone(),
            access_expires_at_ms: tokens.access_expires_at_ms,
        };
        self.0.save(key.as_str().to_owned(), saved).map_err(vault_error)
    }

    fn delete(&self, key: &AccountKey) -> Result<(), VaultError> {
        self.0.delete(key.as_str().to_owned()).map_err(vault_error)
    }
}

fn vault_error(error: TokenStoreError) -> VaultError {
    VaultError::new(error.to_string())
}

#[cfg(test)]
#[path = "vault_tests.rs"]
mod tests;
