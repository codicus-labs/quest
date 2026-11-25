//! The server's OAuth endpoints for the apps: code exchange, refresh and revocation (RFC 6749,
//! 7009, 8707), and finishing a sign-in. Translates transport and OAuth errors at this boundary.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use quest_protocol::{
    ACCOUNT_PATH, AccountInfo, ApiError, OAuthErrorBody, REVOKE_PATH, TOKEN_PATH, TokenResponse, app_resource,
};
use url::Url;

use crate::error::StoreError;
use crate::layout::origin;
use crate::login::LoginRequest;
use crate::profiles::SignedIn;
use crate::remote::{http_client, invalid_url, redirected, unreachable};
use crate::vault::StoredTokens;

const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
/// Signing out must not hang on an unreachable server.
const REVOKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Why a token request failed.
#[derive(Debug)]
pub(crate) enum GrantFailure {
    /// The code or refresh token is no longer valid: sign in again.
    InvalidGrant,
    /// Network trouble or another server answer; retrying later may help.
    Failed(StoreError),
}

impl LoginRequest {
    /// Finishes the sign-in with the redirect the browser came back with: checks it, exchanges the
    /// code for tokens and fetches the account. Blocks on the network: never call it on the UI
    /// thread. Hand the result to `Profiles::sign_in`.
    pub fn finish(&self, callback_url: &Url) -> Result<SignedIn, StoreError> {
        let code = self.authorization_code(callback_url)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| unreachable(self.server_url(), e))?;
        runtime.block_on(self.exchange(&code))
    }

    async fn exchange(&self, code: &str) -> Result<SignedIn, StoreError> {
        let client = self.client();
        let redirect_uri = client.redirect_uri(self.server_url());
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &redirect_uri),
            ("code_verifier", self.code_verifier()),
            ("client_id", client.client_id()),
        ];
        let response = token_request(self.server_url(), &form).await.map_err(|failure| match failure {
            GrantFailure::InvalidGrant => StoreError::SignInFailed {
                reason: "the server did not accept the sign-in (it may have taken too long)".into(),
            },
            GrantFailure::Failed(error) => error,
        })?;
        let tokens = stored_tokens(response, None, now_ms())?;
        let account = fetch_account(self.server_url(), &tokens.access_token).await?;
        Ok(SignedIn {
            server_url: self.server_url().clone(),
            client_id: client.client_id().to_owned(),
            tokens,
            account,
        })
    }
}

/// Swaps the refresh token for a new pair (the server rotates it).
pub(crate) async fn refresh(
    server_url: &Url,
    client_id: &str,
    refresh_token: &str,
) -> Result<StoredTokens, GrantFailure> {
    let form = [("grant_type", "refresh_token"), ("refresh_token", refresh_token), ("client_id", client_id)];
    let response = token_request(server_url, &form).await?;
    stored_tokens(response, Some(refresh_token), now_ms()).map_err(GrantFailure::Failed)
}

/// Revokes the refresh token (RFC 7009), which ends the grant on the server.
pub(crate) async fn revoke(server_url: &Url, client_id: &str, refresh_token: &str) -> Result<(), StoreError> {
    let endpoint = server_url.join(REVOKE_PATH).map_err(|e| invalid_url(server_url, e))?;
    let form = [("token", refresh_token), ("token_type_hint", "refresh_token"), ("client_id", client_id)];
    let response = http_client(server_url, REVOKE_TIMEOUT)?
        .post(endpoint)
        .form(&form)
        .send()
        .await
        .map_err(|e| unreachable(server_url, e))?;
    if response.status().is_success() {
        return Ok(());
    }
    Err(StoreError::Server { message: format!("revoking the sign-in failed with HTTP {}", response.status()) })
}

/// `GET /v1/account` with a token known to be fresh.
pub(crate) async fn fetch_account(server_url: &Url, access_token: &str) -> Result<AccountInfo, StoreError> {
    let endpoint = server_url.join(ACCOUNT_PATH).map_err(|e| invalid_url(server_url, e))?;
    let response = http_client(server_url, HTTP_TIMEOUT)?
        .get(endpoint)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| unreachable(server_url, e))?;
    let status = response.status();
    if status.is_success() {
        return response.json().await.map_err(|e| StoreError::Server { message: format!("unreadable account: {e}") });
    }
    if status.is_redirection() {
        return Err(redirected(server_url, &response));
    }
    match response.json::<ApiError>().await {
        Ok(error) => Err(StoreError::Server { message: error.message }),
        Err(_) => Err(StoreError::Server { message: format!("unexpected HTTP status {status}") }),
    }
}

async fn token_request(server_url: &Url, form: &[(&str, &str)]) -> Result<TokenResponse, GrantFailure> {
    let failed = GrantFailure::Failed;
    let endpoint = server_url.join(TOKEN_PATH).map_err(|e| failed(invalid_url(server_url, e)))?;
    let resource = app_resource(&origin(server_url));
    let mut body: Vec<(&str, &str)> = form.to_vec();
    body.push(("resource", &resource));
    let response = http_client(server_url, HTTP_TIMEOUT)
        .map_err(failed)?
        .post(endpoint)
        .form(&body)
        .send()
        .await
        .map_err(|e| failed(unreachable(server_url, e)))?;
    let status = response.status();
    if status.is_success() {
        return response
            .json()
            .await
            .map_err(|e| failed(StoreError::Server { message: format!("unreadable token response: {e}") }));
    }
    if status.is_redirection() {
        return Err(failed(redirected(server_url, &response)));
    }
    match response.json::<OAuthErrorBody>().await {
        Ok(error) if error.error == "invalid_grant" => Err(GrantFailure::InvalidGrant),
        Ok(error) => {
            let description = error.error_description.map(|d| format!(": {d}")).unwrap_or_default();
            Err(failed(StoreError::Server { message: format!("{}{description}", error.error) }))
        }
        Err(_) => Err(failed(StoreError::Server { message: format!("unexpected HTTP status {status}") })),
    }
}

/// What to keep of a token response. Without a new refresh token the previous one stays valid.
pub(crate) fn stored_tokens(
    response: TokenResponse,
    previous_refresh: Option<&str>,
    now_ms: u64,
) -> Result<StoredTokens, StoreError> {
    if !response.token_type.eq_ignore_ascii_case("bearer") {
        return Err(StoreError::Server { message: format!("unsupported token type {:?}", response.token_type) });
    }
    Ok(StoredTokens {
        access_token: response.access_token,
        refresh_token: response.refresh_token.or_else(|| previous_refresh.map(str::to_owned)),
        access_expires_at_ms: now_ms.saturating_add(response.expires_in.saturating_mul(1000)),
    })
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(token_type: &str, refresh: Option<&str>) -> TokenResponse {
        TokenResponse {
            access_token: "a".into(),
            token_type: token_type.into(),
            expires_in: 3600,
            refresh_token: refresh.map(str::to_owned),
            scope: "quest:write offline_access".into(),
        }
    }

    #[test]
    fn tokens_expire_after_the_granted_lifetime() {
        let tokens = stored_tokens(response("Bearer", Some("r2")), Some("r1"), 1_000).unwrap();
        assert_eq!(tokens.access_expires_at_ms, 3_601_000);
        assert_eq!(tokens.refresh_token.as_deref(), Some("r2"));
    }

    #[test]
    fn without_a_new_refresh_token_the_old_one_stays() {
        let tokens = stored_tokens(response("bearer", None), Some("r1"), 0).unwrap();
        assert_eq!(tokens.refresh_token.as_deref(), Some("r1"));
    }

    #[test]
    fn other_token_types_are_refused() {
        assert!(stored_tokens(response("DPoP", None), None, 0).is_err());
    }
}
