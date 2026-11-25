//! Fixtures shared by the unit tests: ids, instants and entities by number, frozen time.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, NaiveDate, Utc};
use quest_domain::{EntityId, EntryKind, Quest, QuestColor, QuestStatus, Task, TimeEntry};

use crate::time::Now;
use crate::types::Moment;
use crate::vault::{SavedTokens, TokenStore, TokenStoreError};

/// Tokens in memory, as the platform would keep them.
#[derive(Default)]
pub(crate) struct MemoryTokens(Mutex<HashMap<String, SavedTokens>>);

impl TokenStore for MemoryTokens {
    fn load(&self, key: String) -> Result<Option<SavedTokens>, TokenStoreError> {
        Ok(self.0.lock().unwrap().get(&key).cloned())
    }

    fn save(&self, key: String, tokens: SavedTokens) -> Result<(), TokenStoreError> {
        self.0.lock().unwrap().insert(key, tokens);
        Ok(())
    }

    fn delete(&self, key: String) -> Result<(), TokenStoreError> {
        self.0.lock().unwrap().remove(&key);
        Ok(())
    }
}

pub(crate) fn tokens() -> Arc<dyn TokenStore> {
    Arc::new(MemoryTokens::default())
}

pub(crate) fn id(n: u128) -> EntityId {
    EntityId(uuid::Uuid::from_u128(n))
}

pub(crate) fn at(rfc3339: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(rfc3339).unwrap().with_timezone(&Utc)
}

pub(crate) fn day(iso: &str) -> NaiveDate {
    iso.parse().unwrap()
}

/// `rfc3339` in Berlin.
pub(crate) fn berlin(rfc3339: &str) -> Now {
    Now::parse(&moment(rfc3339)).unwrap()
}

pub(crate) fn moment(rfc3339: &str) -> Moment {
    Moment { epoch_ms: at(rfc3339).timestamp_millis(), time_zone: "Europe/Berlin".into() }
}

pub(crate) fn quest(n: u128, title: &str) -> Quest {
    Quest {
        id: id(n),
        title: title.into(),
        color: QuestColor::Blue,
        status: QuestStatus::Active,
        sort_key: format!("{n}"),
    }
}

pub(crate) fn task(n: u128, title: &str, quest: Option<u128>, on: Option<&str>) -> Task {
    Task {
        id: id(n),
        title: title.into(),
        quest_id: quest.map(id),
        planned_on: on.map(day),
        due_on: None,
        sort_key: format!("{n:04}"),
        completed_at: None,
        description: String::new(),
        repeat: None,
        series: None,
    }
}

pub(crate) fn entry(n: u128, task: Option<u128>, quest: Option<u128>, from: &str, to: Option<&str>) -> TimeEntry {
    TimeEntry {
        id: id(n),
        kind: EntryKind::Task,
        task_id: task.map(id),
        quest_id: quest.map(id),
        origin_task_id: None,
        started_at: at(from),
        ended_at: to.map(at),
    }
}

/// Lists `snapshot`'s running entries as its running focus sessions, as `Snapshot::from_store`
/// does, for snapshots built by hand.
pub(crate) fn with_running(mut snapshot: quest_domain::Snapshot) -> quest_domain::Snapshot {
    let running = snapshot.time_entries.iter().filter(|e| e.is_running()).cloned().collect::<Vec<_>>();
    snapshot.running = running
        .into_iter()
        .rev()
        .map(|entry| quest_domain::RunningFocus {
            task_title: entry.task_id.and_then(|id| snapshot.task(id)).map(|t| t.title.clone()),
            workspace: snapshot.workspace,
            entry,
        })
        .collect();
    snapshot
}
