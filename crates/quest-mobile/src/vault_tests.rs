use quest_store::Url;

use super::*;
use crate::test_support::MemoryTokens;

#[test]
fn tokens_round_trip_through_the_platform_store() {
    let vault = ForeignVault::new(Arc::new(MemoryTokens::default()));
    let server: Url = "https://quest.example.com".parse().unwrap();
    let key = AccountKey::new(&server, "https://id.example.com", "abc");
    let tokens = StoredTokens { access_token: "a".into(), refresh_token: Some("r".into()), access_expires_at_ms: 42 };
    assert_eq!(vault.load(&key).unwrap(), None);
    vault.save(&key, &tokens).unwrap();
    assert_eq!(vault.load(&key).unwrap(), Some(tokens));
    vault.delete(&key).unwrap();
    vault.delete(&key).unwrap();
    assert_eq!(vault.load(&key).unwrap(), None);
}

struct Broken;

impl TokenStore for Broken {
    fn load(&self, _: String) -> Result<Option<SavedTokens>, TokenStoreError> {
        Err(TokenStoreError::Failed { detail: "the Keystore key is gone".into() })
    }

    fn save(&self, _: String, _: SavedTokens) -> Result<(), TokenStoreError> {
        Err(TokenStoreError::Failed { detail: "disk full".into() })
    }

    fn delete(&self, _: String) -> Result<(), TokenStoreError> {
        Ok(())
    }
}

#[test]
fn platform_failures_reach_the_store_with_their_detail() {
    let vault = ForeignVault::new(Arc::new(Broken));
    let key = AccountKey::new(&"https://quest.example.com".parse().unwrap(), "i", "s");
    assert_eq!(vault.load(&key).unwrap_err().message, "the Keystore key is gone");
}
