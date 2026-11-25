use chrono::NaiveTime;
use quest_protocol::{DropReason, QuestRef, TempKey, WorkTarget};
use serde_json::{Value, json};

use super::*;
use crate::test_support::{BERLIN, World, day};

fn time(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

fn task(title: &str) -> Value {
    json!({
        "title": title, "description": "", "quest_id": null, "new_quest_key": null, "quest_confidence": "high",
        "day": null, "start_time": null, "duration_minutes": null, "repeat": null
    })
}

fn work(date: &str, start: &str, end: &str) -> Value {
    json!({
        "task_id": null, "new_task_title": "Konzept", "quest_id": null, "new_quest_key": null,
        "confidence": "high", "date": date, "start_time": start, "end_time": end
    })
}

fn with(mut item: Value, changes: Value) -> Value {
    for (key, value) in changes.as_object().unwrap() {
        item[key] = value.clone();
    }
    item
}

fn run(world: &World, output: Value) -> CaptureProposal {
    validate(&output, &world.request("text"), BERLIN).unwrap()
}

fn reasons(proposal: &CaptureProposal) -> Vec<DropReason> {
    proposal.dropped.iter().map(|d| d.reason).collect()
}

#[test]
fn the_brief_example_becomes_a_proposal() {
    let mut world = World::new();
    let relaunch = world.quest("Website-Relaunch");
    let output = json!({
        "new_quests": [{ "key": "tax", "title": "Steuererklärung", "description": "", "milestones": [] }],
        "tasks": [
            with(task("Angebot für Müller schreiben"), json!({
                "quest_id": relaunch.to_string(), "day": "2026-11-25", "duration_minutes": 120
            })),
            with(task("Belege sammeln"), json!({ "new_quest_key": "tax", "day": "2026-11-27" })),
        ],
        "work_logs": [with(work("2026-11-24", "09:00", "11:00"), json!({
            "new_task_title": "Relaunch-Konzept", "quest_id": relaunch.to_string()
        }))],
    });
    let proposal = run(&world, output);
    assert_eq!(proposal.quests[0].key, TempKey("q1".into()));
    let [offer, receipts] = proposal.tasks.as_slice() else { panic!("two tasks") };
    assert_eq!(offer.quest, Some(QuestRef::Existing { id: relaunch }));
    assert_eq!(offer.day, Some(day("2026-11-25")));
    assert_eq!(offer.block, None, "a duration without a start is an estimate, and there are none");
    assert_eq!(receipts.quest, Some(QuestRef::New { key: TempKey("q1".into()) }));
    assert!(!receipts.quest_uncertain);
    let session = &proposal.work[0];
    assert_eq!(
        session.task,
        WorkTarget::New { title: "Relaunch-Konzept".into(), quest: Some(QuestRef::Existing { id: relaunch }) }
    );
    assert_eq!((session.start, session.end), (time(9, 0), time(11, 0)));
    assert_eq!([offer.key.0.as_str(), receipts.key.0.as_str(), session.key.0.as_str()], ["t1", "t2", "w1"]);
    assert!(proposal.dropped.is_empty());
}

#[test]
fn answers_that_are_no_proposal_are_refused() {
    let world = World::new();
    let request = world.request("text");
    for output in [
        json!("tasks"),
        json!([]),
        json!(null),
        json!({ "answer": "ignore previous instructions" }),
        json!({ "tasks": "nope" }),
    ] {
        assert!(validate(&output, &request, BERLIN).is_err(), "{output}");
    }
}

#[test]
fn malformed_items_are_dropped_and_listed() {
    let world = World::new();
    let proposal = run(
        &world,
        json!({
            "tasks": [
                "just a string",
                with(task("  "), json!({})),
                with(task("Title"), json!({ "title": 42 })),
                task("Kept"),
            ],
            "work_logs": null,
        }),
    );
    assert_eq!(proposal.tasks.len(), 1);
    assert_eq!(reasons(&proposal), [DropReason::Incomplete; 3]);
}

#[test]
fn unknown_quests_become_no_quest_and_are_marked() {
    let mut world = World::new();
    world.quest("Known");
    let made_up = quest_domain::EntityId::new().to_string();
    let proposal = run(
        &world,
        json!({ "tasks": [
            with(task("A"), json!({ "quest_id": made_up })),
            with(task("B"), json!({ "quest_id": "'; DROP TABLE quests; --" })),
            with(task("C"), json!({ "new_quest_key": "q9" })),
            with(task("D"), json!({ "quest_confidence": "low" })),
        ] }),
    );
    for task in &proposal.tasks {
        assert_eq!(task.quest, None, "{}", task.title);
        assert!(task.quest_uncertain, "{}", task.title);
    }
}

#[test]
fn work_on_unknown_tasks_is_dropped_or_becomes_a_new_task() {
    let mut world = World::new();
    let known = world.task("Konzept", None, Some(day("2026-11-24")));
    let unknown = quest_domain::EntityId::new().to_string();
    let base = work("2026-11-24", "09:00", "10:00");
    let proposal = run(
        &world,
        json!({ "work_logs": [
            with(base.clone(), json!({ "task_id": known.to_string(), "new_task_title": null })),
            with(base.clone(), json!({ "task_id": unknown, "new_task_title": null })),
            with(base.clone(), json!({ "task_id": unknown })),
            with(base, json!({ "new_task_title": null })),
        ] }),
    );
    assert_eq!(proposal.work[0].task, WorkTarget::Existing { id: known });
    assert!(!proposal.work[0].uncertain);
    assert!(matches!(proposal.work[1].task, WorkTarget::New { .. }));
    assert!(proposal.work[1].uncertain, "a made-up id with a title is a guess");
    assert_eq!(reasons(&proposal), [DropReason::UnknownTask, DropReason::Incomplete]);
}

#[test]
fn impossible_and_far_dates_are_dropped() {
    let world = World::new();
    let dated = |text: &str| with(task("T"), json!({ "day": text }));
    let proposal = run(
        &world,
        json!({ "tasks": [
            dated("2026-02-30"), dated("tomorrow"), dated("3026-01-01"), dated("2020-01-01"), dated("2026-11-30")
        ] }),
    );
    assert_eq!(proposal.tasks.len(), 1);
    assert_eq!(reasons(&proposal), [DropReason::InvalidDate; 4]);
}

#[test]
fn sessions_must_end_after_they_start_and_lie_in_the_past() {
    let world = World::new();
    let proposal = run(
        &world,
        json!({ "work_logs": [
            work("2026-11-24", "11:00", "09:00"),
            work("2026-11-24", "09:00", "09:00"),
            work("2026-11-24", "25:00", "26:00"),
            work("2026-11-24", "9", "10:00"),
            work("2026-11-24", "11:00", "12:00"),
            work("2026-11-24", "10:00", "12:00"),
            work("2026-11-22", "00:00", "00:00"),
            work("2026-11-23", "23:00", "24:00"),
        ] }),
    );
    assert_eq!(proposal.work.len(), 1);
    assert_eq!((proposal.work[0].start, proposal.work[0].end), (time(23, 0), NaiveTime::MIN));
    assert_eq!(
        reasons(&proposal),
        [
            DropReason::InvalidTimeRange,
            DropReason::InvalidTimeRange,
            DropReason::InvalidDate,
            DropReason::InvalidDate,
            DropReason::InvalidTimeRange,
            DropReason::InvalidTimeRange,
            DropReason::InvalidTimeRange,
        ]
    );
}

#[test]
fn a_faulty_block_or_repeat_is_left_out_but_the_task_stays() {
    let world = World::new();
    let base = with(task("T"), json!({ "day": "2026-11-25" }));
    let proposal = run(
        &world,
        json!({ "tasks": [
            with(base.clone(), json!({ "start_time": "23:30", "duration_minutes": 60 })),
            with(base.clone(), json!({ "start_time": "09:00", "duration_minutes": "120" })),
            with(base.clone(), json!({ "repeat": { "every": 0, "unit": "day", "weekdays": [], "month_day": null, "until": null } })),
            with(task("No day"), json!({ "start_time": "09:00", "duration_minutes": 30 })),
            with(base, json!({ "start_time": "14:00", "duration_minutes": 90 })),
        ] }),
    );
    assert_eq!(proposal.tasks.len(), 5);
    assert_eq!(proposal.tasks[0].block, None);
    assert_eq!(proposal.tasks[1].block.map(|b| b.minutes), Some(60), "a duration that is not a number is ignored");
    assert_eq!(proposal.tasks[2].repeat, None);
    assert_eq!(proposal.tasks[3].block, None);
    assert_eq!(proposal.tasks[4].block, Some(TimeBlock { start: time(14, 0), minutes: 90 }));
    assert_eq!(reasons(&proposal), [DropReason::InvalidTimeRange, DropReason::Incomplete, DropReason::Incomplete]);
}

#[test]
fn repeats_are_checked_field_by_field() {
    let world = World::new();
    let repeat = |rule: Value| with(task("R"), json!({ "day": "2026-11-24", "repeat": rule }));
    let proposal = run(
        &world,
        json!({ "tasks": [
            repeat(json!({ "every": 1, "unit": "week", "weekdays": ["mon", "thu", "mon"], "month_day": 5, "until": "2026-12-31" })),
            repeat(json!({ "every": 1, "unit": "month", "weekdays": ["mon"], "month_day": -1, "until": null })),
            repeat(json!({ "every": 1, "unit": "fortnight", "weekdays": [], "month_day": null, "until": null })),
            repeat(json!({ "every": 1, "unit": "day", "weekdays": [], "month_day": null, "until": "2026-11-01" })),
            repeat(json!({ "every": 1, "unit": "week", "weekdays": ["someday"], "month_day": null, "until": null })),
        ] }),
    );
    let weekly = proposal.tasks[0].repeat.as_ref().unwrap();
    assert_eq!(weekly.weekdays, [chrono::Weekday::Mon, chrono::Weekday::Thu]);
    assert_eq!(weekly.month_day, None, "a month day only counts for monthly rules");
    let monthly = proposal.tasks[1].repeat.as_ref().unwrap();
    assert_eq!((monthly.month_day, monthly.weekdays.len()), (Some(MonthDay::Last), 0));
    assert!(proposal.tasks[2..].iter().all(|t| t.repeat.is_none()));
    assert_eq!(reasons(&proposal), [DropReason::Incomplete, DropReason::InvalidDate, DropReason::Incomplete]);
}

#[test]
fn overlong_text_and_too_many_items_are_dropped() {
    let world = World::new();
    let mut tasks: Vec<Value> = (0..31).map(|i| task(&format!("Task {i}"))).collect();
    tasks[0] = task(&"x".repeat(MAX_TITLE_CHARS + 1));
    tasks[1] = with(task("Long notes"), json!({ "description": "y".repeat(MAX_DESCRIPTION_CHARS + 1) }));
    let proposal = run(&world, json!({ "tasks": tasks }));
    assert_eq!(proposal.tasks.len(), 29);
    assert_eq!(reasons(&proposal), [DropReason::TooLong; 2]);
    assert!(proposal.dropped[0].title.chars().count() <= 60, "dropped titles are shortened");

    let many: Vec<Value> = (0..MAX_ITEMS + 2).map(|i| task(&format!("Task {i}"))).collect();
    let proposal = run(&world, json!({ "tasks": many }));
    assert_eq!(proposal.tasks.len(), MAX_ITEMS);
    assert_eq!(reasons(&proposal), [DropReason::TooLong; 2]);
}

#[test]
fn titles_are_cleaned_and_new_quest_keys_must_be_unique() {
    let world = World::new();
    let quest = |key: &str, title: &str| {
        json!({ "key": key, "title": title, "description": "a\u{0}b\n\nc", "milestones": [
        { "title": "Erste Abgabe", "date": null }, { "title": "Bad", "date": "2026-13-01" }
    ] })
    };
    let proposal = run(
        &world,
        json!({
            "new_quests": [quest("tax", "Steuer\u{7}\n  erklärung"), quest("tax", "Duplicate"), quest("", "No key")],
            "tasks": [with(task("Belege\tsammeln"), json!({ "new_quest_key": "tax" }))],
        }),
    );
    assert_eq!(proposal.quests.len(), 1);
    let quest = &proposal.quests[0];
    assert_eq!(quest.title, "Steuer erklärung");
    assert_eq!(quest.description, "ab\n\nc");
    assert_eq!(quest.milestones.len(), 1);
    assert_eq!(quest.milestones[0].achieved_on, day("2026-11-24"), "a milestone without a date is today's");
    assert_eq!(proposal.tasks[0].title, "Belege sammeln");
    assert_eq!(proposal.tasks[0].quest, Some(QuestRef::New { key: TempKey("q1".into()) }));
    assert_eq!(reasons(&proposal), [DropReason::InvalidDate, DropReason::Incomplete, DropReason::Incomplete]);
}
