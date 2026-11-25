//! State shared between the caller's thread and the worker: in-memory fields, the snapshot
//! built from them, the clock, and what `status()` reports. Never touches the disk.
//!
//! Snapshots are built outside every lock: writers bump a version under the field lock, take a
//! reference-counted handle to the fields (copy-on-write) and publish the snapshot only if no
//! newer one was published meanwhile.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use quest_domain::{
    Command, CommandError, EntityId, EntityKind, FieldChange, FieldStore, FieldWrite, Hlc, HlcClock, NodeId,
    PlannedBatch, Snapshot, plan, plan_all,
};
use quest_protocol::{AccountInfo, MAX_CLOCK_DRIFT_MS};
use serde_json::Value;

use crate::SyncStatus;
use crate::conn_state::ConnState;
use crate::db::Loaded;
use crate::error::StoreError;
use crate::idle::{ChangeSignal, SyncOutcome, settled_outcome};
use crate::session::AccountSession;

/// Source of physical time for the hybrid logical clock.
pub(crate) trait WallClock: Send + Sync {
    fn now_ms(&self) -> u64;
}

pub(crate) struct SystemClock;

impl WallClock for SystemClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
    }
}

struct Fields {
    store: Arc<FieldStore>,
    clock: HlcClock,
    version: u64,
    /// The workspace snapshots show. Kept with the fields so a snapshot never mixes the fields of
    /// one version with the workspace of another.
    workspace: EntityId,
}

struct Published {
    version: u64,
    snapshot: Arc<Snapshot>,
}

/// Snapshots of other workspaces than the shown one, built from the fields of `version`. The
/// running timer's notification reads one every second, so it is built once per change.
#[derive(Default)]
struct Others {
    version: u64,
    snapshots: HashMap<EntityId, Arc<Snapshot>>,
}

struct Connection {
    state: ConnState,
    since: Instant,
}

pub(crate) struct Shared {
    fields: Mutex<Fields>,
    published: Mutex<Published>,
    /// Set when the shown workspace was deleted and the store fell back to another one (the default
    /// one while it is live, else the first live one).
    fallback: Mutex<Option<EntityId>>,
    others: Mutex<Others>,
    connection: Mutex<Connection>,
    caught_up: AtomicBool,
    /// Overrides the connection status while set: local saving is failing.
    storage_error: Mutex<Option<StoreError>>,
    /// The account this store syncs with; `None` for the guest and after signing out.
    session: Mutex<Option<Arc<AccountSession>>>,
    account_info: Mutex<Option<AccountInfo>>,
    /// Wakes a worker that waits to retry the connection (`Store::reconnect_now`).
    pub(crate) reconnect: tokio::sync::Notify,
    pending: AtomicUsize,
    rejected: AtomicUsize,
    wall_clock: Box<dyn WallClock>,
    on_change: Box<dyn Fn() + Send + Sync>,
    changed: ChangeSignal,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    pub fn new(
        loaded: &Loaded,
        session: Option<Arc<AccountSession>>,
        wall_clock: Box<dyn WallClock>,
        on_change: Box<dyn Fn() + Send + Sync>,
    ) -> Shared {
        let clock = resume_clock(loaded.node, &loaded.fields, wall_clock.now_ms());
        let snapshot = Arc::new(Snapshot::from_store(&loaded.fields, loaded.workspace));
        let workspace = snapshot.workspace;
        Shared {
            fields: Mutex::new(Fields { store: Arc::new(loaded.fields.clone()), clock, version: 0, workspace }),
            published: Mutex::new(Published { version: 0, snapshot }),
            fallback: Mutex::new(None),
            others: Mutex::new(Others::default()),
            connection: Mutex::new(Connection { state: ConnState::SignedOut, since: Instant::now() }),
            caught_up: AtomicBool::new(false),
            storage_error: Mutex::new(None),
            session: Mutex::new(session),
            account_info: Mutex::new(loaded.account_info.clone()),
            reconnect: tokio::sync::Notify::new(),
            pending: AtomicUsize::new(loaded.pending),
            rejected: AtomicUsize::new(loaded.rejected),
            wall_clock,
            on_change,
            changed: ChangeSignal::default(),
        }
    }

    /// The workspace the store fell back to since the last call, to remember for the next start.
    pub fn take_fallback(&self) -> Option<EntityId> {
        lock(&self.fallback).take()
    }

    pub fn snapshot(&self) -> Arc<Snapshot> {
        lock(&self.published).snapshot.clone()
    }

    /// A snapshot of `workspace` (not the shown one), e.g. for the running timer's task in
    /// another workspace. Built on first use after a change. None unless that workspace is live.
    pub fn snapshot_in(&self, workspace: EntityId) -> Option<Arc<Snapshot>> {
        let (version, store) = {
            let fields = lock(&self.fields);
            (fields.version, fields.store.clone())
        };
        if let Some(cached) = self.cached_other(version, workspace) {
            return Some(cached);
        }
        if !Snapshot::is_live_workspace(&store, workspace) {
            return None;
        }
        let snapshot = Arc::new(Snapshot::from_store(&store, workspace));
        let mut others = lock(&self.others);
        if others.version != version {
            *others = Others { version, snapshots: HashMap::new() };
        }
        others.snapshots.insert(workspace, snapshot.clone());
        Some(snapshot)
    }

    fn cached_other(&self, version: u64, workspace: EntityId) -> Option<Arc<Snapshot>> {
        let others = lock(&self.others);
        (others.version == version).then(|| others.snapshots.get(&workspace).cloned()).flatten()
    }

    /// Plans the command against the current snapshot, stamps and applies the writes.
    /// Returns the stamped changes to persist.
    pub fn apply_local(&self, command: &Command) -> Result<Vec<FieldChange>, CommandError> {
        self.apply_writes(plan(command, &self.snapshot())?)
    }

    /// Like `apply_local`, planned against `workspace` instead of the shown one.
    pub fn apply_local_in(&self, workspace: EntityId, command: &Command) -> Result<Vec<FieldChange>, CommandError> {
        let snapshot = self.snapshot_in(workspace).ok_or(CommandError::UnknownWorkspace(workspace))?;
        self.apply_writes(plan(command, &snapshot)?)
    }

    fn apply_writes(&self, writes: Vec<FieldWrite>) -> Result<Vec<FieldChange>, CommandError> {
        if writes.is_empty() {
            return Ok(Vec::new());
        }
        let now = self.wall_clock.now_ms();
        let (changes, version, store, workspace) = {
            let mut fields = lock(&self.fields);
            let changes: Vec<FieldChange> =
                writes.into_iter().map(|write| write.stamp(fields.clock.now(now))).collect();
            let store = Arc::make_mut(&mut fields.store);
            for change in &changes {
                store.apply(change);
            }
            fields.version += 1;
            (changes, fields.version, fields.store.clone(), fields.workspace)
        };
        self.publish(version, &store, workspace);
        Ok(changes)
    }

    /// Shows `workspace` from now on. Refused unless it exists and is not deleted.
    pub fn switch_workspace(&self, workspace: EntityId) -> Result<(), CommandError> {
        let (version, store) = {
            let mut fields = lock(&self.fields);
            if !Snapshot::is_live_workspace(&fields.store, workspace) {
                return Err(CommandError::UnknownWorkspace(workspace));
            }
            fields.workspace = workspace;
            fields.version += 1;
            (fields.version, fields.store.clone())
        };
        self.publish(version, &store, workspace);
        Ok(())
    }

    /// Like `apply_local` for several commands, all or nothing: each is planned against the state
    /// the ones before it leave, and nothing is applied if one is refused.
    pub fn apply_local_all(&self, commands: &[Command]) -> Result<Vec<FieldChange>, CommandError> {
        let now = self.wall_clock.now_ms();
        let (changes, version, store, workspace) = {
            let mut fields = lock(&self.fields);
            let PlannedBatch { changes, store, clock } =
                plan_all(commands, &fields.store, fields.workspace, &fields.clock, now)?;
            if changes.is_empty() {
                return Ok(changes);
            }
            fields.clock = clock;
            fields.store = Arc::new(store);
            fields.version += 1;
            (changes, fields.version, fields.store.clone(), fields.workspace)
        };
        self.publish(version, &store, workspace);
        Ok(changes)
    }

    /// Merges changes from the server without publishing a snapshot (see `publish_latest`).
    /// Far-future timestamps are merged but do not move the local clock. Returns true if
    /// anything changed.
    pub fn apply_remote(&self, changes: &[FieldChange]) -> bool {
        let now = self.wall_clock.now_ms();
        let mut fields = lock(&self.fields);
        let mut changed = false;
        for change in changes {
            fields.clock.observe(change.hlc, now, MAX_CLOCK_DRIFT_MS);
            changed |= Arc::make_mut(&mut fields.store).apply(change);
        }
        if changed {
            fields.version += 1;
        }
        changed
    }

    /// Publishes a snapshot of the current fields if the published one is older.
    pub fn publish_latest(&self) {
        let (version, store, workspace) = {
            let fields = lock(&self.fields);
            (fields.version, fields.store.clone(), fields.workspace)
        };
        if lock(&self.published).version < version {
            self.publish(version, &store, workspace);
        }
    }

    /// If `workspace` was deleted (here or on another device), the snapshot shows another one
    /// (see `Snapshot::from_store`), and the store stays there even if it is restored later: the user is not moved back
    /// and forth. `take_fallback` tells the worker to remember that.
    fn publish(&self, version: u64, store: &FieldStore, workspace: EntityId) {
        let snapshot = Arc::new(Snapshot::from_store(store, workspace));
        if snapshot.workspace != workspace {
            let mut fields = lock(&self.fields);
            if fields.workspace == workspace {
                fields.workspace = snapshot.workspace;
                *lock(&self.fallback) = Some(snapshot.workspace);
            }
        }
        let mut published = lock(&self.published);
        if version > published.version {
            *published = Published { version, snapshot };
        }
    }

    /// Writes fields as local changes stamped with this store's clock, as if the user had just
    /// made them. Returns the stamped changes to persist.
    pub fn apply_imported(&self, writes: Vec<ImportedField>) -> Vec<FieldChange> {
        if writes.is_empty() {
            return Vec::new();
        }
        let now = self.wall_clock.now_ms();
        let (changes, version, store, workspace) = {
            let mut fields = lock(&self.fields);
            // The account wins: skip what it got since the import was planned (e.g. from the server).
            let missing: Vec<ImportedField> = writes
                .into_iter()
                .filter(|write| {
                    fields.store.get(write.kind, write.id).is_none_or(|existing| !existing.contains_key(&write.field))
                })
                .collect();
            let changes: Vec<FieldChange> = missing
                .into_iter()
                .map(|write| FieldChange {
                    kind: write.kind,
                    id: write.id,
                    field: write.field,
                    value: write.value,
                    hlc: fields.clock.now(now),
                })
                .collect();
            if changes.is_empty() {
                return Vec::new();
            }
            let store = Arc::make_mut(&mut fields.store);
            for change in &changes {
                store.apply(change);
            }
            fields.version += 1;
            (changes, fields.version, fields.store.clone(), fields.workspace)
        };
        self.publish(version, &store, workspace);
        changes
    }

    /// The current fields (copy-on-write handle).
    pub fn fields(&self) -> Arc<FieldStore> {
        lock(&self.fields).store.clone()
    }

    pub fn pending(&self) -> usize {
        self.pending.load(Ordering::SeqCst)
    }

    pub fn add_pending(&self, count: usize) {
        self.pending.fetch_add(count, Ordering::SeqCst);
    }

    pub fn remove_pending(&self, count: usize) {
        let _ = self.pending.try_update(Ordering::SeqCst, Ordering::SeqCst, |p| Some(p.saturating_sub(count)));
    }

    pub fn add_rejected(&self, count: usize) {
        self.rejected.fetch_add(count, Ordering::SeqCst);
    }

    pub fn rejected(&self) -> usize {
        self.rejected.load(Ordering::SeqCst)
    }

    pub fn set_connection(&self, state: &ConnState) {
        self.caught_up.store(false, Ordering::SeqCst);
        *lock(&self.connection) = Connection { state: state.clone(), since: Instant::now() };
    }

    pub fn set_caught_up(&self) {
        self.caught_up.store(true, Ordering::SeqCst);
    }

    /// Returns true if the value changed.
    pub fn set_storage_error(&self, error: Option<StoreError>) -> bool {
        let mut current = lock(&self.storage_error);
        let changed = *current != error;
        *current = error;
        changed
    }

    pub fn status(&self) -> SyncStatus {
        if let Some(error) = lock(&self.storage_error).clone() {
            return SyncStatus::Failed { error };
        }
        let pending = self.pending.load(Ordering::SeqCst);
        let connection = lock(&self.connection);
        match &connection.state {
            ConnState::SignedOut => SyncStatus::SignedOut,
            ConnState::Connecting { attempt: 0 } => SyncStatus::Connecting,
            // A retry in progress: still offline from the user's point of view.
            ConnState::Connecting { .. } => SyncStatus::Offline { pending, retry_in: Duration::ZERO },
            ConnState::Live if !self.caught_up.load(Ordering::SeqCst) => SyncStatus::Connecting,
            ConnState::Live => SyncStatus::Live { pending },
            ConnState::Waiting { retry_in, .. } => {
                SyncStatus::Offline { pending, retry_in: retry_in.saturating_sub(connection.since.elapsed()) }
            }
            ConnState::Failed { error } => SyncStatus::Failed { error: error.clone() },
        }
    }

    pub fn session(&self) -> Option<Arc<AccountSession>> {
        lock(&self.session).clone()
    }

    /// Ends syncing for good (signing out): REST calls stop too.
    pub fn close_session(&self) {
        if let Some(session) = lock(&self.session).take() {
            session.close();
        }
    }

    pub fn account_info(&self) -> Option<AccountInfo> {
        lock(&self.account_info).clone()
    }

    pub fn set_account_info(&self, info: AccountInfo) {
        *lock(&self.account_info) = Some(info);
    }

    pub fn notify(&self) {
        self.changed.notify();
        (self.on_change)();
    }

    /// Blocks until the status settles (see `idle::settled_outcome`) or `timeout` runs out.
    pub fn wait_until_settled(&self, timeout: Duration) -> SyncOutcome {
        self.changed.wait_until_settled(timeout, || {
            let status = self.status();
            let connection = lock(&self.connection).state.clone();
            settled_outcome(&status, &connection)
        })
    }
}

/// Resumes after this node's own last timestamp, and after plausible remote ones. A stored
/// far-future timestamp from another device must not drag the clock along.
fn resume_clock(node: NodeId, fields: &FieldStore, now_ms: u64) -> HlcClock {
    let timestamps: Vec<Hlc> = EntityKind::ALL
        .into_iter()
        .flat_map(|kind| fields.entities(kind))
        .flat_map(|(_, values)| values.values().map(|(_, hlc)| *hlc))
        .collect();
    let own_highest = timestamps.iter().filter(|hlc| hlc.node == node).max();
    let mut clock = own_highest.map_or_else(|| HlcClock::new(node), |hlc| HlcClock::resume(node, *hlc));
    for hlc in timestamps.iter().filter(|hlc| hlc.node != node) {
        clock.observe(*hlc, now_ms, MAX_CLOCK_DRIFT_MS);
    }
    clock
}

/// A field to write into this store with a fresh timestamp (`Shared::apply_imported`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ImportedField {
    pub kind: EntityKind,
    pub id: quest_domain::EntityId,
    pub field: String,
    pub value: Value,
}

#[cfg(test)]
#[path = "shared_tests.rs"]
mod tests;
