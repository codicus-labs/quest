//! Deleting safely, pure: what can be deleted (`Deletable`), and the undo state behind the toast
//! at the bottom ("Deleted “…” · Undo" or "Completed quest “…” · Undo"). `UndoState::step`
//! decides; the app performs the `UndoEffect` it returns. Time is passed in.

#[cfg(test)]
mod tests;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use quest_domain::{
    Command, EntityId, EntityKind, PreviousTaskOrder, QuestStatus, Snapshot, Task, TaskDates, TaskList,
};

/// How long a toast shows.
pub const TOAST_SECONDS: i64 = 6;
pub const LINGER_SECONDS: i64 = 4;

pub fn linger_until(completed_at: DateTime<Utc>) -> DateTime<Utc> {
    completed_at + Duration::seconds(LINGER_SECONDS)
}

pub fn lingering(local_completion: DateTime<Utc>, task: &Task, now: DateTime<Utc>) -> bool {
    task.completed_at == Some(local_completion) && now < linger_until(local_completion)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskEdit {
    Quest {
        id: EntityId,
        previous: Option<EntityId>,
    },
    Dates {
        id: EntityId,
        dates: Option<TaskDates>,
        sort_key: Option<String>,
        day_order: Option<String>,
        rekeyed: Vec<PreviousTaskOrder>,
        quest_id: Option<Option<EntityId>>,
        postponements: Vec<EntityId>,
    },
    Completion {
        id: EntityId,
        previous: Option<DateTime<Utc>>,
        previous_plan: Option<(TaskDates, String)>,
    },
}

impl TaskEdit {
    pub fn dates(before: &Task, after: &Task, old: &Snapshot, new: &Snapshot) -> Option<Self> {
        (before.dates() != after.dates()
            || before.sort_key != after.sort_key
            || before.day_order != after.day_order
            || before.quest_id != after.quest_id)
            .then(|| Self::Dates {
                id: before.id,
                dates: (before.dates() != after.dates()).then(|| before.dates()),
                sort_key: (before.sort_key != after.sort_key).then(|| before.sort_key.clone()),
                day_order: (before.day_order != after.day_order).then(|| before.day_order.clone()),
                rekeyed: Vec::new(),
                quest_id: (before.quest_id != after.quest_id).then_some(before.quest_id),
                postponements: new
                    .postponements_of(before.id)
                    .filter(|p| !old.postponements_of(before.id).any(|old| old.id == p.id))
                    .map(|p| p.id)
                    .collect(),
            })
    }

    pub fn placement(
        before: &Task,
        after: &Task,
        old: &Snapshot,
        new: &Snapshot,
        list: TaskList,
        today: NaiveDate,
    ) -> Option<Self> {
        let mut edit = Self::dates(before, after, old, new)?;
        if let Self::Dates { rekeyed, .. } = &mut edit {
            let new_keys: std::collections::HashMap<_, _> = new
                .tasks
                .iter()
                .filter(|task| list.contains(task, today))
                .map(|task| (task.id, list.key(task)))
                .collect();
            rekeyed.extend(old.tasks.iter().filter_map(|task| {
                let key = list.key(task);
                (task.id != before.id
                    && list.contains(task, today)
                    && new_keys.get(&task.id).is_some_and(|updated| *updated != key))
                .then(|| PreviousTaskOrder { id: task.id, list, key: key.to_owned() })
            }));
        }
        Some(edit)
    }

    pub fn completion(before: &Task, after: &Task) -> Option<Self> {
        (before.completed_at != after.completed_at).then(|| Self::Completion {
            id: before.id,
            previous: before.completed_at,
            previous_plan: (before.dates() != after.dates()).then(|| (before.dates(), before.day_order.clone())),
        })
    }

    pub fn command(self, at: DateTime<Utc>) -> Command {
        match self {
            Self::Quest { id, previous } => Command::AssignTask { id, quest_id: previous },
            Self::Dates { id, dates, sort_key, day_order, rekeyed, quest_id, postponements } => {
                Command::RestoreTaskEdit {
                    id,
                    dates,
                    sort_key,
                    day_order,
                    rekeyed,
                    quest_id,
                    completed_at: None,
                    postponements,
                    at,
                }
            }
            Self::Completion { id, previous, previous_plan } => {
                let (dates, day_order) = previous_plan.unzip();
                Command::RestoreTaskEdit {
                    id,
                    dates,
                    sort_key: None,
                    day_order,
                    rekeyed: Vec::new(),
                    quest_id: None,
                    completed_at: Some(previous),
                    postponements: Vec::new(),
                    at,
                }
            }
        }
    }
}

/// Something the user can delete (softly: it goes to the archive and can be restored).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deletable {
    Task(EntityId),
    Quest(EntityId),
    ChecklistItem(EntityId),
    Entry(EntityId),
}

impl Deletable {
    pub fn kind(self) -> EntityKind {
        match self {
            Deletable::Task(_) => EntityKind::Task,
            Deletable::Quest(_) => EntityKind::Quest,
            Deletable::ChecklistItem(_) => EntityKind::ChecklistItem,
            Deletable::Entry(_) => EntityKind::TimeEntry,
        }
    }

    pub fn id(self) -> EntityId {
        match self {
            Deletable::Task(id) | Deletable::Quest(id) | Deletable::ChecklistItem(id) | Deletable::Entry(id) => id,
        }
    }

    pub fn from_kind(kind: EntityKind, id: EntityId) -> Option<Deletable> {
        match kind {
            EntityKind::Task => Some(Deletable::Task(id)),
            EntityKind::Quest => Some(Deletable::Quest(id)),
            EntityKind::ChecklistItem => Some(Deletable::ChecklistItem(id)),
            EntityKind::TimeEntry => Some(Deletable::Entry(id)),
            EntityKind::Postponement | EntityKind::Settings | EntityKind::Workspace => None,
        }
    }

    pub fn delete(self, at: DateTime<Utc>) -> Command {
        match self {
            Deletable::Task(id) => Command::DeleteTask { id, at },
            Deletable::Quest(id) => Command::DeleteQuest { id, at },
            Deletable::ChecklistItem(id) => Command::DeleteChecklistItem { id, at },
            Deletable::Entry(id) => Command::DeleteEntry { id, at },
        }
    }

    pub fn restore(self) -> Command {
        Command::Restore { kind: self.kind(), id: self.id() }
    }
}

/// What ⌘Z takes back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Undoable {
    Delete(Deletable),
    Task(TaskEdit),
    /// A quest was completed or reopened: it goes back to `back_to`.
    QuestStatus {
        quest: EntityId,
        back_to: QuestStatus,
    },
}

/// What the toast says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Toast {
    Done,
    DateChanged,
    QuestChanged,
    Deleted {
        item: Deletable,
        title: String,
    },
    /// A quest was completed or reopened; `status` is the one it has now.
    QuestStatus {
        quest: EntityId,
        title: String,
        status: QuestStatus,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub toast: Toast,
    pub until: DateTime<Utc>,
}

/// The last undoable change (⌘Z takes it back, also after its toast is gone) and the toast on
/// screen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UndoState {
    pub last: Option<Undoable>,
    pub shown: Option<Shown>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UndoEvent {
    TaskEdited(TaskEdit),
    TaskPlaced(TaskEdit),
    Deleted {
        item: Deletable,
        title: String,
    },
    /// A quest went `from` → `to` (completed or reopened).
    QuestStatusChanged {
        quest: EntityId,
        title: String,
        from: QuestStatus,
        to: QuestStatus,
    },
    /// ⌘Z or the toast's Undo.
    Undo,
    /// Restored some other way (the archive): ⌘Z no longer refers to it.
    Restored(Deletable),
    Dismiss,
    /// Time passed: the toast goes once its time is up.
    Tick,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UndoEffect {
    None,
    Task(TaskEdit),
    Restore(Deletable),
    SetQuestStatus { quest: EntityId, status: QuestStatus },
}

impl UndoState {
    pub fn step(&self, event: UndoEvent, now: DateTime<Utc>) -> (UndoState, UndoEffect) {
        let until = now + Duration::seconds(TOAST_SECONDS);
        let mut next = self.clone();
        let effect = match event {
            UndoEvent::TaskEdited(edit) => {
                next.shown = matches!(edit, TaskEdit::Completion { previous: None, .. })
                    .then_some(Shown { toast: Toast::Done, until: now + Duration::seconds(5) });
                next.last = Some(Undoable::Task(edit));
                UndoEffect::None
            }
            UndoEvent::TaskPlaced(edit) => {
                let toast = match &edit {
                    TaskEdit::Dates { dates: Some(_), .. } => Some(Toast::DateChanged),
                    TaskEdit::Dates { quest_id: Some(_), .. } => Some(Toast::QuestChanged),
                    _ => None,
                };
                next.shown = toast.map(|toast| Shown { toast, until });
                next.last = Some(Undoable::Task(edit));
                UndoEffect::None
            }
            UndoEvent::Deleted { item, title } => {
                next.last = Some(Undoable::Delete(item));
                next.shown = Some(Shown { toast: Toast::Deleted { item, title }, until });
                UndoEffect::None
            }
            UndoEvent::QuestStatusChanged { quest, title, from, to } => {
                next.last = Some(Undoable::QuestStatus { quest, back_to: from });
                next.shown = Some(Shown { toast: Toast::QuestStatus { quest, title, status: to }, until });
                UndoEffect::None
            }
            UndoEvent::Undo => match next.last.take() {
                None => UndoEffect::None,
                Some(Undoable::Task(edit)) => {
                    next.shown = None;
                    UndoEffect::Task(edit)
                }
                Some(Undoable::Delete(item)) => {
                    next.forget_toast_about(item);
                    UndoEffect::Restore(item)
                }
                Some(Undoable::QuestStatus { quest, back_to }) => {
                    if matches!(&next.shown, Some(Shown { toast: Toast::QuestStatus { quest: shown, .. }, .. }) if *shown == quest)
                    {
                        next.shown = None;
                    }
                    UndoEffect::SetQuestStatus { quest, status: back_to }
                }
            },
            UndoEvent::Restored(item) => {
                if next.last == Some(Undoable::Delete(item)) {
                    next.last = None;
                }
                next.forget_toast_about(item);
                UndoEffect::None
            }
            UndoEvent::Dismiss => {
                next.shown = None;
                UndoEffect::None
            }
            UndoEvent::Tick => {
                next.shown = next.shown.filter(|s| now < s.until);
                UndoEffect::None
            }
        };
        (next, effect)
    }

    /// When the toast on screen goes by itself, if one shows.
    pub fn expires(&self) -> Option<DateTime<Utc>> {
        self.shown.as_ref().map(|s| s.until)
    }

    fn forget_toast_about(&mut self, item: Deletable) {
        if matches!(&self.shown, Some(Shown { toast: Toast::Deleted { item: shown, .. }, .. }) if *shown == item) {
            self.shown = None;
        }
    }
}
