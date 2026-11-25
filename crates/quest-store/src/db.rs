//! SQLite adapter: the only place that knows the local schema. Runs on the worker thread,
//! except for the initial load in `Store::open`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use quest_domain::{EntityId, FieldChange, FieldStore, Hlc, NodeId};
use quest_protocol::{AccountInfo, Cursor, Epoch};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;

use crate::error::StoreError;

/// The database file in a profile's folder.
pub(crate) const FILE_NAME: &str = "quest.sqlite";

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS fields (
    kind TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    field TEXT NOT NULL,
    value_json TEXT NOT NULL,
    hlc TEXT NOT NULL,
    PRIMARY KEY (kind, entity_id, field)
);
CREATE TABLE IF NOT EXISTS outbox (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    change_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS outbox_rejected (
    seq INTEGER PRIMARY KEY,
    change_json TEXT NOT NULL,
    reason TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

/// Last writer wins: keeps the stored value unless the incoming timestamp is higher, or equal
/// with a greater serialised value (the same tie-break as `FieldChange::supersedes`). HLC
/// strings are fixed-width hex, so text comparison equals timestamp order.
const UPSERT_FIELD: &str = "
INSERT INTO fields (kind, entity_id, field, value_json, hlc) VALUES (?1, ?2, ?3, ?4, ?5)
ON CONFLICT (kind, entity_id, field) DO UPDATE SET value_json = excluded.value_json, hlc = excluded.hlc
WHERE excluded.hlc > fields.hlc OR (excluded.hlc = fields.hlc AND excluded.value_json > fields.value_json)
";

const SET_META: &str =
    "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value";

/// The cursor only moves forward while syncing.
const ADVANCE_CURSOR: &str = "
INSERT INTO meta (key, value) VALUES ('cursor', ?1) ON CONFLICT (key) DO UPDATE SET value = excluded.value
WHERE CAST(excluded.value AS INTEGER) > CAST(meta.value AS INTEGER)
";

/// Queues every field this node wrote last, with its original timestamp. The server only
/// accepts a node's own changes; other devices re-push theirs. The node is the HLC's last
/// 16 hex digits.
const ENQUEUE_OWN_FIELDS: &str = "
INSERT INTO outbox (change_json)
SELECT json_object('kind', kind, 'id', entity_id, 'field', field, 'value', json(value_json), 'hlc', hlc)
FROM fields WHERE substr(hlc, -16) = ?1 ORDER BY hlc
";

const NODE_KEY: &str = "node_id";
const CURSOR_KEY: &str = "cursor";
const EPOCH_KEY: &str = "epoch";
/// The account's last known `AccountInfo`, for showing it before the server answers.
const ACCOUNT_INFO_KEY: &str = "account_info";
/// The workspace this device last showed for this profile.
const WORKSPACE_KEY: &str = "workspace";
/// Which entity kinds the stored cursor's catch-up understood. Earlier versions skipped
/// workspaces they did not know yet, so a store from before them catches up from the start
/// once (merging is idempotent).
const KINDS_KEY: &str = "known_kinds";
const KINDS_WITH_WORKSPACES: &str = "workspaces";

const BUSY_TIMEOUT: Duration = Duration::from_secs(1);

pub(crate) struct Loaded {
    pub node: NodeId,
    /// The remembered workspace; the default one if none is.
    pub workspace: EntityId,
    pub account_info: Option<AccountInfo>,
    pub fields: FieldStore,
    pub pending: usize,
    pub rejected: usize,
}

/// One outbox row: its sequence number and the change waiting for the server's ack.
pub(crate) type OutboxEntry = (i64, FieldChange);

pub(crate) struct Db {
    conn: Connection,
    path: PathBuf,
}

impl Db {
    pub fn open(data_dir: &Path) -> Result<Db, StoreError> {
        let path = data_dir.join(FILE_NAME);
        let fail = |message: String| StoreError::Storage { path: path.display().to_string(), message };
        std::fs::create_dir_all(data_dir).map_err(|e| fail(e.to_string()))?;
        let conn = Connection::open(&path).map_err(|e| fail(e.to_string()))?;
        let mut db = Db { conn, path };
        db.conn.busy_timeout(BUSY_TIMEOUT).map_err(|e| db.error(e))?;
        db.conn.pragma_update(None, "journal_mode", "WAL").map_err(|e| db.error(e))?;
        db.conn.pragma_update(None, "synchronous", "NORMAL").map_err(|e| db.error(e))?;
        db.conn.execute_batch(SCHEMA).map_err(|e| db.error(e))?;
        if db.meta(KINDS_KEY)?.as_deref() != Some(KINDS_WITH_WORKSPACES) {
            db.write(|tx| {
                set_meta(tx, CURSOR_KEY, "0")?;
                set_meta(tx, KINDS_KEY, KINDS_WITH_WORKSPACES)
            })?;
        }
        Ok(db)
    }

    fn error(&self, error: impl std::fmt::Display) -> StoreError {
        storage_error(&self.path, error)
    }

    /// Runs `work` in one transaction.
    fn write<T>(&mut self, work: impl FnOnce(&Transaction<'_>) -> rusqlite::Result<T>) -> Result<T, StoreError> {
        let path = self.path.clone();
        let tx = self.conn.transaction().map_err(|e| storage_error(&path, e))?;
        let output = work(&tx).map_err(|e| storage_error(&path, e))?;
        tx.commit().map_err(|e| storage_error(&path, e))?;
        Ok(output)
    }

    /// Reads everything needed to start: creates the node id on first use.
    pub fn load(&mut self) -> Result<Loaded, StoreError> {
        let node = match self.meta(NODE_KEY)? {
            Some(raw) => NodeId(raw.parse().map_err(|e| self.error(format!("corrupt node id: {e}")))?),
            None => {
                let node = NodeId(rand::random());
                self.write(|tx| set_meta(tx, NODE_KEY, &node.0.to_string()))?;
                node
            }
        };
        let account_info = self.meta_json(ACCOUNT_INFO_KEY)?;
        let workspace = self.meta_json(WORKSPACE_KEY)?.unwrap_or(EntityId::DEFAULT_WORKSPACE);
        let fields = self.load_fields()?;
        let pending = self.count("outbox")?;
        let rejected = self.count("outbox_rejected")?;
        Ok(Loaded { node, workspace, account_info, fields, pending, rejected })
    }

    pub fn set_workspace(&mut self, workspace: EntityId) -> Result<(), StoreError> {
        self.write(|tx| set_meta(tx, WORKSPACE_KEY, &to_json(&workspace)?))
    }

    fn count(&self, table: &str) -> Result<usize, StoreError> {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let count: i64 = self.conn.query_row(&sql, [], |row| row.get(0)).map_err(|e| self.error(e))?;
        Ok(count as usize)
    }

    fn load_fields(&self) -> Result<FieldStore, StoreError> {
        let mut statement = self
            .conn
            .prepare("SELECT kind, entity_id, field, value_json, hlc FROM fields")
            .map_err(|e| self.error(e))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| self.error(e))?;
        let mut fields = FieldStore::default();
        for row in rows {
            let (kind, id, field, value, hlc) = row.map_err(|e| self.error(e))?;
            let corrupt = |what: &str, e: &dyn std::fmt::Display| self.error(format!("corrupt {what} in fields: {e}"));
            let change = FieldChange {
                kind: kind.parse().map_err(|e: String| corrupt("kind", &e))?,
                id: id.parse::<EntityId>().map_err(|e| corrupt("entity id", &e))?,
                field,
                value: serde_json::from_str(&value).map_err(|e| corrupt("value", &e))?,
                hlc: hlc.parse::<Hlc>().map_err(|e| corrupt("hlc", &e))?,
            };
            fields.apply(&change);
        }
        Ok(fields)
    }

    /// Stores local changes and, if the profile syncs, queues them for the server, atomically.
    pub fn persist_local(&mut self, changes: &[FieldChange], queue: bool) -> Result<(), StoreError> {
        self.write(|tx| {
            upsert_fields(tx, changes)?;
            if !queue {
                return Ok(());
            }
            let mut insert = tx.prepare_cached("INSERT INTO outbox (change_json) VALUES (?1)")?;
            for change in changes {
                insert.execute([to_json(change)?])?;
            }
            Ok(())
        })
    }

    /// Merges changes from the server and records how far we have applied, atomically.
    pub fn apply_remote(&mut self, changes: &[FieldChange], cursor: Cursor) -> Result<(), StoreError> {
        self.write(|tx| {
            upsert_fields(tx, changes)?;
            tx.execute(ADVANCE_CURSOR, [cursor.0.to_string()])?;
            Ok(())
        })
    }

    /// Where to resume syncing: the cursor and the server epoch it belongs to.
    pub fn sync_position(&self) -> Result<(Cursor, Option<Epoch>), StoreError> {
        let cursor = self.meta(CURSOR_KEY)?.and_then(|raw| raw.parse().ok()).map(Cursor).unwrap_or_default();
        let epoch = self.meta(EPOCH_KEY)?.and_then(|raw| raw.parse().ok()).map(Epoch);
        Ok((cursor, epoch))
    }

    /// Records the server's epoch. On a cursor reset, restarts from zero and queues this node's
    /// full local state, since the server may have lost acknowledged changes. Returns how many
    /// changes were queued.
    pub fn welcome(&mut self, epoch: Epoch, cursor_reset: bool, node: NodeId) -> Result<usize, StoreError> {
        self.write(|tx| {
            set_meta(tx, EPOCH_KEY, &epoch.0.to_string())?;
            if !cursor_reset {
                return Ok(0);
            }
            set_meta(tx, CURSOR_KEY, "0")?;
            tx.execute(ENQUEUE_OWN_FIELDS, [format!("{:016x}", node.0)])
        })
    }

    /// The oldest unacknowledged changes, at most `limit`.
    pub fn outbox_batch(&self, limit: usize) -> Result<Vec<OutboxEntry>, StoreError> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT seq, change_json FROM outbox ORDER BY seq LIMIT ?1")
            .map_err(|e| self.error(e))?;
        let rows = statement
            .query_map([limit as i64], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
            .map_err(|e| self.error(e))?;
        let mut batch = Vec::new();
        for row in rows {
            let (seq, json) = row.map_err(|e| self.error(e))?;
            let change =
                serde_json::from_str(&json).map_err(|e| self.error(format!("corrupt outbox row {seq}: {e}")))?;
            batch.push((seq, change));
        }
        Ok(batch)
    }

    pub fn remove_outbox(&mut self, seqs: &[i64]) -> Result<(), StoreError> {
        self.write(|tx| {
            let mut delete = tx.prepare_cached("DELETE FROM outbox WHERE seq = ?1")?;
            for seq in seqs {
                delete.execute([seq])?;
            }
            Ok(())
        })
    }

    /// Sets changes the server will never accept aside, so the rest of the outbox can flow.
    pub fn reject_outbox(&mut self, seqs: &[i64], reason: &str) -> Result<(), StoreError> {
        self.write(|tx| {
            let mut keep = tx.prepare_cached(
                "INSERT OR REPLACE INTO outbox_rejected (seq, change_json, reason) SELECT seq, change_json, ?2 FROM outbox WHERE seq = ?1",
            )?;
            let mut delete = tx.prepare_cached("DELETE FROM outbox WHERE seq = ?1")?;
            for seq in seqs {
                keep.execute(params![seq, reason])?;
                delete.execute([seq])?;
            }
            Ok(())
        })
    }

    pub fn set_account_info(&mut self, info: &AccountInfo) -> Result<(), StoreError> {
        self.write(|tx| set_meta(tx, ACCOUNT_INFO_KEY, &to_json(info)?))
    }

    /// Deletes all user data and sync state. Keeps the node id.
    pub fn reset_local_data(&mut self) -> Result<(), StoreError> {
        self.write(|tx| {
            tx.execute_batch("DELETE FROM fields; DELETE FROM outbox; DELETE FROM outbox_rejected;")?;
            tx.execute("DELETE FROM meta WHERE key <> ?1", [NODE_KEY])?;
            Ok(())
        })
    }

    fn meta(&self, key: &str) -> Result<Option<String>, StoreError> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| row.get(0))
            .optional()
            .map_err(|e| self.error(e))
    }

    fn meta_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>, StoreError> {
        let Some(raw) = self.meta(key)? else { return Ok(None) };
        serde_json::from_str(&raw).map(Some).map_err(|e| self.error(format!("corrupt {key}: {e}")))
    }
}

/// Copies the database at `legacy` (including its write-ahead log) to `target` as a guest store:
/// the data stays, the sync state, session and account binding of earlier versions do not.
pub(crate) fn copy_as_guest(legacy: &Path, target: &Path) -> Result<(), StoreError> {
    let conn = Connection::open(legacy).map_err(|e| storage_error(legacy, e))?;
    conn.busy_timeout(BUSY_TIMEOUT).map_err(|e| storage_error(legacy, e))?;
    let target_text = target.to_str().ok_or_else(|| storage_error(target, "the path is not valid UTF-8"))?;
    conn.execute("VACUUM INTO ?1", [target_text]).map_err(|e| storage_error(legacy, e))?;
    drop(conn);
    let copy = Connection::open(target).map_err(|e| storage_error(target, e))?;
    copy.execute_batch(SCHEMA).map_err(|e| storage_error(target, e))?;
    copy.execute_batch("DELETE FROM outbox; DELETE FROM outbox_rejected;").map_err(|e| storage_error(target, e))?;
    copy.execute("DELETE FROM meta WHERE key <> ?1", [NODE_KEY]).map_err(|e| storage_error(target, e))?;
    Ok(())
}

fn storage_error(path: &Path, error: impl std::fmt::Display) -> StoreError {
    StoreError::Storage { path: path.display().to_string(), message: error.to_string() }
}

fn set_meta(tx: &Transaction<'_>, key: &str, value: &str) -> rusqlite::Result<()> {
    tx.execute(SET_META, params![key, value]).map(|_| ())
}

fn to_json(value: &impl Serialize) -> rusqlite::Result<String> {
    serde_json::to_string(value).map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
}

fn upsert_fields(tx: &Transaction<'_>, changes: &[FieldChange]) -> rusqlite::Result<()> {
    let mut upsert = tx.prepare_cached(UPSERT_FIELD)?;
    for change in changes {
        upsert.execute(params![
            change.kind.as_str(),
            change.id.to_string(),
            change.field,
            to_json(&change.value)?,
            change.hlc.to_string()
        ])?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "db_tests.rs"]
mod tests;
