use chrono::{NaiveTime, Weekday};
use quest_domain::Frequency;
use quest_protocol::{ProposedQuest, ProposedTask};

use super::*;
use crate::review::Draft;
use crate::test_support::{BERLIN, World, day, now};

fn time(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

fn key(text: &str) -> TempKey {
    TempKey(text.into())
}

fn new_quest(k: &str, title: &str) -> ProposedQuest {
    ProposedQuest { key: key(k), title: title.into() }
}

fn new_task(k: &str, title: &str, quest: Option<QuestRef>) -> ProposedTask {
    ProposedTask {
        key: key(k),
        title: title.into(),
        description: String::new(),
        quest,
        quest_uncertain: false,
        planned_on: Some(day("2026-11-25")),
        due_on: None,
        repeat: None,
    }
}

fn session(k: &str, task: WorkTarget, on: &str, start: NaiveTime, end: NaiveTime) -> ProposedWork {
    ProposedWork { key: key(k), task, uncertain: false, day: day(on), start, end }
}

/// Plans `proposal` and runs it all at once, like `Store::dispatch_all`.
fn apply(world: &mut World, proposal: &CaptureProposal) -> CapturePlan {
    let snapshot = world.snapshot();
    let plan = plan(proposal, &snapshot, now(), BERLIN, || world.id()).unwrap();
    world.run(&plan.commands).unwrap();
    plan
}

#[test]
fn a_new_quest_and_its_task_are_created_together() {
    let mut world = World::new();
    world.quest("Existing");
    let mut task = new_task("t1", "Belege sammeln", Some(QuestRef::New { key: key("q1") }));
    task.due_on = Some(day("2026-11-30"));
    let proposal = CaptureProposal {
        quests: vec![new_quest("q1", "Steuererklärung"), new_quest("q2", "Garten")],
        tasks: vec![task],
        ..CaptureProposal::default()
    };
    let plan = apply(&mut world, &proposal);
    let snapshot = world.snapshot();
    let tax = snapshot.quests.iter().find(|q| q.title == "Steuererklärung").unwrap();
    let task = snapshot.tasks.iter().find(|t| t.title == "Belege sammeln").unwrap();
    assert_eq!(task.quest_id, Some(tax.id));
    assert_eq!((task.planned_on, task.due_on), (Some(day("2026-11-25")), Some(day("2026-11-30"))));
    let colors: Vec<_> = snapshot.quests.iter().map(|q| q.color).collect();
    assert_eq!(colors, [QuestColor::Red, QuestColor::Orange, QuestColor::Yellow], "each new quest gets a free colour");
    assert_eq!(plan.created, [Created::Quest(tax.id), Created::Quest(snapshot.quests[2].id), Created::Task(task.id)]);
}

#[test]
fn a_task_of_a_deselected_new_quest_gets_no_quest() {
    let mut world = World::new();
    let proposal = CaptureProposal {
        quests: vec![new_quest("q1", "Steuererklärung")],
        tasks: vec![new_task("t1", "Belege sammeln", Some(QuestRef::New { key: key("q1") }))],
        ..CaptureProposal::default()
    };
    let mut draft = Draft::new(proposal);
    draft.set_selected(&key("q1"), false);
    apply(&mut world, &draft.confirmed());
    let snapshot = world.snapshot();
    assert!(snapshot.quests.is_empty());
    assert_eq!(snapshot.tasks[0].quest_id, None);
}

#[test]
fn repeats_start_on_the_plan_date() {
    let mut world = World::new();
    let mut weekly = new_task("t1", "Standup", None);
    weekly.repeat = Some(ProposedRepeat {
        every: 1,
        frequency: Frequency::Week,
        weekdays: vec![Weekday::Mon, Weekday::Wed],
        month_day: None,
        ends: None,
    });
    let mut unplanned = new_task("t2", "Someday", None);
    unplanned.planned_on = None;
    unplanned.repeat = weekly.repeat.clone();
    apply(&mut world, &CaptureProposal { tasks: vec![weekly, unplanned], ..CaptureProposal::default() });
    let snapshot = world.snapshot();
    let standup = &snapshot.tasks.iter().find(|t| t.title == "Standup").unwrap();
    let rule = standup.repeat.unwrap().rule;
    assert_eq!((rule.starts, rule.time_zone, rule.every), (day("2026-11-25"), BERLIN, 1));
    assert!(rule.weekdays.contains(Weekday::Mon) && rule.weekdays.contains(Weekday::Wed));
    let someday = &snapshot.tasks.iter().find(|t| t.title == "Someday").unwrap();
    assert_eq!((someday.planned_on, someday.repeat), (None, None), "an unplanned task does not repeat");
}

#[test]
fn work_on_an_existing_task_counts_towards_its_quest() {
    let mut world = World::new();
    let quest = world.quest("Website-Relaunch");
    let task = world.task("Konzept", Some(quest), Some(day("2026-11-24")));
    let work = session("w1", WorkTarget::Existing { id: task }, "2026-11-24", time(9, 0), time(11, 0));
    apply(&mut world, &CaptureProposal { work: vec![work], ..CaptureProposal::default() });
    let entry = &world.snapshot().time_entries[0];
    assert_eq!((entry.task_id, entry.quest_id), (Some(task), Some(quest)));
    assert_eq!(entry.started_at, "2026-11-24T08:00:00Z".parse::<DateTime<Utc>>().unwrap());
    assert_eq!(entry.ended_at, Some("2026-11-24T10:00:00Z".parse().unwrap()));
}

#[test]
fn work_can_be_logged_on_a_done_task() {
    let mut world = World::new();
    let task = world.task("Angebot", None, Some(day("2026-11-23")));
    world.run(&[Command::CompleteTask { id: task, at: now() }]).unwrap();
    let work = session("w1", WorkTarget::Existing { id: task }, "2026-11-23", time(14, 0), time(15, 30));
    apply(&mut world, &CaptureProposal { work: vec![work], ..CaptureProposal::default() });
    let snapshot = world.snapshot();
    assert!(snapshot.task(task).unwrap().is_done());
    assert_eq!(snapshot.task_tracked_ms(task, now()), 90 * 60_000);
}

#[test]
fn work_on_a_new_task_creates_it_done_in_the_new_quest() {
    let mut world = World::new();
    let target = WorkTarget::New { title: "Relaunch-Konzept".into(), quest: Some(QuestRef::New { key: key("q1") }) };
    let proposal = CaptureProposal {
        quests: vec![new_quest("q1", "Relaunch")],
        work: vec![session("w1", target, "2026-11-24", time(9, 0), time(11, 0))],
        ..CaptureProposal::default()
    };
    let plan = apply(&mut world, &proposal);
    let snapshot = world.snapshot();
    let (quest, task, entry) = (&snapshot.quests[0], &snapshot.tasks[0], &snapshot.time_entries[0]);
    assert!(task.is_done());
    assert_eq!((task.quest_id, task.planned_on), (Some(quest.id), Some(day("2026-11-24"))));
    assert_eq!((entry.task_id, entry.quest_id), (Some(task.id), Some(quest.id)));
    assert_eq!(plan.created.len(), 3);
}

#[test]
fn an_end_at_midnight_ends_the_day() {
    let mut world = World::new();
    let target = WorkTarget::New { title: "Late".into(), quest: None };
    let work = session("w1", target, "2026-11-23", time(23, 0), NaiveTime::MIN);
    apply(&mut world, &CaptureProposal { work: vec![work], ..CaptureProposal::default() });
    let entry = &world.snapshot().time_entries[0];
    assert_eq!(entry.started_at, "2026-11-23T22:00:00Z".parse::<DateTime<Utc>>().unwrap());
    assert_eq!(entry.ended_at, Some("2026-11-23T23:00:00Z".parse().unwrap()));
}

#[test]
fn midnight_on_a_daylight_saving_day_is_the_local_one() {
    // Berlin falls back on 2026-10-25: that day has 25 hours.
    let target = WorkTarget::New { title: "Long day".into(), quest: None };
    let work = session("w1", target, "2026-10-25", time(1, 0), NaiveTime::MIN);
    let (start, end) = session_times(&work, BERLIN).unwrap();
    assert_eq!((end - start).num_hours(), 24);
    let whole_day = session(
        "w2",
        WorkTarget::New { title: "All day".into(), quest: None },
        "2026-10-25",
        time(0, 0),
        NaiveTime::MIN,
    );
    assert_eq!(
        session_times(&whole_day, BERLIN),
        Err(ApplyError::InvalidTimeRange),
        "00:00 to midnight is not a session"
    );
    let skipped = instant(BERLIN, day("2026-03-29").and_time(time(2, 30)));
    assert_eq!(skipped, "2026-03-29T01:30:00Z".parse::<DateTime<Utc>>().unwrap(), "a skipped time reads an hour later");
}

#[test]
fn stale_references_and_empty_titles_are_refused() {
    let mut world = World::new();
    let gone = EntityId::new();
    let snapshot = world.snapshot();
    let with_task = |task| CaptureProposal { tasks: vec![task], ..CaptureProposal::default() };
    let refused = |proposal: &CaptureProposal| plan(proposal, &snapshot, now(), BERLIN, EntityId::new).unwrap_err();
    assert_eq!(
        refused(&with_task(new_task("t1", "T", Some(QuestRef::Existing { id: gone })))),
        ApplyError::UnknownQuest(gone)
    );
    assert_eq!(refused(&with_task(new_task("t1", "  ", None))), ApplyError::EmptyTitle);
    let work = session("w1", WorkTarget::Existing { id: gone }, "2026-11-24", time(9, 0), time(10, 0));
    assert_eq!(
        refused(&CaptureProposal { work: vec![work], ..CaptureProposal::default() }),
        ApplyError::UnknownTask(gone)
    );
    let backwards =
        session("w1", WorkTarget::New { title: "T".into(), quest: None }, "2026-11-24", time(10, 0), time(9, 0));
    let backwards = CaptureProposal { work: vec![backwards], ..CaptureProposal::default() };
    assert_eq!(refused(&backwards), ApplyError::InvalidTimeRange);
    assert!(world.run(&[]).is_ok());
}

#[test]
fn undo_takes_everything_back_and_skips_what_is_gone() {
    let mut world = World::new();
    let target = WorkTarget::New { title: "Konzept".into(), quest: Some(QuestRef::New { key: key("q1") }) };
    let proposal = CaptureProposal {
        quests: vec![new_quest("q1", "Relaunch")],
        tasks: vec![new_task("t1", "Angebot", Some(QuestRef::New { key: key("q1") }))],
        work: vec![session("w1", target, "2026-11-24", time(9, 0), time(10, 0))],
        ..CaptureProposal::default()
    };
    let plan = apply(&mut world, &proposal);
    let first_task = world.snapshot().tasks.iter().find(|t| t.title == "Angebot").unwrap().id;
    world.run(&[Command::DeleteTask { id: first_task, at: now() }]).unwrap();
    let commands = undo(&plan.created, &world.snapshot(), now());
    assert_eq!(commands.len(), 3, "the deleted task is skipped: {commands:?}");
    world.run(&commands).unwrap();
    let snapshot = world.snapshot();
    assert!(snapshot.quests.is_empty() && snapshot.tasks.is_empty() && snapshot.time_entries.is_empty());
}
