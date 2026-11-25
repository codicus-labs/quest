//! The macOS keychain as token store: one generic password per account, holding the tokens as JSON.

use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};

use super::{StoredTokens, TokenVault, VaultError};
use crate::layout::AccountKey;

/// `errSecItemNotFound`.
const ITEM_NOT_FOUND: i32 = -25300;

/// Tokens in the login keychain under `service`, one item per account key.
pub struct KeychainVault {
    service: String,
}

impl KeychainVault {
    /// `service` names the keychain items, e.g. the app's bundle id.
    pub fn new(service: impl Into<String>) -> KeychainVault {
        KeychainVault { service: service.into() }
    }
}

impl TokenVault for KeychainVault {
    fn load(&self, key: &AccountKey) -> Result<Option<StoredTokens>, VaultError> {
        let raw = match get_generic_password(&self.service, key.as_str()) {
            Ok(raw) => raw,
            Err(e) if e.code() == ITEM_NOT_FOUND => return Ok(None),
            Err(e) => return Err(VaultError::new(format!("could not read from the keychain: {e}"))),
        };
        serde_json::from_slice(&raw)
            .map(Some)
            .map_err(|e| VaultError::new(format!("the keychain entry is unreadable: {e}")))
    }

    fn save(&self, key: &AccountKey, tokens: &StoredTokens) -> Result<(), VaultError> {
        let json = serde_json::to_vec(tokens).map_err(|e| VaultError::new(e.to_string()))?;
        set_generic_password(&self.service, key.as_str(), &json)
            .map_err(|e| VaultError::new(format!("could not write to the keychain: {e}")))
    }

    fn delete(&self, key: &AccountKey) -> Result<(), VaultError> {
        match delete_generic_password(&self.service, key.as_str()) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == ITEM_NOT_FOUND => Ok(()),
            Err(e) => Err(VaultError::new(format!("could not delete from the keychain: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Touches the user's real login keychain (and may ask for permission): run by hand with
    /// `cargo test -p quest-store --features keychain -- --ignored keychain`.
    #[test]
    #[ignore]
    fn keychain_round_trip() {
        let vault = KeychainVault::new("app.quest.store-test");
        let key = AccountKey::new(&"http://127.0.0.1:1".parse().unwrap(), "issuer", "keychain-test");
        let tokens =
            StoredTokens { access_token: "a".into(), refresh_token: Some("r".into()), access_expires_at_ms: 1 };
        vault.save(&key, &tokens).unwrap();
        assert_eq!(vault.load(&key).unwrap(), Some(tokens));
        vault.delete(&key).unwrap();
        assert_eq!(vault.load(&key).unwrap(), None);
        vault.delete(&key).unwrap();
    }
}
