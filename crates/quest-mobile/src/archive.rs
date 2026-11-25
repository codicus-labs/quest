//! The archive, pure, like the desktop's: quests an older version archived (to unarchive), then
//! everything deleted, grouped (quests, tasks, checklist items, sessions), newest first, each
//! restorable. Completed quests are on the Quests page instead.

use quest_domain::{EntityId, EntityKind, QuestStatus, Snapshot, Trashed};
use quest_view::undo::Deletable;

use crate::home::color_hex;
use crate::quest_types::{ArchiveData, ArchiveGroup, ArchiveItem, TrashKind};
use crate::time::Now;

/// The groups, in the order they show.
const GROUPS: [(TrashKind, &str); 4] = [
    (TrashKind::Quest, "Deleted quests"),
    (TrashKind::Task, "Deleted tasks"),
    (TrashKind::ChecklistItem, "Deleted checklist items"),
    (TrashKind::Session, "Deleted sessions"),
];

pub(crate) fn archive(snapshot: &Snapshot, now: Now) -> ArchiveData {
    let groups = GROUPS
        .into_iter()
        .map(|(kind, title)| ArchiveGroup {
            title: title.to_owned(),
            items: snapshot
                .trash
                .iter()
                .filter(|t| t.kind == entity_kind(kind))
                .map(|t| item(snapshot, t, kind, now))
                .collect(),
        })
        .filter(|g| !g.items.is_empty())
        .collect();
    let archived_quests = snapshot
        .quests
        .iter()
        .filter(|q| q.status == QuestStatus::Archived)
        .map(|q| ArchiveItem {
            kind: TrashKind::Quest,
            id: q.id.to_string(),
            title: q.title.clone(),
            meta: "Archived quest".to_owned(),
            color_hex: Some(color_hex(q.color).to_owned()),
        })
        .collect();
    ArchiveData { archived_quests, groups }
}

fn item(snapshot: &Snapshot, trashed: &Trashed, kind: TrashKind, now: Now) -> ArchiveItem {
    let quest = trashed.quest_id.filter(|q| *q != trashed.id).and_then(|q| snapshot.quest(q));
    let title = match (&trashed.title, kind) {
        (Some(title), _) => title.clone(),
        (None, TrashKind::Session) => "Other work".to_owned(),
        (None, _) => "Untitled".to_owned(),
    };
    let deleted = now.local(trashed.deleted_at).format("Deleted %b %-d, %H:%M").to_string();
    ArchiveItem {
        kind,
        id: trashed.id.to_string(),
        title,
        meta: quest.map_or_else(|| deleted.clone(), |q| format!("{} · {deleted}", q.title)),
        // Like the desktop: the colour of the quest it belonged to (a deleted quest has none).
        color_hex: quest.map(|q| color_hex(q.color).to_owned()),
    }
}

fn entity_kind(kind: TrashKind) -> EntityKind {
    match kind {
        TrashKind::Quest => EntityKind::Quest,
        TrashKind::Task => EntityKind::Task,
        TrashKind::ChecklistItem => EntityKind::ChecklistItem,
        TrashKind::Session => EntityKind::TimeEntry,
    }
}

/// What Restore brings back.
pub(crate) fn deletable(kind: TrashKind, id: EntityId) -> Deletable {
    match kind {
        TrashKind::Quest => Deletable::Quest(id),
        TrashKind::Task => Deletable::Task(id),
        TrashKind::ChecklistItem => Deletable::ChecklistItem(id),
        TrashKind::Session => Deletable::Entry(id),
    }
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
