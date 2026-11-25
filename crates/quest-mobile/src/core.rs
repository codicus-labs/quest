//! `QuestCore`: the one object a native UI holds. Wraps the active profile's `quest_store::Store`
//! (the guest's or the signed-in account's) and switches it on signing in and out. Every method is
//! memory-only and safe to call from the UI thread, except those documented as blocking (`open`,
//! signing in and out, account settings, network calls).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};

use quest_domain::{Command, EntityId};
use quest_store::{LoginRequest, Profiles, ProfilesConfig, Store, SyncStatus};

use crate::capture::CaptureUndo;
use crate::error::MobileError;
use crate::guard::guard;
use crate::home::Choice;
use crate::ids::parse_id;
use crate::plan::PlanMemory;
use crate::ticker::{self, TickMemory};
use crate::time::Now;
use crate::types::{AddPosition, HomeData, HomeQuery, Moment, SettingsData, SyncState, TaskSection};
use crate::undo::MobileUndo;
use crate::vault::{ForeignVault, TokenStore};
use crate::{commands, home, plan, settings, status};

/// Called whenever data or sync status changed. Implementations should re-read what they show
/// (e.g. `home`, `sync_state`) and hand it to the UI thread.
///
/// Usually on the store's worker thread, but it may also run synchronously on the thread of a
/// `QuestCore` call that is still holding the core's lock (e.g. when the worker has stopped). So
/// it must never call back into `QuestCore` directly, or it deadlocks: only signal (post to a
/// channel or the main thread) and read the core afterwards.
#[uniffi::export(with_foreign)]
pub trait ChangeListener: Send + Sync {
    fn on_change(&self);
}

#[derive(uniffi::Object)]
pub struct QuestCore {
    /// Opens and switches the guest's and the accounts' stores.
    pub(crate) profiles: Profiles,
    /// The active profile's store. Only cloned out under the lock, so a blocking call on a store
    /// (signing out, importing) never blocks other calls.
    store: Mutex<Arc<Store>>,
    /// Serializes signing in, importing and signing out. Taken before any other lock.
    pub(crate) switching: Mutex<()>,
    /// True while signing out: edits are refused instead of landing in a store that is being
    /// deleted. Edits hold the read lock while they run; signing out sets it with the write lock,
    /// so it waits for edits already under way.
    pub(crate) signing_out: RwLock<bool>,
    /// The sign-in waiting for its redirect from the browser. Never held while taking another lock.
    pub(crate) login: Mutex<Option<LoginRequest>>,
    /// Told after a switch, as the new store does not report one by itself.
    pub(crate) listener: Arc<dyn ChangeListener>,
    /// Only `tick` takes it, before the other locks.
    ticks: Mutex<TickMemory>,
    /// The undo snackbar. Taken before the store lock, never while holding it.
    pub(crate) undo: Mutex<MobileUndo>,
    /// What `plan` showed last. Only copied out, never held while taking another lock.
    plan: Mutex<Option<PlanMemory>>,
    /// What the last applied AI capture created, for its Undo. Never held while taking another lock.
    pub(crate) capture_undo: Mutex<Option<CaptureUndo>>,
    /// The workspace the memory above belongs to; None until first seen in this store.
    /// Taken before the memory's locks, never while holding the store lock.
    shown_workspace: Mutex<Option<EntityId>>,
}

#[uniffi::export]
impl QuestCore {
    /// Opens the active profile in `data_dir` (the signed-in account, else the guest) and starts
    /// syncing if signed in. `tokens` keeps the accounts' tokens (the platform's secure storage).
    /// Does disk IO: call it off the UI thread.
    #[uniffi::constructor]
    pub fn open(
        data_dir: String,
        tokens: Arc<dyn TokenStore>,
        listener: Arc<dyn ChangeListener>,
    ) -> Result<Arc<QuestCore>, MobileError> {
        guard(|| {
            let config =
                ProfilesConfig { data_dir: PathBuf::from(data_dir), vault: Arc::new(ForeignVault::new(tokens)) };
            let on_change = listener.clone();
            let (profiles, store) = Profiles::open(config, move || on_change.on_change())
                .map_err(|e| MobileError::Storage { detail: e.to_string() })?;
            Ok(Arc::new(QuestCore {
                profiles,
                store: Mutex::new(Arc::new(store)),
                switching: Mutex::default(),
                signing_out: RwLock::default(),
                login: Mutex::default(),
                listener,
                ticks: Mutex::default(),
                undo: Mutex::default(),
                plan: Mutex::default(),
                capture_undo: Mutex::default(),
                shown_workspace: Mutex::default(),
            }))
        })
    }

    /// Today's Home. `query` is the screen's quest filter and the quest picked for new work.
    pub fn home(&self, now: Moment, query: HomeQuery) -> Result<HomeData, MobileError> {
        guard(|| {
            let (now, choice) = (Now::parse(&now)?, Choice::parse(&query)?);
            let toast = self.undo_state().toast();
            Ok(home::home(&self.store().snapshot(), now, choice, toast))
        })
    }

    /// Adds a task for today to one of Home's lists, at its top or bottom. Quest work gets the
    /// filter's quest, else the picked one, else the one used last (desktop rule). Returns its id.
    pub fn add_task(
        &self,
        title: String,
        section: TaskSection,
        position: AddPosition,
        query: HomeQuery,
        now: Moment,
    ) -> Result<String, MobileError> {
        guard(|| {
            let (now, choice) = (Now::parse(&now)?, Choice::parse(&query)?);
            let id = EntityId::new();
            let snapshot = self.store().snapshot();
            self.dispatch(commands::add_task(&snapshot, id, &title, section, position, choice, now)?)?;
            Ok(id.to_string())
        })
    }

    /// Completes an open task or reopens a done one; the snackbar offers to undo it. As in the
    /// desktop's plan, a repeating task completed ahead of its day moves to today.
    pub fn toggle_task(&self, task_id: String, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let task = self.task(&task_id)?;
            self.dispatch(commands::toggle_task(&task, now))?;
            self.remember_completion(&task, now);

            Ok(())
        })
    }

    /// When to call `tick` next, as epoch ms: the next whole second while a timer runs, else
    /// the next whole minute, never later than the next midnight in `now.time_zone`.
    pub fn next_wake_ms(&self, now: Moment) -> Result<i64, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let toast_until = self.undo_state().expires();
            Ok(self.next_wake(&self.store(), now, toast_until))
        })
    }

    /// Call on open and at every `next_wake_ms`. Lets an expired undo snackbar go while the app is in the foreground. Creates due
    /// recurring tasks once per minute, at midnight and after data changed. Returns the next
    /// wake time like `next_wake_ms`.
    pub fn tick(&self, now: Moment) -> Result<i64, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let mut ticks = self.ticks.lock().unwrap_or_else(PoisonError::into_inner);
            let toast_until = {
                let mut undo = self.undo_state();
                undo.tick(now.at);
                undo.expires()
            };
            let store = self.store();
            generate_due_tasks(&store, &mut ticks, now);
            Ok(self.next_wake(&store, now, toast_until))
        })
    }

    pub fn settings(&self) -> SettingsData {
        settings::settings(&self.store().snapshot())
    }

    /// Sets the weekly target for Quest Hours (1 to 168 hours, in minutes).
    pub fn set_weekly_target(&self, minutes: u32) -> Result<(), MobileError> {
        guard(|| self.dispatch(settings::set_weekly_target(minutes)?))
    }

    pub fn sync_state(&self) -> SyncState {
        status::sync_state(&self.store().status())
    }
}

impl QuestCore {
    pub(crate) fn remember_completion(&self, before: &quest_domain::Task, now: Now) {
        let after = self.store().snapshot();
        if let Some(edit) = after.task(before.id).and_then(|task| quest_view::undo::TaskEdit::completion(before, task))
        {
            self.undo_state().toggled(edit, &before.title, now.at);
        }
    }

    /// The active store. Dropping the last handle to a replaced store waits for its worker (up to
    /// 2 s), so callers keep it only for the call.
    pub(crate) fn store(&self) -> Arc<Store> {
        self.store.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Makes `next` the active store and forgets what belonged to the one before (undo, plan,
    /// capture undo, tick memory). Returns the replaced store for the caller to drop off the lock.
    pub(crate) fn replace_store(&self, next: Store) -> Arc<Store> {
        let previous =
            std::mem::replace(&mut *self.store.lock().unwrap_or_else(PoisonError::into_inner), Arc::new(next));
        *self.shown_workspace.lock().unwrap_or_else(PoisonError::into_inner) = None;
        self.forget_shown_data();
        self.listener.on_change();
        previous
    }

    /// Records that `workspace` is shown; if another one was shown before, forgets what belonged
    /// to it (see `core_workspaces`).
    pub(crate) fn follow_workspace(&self, workspace: EntityId) {
        let mut shown = self.shown_workspace.lock().unwrap_or_else(PoisonError::into_inner);
        if shown.is_some_and(|before| before != workspace) {
            self.forget_shown_data();
        }
        *shown = Some(workspace);
    }

    /// Forgets the memory of what was shown: undo, plan, capture undo and ticks.
    fn forget_shown_data(&self) {
        *self.ticks.lock().unwrap_or_else(PoisonError::into_inner) = TickMemory::default();
        *self.undo_state() = MobileUndo::default();
        self.forget_plan();
        *self.capture_undo.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    pub(crate) fn undo_state(&self) -> MutexGuard<'_, MobileUndo> {
        self.undo.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn plan_memory(&self) -> Option<PlanMemory> {
        *self.plan.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn forget_plan(&self) {
        *self.plan.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    pub(crate) fn remember_plan(&self, memory: PlanMemory) {
        *self.plan.lock().unwrap_or_else(PoisonError::into_inner) = Some(memory);
    }

    /// Carries out `command`; once the plan was shown, an edit on a repeat it previews saves
    /// that repeat first (`plan::plan_edit`).
    pub(crate) fn dispatch(&self, command: Command) -> Result<(), MobileError> {
        let memory = self.plan_memory();
        self.edit(|store| {
            let command = match memory {
                Some(memory) => plan::plan_edit(&store.snapshot(), memory, command),
                None => command,
            };
            store.dispatch(command).map_err(|e| MobileError::Rejected { reason: e.to_string() })
        })
    }

    /// Runs `edit` on the active store, unless signing out is under way (`SigningOut`).
    pub(crate) fn edit<T>(&self, edit: impl FnOnce(&Store) -> Result<T, MobileError>) -> Result<T, MobileError> {
        let signing_out = self.signing_out.read().unwrap_or_else(PoisonError::into_inner);
        if *signing_out {
            return Err(MobileError::SigningOut);
        }
        edit(&self.store())
    }

    /// A copy of a task that exists now (not deleted), or of a repeat the plan previews.
    pub(crate) fn task(&self, task_id: &str) -> Result<quest_domain::Task, MobileError> {
        let id = parse_id(task_id)?;
        let memory = self.plan_memory();
        let snapshot = self.store().snapshot();
        snapshot
            .task(id)
            .cloned()
            .or_else(|| memory.and_then(|memory| plan::preview(&snapshot, memory, id)))
            .ok_or_else(|| MobileError::UnknownTask { id: task_id.to_owned() })
    }

    fn next_wake(&self, store: &Store, now: Now, toast_until: Option<chrono::DateTime<chrono::Utc>>) -> i64 {
        let timer_running = store.snapshot().running_focus().is_some();
        ticker::next_wake(now, timer_running, toast_until).timestamp_millis()
    }
}

/// Like the desktop: never while connecting, so a fresh device does not create occurrences
/// the server is about to send. A refused generation is left for the next minute.
fn generate_due_tasks(store: &Store, ticks: &mut TickMemory, now: Now) {
    if store.status() == SyncStatus::Connecting {
        ticks.postpone();
        return;
    }
    if !ticks.should_generate(&store.snapshot(), now) {
        return;
    }
    let _refused = store.dispatch(Command::GenerateDueTasks { now: now.at });
    ticks.generated(store.snapshot(), now);
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
