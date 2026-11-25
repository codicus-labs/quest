//! All SQL lives here. Callers see domain and protocol types only.

use std::str::FromStr;
use std::time::Duration;

use quest_domain::{EntityId, EntityKind, FieldChange, Hlc};
use quest_protocol::{Cursor, Epoch, UserId};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::{Row, Sqlite, Transaction};
use uuid::Uuid;

use crate::error::ServerError;

mod accounts;
mod oauth;

pub use accounts::{BrowserSession, NewBrowserSession, NewLogin, StartedLogin};
pub use oauth::{GrantRef, GrantSummary, NewCode, NewGrant, NewTokens, StoredClient, TokenKind};

/// A change as stored on the server, with its position in the user's sequence.
#[derive(Clone, Debug, PartialEq)]
pub struct Sequenced {
    pub seq: Cursor,
    pub change: FieldChange,
}

pub struct SequenceState {
    pub epoch: Epoch,
    pub last: Cursor,
}

#[derive(Clone)]
pub struct Repository {
    pool: SqlitePool,
}

impl Repository {
    pub async fn open(database_url: &str) -> Result<Repository, ServerError> {
        let open_error = |source| ServerError::OpenDatabase { url: database_url.to_owned(), source };
        let options = SqliteConnectOptions::from_str(database_url)
            .map_err(open_error)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new().max_connections(8).connect_with(options).await.map_err(open_error)?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Repository { pool })
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    /// Whether the database answers a query.
    pub async fn ping(&self) -> sqlx::Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await.map(drop)
    }

    /// The user's current epoch and the cursor after their latest stored change.
    pub async fn sequence_state(&self, user: UserId) -> sqlx::Result<SequenceState> {
        let row = sqlx::query("SELECT epoch, last_seq FROM users WHERE id = ?")
            .bind(user.0.to_string())
            .fetch_one(&self.pool)
            .await?;
        let epoch: String = row.try_get("epoch")?;
        Ok(SequenceState {
            epoch: Epoch(Uuid::parse_str(&epoch).map_err(|error| corrupt("epoch", error))?),
            last: to_cursor(row.try_get("last_seq")?),
        })
    }

    /// Merges `changes` last-writer-wins in one transaction. Returns the winners with their new
    /// sequence numbers, ascending. Callers must serialize pushes per user so commits and
    /// broadcasts happen in sequence order.
    pub async fn apply_changes(&self, user: UserId, changes: &[FieldChange]) -> sqlx::Result<Vec<Sequenced>> {
        let user_key = user.0.to_string();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let mut seq: i64 =
            sqlx::query_scalar("SELECT last_seq FROM users WHERE id = ?").bind(&user_key).fetch_one(&mut *tx).await?;
        let mut winners = Vec::new();
        for change in changes {
            let next = seq + 1;
            if upsert_if_newer(&mut tx, &user_key, change, next).await? {
                seq = next;
                winners.push(Sequenced { seq: to_cursor(seq), change: change.clone() });
            }
        }
        sqlx::query("UPDATE users SET last_seq = ? WHERE id = ?").bind(seq).bind(&user_key).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(winners)
    }

    /// Every stored field of the user: their whole account state, e.g. to build a snapshot.
    pub async fn all_changes(&self, user: UserId) -> sqlx::Result<Vec<FieldChange>> {
        let page = self.changes_after(user, Cursor(0), usize::MAX).await?;
        Ok(page.into_iter().map(|sequenced| sequenced.change).collect())
    }

    /// Up to `limit` stored changes after `cursor`, in sequence order.
    pub async fn changes_after(&self, user: UserId, cursor: Cursor, limit: usize) -> sqlx::Result<Vec<Sequenced>> {
        let rows = sqlx::query(
            "SELECT kind, entity_id, field, value_json, hlc, seq FROM fields
             WHERE user_id = ? AND seq > ? ORDER BY seq LIMIT ?",
        )
        .bind(user.0.to_string())
        .bind(i64::try_from(cursor.0).unwrap_or(i64::MAX))
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(decode_sequenced).collect()
    }
}

/// Writes the change if it supersedes the stored one, mirroring `FieldChange::supersedes`: newer
/// HLC wins; on equal HLCs the greater serialised value wins. HLC strings are fixed-width hex and
/// SQLite compares text bytewise, so text comparison matches the Rust order.
async fn upsert_if_newer(
    tx: &mut Transaction<'_, Sqlite>,
    user_key: &str,
    change: &FieldChange,
    seq: i64,
) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "INSERT INTO fields (user_id, kind, entity_id, field, value_json, hlc, seq) VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (user_id, kind, entity_id, field) DO UPDATE
         SET value_json = excluded.value_json, hlc = excluded.hlc, seq = excluded.seq
         WHERE excluded.hlc > fields.hlc
            OR (excluded.hlc = fields.hlc AND excluded.value_json > fields.value_json)",
    )
    .bind(user_key)
    .bind(change.kind.as_str())
    .bind(change.id.to_string())
    .bind(&change.field)
    .bind(change.value.to_string())
    .bind(change.hlc.to_string())
    .bind(seq)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() > 0)
}

fn to_cursor(seq: i64) -> Cursor {
    Cursor(u64::try_from(seq).unwrap_or(0))
}

fn decode_user_id(row: &SqliteRow, column: &str) -> sqlx::Result<UserId> {
    let raw: String = row.try_get(column)?;
    Uuid::parse_str(&raw).map(UserId).map_err(|error| corrupt(column, error))
}

fn decode_sequenced(row: &SqliteRow) -> sqlx::Result<Sequenced> {
    let kind: String = row.try_get("kind")?;
    let id: String = row.try_get("entity_id")?;
    let value: String = row.try_get("value_json")?;
    let hlc: String = row.try_get("hlc")?;
    let change = FieldChange {
        kind: EntityKind::from_str(&kind).map_err(|error| corrupt("kind", error))?,
        id: EntityId::from_str(&id).map_err(|error| corrupt("entity_id", error))?,
        field: row.try_get("field")?,
        value: serde_json::from_str(&value).map_err(|error| corrupt("value_json", error))?,
        hlc: Hlc::from_str(&hlc).map_err(|error| corrupt("hlc", error))?,
    };
    Ok(Sequenced { seq: to_cursor(row.try_get("seq")?), change })
}

fn corrupt(column: &str, error: impl std::fmt::Display) -> sqlx::Error {
    sqlx::Error::Decode(format!("stored column {column} is invalid: {error}").into())
}
