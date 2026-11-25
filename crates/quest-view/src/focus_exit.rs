use chrono::{DateTime, TimeDelta, Utc};
use quest_domain::{Command, EntityId, EntryKind, Snapshot, TimeEntry};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FocusExit(Option<Departure>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Departure {
    entry: EntityId,
    since: DateTime<Utc>,
    kept: bool,
}

impl FocusExit {
    pub fn leave(&mut self, snap: &Snapshot, at: DateTime<Utc>) {
        self.0 =
            running_task(snap).map(|entry| Departure { entry: entry.id, since: at.max(entry.started_at), kept: false });
    }

    pub fn open(&mut self) {
        self.0 = None;
    }

    pub fn departure(&self, snap: &Snapshot) -> Option<(EntityId, DateTime<Utc>)> {
        let left = self.0?;
        let running = running_task(snap)?;
        (running.id == left.entry && running.started_at <= left.since).then_some((left.entry, left.since))
    }

    pub fn hint(&self, snap: &Snapshot, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let (_, since) = self.departure(snap)?;
        (!self.0?.kept && now - since >= TimeDelta::minutes(2)).then_some(since)
    }

    pub fn keep(&mut self, snap: &Snapshot, now: DateTime<Utc>) {
        if self.hint(snap, now).is_some()
            && let Some(left) = &mut self.0
        {
            left.kept = true;
        }
    }

    pub fn book(&self, snap: &Snapshot, entry_id: EntityId, now: DateTime<Utc>) -> Option<Command> {
        Some(Command::RebookAsOrganizing {
            entry_id,
            expected_task_entry: Some(self.departure(snap)?.0),
            since: self.hint(snap, now)?,
            now,
        })
    }
}

fn running_task(snap: &Snapshot) -> Option<&TimeEntry> {
    snap.running_entry().filter(|entry| entry.kind == EntryKind::Task && entry.task_id.is_some())
}

pub fn organize(snap: &Snapshot, entry_id: EntityId, at: DateTime<Utc>) -> Option<Command> {
    Some(Command::StartOrganizing { entry_id, origin_task_id: running_task(snap)?.task_id, at })
}

pub fn back_to_task(snap: &Snapshot, entry_id: EntityId, at: DateTime<Utc>) -> Option<Command> {
    let running = snap.running_entry().filter(|entry| entry.kind == EntryKind::Organizing)?;
    let task = snap.task(running.origin_task_id?).filter(|task| !task.is_done())?;
    Some(Command::StartFocus { entry_id, task_id: task.id, at })
}
