//! The account for the UI, pure: what Account and Settings show, the redirect the browser comes back
//! with, the warning before signing out, and the store's sign-in errors as `MobileError`s.

use quest_store::{AccountError, StoreError, Url};

use crate::error::MobileError;
use crate::types::Account;

pub(crate) fn account(account: &quest_store::Account) -> Account {
    let info = &account.info;
    let display_name = info
        .email
        .clone()
        .or_else(|| info.name.clone())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Signed in".to_owned());
    Account {
        display_name,
        email: info.email.clone(),
        name: info.name.clone(),
        server_url: account.server_url.as_str().trim_end_matches('/').to_owned(),
        ai_enabled: info.ai_enabled,
        ai_available: info.ai_available,
        ai_features: info.ai_enabled && info.ai_available,
    }
}

/// Whether the browser came back with the answer to a sign-in (a code or an error), rather than,
/// say, from signing out (which returns to the same address without either).
pub(crate) fn is_sign_in_answer(redirect: &Url) -> bool {
    redirect.query_pairs().any(|(key, _)| key == "code" || key == "error")
}

/// Whether `redirect` answers the sign-in that opened `authorize_url`: the same `state`. The store
/// checks it again (with `iss` and PKCE); this only keeps a stray answer from using up the sign-in.
pub(crate) fn answers(authorize_url: &Url, redirect: &Url) -> bool {
    let state = |url: &Url| url.query_pairs().find(|(key, _)| key == "state").map(|(_, value)| value.into_owned());
    state(authorize_url).is_some_and(|expected| state(redirect) == Some(expected))
}

/// What to tell the user before signing out, while local changes have not reached the server.
pub(crate) fn sign_out_warning(pending: usize) -> Option<String> {
    match pending {
        0 => None,
        1 => Some(
            "1 change has not reached the server yet. Signing out deletes it from this device. Connect to the \
             internet and wait until Quest is live to keep it."
                .to_owned(),
        ),
        n => Some(format!(
            "{n} changes have not reached the server yet. Signing out deletes them from this device. Connect to \
             the internet and wait until Quest is live to keep them."
        )),
    }
}

pub(crate) fn sign_in_error(error: StoreError) -> MobileError {
    match error {
        StoreError::SignInCancelled => MobileError::SignInCancelled,
        StoreError::Storage { .. } | StoreError::TokenStore { .. } | StoreError::WorkerStopped => {
            MobileError::Storage { detail: error.to_string() }
        }
        other => MobileError::SignInFailed { reason: other.to_string() },
    }
}

pub(crate) fn account_error(error: AccountError) -> MobileError {
    match error {
        AccountError::SignedOut => MobileError::NotSignedIn,
        AccountError::SessionExpired => MobileError::SessionExpired,
        AccountError::Unreachable { message } => MobileError::ServerUnreachable { detail: message },
        AccountError::Server { message } => MobileError::Rejected { reason: message },
    }
}

#[cfg(test)]
#[path = "account_tests.rs"]
mod tests;
