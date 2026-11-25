//! Account settings over HTTP with the store's session: the adapter for `PUT /v1/account/settings`.
//! Translates transport and server errors into `AccountError`s at this boundary.

use std::time::Duration;

use quest_protocol::{ACCOUNT_SETTINGS_PATH, AccountInfo, AccountSettingsUpdate, ApiError};

use crate::Store;
use crate::remote::http_client;
use crate::session::RequestFailure;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Why changing an account setting failed. Messages say what to do.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AccountError {
    #[error("this needs an account. Sign in first.")]
    SignedOut,
    #[error("your sign-in has expired. Sign in again.")]
    SessionExpired,
    #[error("could not reach your Quest server: {message}. Check your connection and try again.")]
    Unreachable { message: String },
    /// The server refused the request, or it could not be sent (e.g. the token store failed).
    #[error("the request failed: {message}")]
    Server { message: String },
}

impl Store {
    /// Turns the account's AI features on or off on the server (applies to all devices) and
    /// returns the updated account, which `account()` shows from then on. Blocks on the network:
    /// not for the UI thread.
    pub fn set_ai_enabled(&self, enabled: bool) -> Result<AccountInfo, AccountError> {
        let session = self.shared.session().ok_or(AccountError::SignedOut)?;
        let server_url = session.server_url.clone();
        let endpoint = server_url.join(ACCOUNT_SETTINGS_PATH).map_err(server)?;
        let client = http_client(&server_url, REQUEST_TIMEOUT).map_err(server)?;
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(server)?;
        let update = AccountSettingsUpdate { ai_enabled: enabled };
        let info = runtime.block_on(async {
            let response = session
                .send_authorized(|token| client.put(endpoint.clone()).bearer_auth(token).json(&update).send())
                .await
                .map_err(request_failure)?;
            let status = response.status();
            if status.is_success() {
                return response.json::<AccountInfo>().await.map_err(server);
            }
            match response.json::<ApiError>().await {
                Ok(error) => Err(AccountError::Server { message: error.message }),
                Err(_) => Err(AccountError::Server { message: format!("unexpected HTTP status {status}") }),
            }
        })?;
        self.update_account_info(info.clone());
        Ok(info)
    }
}

fn server(error: impl std::fmt::Display) -> AccountError {
    AccountError::Server { message: error.to_string() }
}

fn request_failure(failure: RequestFailure) -> AccountError {
    match failure {
        RequestFailure::SessionExpired => AccountError::SessionExpired,
        RequestFailure::Unreachable(message) => AccountError::Unreachable { message },
        RequestFailure::Local(message) => AccountError::Server { message },
    }
}
