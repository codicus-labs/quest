use quest_domain::{EntityId, Position, Snapshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub done: u32,
    pub total: u32,
}

impl std::fmt::Display for Progress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.done, self.total)
    }
}

pub fn progress(snapshot: &Snapshot, task: EntityId) -> Option<Progress> {
    let (mut done, mut total) = (0u32, 0u32);
    for item in snapshot.checklist_of(task) {
        done = done.saturating_add(u32::from(item.done));
        total = total.saturating_add(1);
    }
    (total != 0).then_some(Progress { done, total })
}

pub fn after(snapshot: &Snapshot, item: EntityId) -> Option<Position> {
    let task = snapshot.checklist_item(item)?.task_id;
    let mut items = snapshot.checklist_of(task);
    items.find(|candidate| candidate.id == item)?;
    Some(items.next().map_or(Position::Bottom, |next| Position::Before(next.id)))
}

pub fn move_one(snapshot: &Snapshot, item: EntityId, down: bool) -> Option<Position> {
    let task = snapshot.checklist_item(item)?.task_id;
    let mut items = snapshot.checklist_of(task);
    if down {
        items.find(|candidate| candidate.id == item)?;
        return after(snapshot, items.next()?.id);
    }
    let mut previous = None;
    for candidate in items {
        if candidate.id == item {
            return previous.map(Position::Before);
        }
        previous = Some(candidate.id);
    }
    None
}

#[cfg(test)]
#[path = "checklist_tests.rs"]
mod tests;
