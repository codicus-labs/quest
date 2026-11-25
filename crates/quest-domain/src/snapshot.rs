use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};

use crate::change::{EntityId, EntityKind, FieldStore, FieldValues};
use crate::model::{Milestone, Quest, QuestStatus, Settings, Task, TimeEntry, Workspace, deleted_at};
use crate::workspace::{Placements, workspace_records};

/// A consistent, typed read model of one workspace, built from merged fields. Collections are
/// sorted for display: quests and tasks by sort key, milestones by date, time entries by start.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// The workspace everything below belongs to; commands create new records in it.
    pub workspace: EntityId,
    /// Every workspace of the account, deleted ones included: the default one first, then in
    /// the order they were created.
    pub workspaces: Vec<Workspace>,
    pub quests: Vec<Quest>,
    pub tasks: Vec<Task>,
    pub milestones: Vec<Milestone>,
    pub time_entries: Vec<TimeEntry>,
    pub settings: Settings,
    /// Soft-deleted records that can be restored, most recently deleted first.
    pub trash: Vec<Trashed>,
    /// The default workspace is deleted in storage but shown, because every workspace was
    /// deleted (see `workspace_records`); the next workspace command makes that stick.
    pub default_revived: bool,
    /// Every running focus session in any workspace, latest start first. One person has one
    /// timer: starting focus stops all of them, and every workspace shows the first one.
    pub running: Vec<RunningFocus>,
    /// When each task was last written: its creation or its latest change, from the newest
    /// field's HLC wall time.
    pub task_written: BTreeMap<EntityId, DateTime<Utc>>,
}

/// A running focus session, wherever it is.
#[derive(Clone, Debug, PartialEq)]
pub struct RunningFocus {
    pub entry: TimeEntry,
    pub workspace: EntityId,
    /// The task's title; `None` for other work.
    pub task_title: Option<String>,
}

/// A soft-deleted record, described for an archive/trash list.
#[derive(Clone, Debug, PartialEq)]
pub struct Trashed {
    pub kind: EntityKind,
    pub id: EntityId,
    /// The record's title; for sessions the task's title (or `None` for other work).
    pub title: Option<String>,
    /// The quest the record belongs to (a quest's own id for quests).
    pub quest_id: Option<EntityId>,
    pub deleted_at: DateTime<Utc>,
}

impl Default for Snapshot {
    /// An empty store: just the default workspace.
    fn default() -> Self {
        Snapshot::from_store(&FieldStore::default(), EntityId::DEFAULT_WORKSPACE)
    }
}

/// Live records of one kind in one workspace.
fn live<'a, T>(
    store: &'a FieldStore,
    placements: &'a Placements,
    kind: EntityKind,
    workspace: EntityId,
    build: impl Fn(EntityId, &FieldValues) -> Option<T> + 'a,
) -> impl Iterator<Item = T> + 'a {
    store
        .entities(kind)
        .filter(|(_, values)| deleted_at(values).is_none())
        .filter(move |(id, _)| placements.of(kind, *id) == workspace)
        .filter_map(move |(id, values)| build(id, values))
}

impl Snapshot {
    /// The read model of the `preferred` workspace, or of the default workspace if that one is
    /// deleted or unknown (`workspace` tells which it is).
    pub fn from_store(store: &FieldStore, preferred: EntityId) -> Self {
        let placements = Placements::new(store);
        let (workspaces, default_revived) = workspaces(store, &placements);
        let is_live = |id: EntityId| workspaces.iter().any(|w| w.id == id && !w.is_deleted());
        let workspace = if is_live(preferred) {
            preferred
        } else if is_live(EntityId::DEFAULT_WORKSPACE) {
            EntityId::DEFAULT_WORKSPACE
        } else {
            workspaces.iter().find(|w| !w.is_deleted()).map_or(EntityId::DEFAULT_WORKSPACE, |w| w.id)
        };
        let placements = &placements;
        let mut quests: Vec<Quest> =
            live(store, placements, EntityKind::Quest, workspace, Quest::from_fields).collect();
        quests.sort_by(|a, b| a.sort_key.cmp(&b.sort_key).then(a.id.cmp(&b.id)));
        let mut tasks: Vec<Task> = live(store, placements, EntityKind::Task, workspace, Task::from_fields).collect();
        tasks.sort_by(|a, b| a.sort_key.cmp(&b.sort_key).then(a.id.cmp(&b.id)));
        let mut milestones: Vec<Milestone> =
            live(store, placements, EntityKind::Milestone, workspace, Milestone::from_fields).collect();
        milestones.sort_by_key(|m| (m.achieved_on, m.id));
        let mut time_entries: Vec<TimeEntry> =
            live(store, placements, EntityKind::TimeEntry, workspace, TimeEntry::from_fields).collect();
        time_entries.sort_by_key(|e| (e.started_at, e.id));
        let settings = Settings::from_fields(store.get(EntityKind::Settings, workspace));
        let trash = trash(store, placements, workspace);
        let running = running(store, placements, &workspaces);
        let task_written =
            tasks.iter().filter_map(|t| Some((t.id, written(store.get(EntityKind::Task, t.id)?)?))).collect();
        Snapshot {
            workspace,
            workspaces,
            quests,
            tasks,
            milestones,
            time_entries,
            settings,
            trash,
            default_revived,
            running,
            task_written,
        }
    }

    pub fn workspace(&self, id: EntityId) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.id == id)
    }

    pub fn live_workspace(&self, id: EntityId) -> Option<&Workspace> {
        self.workspace(id).filter(|w| !w.is_deleted())
    }

    /// The workspace this snapshot shows.
    pub fn active_workspace(&self) -> &Workspace {
        self.workspace(self.workspace).unwrap_or(&self.workspaces[0])
    }

    /// Workspaces to switch between, in display order.
    pub fn live_workspaces(&self) -> impl Iterator<Item = &Workspace> {
        self.workspaces.iter().filter(|w| !w.is_deleted())
    }

    /// Deleted workspaces that can be restored, most recently deleted first.
    pub fn deleted_workspaces(&self) -> Vec<&Workspace> {
        let mut deleted: Vec<&Workspace> = self.workspaces.iter().filter(|w| w.is_deleted()).collect();
        deleted.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at).then(a.id.cmp(&b.id)));
        deleted
    }

    pub fn quest(&self, id: EntityId) -> Option<&Quest> {
        self.quests.iter().find(|q| q.id == id)
    }

    pub fn active_quests(&self) -> impl Iterator<Item = &Quest> {
        self.quests.iter().filter(|q| q.status == QuestStatus::Active)
    }

    pub fn completed_quests(&self) -> impl Iterator<Item = &Quest> {
        self.quests.iter().filter(|q| q.status == QuestStatus::Completed)
    }

    pub fn task(&self, id: EntityId) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Tasks planned for `day`, in display order.
    pub fn tasks_on(&self, day: NaiveDate) -> impl Iterator<Item = &Task> {
        self.tasks.iter().filter(move |t| t.day == Some(day))
    }

    /// Unscheduled tasks ("someday").
    pub fn backlog(&self) -> impl Iterator<Item = &Task> {
        self.tasks.iter().filter(|t| t.day.is_none())
    }

    pub fn milestones_of(&self, quest: EntityId) -> impl Iterator<Item = &Milestone> {
        self.milestones.iter().filter(move |m| m.quest_id == quest)
    }

    /// The running focus session, in whichever workspace it is. If two devices started one while
    /// offline, the later start wins.
    pub fn running_focus(&self) -> Option<&RunningFocus> {
        self.running.first()
    }

    /// The running focus session if it is in this workspace.
    pub fn running_entry(&self) -> Option<&TimeEntry> {
        self.running_focus().filter(|running| running.workspace == self.workspace).map(|running| &running.entry)
    }

    pub fn task_tracked_ms(&self, task: EntityId, now: DateTime<Utc>) -> i64 {
        self.time_entries.iter().filter(|e| e.task_id == Some(task)).map(|e| e.tracked_ms(now)).sum()
    }

    /// Milliseconds tracked within `[from, to)`, across all quests and other work.
    pub fn tracked_ms_between(&self, from: DateTime<Utc>, to: DateTime<Utc>, now: DateTime<Utc>) -> i64 {
        self.time_entries.iter().map(|e| overlap_ms(e, from, to, now)).sum()
    }
}

/// The wall time of a record's newest field write.
fn written(values: &FieldValues) -> Option<DateTime<Utc>> {
    let ms = values.values().map(|(_, hlc)| hlc.wall_ms).max()?;
    DateTime::from_timestamp_millis(i64::try_from(ms).ok()?)
}

/// Every workspace (see `workspace_records`) with what it holds.
fn workspaces(store: &FieldStore, placements: &Placements) -> (Vec<Workspace>, bool) {
    let (mut workspaces, revived) = workspace_records(store);
    let mut content = BTreeMap::new();
    let mut count = |kind: EntityKind, add: &dyn Fn(&mut crate::model::WorkspaceContent, &FieldValues)| {
        for (id, values) in store.entities(kind).filter(|(_, values)| deleted_at(values).is_none()) {
            add(content.entry(placements.of(kind, id)).or_default(), values);
        }
    };
    count(EntityKind::Quest, &|c, values| {
        c.quests += usize::from(Quest::from_fields(EntityId::DEFAULT_WORKSPACE, values).is_some())
    });
    count(EntityKind::Task, &|c, values| {
        c.tasks += usize::from(Task::from_fields(EntityId::DEFAULT_WORKSPACE, values).is_some())
    });
    count(EntityKind::TimeEntry, &|c, values| {
        let entry = TimeEntry::from_fields(EntityId::DEFAULT_WORKSPACE, values);
        if let Some((started_at, Some(ended_at))) = entry.map(|e| (e.started_at, e.ended_at)) {
            c.tracked_ms += (ended_at - started_at).num_milliseconds().max(0);
        }
    });
    for workspace in &mut workspaces {
        workspace.content = content.get(&workspace.id).copied().unwrap_or_default();
    }
    (workspaces, revived)
}

/// Running sessions in live workspaces. One in a workspace deleted elsewhere while offline stays
/// hidden with it until it is restored.
fn running(store: &FieldStore, placements: &Placements, workspaces: &[Workspace]) -> Vec<RunningFocus> {
    let live = |id: EntityId| workspaces.iter().any(|w| w.id == id && !w.is_deleted());
    let task_title = |id: EntityId| Task::from_fields(id, store.get(EntityKind::Task, id)?).map(|t| t.title);
    let mut running: Vec<RunningFocus> = store
        .entities(EntityKind::TimeEntry)
        .filter(|(_, values)| deleted_at(values).is_none())
        .filter_map(|(id, values)| TimeEntry::from_fields(id, values))
        .filter(TimeEntry::is_running)
        .map(|entry| RunningFocus {
            workspace: placements.of(EntityKind::TimeEntry, entry.id),
            task_title: entry.task_id.and_then(task_title),
            entry,
        })
        .filter(|running| live(running.workspace))
        .collect();
    running.sort_by(|a, b| b.entry.started_at.cmp(&a.entry.started_at).then(a.entry.id.cmp(&b.entry.id)));
    running
}

fn trash(store: &FieldStore, placements: &Placements, workspace: EntityId) -> Vec<Trashed> {
    let task_title = |id: Option<EntityId>| {
        let values = store.get(EntityKind::Task, id?)?;
        Task::from_fields(id?, values).map(|t| t.title)
    };
    // (title, quest) of a record, or None if it can't be materialised.
    type Described = Option<(Option<String>, Option<EntityId>)>;
    let describe = |kind: EntityKind, id: EntityId, values: &FieldValues| -> Described {
        match kind {
            EntityKind::Quest => Quest::from_fields(id, values).map(|q| (Some(q.title), Some(id))),
            EntityKind::Task => Task::from_fields(id, values).map(|t| (Some(t.title), t.quest_id)),
            EntityKind::Milestone => Milestone::from_fields(id, values).map(|m| (Some(m.title), Some(m.quest_id))),
            EntityKind::TimeEntry => TimeEntry::from_fields(id, values).map(|e| (task_title(e.task_id), e.quest_id)),
            EntityKind::Settings | EntityKind::Workspace => None,
        }
    };
    let mut trash: Vec<Trashed> = EntityKind::ALL
        .into_iter()
        .flat_map(|kind| store.entities(kind).map(move |(id, values)| (kind, id, values)))
        .filter_map(|(kind, id, values)| {
            let deleted_at = deleted_at(values)?;
            if placements.of(kind, id) != workspace {
                return None;
            }
            let (title, quest_id) = describe(kind, id, values)?;
            Some(Trashed { kind, id, title, quest_id, deleted_at })
        })
        .collect();
    trash.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at).then(a.id.cmp(&b.id)));
    trash
}

/// How much of an entry falls inside `[from, to)`.
pub(crate) fn overlap_ms(entry: &TimeEntry, from: DateTime<Utc>, to: DateTime<Utc>, now: DateTime<Utc>) -> i64 {
    let start = entry.started_at.max(from);
    let end = entry.ended_at.unwrap_or(now).min(to);
    (end - start).num_milliseconds().max(0)
}
