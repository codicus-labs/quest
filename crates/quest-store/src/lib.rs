//! Quest's client-side store: keeps every field in memory for instant, IO-free reads and
//! dispatches, persists to SQLite and syncs with the server on a background worker thread.
//!
//! Threads: the caller's thread only touches memory. The worker thread owns the database and
//! the sync connection; it calls `on_change` whenever the snapshot or the status changes.
//!
//! Profiles: each account has its own store, and there is a guest store for using the app
//! without one. `Profiles` opens the active one and switches on signing in and out; signing in
//! goes through `LoginRequest`.

mod account;
mod ai;
mod batch;
mod conn_state;
mod connections;
mod db;
mod error;
mod guest_import;
mod idle;
mod layout;
mod login;
mod oauth;
mod profiles;
mod remote;
mod session;
mod shared;
mod vault;
mod worker;

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc as std_mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use quest_domain::{Command, CommandError, EntityId, EntityKind, FieldChange, Snapshot};
use tokio::sync::mpsc;

pub use account::AccountError;
pub use ai::{AiClient, CaptureError};
pub use connections::{ConnectionsClient, ConnectionsError};
pub use error::StoreError;
pub use idle::SyncOutcome;
pub use layout::AccountKey;
pub use login::{AppClient, LoginRequest};
pub use profiles::{Profiles, ProfilesConfig, SignInOutcome, SignedIn};
pub use quest_protocol::{AccountInfo, AiConnection};
pub use url::Url;
#[cfg(all(feature = "keychain", target_os = "macos"))]
pub use vault::KeychainVault;
pub use vault::{FileVault, MemoryVault, StoredTokens, TokenVault, VaultError};

use crate::conn_state::{ConnEvent, ConnState, step};
use crate::session::AccountSession;
use crate::shared::{Shared, SystemClock};
use crate::worker::{Request, Timing, Worker};

/// How long dropping a store waits for the worker to finish saving.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// How long signing out waits for the worker to stop syncing and save.
const CLOSE_GRACE: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncStatus {
    /// Guest, or signed out: the data is local only.
    SignedOut,
    Connecting,
    Live {
        pending: usize,
    },
    Offline {
        pending: usize,
        retry_in: Duration,
    },
    Failed {
        error: StoreError,
    },
}

/// The account a store belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    /// The server's origin.
    pub server_url: Url,
    /// As last reported by the server.
    pub info: AccountInfo,
}

/// Send + Sync: share it (e.g. in an `Arc`) between the UI thread and background threads.
pub struct Store {
    shared: Arc<Shared>,
    requests: mpsc::UnboundedSender<Request>,
    worker: Option<JoinHandle<()>>,
    /// In a mutex only so `Store` is `Sync`: just `Drop` reads it.
    worker_done: std::sync::Mutex<std_mpsc::Receiver<()>>,
}

impl Store {
    /// Opens or creates the database in `dir`, loads it into memory and starts the background
    /// worker, which syncs if there is a session. `on_change` runs on the worker thread.
    pub(crate) fn open_with(
        dir: &Path,
        session: Option<Arc<AccountSession>>,
        timing: Timing,
        on_change: Box<dyn Fn() + Send + Sync>,
    ) -> Result<Store, StoreError> {
        let mut db = db::Db::open(dir)?;
        let loaded = db.load()?;
        let initial = if session.is_some() {
            step(&ConnState::SignedOut, ConnEvent::SessionRestored, 0.0)
        } else {
            ConnState::SignedOut
        };
        let shared = Arc::new(Shared::new(&loaded, session, Box::new(SystemClock), on_change));
        // Before the worker starts, so a restored session never reads as signed out.
        shared.set_connection(&initial);
        let (requests, inbox) = mpsc::unbounded_channel();
        let (done, worker_done) = std_mpsc::channel();
        let worker = Worker::new(db, loaded, shared.clone(), (requests.clone(), inbox), timing);
        let handle = worker.spawn(initial, done).map_err(|e| StoreError::Storage {
            path: dir.display().to_string(),
            message: format!("could not start the background worker: {e}"),
        })?;
        Ok(Store { shared, requests, worker: Some(handle), worker_done: std::sync::Mutex::new(worker_done) })
    }

    /// The current read model. Cheap; never touches the disk.
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.shared.snapshot()
    }

    /// A read model of another workspace than the shown one (the running timer's, say). Built
    /// from memory on the first call after a change. None unless the workspace is live.
    pub fn snapshot_in(&self, workspace: EntityId) -> Option<Arc<Snapshot>> {
        self.shared.snapshot_in(workspace)
    }

    /// Validates the command and applies it in memory right away; persisting and syncing
    /// happen on the worker.
    pub fn dispatch(&self, command: Command) -> Result<(), CommandError> {
        let changes = self.shared.apply_local(&command)?;
        if changes.is_empty() {
            return Ok(());
        }
        self.queue(changes);
        Ok(())
    }

    /// Like `dispatch`, validated against `workspace` instead of the shown one (the running
    /// timer's task in another workspace, say). Refused unless the workspace is live.
    pub fn dispatch_in(&self, workspace: EntityId, command: Command) -> Result<(), CommandError> {
        let changes = self.shared.apply_local_in(workspace, &command)?;
        if changes.is_empty() {
            return Ok(());
        }
        self.queue(changes);
        Ok(())
    }

    /// Like `dispatch` for several commands, as one unit: each is validated against the state the
    /// ones before it leave, and if one is refused none is applied.
    pub fn dispatch_all(&self, commands: &[Command]) -> Result<(), CommandError> {
        let changes = self.shared.apply_local_all(commands)?;
        if changes.is_empty() {
            return Ok(());
        }
        self.queue(changes);
        Ok(())
    }

    /// Like `dispatch_all`, planned against a specific live workspace.
    pub fn dispatch_all_in(&self, workspace: EntityId, commands: &[Command]) -> Result<(), CommandError> {
        let changes = self.shared.apply_local_batch(Some(workspace), commands)?;
        if !changes.is_empty() {
            self.queue(changes);
        }
        Ok(())
    }

    /// Shows `workspace` from now on and remembers it for the next start of this profile on
    /// this device. Refused unless the workspace exists and is not deleted. Callers drop what
    /// belongs to the previous workspace (open dialogs, selection, undo).
    pub fn switch_workspace(&self, workspace: EntityId) -> Result<(), CommandError> {
        self.shared.switch_workspace(workspace)?;
        self.shared.take_fallback();
        self.send(Request::RememberWorkspace(workspace));
        Ok(())
    }

    pub fn status(&self) -> SyncStatus {
        self.shared.status()
    }

    /// Retries a lost connection now instead of after the backoff, e.g. when the device's network
    /// came back. Does nothing unless the store is waiting to retry.
    pub fn reconnect_now(&self) {
        self.shared.reconnect.notify_waiters();
    }

    /// A handle for AI capture requests with this store's session. Its calls block: not for the UI
    /// thread. Check `account().info.ai_enabled` before offering AI features.
    pub fn ai(&self) -> AiClient {
        AiClient::new(self.shared.clone())
    }

    /// Blocks until sync is idle (live, caught up, outbox empty), cannot make progress
    /// (`Offline`: signed out, failed, or offline after a failed retry; a single dropped
    /// connection is ridden out) or `timeout` runs out. Wakes on status changes, never polls. Not for the UI thread.
    pub fn sync_until_idle(&self, timeout: Duration) -> SyncOutcome {
        self.waiter().sync_until_idle(timeout)
    }

    /// Blocks until every local change is saved to disk (true), or `timeout` runs out or saving
    /// fails (false; the changes stay in memory and are retried). Not for the UI thread.
    pub fn flush(&self, timeout: Duration) -> bool {
        self.waiter().flush(timeout)
    }

    /// A handle for the blocking waits that does not borrow the store, so a caller that keeps
    /// the store behind a lock can wait without holding it.
    pub fn waiter(&self) -> SyncWaiter {
        SyncWaiter { shared: self.shared.clone(), requests: self.requests.clone() }
    }

    /// Local changes the server refused for good (e.g. too large). They stay applied locally
    /// but will not reach other devices.
    pub fn rejected_count(&self) -> usize {
        self.shared.rejected()
    }

    /// The account this store syncs with; `None` for the guest.
    pub fn account(&self) -> Option<Account> {
        let session = self.shared.session()?;
        let info = self.shared.account_info()?;
        Some(Account { server_url: session.server_url.clone(), info })
    }

    /// Local changes not yet acknowledged by the server. Signing out discards them: warn first.
    pub fn pending(&self) -> usize {
        self.shared.pending()
    }

    pub(crate) fn session(&self) -> Option<Arc<AccountSession>> {
        self.shared.session()
    }

    /// Whether the store holds any data (memory only).
    pub(crate) fn has_data(&self) -> bool {
        EntityKind::ALL.into_iter().any(|kind| self.shared.fields().entities(kind).next().is_some())
    }

    /// Shows `info` at once and keeps it for the next start.
    pub(crate) fn update_account_info(&self, info: AccountInfo) {
        self.shared.set_account_info(info.clone());
        self.send(Request::AccountInfo(info));
    }

    /// Connects again after the user signed in anew (tokens were replaced).
    pub(crate) fn resume(&self) {
        self.send(Request::Resume);
    }

    /// Stops syncing for good and saves what is unsaved; waits up to `CLOSE_GRACE`.
    pub(crate) fn close_session(&self) {
        let (reply, closed) = std_mpsc::channel();
        self.send(Request::CloseSession(reply));
        let _ = closed.recv_timeout(CLOSE_GRACE);
    }

    /// Saves applied local changes and, if the store syncs, queues them for the server.
    pub(crate) fn queue(&self, changes: Vec<FieldChange>) {
        if self.shared.session().is_some() {
            self.shared.add_pending(changes.len());
        }
        self.send(Request::Persist(changes));
    }

    fn send(&self, request: Request) {
        if self.requests.send(request).is_err() {
            self.shared.set_storage_error(Some(StoreError::WorkerStopped));
            self.shared.notify();
        }
    }
}

/// Apps share one store between the UI thread and background threads (e.g. `Profiles::sign_out`
/// while the UI still reads it).
const _: () = {
    const fn shareable<T: Send + Sync>() {}
    shareable::<Store>();
    shareable::<Profiles>();
};

/// The store's blocking waits (`Store::waiter`). Cheap to clone; safe from any thread.
#[derive(Clone)]
pub struct SyncWaiter {
    shared: Arc<Shared>,
    requests: mpsc::UnboundedSender<Request>,
}

impl SyncWaiter {
    /// See `Store::sync_until_idle`.
    pub fn sync_until_idle(&self, timeout: Duration) -> SyncOutcome {
        self.shared.wait_until_settled(timeout)
    }

    /// See `Store::flush`. Covers the changes dispatched before the call, not ones other threads
    /// dispatch meanwhile. False at once once the store is gone.
    pub fn flush(&self, timeout: Duration) -> bool {
        let (reply, saved) = std_mpsc::channel();
        if self.requests.send(Request::Flush(reply)).is_err() {
            return false;
        }
        saved.recv_timeout(timeout).unwrap_or(false)
    }
}

impl Drop for Store {
    /// Lets the worker finish pending writes, so reopening the same folder sees them, but
    /// never blocks the caller for longer than `SHUTDOWN_GRACE`.
    fn drop(&mut self) {
        let _ = self.requests.send(Request::Shutdown);
        let worker_done = self.worker_done.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner);
        let finished = worker_done.recv_timeout(SHUTDOWN_GRACE).is_ok();
        if let Some(worker) = self.worker.take()
            && finished
        {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod sync_tests;
