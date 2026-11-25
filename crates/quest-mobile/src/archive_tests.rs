use quest_domain::{EntityKind, Quest, QuestStatus, Snapshot, Trashed};

use super::*;
use crate::test_support::{at, berlin, id, quest};

fn trashed(kind: EntityKind, n: u128, title: Option<&str>, quest: Option<u128>, deleted: &str) -> Trashed {
    Trashed { kind, id: id(n), title: title.map(str::to_owned), quest_id: quest.map(id), deleted_at: at(deleted) }
}

#[test]
fn deleted_things_group_by_kind_newest_first() {
    let snapshot = Snapshot {
        quests: vec![quest(1, "Photo series")],
        trash: vec![
            trashed(EntityKind::Task, 11, Some("Print"), Some(1), "2026-11-24T13:05:00Z"),
            trashed(EntityKind::Quest, 2, Some("Garden"), Some(2), "2026-11-24T12:00:00Z"),
            trashed(EntityKind::TimeEntry, 20, None, None, "2026-11-23T08:00:00Z"),
            trashed(EntityKind::Task, 10, Some("Mail"), None, "2026-11-22T08:00:00Z"),
        ],
        ..Snapshot::default()
    };
    let data = archive(&snapshot, berlin("2026-11-25T10:00:00+01:00"));
    let titles: Vec<&str> = data.groups.iter().map(|g| g.title.as_str()).collect();
    assert_eq!(titles, ["Deleted quests", "Deleted tasks", "Deleted sessions"]);
    let quest = &data.groups[0].items[0];
    assert_eq!(
        (quest.kind, quest.title.as_str(), quest.meta.as_str()),
        (TrashKind::Quest, "Garden", "Deleted Nov 24, 13:00")
    );
    assert_eq!(quest.color_hex, None);
    let print = &data.groups[1].items[0];
    assert_eq!(print.meta, "Photo series · Deleted Nov 24, 14:05", "local time, with its quest");
    assert_eq!(print.color_hex.as_deref(), Some("#5c8fd9"));
    assert_eq!(data.groups[1].items[1].title, "Mail");
    assert_eq!(data.groups[2].items[0].title, "Other work");
}

#[test]
fn nothing_deleted_has_no_groups() {
    let data = archive(&Snapshot::default(), berlin("2026-11-25T10:00:00+01:00"));
    assert!(data.groups.is_empty() && data.archived_quests.is_empty());
}

#[test]
fn quests_an_older_version_archived_come_first_to_unarchive() {
    let snapshot = Snapshot {
        quests: vec![
            quest(1, "Photo series"),
            Quest { status: QuestStatus::Archived, ..quest(2, "Old plan") },
            Quest { status: QuestStatus::Completed, ..quest(3, "Garden") },
        ],
        ..Snapshot::default()
    };
    let data = archive(&snapshot, berlin("2026-11-25T10:00:00+01:00"));
    let archived: Vec<(&str, &str, TrashKind)> =
        data.archived_quests.iter().map(|q| (q.title.as_str(), q.meta.as_str(), q.kind)).collect();
    assert_eq!(archived, [("Old plan", "Archived quest", TrashKind::Quest)], "completed quests are not archived");
    assert_eq!(data.archived_quests[0].color_hex.as_deref(), Some("#5c8fd9"));
    assert!(data.groups.is_empty());
}

#[test]
fn restore_targets_match_the_kind() {
    assert_eq!(deletable(TrashKind::Session, id(1)), Deletable::Entry(id(1)));
    assert_eq!(deletable(TrashKind::Milestone, id(1)), Deletable::Milestone(id(1)));
}
