use chrono::NaiveTime;
use quest_domain::EntityId;
use quest_protocol::{ProposedQuest, WorkTarget};

use super::*;
use crate::test_support::{World, day};

fn time(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

fn key(text: &str) -> TempKey {
    TempKey(text.into())
}

fn proposal(existing_task: EntityId) -> CaptureProposal {
    let quest = QuestRef::New { key: key("q1") };
    CaptureProposal {
        quests: vec![ProposedQuest { key: key("q1"), title: "Tax".into() }],
        tasks: vec![ProposedTask {
            key: key("t1"),
            title: "Receipts".into(),
            description: String::new(),
            quest: Some(quest.clone()),
            quest_uncertain: false,
            planned_on: Some(day("2026-11-25")),
            due_on: Some(day("2026-11-27")),
            repeat: None,
        }],
        work: vec![
            ProposedWork {
                key: key("w1"),
                task: WorkTarget::New { title: "Forms".into(), quest: Some(quest) },
                uncertain: false,
                day: day("2026-11-24"),
                start: time(22, 30),
                end: NaiveTime::MIN,
            },
            ProposedWork {
                key: key("w2"),
                task: WorkTarget::Existing { id: existing_task },
                uncertain: true,
                day: day("2026-11-24"),
                start: time(9, 0),
                end: time(11, 15),
            },
        ],
        dropped: vec![],
    }
}

#[test]
fn deselecting_a_new_quest_takes_it_out_of_its_items() {
    let mut world = World::new();
    let existing = world.task("Konzept", None, Some(day("2026-11-24")));
    let mut draft = Draft::new(proposal(existing));
    assert_eq!(draft.selected_count(), 4);
    draft.set_selected(&key("q1"), false);
    draft.set_selected(&key("w2"), false);
    assert_eq!(draft.selected_count(), 2);
    let confirmed = draft.confirmed();
    assert!(confirmed.quests.is_empty());
    assert_eq!(confirmed.tasks[0].quest, None);
    assert_eq!(confirmed.work.len(), 1);
    assert_eq!(confirmed.work[0].task, WorkTarget::New { title: "Forms".into(), quest: None });
    draft.set_selected(&key("q1"), true);
    assert!(draft.confirmed().tasks[0].quest.is_some());
}

#[test]
fn errors_block_applying_only_for_selected_items() {
    let mut world = World::new();
    let existing = world.task("Konzept", None, Some(day("2026-11-24")));
    let snapshot = world.snapshot();
    let mut draft = Draft::new(proposal(existing));
    assert!(draft.can_apply(&snapshot));
    draft.proposal.tasks[0].title = " ".into();
    assert_eq!(draft.task_error(&draft.proposal.tasks[0], &snapshot), Some("Type a title."));
    assert!(!draft.can_apply(&snapshot));
    draft.set_selected(&key("t1"), false);
    assert!(draft.can_apply(&snapshot));
    draft.proposal.work[1].end = time(8, 0);
    assert_eq!(draft.work_error(&draft.proposal.work[1], &snapshot), Some("The end must be after the start."));
    (draft.proposal.work[1].start, draft.proposal.work[1].end) = (NaiveTime::MIN, NaiveTime::MIN);
    assert_eq!(
        draft.work_error(&draft.proposal.work[1], &snapshot),
        Some("The end must be after the start."),
        "not a whole day"
    );
    let empty = World::new().snapshot();
    assert_eq!(
        draft.work_error(&draft.proposal.work[1], &empty),
        Some("This task no longer exists. Pick another one.")
    );
    for k in ["q1", "w1", "w2"] {
        draft.set_selected(&key(k), false);
    }
    assert!(!draft.can_apply(&snapshot), "nothing selected");
}

#[test]
fn labels_read_like_the_rest_of_the_app() {
    let today = day("2026-11-24");
    let p = proposal(EntityId::new());
    assert_eq!(task_when(p.tasks[0].planned_on, p.tasks[0].due_on, today), "Tomorrow · due Fri, Nov 27");
    assert_eq!(task_when(Some(day("2026-11-27")), None, today), "Fri, Nov 27");
    assert_eq!(task_when(None, Some(day("2026-11-25")), today), "Due Tomorrow");
    assert_eq!(task_when(None, None, today), "Someday");
    assert_eq!(work_when(&p.work[0], today), "Today · 22:30–24:00");
    assert_eq!(work_duration(&p.work[0]), "1:30");
    assert_eq!(work_duration(&p.work[1]), "2:15");
    assert_eq!(apply_label(3), "Create (3)");
    assert_eq!(created_message(1, 2, 1), "Created 1 quest, 2 tasks and 1 session");
    assert_eq!(created_message(0, 1, 0), "Created 1 task");
    assert_eq!(created_message(0, 0, 0), "Nothing created");
    let dropped = Dropped { title: "Pay".into(), reason: DropReason::UnknownTask };
    assert_eq!(dropped_message(&dropped), "“Pay” was left out: the task was not found.");
}
