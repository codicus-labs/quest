use quest_domain::{EntityId, Task};

use super::*;
use crate::palette::Mode;
use crate::undo::Deletable;

fn task(quest: bool, done: bool) -> Task {
    Task {
        id: EntityId::new(),
        title: "Ship the beta".into(),
        quest_id: quest.then(EntityId::new),
        planned_on: None,
        due_on: None,
        sort_key: "a".into(),
        completed_at: done.then(|| "2026-10-15T14:40:00Z".parse().unwrap()),
        description: String::new(),
        repeat: None,
        day_order: String::new(),
        series: None,
    }
}

#[test]
fn open_quest_tasks_offer_everything_and_end_with_delete() {
    let rows = task_actions(&task(true, false), true);
    assert_eq!(rows[..3], [Some(TaskAction::Open), None, Some(TaskAction::Focus)], "Open first, set apart");
    assert_eq!(rows.last(), Some(&Some(TaskAction::Delete)));
    assert!(rows.contains(&Some(TaskAction::OpenQuest)));
    assert_eq!(rows[rows.len() - 2], None, "a separator sets Delete apart");
}

#[test]
fn done_tasks_and_tasks_without_quest_offer_less() {
    let done = task_actions(&task(true, true), true);
    assert!(!done.contains(&Some(TaskAction::Focus)));
    let loose = task_actions(&task(false, false), false);
    assert!(!loose.contains(&Some(TaskAction::OpenQuest)));
    let orphan = task_actions(&task(true, false), false);
    assert!(!orphan.contains(&Some(TaskAction::OpenQuest)), "the quest was deleted");
}

#[test]
fn actions_become_steps() {
    let t = task(true, false);
    assert_eq!(task_step(TaskAction::Open, &t), Some(Step::Open(t.id)));
    assert_eq!(task_step(TaskAction::Delete, &t), Some(Step::Delete(Deletable::Task(t.id))));
    assert_eq!(task_step(TaskAction::MoveTo, &t), Some(Step::Palette(Mode::MoveTo(t.id))));
    assert_eq!(task_step(TaskAction::AssignQuest, &t), Some(Step::Palette(Mode::AssignQuest(t.id))));
    assert_eq!(task_step(TaskAction::OpenQuest, &t), Some(Step::OpenQuest(t.quest_id.unwrap())));
    assert_eq!(task_step(TaskAction::OpenQuest, &task(false, false)), None);
}

#[test]
fn labels_follow_the_task() {
    assert_eq!(TaskAction::ToggleDone.label(&task(false, true)), "Reopen");
    assert_eq!(TaskAction::ToggleDone.label(&task(false, false)), "Complete");
    assert_eq!(TaskAction::Delete.hint(), Some("⌫"));
}
