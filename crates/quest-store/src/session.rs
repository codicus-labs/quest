//! An account's session: hands out a valid access token for the sync socket and REST calls,
//! refreshing it when it expires or the server rejects it. Refreshes are serialised: the server
//! rotates refresh tokens and treats a reused one as theft, so two concurrent refreshes with the
//! same token would end the session.
//!
//! Serialised per `AccountSession` only: a process must keep one `Profiles` (and so one store) per
//! account. Another process with the same tokens (e.g. a second app instance) can still race; after
//! a refused refresh the vault is read again in case it rotated meanwhile.

use std::future::Future;
use std::sync::Arc;

use reqwest::StatusCode;
use url::Url;

use crate::error::StoreError;
use crate::layout::AccountKey;
use crate::oauth::{self, GrantFailure};
use crate::remote::with_causes;
use crate::vault::{StoredTokens, TokenVault};

/// Refresh a little before the access token expires, so it does not lapse in flight.
const REFRESH_MARGIN_MS: u64 = 60_000;

/// Why no access token is available.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TokenFailure {
    /// No tokens, or the refresh token was refused: the user must sign in again.
    Expired,
    /// The refresh could not reach the server or failed there; retry later.
    Unreachable(String),
    /// The token store failed.
    Vault(String),
}

impl TokenFailure {
    pub fn into_store_error(self, server_url: &Url) -> StoreError {
        match self {
            TokenFailure::Expired => StoreError::SessionExpired,
            TokenFailure::Unreachable(message) => {
                StoreError::Unreachable { server_url: server_url.to_string(), message }
            }
            TokenFailure::Vault(message) => StoreError::TokenStore { message },
        }
    }
}

pub(crate) struct AccountSession {
    /// The server's origin.
    pub server_url: Url,
    pub client_id: String,
    pub key: AccountKey,
    vault: Arc<dyn TokenVault>,
    refreshing: tokio::sync::Mutex<()>,
    /// Set on signing out; held while saving, so no refresh still in flight stores tokens after it.
    closed: std::sync::Mutex<bool>,
}

impl AccountSession {
    pub fn new(server_url: Url, client_id: String, key: AccountKey, vault: Arc<dyn TokenVault>) -> AccountSession {
        AccountSession {
            server_url,
            client_id,
            key,
            vault,
            refreshing: tokio::sync::Mutex::new(()),
            closed: std::sync::Mutex::new(false),
        }
    }

    pub fn tokens(&self) -> Result<Option<StoredTokens>, TokenFailure> {
        if self.is_closed() {
            return Ok(None);
        }
        self.vault.load(&self.key).map_err(|e| TokenFailure::Vault(e.message))
    }

    /// A token that is not about to expire; refreshes first if needed.
    pub async fn access_token(&self) -> Result<String, TokenFailure> {
        let tokens = self.tokens()?.ok_or(TokenFailure::Expired)?;
        if !needs_refresh(&tokens, oauth::now_ms()) {
            return Ok(tokens.access_token);
        }
        self.refresh(&tokens.access_token).await
    }

    /// A new token after the server rejected `rejected`, unless another call has refreshed
    /// meanwhile. The new pair is stored before it is used.
    pub async fn refresh(&self, rejected: &str) -> Result<String, TokenFailure> {
        let _refreshing = self.refreshing.lock().await;
        let tokens = self.tokens()?.ok_or(TokenFailure::Expired)?;
        if tokens.access_token != rejected && !needs_refresh(&tokens, oauth::now_ms()) {
            return Ok(tokens.access_token);
        }
        let refresh_token = tokens.refresh_token.ok_or(TokenFailure::Expired)?;
        let renewed = match oauth::refresh(&self.server_url, &self.client_id, &refresh_token).await {
            Ok(renewed) => renewed,
            Err(GrantFailure::InvalidGrant) => return self.rotated_elsewhere(&refresh_token),
            Err(GrantFailure::Failed(error)) => return Err(TokenFailure::Unreachable(error.to_string())),
        };
        self.save_unless_closed(&renewed)?;
        Ok(renewed.access_token)
    }

    /// After `used` was refused: the vault's pair if it changed meanwhile (another process refreshed,
    /// or the user signed in again), else the session has expired.
    fn rotated_elsewhere(&self, used: &str) -> Result<String, TokenFailure> {
        match self.tokens()? {
            Some(current) if current.refresh_token.as_deref() != Some(used) => Ok(current.access_token),
            _ => Err(TokenFailure::Expired),
        }
    }

    fn save_unless_closed(&self, tokens: &StoredTokens) -> Result<(), TokenFailure> {
        let closed = self.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if *closed {
            return Err(TokenFailure::Expired);
        }
        self.vault.save(&self.key, tokens).map_err(|e| TokenFailure::Vault(e.message))
    }

    /// Signing out: from now on the session hands out no tokens and stores none. Waits for a
    /// save in flight, so deleting the tokens afterwards is final.
    pub fn close(&self) {
        *self.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = true;
    }

    fn is_closed(&self) -> bool {
        *self.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Sends a request with the access token; on 401 refreshes once and sends it again.
    pub async fn send_authorized<F, Fut>(&self, send: F) -> Result<reqwest::Response, RequestFailure>
    where
        F: Fn(String) -> Fut,
        Fut: Future<Output = reqwest::Result<reqwest::Response>>,
    {
        let token = self.access_token().await?;
        let response = send(token.clone()).await.map_err(|e| RequestFailure::Unreachable(with_causes(&e)))?;
        if response.status() != StatusCode::UNAUTHORIZED {
            return Ok(response);
        }
        let token = self.refresh(&token).await?;
        let response = send(token).await.map_err(|e| RequestFailure::Unreachable(with_causes(&e)))?;
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(RequestFailure::SessionExpired);
        }
        Ok(response)
    }

    pub fn delete_tokens(&self) -> Result<(), StoreError> {
        self.vault.delete(&self.key).map_err(|e| StoreError::TokenStore { message: e.message })
    }
}

/// Why an authorised REST call failed before the server answered it.
#[derive(Debug)]
pub(crate) enum RequestFailure {
    SessionExpired,
    Unreachable(String),
    /// Not the network's fault (token store, invalid URL).
    Local(String),
}

impl From<TokenFailure> for RequestFailure {
    fn from(failure: TokenFailure) -> RequestFailure {
        match failure {
            TokenFailure::Expired => RequestFailure::SessionExpired,
            TokenFailure::Unreachable(message) => RequestFailure::Unreachable(message),
            TokenFailure::Vault(message) => RequestFailure::Local(message),
        }
    }
}

/// Whether the access token is expired or about to.
pub(crate) fn needs_refresh(tokens: &StoredTokens, now_ms: u64) -> bool {
    now_ms.saturating_add(REFRESH_MARGIN_MS) >= tokens.access_expires_at_ms
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expiring_at(ms: u64) -> StoredTokens {
        StoredTokens { access_token: "a".into(), refresh_token: None, access_expires_at_ms: ms }
    }

    #[test]
    fn a_token_is_refreshed_a_minute_before_it_expires() {
        assert!(!needs_refresh(&expiring_at(100_000), 39_999));
        assert!(needs_refresh(&expiring_at(100_000), 40_000));
        assert!(needs_refresh(&expiring_at(100_000), 200_000));
    }

    #[tokio::test]
    async fn without_tokens_the_session_has_expired() {
        let vault: Arc<dyn TokenVault> = Arc::new(crate::vault::MemoryVault::default());
        let server_url: Url = "http://127.0.0.1:1".parse().unwrap();
        let key = AccountKey::new(&server_url, "i", "s");
        let session = AccountSession::new(server_url, "quest-desktop".into(), key.clone(), vault.clone());
        assert_eq!(session.access_token().await, Err(TokenFailure::Expired));
        // An expired access token without a refresh token cannot be renewed either.
        vault.save(&key, &expiring_at(0)).unwrap();
        assert_eq!(session.access_token().await, Err(TokenFailure::Expired));
    }

    fn pair(access: &str, refresh: &str) -> StoredTokens {
        StoredTokens { access_token: access.into(), refresh_token: Some(refresh.into()), access_expires_at_ms: 0 }
    }

    /// A session holding `stored` against a token endpoint that refuses every refresh; while the
    /// request is in flight, `meanwhile` stores a pair in the vault (as another process would).
    async fn refused_refresh(stored: StoredTokens, meanwhile: Option<StoredTokens>) -> Result<String, TokenFailure> {
        use tokio::io::AsyncWriteExt;
        let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_url: Url = format!("http://{}/", server.local_addr().unwrap()).parse().unwrap();
        let key = AccountKey::new(&server_url, "i", "s");
        let vault: Arc<dyn TokenVault> = Arc::new(crate::vault::MemoryVault::default());
        vault.save(&key, &stored).unwrap();
        let (other_process, other_key) = (vault.clone(), key.clone());
        let refusing = tokio::spawn(async move {
            let (mut stream, _) = server.accept().await.unwrap();
            crate::remote::tests::read_request(&mut stream).await;
            if let Some(rotated) = meanwhile {
                other_process.save(&other_key, &rotated).unwrap();
            }
            let body = r#"{"error":"invalid_grant"}"#;
            let response = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.shutdown().await.unwrap();
        });
        let session = AccountSession::new(server_url, "quest-desktop".into(), key, vault);
        let result = session.access_token().await;
        refusing.await.unwrap();
        result
    }

    #[tokio::test]
    async fn a_refused_refresh_takes_the_pair_another_process_stored_meanwhile() {
        let result = refused_refresh(pair("old", "r1"), Some(pair("new", "r2"))).await;
        assert_eq!(result, Ok("new".into()));
    }

    #[tokio::test]
    async fn a_refused_refresh_without_a_new_pair_expires_the_session() {
        assert_eq!(refused_refresh(pair("old", "r1"), None).await, Err(TokenFailure::Expired));
    }

    #[tokio::test]
    async fn a_closed_session_hands_out_and_stores_no_tokens() {
        let vault: Arc<dyn TokenVault> = Arc::new(crate::vault::MemoryVault::default());
        let server_url: Url = "http://127.0.0.1:1".parse().unwrap();
        let key = AccountKey::new(&server_url, "i", "s");
        let valid = StoredTokens { access_expires_at_ms: u64::MAX, ..pair("a", "r") };
        vault.save(&key, &valid).unwrap();
        let session = AccountSession::new(server_url, "quest-desktop".into(), key.clone(), vault.clone());
        assert_eq!(session.access_token().await, Ok("a".into()));

        session.close();

        assert_eq!(session.access_token().await, Err(TokenFailure::Expired));
        assert_eq!(session.save_unless_closed(&pair("late", "r2")), Err(TokenFailure::Expired));
        assert_eq!(vault.load(&key).unwrap(), Some(valid));
    }
}
