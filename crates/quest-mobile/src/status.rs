//! Store status, translated for the UI. Pure.

use quest_store::{StoreError, SyncStatus};

use crate::types::SyncState;

pub(crate) fn sync_state(status: &SyncStatus) -> SyncState {
    match status {
        SyncStatus::SignedOut => SyncState::SignedOut,
        SyncStatus::Connecting => SyncState::Connecting,
        SyncStatus::Live { pending } => SyncState::Live { pending: *pending as u64 },
        SyncStatus::Offline { pending, retry_in } => {
            SyncState::Offline { pending: *pending as u64, retry_in_secs: retry_in.as_secs() }
        }
        SyncStatus::Failed { error: StoreError::SessionExpired } => SyncState::SessionExpired,
        SyncStatus::Failed { error } => SyncState::Failed { message: error.to_string() },
    }
}
