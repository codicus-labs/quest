use std::sync::Arc;

use chrono::NaiveTime;
use quest_domain::{Frequency, Snapshot};
use quest_protocol::{DropReason, Dropped};

use super::*;
use crate::core::{ChangeListener, QuestCore};
use crate::quest_types::TrashKind;
use crate::test_support::{berlin, day, id, moment, quest, task};

struct Quiet;

impl ChangeListener for Quiet {
    fn on_change(&self) {}
}

fn now() -> Now {
    // Tuesday, 24 Nov 2026, 10:30 in Berlin.
    berlin("2026-11-24T10:30:00+01:00")
}

fn time(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

fn key(text: &str) -> TempKey {
    TempKey(text.into())
}

fn snapshot() -> Snapshot {
    Snapshot {
        quests: vec![quest(1, "Website-Relaunch")],
        tasks: vec![task(2, "Konzept", Some(1), Some("2026-11-24"))],
        ..Snapshot::default()
    }
}

fn proposal() -> CaptureProposal {
    let tax = QuestRef::New { key: key("q1") };
    CaptureProposal {
        quests: vec![ProposedQuest { key: key("q1"), title: "Steuer".into() }],
        tasks: vec![
            ProposedTask {
                key: key("t1"),
                title: "Angebot schreiben".into(),
                description: String::new(),
                quest: Some(QuestRef::Existing { id: id(1) }),
                quest_uncertain: true,
                planned_on: Some(day("2026-11-25")),
                due_on: Some(day("2026-11-27")),
                repeat: None,
            },
            ProposedTask {
                key: key("t2"),
                title: "Belege sammeln".into(),
                description: String::new(),
                quest: Some(tax),
                quest_uncertain: false,
                planned_on: Some(day("2026-11-27")),
                due_on: None,
                repeat: Some(ProposedRepeat {
                    every: 1,
                    frequency: Frequency::Week,
                    weekdays: vec![Weekday::Fri],
                    month_day: None,
                    ends: None,
                }),
            },
        ],
        work: vec![ProposedWork {
            key: key("w1"),
            task: WorkTarget::Existing { id: id(2) },
            uncertain: false,
            day: day("2026-11-24"),
            start: time(9, 0),
            end: time(11, 0),
        }],
        dropped: vec![Dropped { title: "Garbage".into(), reason: DropReason::InvalidDate }],
    }
}

#[test]
fn a_proposal_opens_as_a_draft_with_everything_selected() {
    let draft = draft(&proposal(), &snapshot(), now());
    assert!(draft.quests[0].selected && draft.tasks.iter().all(|t| t.selected) && draft.work[0].selected);
    assert_eq!(draft.tasks[0].planned_on.as_ref().unwrap().iso, "2026-11-25");
    assert_eq!(draft.tasks[0].due_on.as_ref().unwrap().label, "Nov 27, 2026");
    let repeat = draft.tasks[1].repeat.as_ref().unwrap();
    assert_eq!(
        (repeat.frequency, repeat.weekdays.as_slice()),
        (RepeatFrequency::Week, [RepeatWeekday::Fri].as_slice())
    );
    assert_eq!(repeat.time_zone, "Europe/Berlin");
    assert_eq!(draft.work[0].task, DraftWorkTask::Existing { id: id(2).to_string(), title: "Konzept".into() });
    assert_eq!(draft.dropped, ["“Garbage” was left out: the date or time does not exist or is too far away."]);
}

#[test]
fn the_view_labels_every_item() {
    let snapshot = snapshot();
    let draft = draft(&proposal(), &snapshot, now());
    let view = super::view(&draft, &snapshot, now()).unwrap();
    assert_eq!(view.apply_label, "Create (4)");
    assert!(view.can_apply);
    let offer = &view.tasks[0];
    assert_eq!(offer.when, "Tomorrow · due Fri, Nov 27");
    assert_eq!(offer.quest.as_ref().unwrap().name, "Website-Relaunch");
    let receipts = &view.tasks[1];
    assert_eq!(receipts.quest.as_ref().unwrap().name, "Steuer");
    assert_eq!(receipts.quest.as_ref().unwrap().color_hex, view.quests[0].color_hex);
    assert!(receipts.repeat_summary.is_some());
    let work = &view.work[0];
    assert_eq!((work.when.as_str(), work.duration.as_str()), ("Today · 09:00–11:00", "2:00"));
    assert_eq!(work.quest.as_ref().unwrap().name, "Website-Relaunch", "the session counts towards its task's quest");
    let options: Vec<&str> = view.quest_options.iter().map(|o| o.chip.name.as_str()).collect();
    assert_eq!(options, ["Website-Relaunch", "Steuer"]);
}

#[test]
fn edits_from_the_pickers_show_their_problems() {
    let snapshot = snapshot();
    let mut draft = draft(&proposal(), &snapshot, now());
    draft.quests[0].selected = false;
    draft.tasks[0].planned_on = None;
    draft.work[0].start_minutes = 12 * 60;
    let view = super::view(&draft, &snapshot, now()).unwrap();
    assert_eq!(view.tasks[0].when, "Due Fri, Nov 27");
    assert_eq!(view.tasks[1].quest, None, "its new quest is deselected");
    assert_eq!(view.quest_options.len(), 1);
    assert_eq!(view.work[0].error.as_deref(), Some("The end must be after the start."));
    assert!(!view.can_apply);
    assert_eq!(view.apply_label, "Create (3)");

    draft.work[0].selected = false;
    draft.tasks[0].title = " ".into();
    let view = super::view(&draft, &snapshot, now()).unwrap();
    assert_eq!(view.tasks[0].error.as_deref(), Some("Type a title."));
    draft.tasks[1].repeat.as_mut().unwrap().every = "0".into();
    draft.tasks[0].title = "Angebot schreiben".into();
    let view = super::view(&draft, &snapshot, now()).unwrap();
    assert!(view.tasks[1].error.is_some(), "a broken repeat is reported");
    assert!(!view.can_apply);
}

#[test]
fn applying_through_the_core_is_all_or_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let core =
        QuestCore::open(dir.path().display().to_string(), crate::test_support::tokens(), Arc::new(Quiet)).unwrap();
    let moment = moment("2026-11-24T10:30:00+01:00");
    let existing =
        core.quick_add("Konzept".into(), String::new(), None, Some("2026-11-24".into()), moment.clone()).unwrap();
    let mut proposal = proposal();
    proposal.tasks[0].quest = None;
    proposal.work[0].task = WorkTarget::Existing { id: id(99) };
    let mut draft = draft(&proposal, &core.store().snapshot(), now());

    let refused = core.apply_capture(draft.clone(), moment.clone());
    assert!(matches!(refused, Err(MobileError::InvalidForm { .. })), "{refused:?}");
    assert_eq!(core.store().snapshot().tasks.len(), 1, "nothing was created");

    draft.work[0].task = DraftWorkTask::Existing { id: existing.clone(), title: String::new() };
    let applied = core.apply_capture(draft, moment).unwrap();
    assert_eq!(applied.message, "Created 1 quest, 2 tasks and 1 session");
    let snapshot = core.store().snapshot();
    assert_eq!((snapshot.quests.len(), snapshot.tasks.len(), snapshot.time_entries.len()), (1, 3, 1));
    let receipts = snapshot.tasks.iter().find(|t| t.title == "Belege sammeln").unwrap();
    assert_eq!(receipts.quest_id, Some(snapshot.quests[0].id));
    assert!(receipts.repeat.is_some());
}

#[test]
fn undo_takes_back_what_the_last_capture_created_once() {
    let dir = tempfile::tempdir().unwrap();
    let core =
        QuestCore::open(dir.path().display().to_string(), crate::test_support::tokens(), Arc::new(Quiet)).unwrap();
    let moment = moment("2026-11-24T10:30:00+01:00");
    let existing =
        core.quick_add("Konzept".into(), String::new(), None, Some("2026-11-24".into()), moment.clone()).unwrap();
    let mut proposal = proposal();
    proposal.tasks[0].quest = None;
    let mut draft = draft(&proposal, &core.store().snapshot(), now());
    draft.work[0].task = DraftWorkTask::Existing { id: existing.clone(), title: String::new() };

    let applied = core.apply_capture(draft, moment.clone()).unwrap();
    assert_eq!(applied.undo_label, "Undo");
    assert_eq!(applied.show_for_ms, 6_000, "as long as the desktop's toast");
    let snapshot = core.store().snapshot();
    let created = snapshot.tasks.iter().find(|t| t.title == "Belege sammeln").unwrap().id;
    core.delete_task(created.to_string(), moment.clone()).unwrap();

    core.undo_capture(moment.clone()).unwrap();
    let snapshot = core.store().snapshot();
    assert_eq!((snapshot.quests.len(), snapshot.tasks.len(), snapshot.time_entries.len()), (0, 1, 0));
    assert_eq!(snapshot.tasks[0].id.to_string(), existing, "what was there before stays");
    assert!(snapshot.task(created).is_none(), "a task deleted since stays deleted");

    core.restore(TrashKind::Task, created.to_string(), moment.clone()).unwrap();
    core.undo_capture(moment).unwrap();
    assert!(core.store().snapshot().task(created).is_some(), "a second Undo does nothing");
}

#[test]
fn undo_comes_too_late_after_the_limit() {
    let dir = tempfile::tempdir().unwrap();
    let core =
        QuestCore::open(dir.path().display().to_string(), crate::test_support::tokens(), Arc::new(Quiet)).unwrap();
    let moment = moment("2026-11-24T10:30:00+01:00");
    let mut proposal = proposal();
    proposal.tasks[0].quest = None;
    let mut draft = draft(&proposal, &core.store().snapshot(), now());
    draft.work[0].selected = false;
    core.apply_capture(draft.clone(), moment.clone()).unwrap();
    let created = core.store().snapshot().tasks.len();

    core.undo_capture(crate::test_support::moment("2026-11-24T10:32:00+01:00")).unwrap();
    assert_eq!(core.store().snapshot().tasks.len(), created, "two minutes later it is too late");
}

#[test]
fn undo_without_a_capture_does_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let core =
        QuestCore::open(dir.path().display().to_string(), crate::test_support::tokens(), Arc::new(Quiet)).unwrap();
    let moment = moment("2026-11-24T10:30:00+01:00");
    core.quick_add("Konzept".into(), String::new(), None, None, moment.clone()).unwrap();
    core.undo_capture(moment).unwrap();
    assert_eq!(core.store().snapshot().tasks.len(), 1);
}

#[test]
fn capture_needs_an_account() {
    let dir = tempfile::tempdir().unwrap();
    let core =
        QuestCore::open(dir.path().display().to_string(), crate::test_support::tokens(), Arc::new(Quiet)).unwrap();
    let result = core.capture("Morgen Angebot schreiben".into(), "de-DE".into(), moment("2026-11-24T10:30:00+01:00"));
    assert_eq!(result, Err(MobileError::AiSignedOut));
}
