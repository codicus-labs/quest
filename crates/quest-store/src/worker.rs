//! The background worker: owns the database and the sync connection. Drives the connection
//! state machine by performing what each state asks for and feeding back the outcome.

mod live;

use std::future::Future;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use quest_domain::{EntityId, FieldChange, NodeId};
use quest_protocol::AccountInfo;
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::conn_state::{ConnEvent, ConnState, backoff, step};
use crate::db::{Db, Loaded};
use crate::error::StoreError;
use crate::oauth;
use crate::remote::{self, ConnectFailure, Socket};
use crate::session::{AccountSession, TokenFailure};
use crate::shared::Shared;

pub(crate) enum Request {
    Persist(Vec<FieldChange>),
    /// Saves what is still unsaved, then answers whether nothing is left (requests run in
    /// order, so every earlier `Persist` is done by then).
    Flush(std::sync::mpsc::Sender<bool>),
    /// The server's current view of the account, to keep for the next start.
    AccountInfo(AccountInfo),
    /// The workspace the user switched to, to show again on the next start.
    RememberWorkspace(EntityId),
    /// The user signed in again after the session expired: connect with the new tokens.
    Resume,
    /// Signing out: stop syncing for good, save what is unsaved, then answer.
    CloseSession(std::sync::mpsc::Sender<()>),
    Shutdown,
}

/// Connection timing. Production values follow the protocol; tests shorten them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Timing {
    pub ping_every: Duration,
    /// A connection without any frame (or without an ack for a push) for this long is dead.
    pub silence_limit: Duration,
    pub send_timeout: Duration,
}

impl Timing {
    pub const PRODUCTION: Timing = Timing {
        ping_every: Duration::from_secs(15),
        silence_limit: Duration::from_secs(45),
        send_timeout: Duration::from_secs(10),
    };
}

/// Why the worker stopped what it was doing.
enum Interrupt {
    Event(ConnEvent),
    Shutdown,
}

/// What woke the worker up besides the connection.
enum Input {
    Request(Option<Request>),
    RetryPersist,
}

/// Local changes that could not be saved yet, and when to try again.
#[derive(Default)]
struct Unpersisted {
    changes: Vec<FieldChange>,
    failures: u32,
    retry_at: Option<Instant>,
}

pub(crate) struct Worker {
    db: Db,
    shared: Arc<Shared>,
    requests: mpsc::UnboundedReceiver<Request>,
    /// For background tasks that report back (account info); weak so it never keeps the worker
    /// alive.
    loopback: mpsc::WeakUnboundedSender<Request>,
    timing: Timing,
    node: NodeId,
    /// Local changes go to the outbox only if the profile syncs (not for the guest).
    syncs: bool,
    /// The server rejected the access token on a live connection: refresh before reconnecting.
    token_rejected: bool,
    socket: Option<Socket>,
    unpersisted: Unpersisted,
}

impl Worker {
    pub fn new(
        db: Db,
        loaded: Loaded,
        shared: Arc<Shared>,
        requests: (mpsc::UnboundedSender<Request>, mpsc::UnboundedReceiver<Request>),
        timing: Timing,
    ) -> Worker {
        let syncs = shared.session().is_some();
        Worker {
            db,
            shared,
            loopback: requests.0.downgrade(),
            requests: requests.1,
            timing,
            node: loaded.node,
            syncs,
            token_rejected: false,
            socket: None,
            unpersisted: Unpersisted::default(),
        }
    }

    /// Runs the worker on its own thread; `done` receives a message when it has stopped.
    pub fn spawn(self, initial: ConnState, done: std::sync::mpsc::Sender<()>) -> std::io::Result<JoinHandle<()>> {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        std::thread::Builder::new().name("quest-store".into()).spawn(move || {
            runtime.block_on(self.run(initial));
            let _ = done.send(());
        })
    }

    async fn run(mut self, initial: ConnState) {
        let mut state = initial;
        loop {
            self.shared.set_connection(&state);
            self.shared.notify();
            let outcome = match &state {
                ConnState::SignedOut | ConnState::Failed { .. } => self.idle().await,
                ConnState::Connecting { .. } => self.connect().await,
                ConnState::Live => self.live().await,
                ConnState::Waiting { retry_in, .. } => self
                    .race(wait_to_retry(&self.shared.clone(), *retry_in))
                    .await
                    .err()
                    .unwrap_or(Interrupt::Event(ConnEvent::RetryElapsed)),
            };
            match outcome {
                Interrupt::Shutdown => return,
                Interrupt::Event(event) => state = step(&state, event, rand::random()),
            }
        }
    }

    async fn idle(&mut self) -> Interrupt {
        loop {
            let input = self.next_input().await;
            if let Some(interrupt) = self.handle(input) {
                return interrupt;
            }
        }
    }

    /// Runs `work` while still serving requests; a request that changes course wins.
    async fn race<T>(&mut self, work: impl Future<Output = T>) -> Result<T, Interrupt> {
        tokio::pin!(work);
        loop {
            tokio::select! {
                output = &mut work => return Ok(output),
                input = self.next_input() => {
                    if let Some(interrupt) = self.handle(input) {
                        return Err(interrupt);
                    }
                }
            }
        }
    }

    /// The next request, or the moment to retry saving. Cancel-safe.
    async fn next_input(&mut self) -> Input {
        let retry_at = self.unpersisted.retry_at;
        tokio::select! {
            request = self.requests.recv() => Input::Request(request),
            () = sleep_until(retry_at) => Input::RetryPersist,
        }
    }

    /// Serves one input. Returns an interrupt if the connection must change course.
    fn handle(&mut self, input: Input) -> Option<Interrupt> {
        let request = match input {
            Input::RetryPersist => {
                self.persist(Vec::new());
                return None;
            }
            Input::Request(request) => request,
        };
        match request {
            None | Some(Request::Shutdown) => {
                self.persist(Vec::new());
                Some(Interrupt::Shutdown)
            }
            Some(Request::Persist(changes)) => {
                self.persist(changes);
                self.remember_fallback();
                None
            }
            Some(Request::Flush(reply)) => {
                self.persist(Vec::new());
                let _ = reply.send(self.unpersisted.changes.is_empty());
                None
            }
            Some(Request::AccountInfo(info)) => {
                let result = self.db.set_account_info(&info);
                self.report_storage(result);
                self.shared.set_account_info(info);
                self.shared.notify();
                None
            }
            Some(Request::RememberWorkspace(workspace)) => {
                let result = self.db.set_workspace(workspace);
                self.report_storage(result);
                None
            }
            Some(Request::Resume) => Some(Interrupt::Event(ConnEvent::SessionRestored)),
            Some(Request::CloseSession(reply)) => {
                self.shared.close_session();
                self.persist(Vec::new());
                let _ = reply.send(());
                Some(Interrupt::Event(ConnEvent::SignOutRequested))
            }
        }
    }

    /// Remembers the default workspace if the shown one was deleted (`Shared::take_fallback`).
    fn remember_fallback(&mut self) {
        if let Some(workspace) = self.shared.take_fallback() {
            let result = self.db.set_workspace(workspace);
            self.report_storage(result);
        }
    }

    /// Saves `changes` together with any earlier ones that failed. On failure everything stays
    /// in memory and is retried with backoff; the status shows the storage error meanwhile.
    fn persist(&mut self, changes: Vec<FieldChange>) {
        self.unpersisted.changes.extend(changes);
        if self.unpersisted.changes.is_empty() {
            return;
        }
        match self.db.persist_local(&self.unpersisted.changes, self.syncs) {
            Ok(()) => {
                self.unpersisted = Unpersisted::default();
                self.shared.set_storage_error(None);
            }
            Err(error) => {
                self.unpersisted.failures += 1;
                let delay = backoff(self.unpersisted.failures, rand::random());
                self.unpersisted.retry_at = Some(Instant::now() + delay);
                self.shared.set_storage_error(Some(error));
            }
        }
        self.shared.notify();
    }

    /// Shows a storage failure (or clears one once saving works again).
    fn report_storage(&mut self, result: Result<(), StoreError>) {
        let error = result.err();
        if error.is_none() && !self.unpersisted.changes.is_empty() {
            return;
        }
        if self.shared.set_storage_error(error) {
            self.shared.notify();
        }
    }

    /// A database failure while syncing: report it and reconnect later, which retries.
    fn storage_failed(&mut self, error: StoreError) -> Interrupt {
        self.report_storage(Err(error));
        Interrupt::Event(ConnEvent::ConnectionLost)
    }

    async fn connect(&mut self) -> Interrupt {
        let Some(session) = self.shared.session() else {
            return Interrupt::Event(ConnEvent::SignOutRequested);
        };
        let force_refresh = std::mem::take(&mut self.token_rejected);
        match self.race(open_socket(&session, force_refresh)).await {
            Err(interrupt) => interrupt,
            Ok(Ok(socket)) => {
                self.socket = Some(socket);
                self.refresh_account_info(session);
                Interrupt::Event(ConnEvent::Connected)
            }
            Ok(Err(ConnectFailure::Transient)) => Interrupt::Event(ConnEvent::ConnectionLost),
            Ok(Err(ConnectFailure::Rejected)) => Interrupt::Event(ConnEvent::Fatal(StoreError::SessionExpired)),
            Ok(Err(ConnectFailure::Fatal(error))) => Interrupt::Event(ConnEvent::Fatal(error)),
        }
    }

    /// Fetches the account in the background (e.g. AI opt-in changed on another device); a
    /// failure keeps the last known info.
    fn refresh_account_info(&self, session: Arc<AccountSession>) {
        let loopback = self.loopback.clone();
        tokio::spawn(async move {
            let Ok(token) = session.access_token().await else { return };
            let Ok(info) = oauth::fetch_account(&session.server_url, &token).await else { return };
            if let Some(requests) = loopback.upgrade() {
                let _ = requests.send(Request::AccountInfo(info));
            }
        });
    }
}

/// Opens the sync socket with the access token; a rejected token is refreshed once.
async fn open_socket(session: &AccountSession, force_refresh: bool) -> Result<Socket, ConnectFailure> {
    let mut token = session.access_token().await.map_err(|failure| token_failure(failure, session))?;
    if force_refresh {
        token = session.refresh(&token).await.map_err(|failure| token_failure(failure, session))?;
    }
    match remote::connect(&session.server_url, &token).await {
        Err(ConnectFailure::Rejected) => {
            let token = session.refresh(&token).await.map_err(|failure| token_failure(failure, session))?;
            remote::connect(&session.server_url, &token).await
        }
        outcome => outcome,
    }
}

fn token_failure(failure: TokenFailure, session: &AccountSession) -> ConnectFailure {
    match failure {
        TokenFailure::Unreachable(_) => ConnectFailure::Transient,
        failure => ConnectFailure::Fatal(failure.into_store_error(&session.server_url)),
    }
}

/// Sleeps until the retry is due, or until `Store::reconnect_now` asks for it earlier.
async fn wait_to_retry(shared: &Shared, retry_in: Duration) {
    tokio::select! {
        () = tokio::time::sleep(retry_in) => {}
        () = shared.reconnect.notified() => {}
    }
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}
