use chrono::{DateTime, NaiveDate, Utc};
use serde_json::json;

use super::{CommandError, Position, task_dates::date_writes, write};
use crate::model::fields;
use crate::{EntityId, EntityKind, FieldWrite, Snapshot, Task, is_inbox, is_today, set_planned, sort_key_between};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TaskList {
    Today,
    Day(NaiveDate),
    Inbox,
    Quest(EntityId),
}

impl TaskList {
    pub fn contains(self, task: &Task, today: NaiveDate) -> bool {
        !task.is_done()
            && match self {
                Self::Today => is_today(task, today),
                Self::Day(day) => task.planned_on == Some(day),
                Self::Inbox => is_inbox(task),
                Self::Quest(quest) => task.quest_id == Some(quest),
            }
    }

    pub fn key(self, task: &Task) -> &str {
        match self {
            Self::Today | Self::Day(_) => &task.day_order,
            Self::Inbox | Self::Quest(_) => &task.sort_key,
        }
    }

    fn field(self) -> &'static str {
        match self {
            Self::Today | Self::Day(_) => fields::TASK_DAY_ORDER,
            Self::Inbox | Self::Quest(_) => fields::SORT_KEY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    InList { list: TaskList, position: Position },
    FileIn(TaskList),
}

impl Placement {
    pub fn list(self) -> TaskList {
        match self {
            Self::InList { list, .. } | Self::FileIn(list) => list,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviousTaskOrder {
    pub id: EntityId,
    pub list: TaskList,
    pub key: String,
}

impl PreviousTaskOrder {
    pub(super) fn write(&self) -> FieldWrite {
        write(EntityKind::Task, self.id, self.list.field(), json!(self.key))
    }
}

pub(super) fn append_day(snapshot: &Snapshot, id: EntityId) -> FieldWrite {
    // The destination can share its order with carried-over tasks from any earlier day.
    let last = snapshot.tasks.iter().filter(|t| t.id != id && !t.is_done()).map(|t| t.day_order.as_str()).max();
    write(EntityKind::Task, id, fields::TASK_DAY_ORDER, json!(sort_key_between(last, None)))
}

pub(super) fn order_writes(
    snapshot: &Snapshot,
    id: EntityId,
    list: TaskList,
    position: Position,
    today: NaiveDate,
) -> Vec<FieldWrite> {
    let mut siblings: Vec<_> = snapshot.tasks.iter().filter(|t| t.id != id && list.contains(t, today)).collect();
    siblings.sort_by_key(|t| (list.key(t), t.id));
    let index = match position {
        Position::Top => 0,
        Position::Bottom => siblings.len(),
        Position::Before(anchor) => siblings.iter().position(|task| task.id == anchor).unwrap_or(siblings.len()),
    };
    let lower = index.checked_sub(1).map(|i| list.key(siblings[i]));
    let upper = siblings.get(index).map(|task| list.key(task));
    let write_key = |id, key| write(EntityKind::Task, id, list.field(), json!(key));
    if let Some(tied) = lower.filter(|key| Some(*key) == upper) {
        // Only split the suffix of a tied run; all other positions still fit between their existing keys.
        let end =
            siblings[index..].iter().position(|task| list.key(task) != tied).map_or(siblings.len(), |i| index + i);
        let upper = siblings.get(end).map(|task| list.key(task));
        let mut key = sort_key_between(Some(tied), upper);
        let mut writes = vec![write_key(id, key.clone())];
        for sibling in &siblings[index..end] {
            key = sort_key_between(Some(&key), upper);
            writes.push(write_key(sibling.id, key.clone()));
        }
        return writes;
    }
    vec![write_key(id, sort_key_between(lower, upper))]
}

pub(super) fn place(
    snapshot: &Snapshot,
    id: EntityId,
    placement: Placement,
    today: NaiveDate,
    at: DateTime<Utc>,
) -> Result<Vec<FieldWrite>, CommandError> {
    let task = snapshot.task(id).filter(|t| !t.is_done()).ok_or(CommandError::UnknownTask(id))?;
    let (list, position, file) = match placement {
        Placement::InList { position: Position::Before(anchor), .. } if anchor == id => return Ok(Vec::new()),
        Placement::InList { list, position } => (list, position, false),
        Placement::FileIn(list) => (list, Position::Bottom, true),
    };
    let mut dates = task.dates();
    let mut quest = task.quest_id;
    if file || !list.contains(task, today) {
        match list {
            TaskList::Today => dates = set_planned(dates, Some(today)),
            TaskList::Day(day) => dates = set_planned(dates, Some(day)),
            TaskList::Inbox => {
                quest = None;
                dates = set_planned(dates, None);
            }
            TaskList::Quest(id) => {
                snapshot.quest(id).ok_or(CommandError::UnknownQuest(id))?;
                quest = Some(id);
            }
        }
    }
    let mut writes = date_writes(snapshot, id, task.dates(), dates, at);
    if quest != task.quest_id {
        writes.push(write(EntityKind::Task, id, fields::QUEST_ID, json!(quest)));
    }
    writes.retain(|w| w.field != list.field());
    writes.extend(order_writes(snapshot, id, list, position, today));
    Ok(writes)
}
