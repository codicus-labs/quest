use quest_domain::{EntityId, Quest, QuestColor, QuestStatus, Snapshot, Task};

use super::*;

fn id(n: u128) -> EntityId {
    EntityId(uuid::Uuid::from_u128(n))
}

fn quest(n: u128, status: QuestStatus) -> Quest {
    Quest { id: id(n), title: format!("Q{n}"), color: QuestColor::Blue, status, sort_key: format!("{n}") }
}

fn task(n: u128, quest: Option<u128>) -> Task {
    Task {
        id: id(n),
        title: format!("T{n}"),
        quest_id: quest.map(id),
        planned_on: None,
        due_on: None,
        sort_key: format!("{n}"),
        completed_at: None,
        description: String::new(),
        repeat: None,
        series: None,
    }
}

fn snapshot() -> Snapshot {
    Snapshot {
        quests: vec![quest(1, QuestStatus::Active), quest(2, QuestStatus::Active), quest(3, QuestStatus::Archived)],
        tasks: vec![task(10, Some(1)), task(11, Some(2)), task(12, Some(3)), task(13, None)],
        ..Snapshot::default()
    }
}

#[test]
fn the_filter_wins_then_the_pick() {
    let snap = snapshot();
    assert_eq!(new_task_quest(&snap, Some(id(1)), Some(id(2))), Some(id(1)));
    assert_eq!(new_task_quest(&snap, None, Some(id(1))), Some(id(1)));
}

#[test]
fn without_a_choice_the_newest_quest_task_decides_skipping_inactive_quests() {
    let snap = snapshot();
    assert_eq!(new_task_quest(&snap, None, None), Some(id(2)), "task 12's quest is archived");
    assert_eq!(new_task_quest(&snap, Some(id(3)), None), Some(id(2)), "an archived filter is not usable");
}

#[test]
fn without_quest_tasks_the_first_active_quest() {
    let snap = Snapshot { tasks: Vec::new(), ..snapshot() };
    assert_eq!(new_task_quest(&snap, None, None), Some(id(1)));
    assert_eq!(new_task_quest(&Snapshot::default(), None, None), None);
}

#[test]
fn rename_skips_empty_and_unchanged_titles_and_keeps_one_line() {
    let t = task(10, None);
    assert_eq!(rename(&t, "  "), None);
    assert_eq!(rename(&t, " T10 "), None);
    assert_eq!(rename(&t, "a\nb\r\nc "), Some(Command::RenameTask { id: t.id, title: "a b c".into() }));
}
