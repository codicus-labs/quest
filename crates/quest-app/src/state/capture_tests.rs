//! The review's own part: title fields written back into the proposal, quest choices, selection.

use chrono::NaiveDate;
use quest_domain::EntityId;
use quest_protocol::{CaptureProposal, ProposedQuest, ProposedTask, ProposedWork, QuestRef, TempKey, WorkTarget};

use super::capture_draft::Review;

fn key(k: &str) -> TempKey {
    TempKey(k.to_owned())
}

fn proposal() -> CaptureProposal {
    let day = NaiveDate::from_ymd_opt(2026, 10, 16);
    CaptureProposal {
        quests: vec![ProposedQuest { key: key("q1"), title: "Taxes".into() }],
        tasks: vec![ProposedTask {
            key: key("t1"),
            title: "Collect receipts".into(),
            description: String::new(),
            quest: Some(QuestRef::New { key: key("q1") }),
            quest_uncertain: true,
            planned_on: day,
            due_on: None,
            repeat: None,
        }],
        work: vec![ProposedWork {
            key: key("w1"),
            task: WorkTarget::New { title: "Concept".into(), quest: None },
            uncertain: false,
            day: day.unwrap_or_default(),
            start: "09:00:00".parse().unwrap_or_default(),
            end: "11:00:00".parse().unwrap_or_default(),
        }],
        dropped: vec![],
    }
}

#[test]
fn edited_titles_reach_the_confirmed_proposal() {
    let mut review = Review::new(proposal());
    review.titles.get_mut(&key("t1")).unwrap().text = "  Collect all receipts ".into();
    review.titles.get_mut(&key("w1")).unwrap().text = "Relaunch concept".into();
    review.sync_titles();
    let confirmed = review.draft.confirmed();
    assert_eq!(confirmed.tasks[0].title, "Collect all receipts");
    assert!(matches!(&confirmed.work[0].task, WorkTarget::New { title, .. } if title == "Relaunch concept"));
}

#[test]
fn deselecting_a_new_quest_leaves_its_task_without_quest() {
    let mut review = Review::new(proposal());
    review.toggle(&key("q1"));
    assert_eq!(review.draft.selected_count(), 2);
    let confirmed = review.draft.confirmed();
    assert!(confirmed.quests.is_empty());
    assert_eq!(confirmed.tasks[0].quest, None);
}

#[test]
fn picking_a_quest_is_no_longer_a_guess() {
    let mut review = Review::new(proposal());
    let quest = EntityId::new();
    review.set_task_quest(&key("t1"), Some(QuestRef::Existing { id: quest }));
    let task = &review.draft.proposal.tasks[0];
    assert_eq!(task.quest, Some(QuestRef::Existing { id: quest }));
    assert!(!task.quest_uncertain);
}

#[test]
fn the_toast_counts_what_was_created() {
    assert_eq!(Review::created_message(&proposal()), "Created 1 quest, 1 task and 1 session");
}
