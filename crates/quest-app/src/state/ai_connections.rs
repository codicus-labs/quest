//! "Connected AI clients" in the account dialog: the MCP clients authorised for the account,
//! loaded and revoked off the UI thread (`rune::Latest`).

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use quest_store::{AiConnection, ConnectionsError};
use rune::Latest;

use super::capture::capitalized;
use super::{Overlay, State};

pub enum ConnectionsView {
    /// Loads when the account dialog shows while signed in.
    NotLoaded,
    Loading,
    Loaded(Vec<AiConnection>),
    Failed(String),
}

pub struct AiConnectionsState {
    pub view: ConnectionsView,
    /// The client whose "Disconnect" waits for confirmation.
    pub confirming: Option<AiConnection>,
    answers: Latest<Result<Vec<AiConnection>, ConnectionsError>>,
}

impl Default for AiConnectionsState {
    fn default() -> Self {
        AiConnectionsState { view: ConnectionsView::NotLoaded, confirming: None, answers: Latest::default() }
    }
}

impl AiConnectionsState {
    /// Wakes the window when an answer lands.
    pub fn set_waker(&self, waker: rune::Waker) {
        self.answers.set_waker(waker);
    }
}

impl State {
    /// Loads the list when the account dialog shows signed in, takes in answers, and forgets the
    /// list when the dialog closes (it reloads next time). True when the dialog changed.
    pub(super) fn refresh_ai_connections(&mut self) -> bool {
        let shown = self.overlay == Overlay::Account && self.account.is_some();
        let connections = &mut self.ai_connections;
        if !shown {
            if matches!(connections.view, ConnectionsView::NotLoaded) {
                return false;
            }
            connections.answers.cancel();
            connections.view = ConnectionsView::NotLoaded;
            connections.confirming = None;
            return true;
        }
        if matches!(connections.view, ConnectionsView::NotLoaded) {
            let client = self.store.connections();
            connections.answers.request((), move |()| client.list());
            connections.view = ConnectionsView::Loading;
            return true;
        }
        let Some(answer) = connections.answers.take() else { return false };
        connections.view = match answer {
            Ok(list) => ConnectionsView::Loaded(list),
            Err(error) => ConnectionsView::Failed(capitalized(&error.to_string())),
        };
        true
    }

    /// "Disconnect" on a row: asks first.
    pub fn ask_revoke_ai_connection(&mut self, connection: &AiConnection) {
        self.ai_connections.confirming = Some(connection.clone());
    }

    pub fn keep_ai_connection(&mut self) {
        self.ai_connections.confirming = None;
    }

    /// Confirmed "Disconnect": revokes the client's tokens, then reloads the list.
    pub fn revoke_ai_connection(&mut self) {
        let Some(connection) = self.ai_connections.confirming.take() else { return };
        let client = self.store.connections();
        self.ai_connections.answers.request(connection.id, move |id| {
            client.revoke(id)?;
            client.list()
        });
        self.ai_connections.view = ConnectionsView::Loading;
    }

    /// "Try again" after a failure.
    pub fn reload_ai_connections(&mut self) {
        self.ai_connections.view = ConnectionsView::NotLoaded;
    }
}

/// The client's name, or a stand-in when it gave none.
pub fn connection_name(connection: &AiConnection) -> &str {
    match connection.client_name.trim() {
        "" => "Unnamed AI client",
        name => name,
    }
}

/// What a connected client may do, from its granted scope.
pub fn scope_label(scope: &str) -> &'static str {
    if scope.split(' ').any(|s| s == quest_protocol::SCOPE_WRITE) { "Read and write" } else { "Read only" }
}

/// "claude.ai · Read only · Connected Nov 3 · Last used Nov 20"; `local_day` maps instants to
/// local dates (`clock::local_day` in the app).
pub fn connection_detail(
    connection: &AiConnection,
    today: NaiveDate,
    local_day: impl Fn(DateTime<Utc>) -> NaiveDate,
) -> String {
    let day = |at: DateTime<Utc>| {
        let day = local_day(at);
        match (today - day).num_days() {
            ..=0 => "today".to_owned(),
            1 => "yesterday".to_owned(),
            _ if day.year() == today.year() => day.format("%b %-d").to_string(),
            _ => day.format("%b %-d, %Y").to_string(),
        }
    };
    let used = connection.last_used_at.map_or_else(|| "Not used yet".to_owned(), |at| format!("Last used {}", day(at)));
    let (host, scope) = (&connection.redirect_host, scope_label(&connection.scope));
    format!("{host} · {scope} · Connected {} · {used}", day(connection.created_at))
}

#[cfg(test)]
#[path = "ai_connections_tests.rs"]
mod tests;
