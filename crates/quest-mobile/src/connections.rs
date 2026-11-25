//! Connected AI clients (MCP over OAuth) for the account settings, pure: rows with ready labels and
//! the store's errors as `MobileError`s.

use chrono::{DateTime, Datelike, Utc};
use quest_protocol::SCOPE_WRITE;
use quest_store::{AiConnection, ConnectionsError};

use crate::error::MobileError;
use crate::time::Now;

/// One AI client connected to the account.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct AiConnectionRow {
    /// For `revoke_ai_connection`.
    pub id: String,
    /// The name the client gave itself, e.g. "Claude Code" (not verified), or "Unnamed AI client".
    pub name: String,
    /// Where it was connected from, e.g. "claude.ai" or "localhost".
    pub host: String,
    /// "Connected Nov 3 · Last used 2 days ago".
    pub detail: String,
    /// What it may do: "Read only" or "Read and write".
    pub scope: String,
}

pub(crate) fn rows(connections: &[AiConnection], now: Now) -> Vec<AiConnectionRow> {
    connections.iter().map(|c| row(c, now)).collect()
}

fn row(connection: &AiConnection, now: Now) -> AiConnectionRow {
    let name = connection.client_name.trim();
    let used =
        connection.last_used_at.map_or_else(|| "Not used yet".to_owned(), |at| format!("Last used {}", ago(at, now)));
    AiConnectionRow {
        id: connection.id.to_string(),
        name: if name.is_empty() { "Unnamed AI client".to_owned() } else { name.to_owned() },
        host: connection.redirect_host.clone(),
        detail: format!("Connected {} · {used}", date(connection.created_at, now)),
        scope: scope_label(&connection.scope).to_owned(),
    }
}

/// `quest:write` includes reading; without it the client only reads.
fn scope_label(scope: &str) -> &'static str {
    if scope.split_whitespace().any(|s| s == SCOPE_WRITE) { "Read and write" } else { "Read only" }
}

/// "Nov 3", or "Nov 3, 2025" in another year (local date).
fn date(at: DateTime<Utc>, now: Now) -> String {
    let day = now.local(at).date();
    let format = if day.year() == now.today().year() { "%b %-d" } else { "%b %-d, %Y" };
    day.format(format).to_string()
}

/// "today", "yesterday", "3 days ago" within a week, else the date (local days).
fn ago(at: DateTime<Utc>, now: Now) -> String {
    match (now.today() - now.local(at).date()).num_days() {
        ..=0 => "today".to_owned(),
        1 => "yesterday".to_owned(),
        days @ 2..=6 => format!("{days} days ago"),
        _ => date(at, now),
    }
}

pub(crate) fn connections_error(error: ConnectionsError) -> MobileError {
    match error {
        ConnectionsError::SignedOut => MobileError::NotSignedIn,
        ConnectionsError::SessionExpired => MobileError::SessionExpired,
        ConnectionsError::Unreachable { message } => MobileError::ServerUnreachable { detail: message },
        ConnectionsError::NotFound => MobileError::UnknownConnection,
        ConnectionsError::Server { message } => MobileError::Rejected { reason: message },
    }
}

#[cfg(test)]
#[path = "connections_tests.rs"]
mod tests;
