//! A tiny in-memory replica for tests: runs commands like the store does (plan, stamp, merge).

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use quest_domain::{
    Command, CommandError, EntityId, FieldStore, HlcClock, NodeId, Position, QuestColor, Snapshot, plan,
};
use quest_protocol::CaptureRequest;

pub(crate) const BERLIN: Tz = chrono_tz::Europe::Berlin;

/// Tuesday, 2026-11-24, 10:30 in Berlin.
pub(crate) fn now() -> DateTime<Utc> {
    "2026-11-24T09:30:00Z".parse().unwrap()
}

pub(crate) fn day(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

pub(crate) struct World {
    pub store: FieldStore,
    /// The workspace commands and requests use.
    pub workspace: EntityId,
    clock: HlcClock,
    /// Ids handed out by `id()`, in order, so tests can name them.
    next: u128,
}

impl World {
    pub fn new() -> World {
        World {
            store: FieldStore::default(),
            workspace: EntityId::DEFAULT_WORKSPACE,
            clock: HlcClock::new(NodeId(1)),
            next: 1,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot::from_store(&self.store, self.workspace)
    }

    /// Deterministic ids: 00000000-0000-0000-0000-000000000001, …
    pub fn id(&mut self) -> EntityId {
        let id = EntityId(uuid::Uuid::from_u128(self.next));
        self.next += 1;
        id
    }

    pub fn run(&mut self, commands: &[Command]) -> Result<(), CommandError> {
        for command in commands {
            let writes = plan(command, &self.snapshot())?;
            for write in writes {
                let change = write.stamp(self.clock.now(now().timestamp_millis() as u64));
                self.store.apply(&change);
            }
        }
        Ok(())
    }

    pub fn quest(&mut self, title: &str) -> EntityId {
        let id = self.id();
        self.run(&[Command::CreateQuest { id, title: title.into(), color: QuestColor::Red }]).unwrap();
        id
    }

    pub fn task(&mut self, title: &str, quest_id: Option<EntityId>, on: Option<NaiveDate>) -> EntityId {
        let id = self.id();
        let create = Command::CreateTask {
            id,
            title: title.into(),
            quest_id,
            planned_on: on,
            due_on: None,
            position: Position::Bottom,
        };
        self.run(&[create]).unwrap();
        id
    }

    pub fn request(&self, text: &str) -> CaptureRequest {
        crate::request(text, now(), BERLIN, "de-DE", &self.snapshot())
    }
}
