use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use quest_domain::{FieldStore, Hlc, HlcClock, NodeId, PlannedBatch, plan_all};

use super::*;
use crate::mcp::types::{BlockInput, CallContext, ColorInput, ItemKind};
use crate::mcp::view::find_task;

struct World {
    store: FieldStore,
    clock: Clock,
}

impl World {
    fn new() -> World {
        let now = Utc.with_ymd_and_hms(2026, 11, 24, 9, 0, 0).unwrap();
        World { store: FieldStore::default(), clock: Clock { now, zone: "Europe/Berlin".parse().unwrap() } }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot::from_store(&self.store, EntityId::DEFAULT_WORKSPACE)
    }

    fn run(&mut self, commands: &[Command]) {
        let clock = HlcClock::resume(NodeId(7), self.store.highest_hlc().unwrap_or(Hlc::ZERO));
        let ms = u64::try_from(self.clock.now.timestamp_millis()).unwrap();
        let PlannedBatch { store, .. } =
            plan_all(commands, &self.store, EntityId::DEFAULT_WORKSPACE, &clock, ms).unwrap();
        self.store = store;
    }

    fn decide(&mut self, decision: Result<(Vec<Command>, EntityId), ToolError>) -> EntityId {
        let (commands, id) = decision.unwrap();
        self.run(&commands);
        id
    }

    fn quest(&mut self, title: &str) -> EntityId {
        let input =
            CreateQuestInput { title: title.into(), description: None, color: None, context: CallContext::default() };
        let decision = create_quest(&self.snapshot(), &input, &mut EntityId::new);
        self.decide(decision)
    }

    fn task(&mut self, input: CreateTaskInput) -> EntityId {
        let decision = create_task(&self.snapshot(), &input, &self.clock.clone(), &mut EntityId::new);
        self.decide(decision)
    }
}

fn new_task(title: &str) -> CreateTaskInput {
    CreateTaskInput {
        title: title.into(),
        description: None,
        day: None,
        quest_id: None,
        block: None,
        repeat: None,
        context: CallContext::default(),
    }
}

fn update(task: EntityId) -> UpdateTaskInput {
    UpdateTaskInput {
        task_id: task.to_string(),
        title: None,
        description: None,
        day: None,
        quest_id: None,
        block: None,
        clear_block: false,
        repeat: None,
        clear_repeat: false,
        context: CallContext::default(),
    }
}

fn daily() -> RepeatInput {
    RepeatInput {
        every: None,
        frequency: FrequencyInput::Day,
        weekdays: vec![],
        month_day: None,
        starts: None,
        ends: None,
    }
}

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, d).unwrap()
}

#[test]
fn create_task_lands_on_today_with_quest_block_and_description() {
    let mut world = World::new();
    let quest = world.quest("Fitness");
    let id = world.task(CreateTaskInput {
        description: Some("  3 sets  ".into()),
        quest_id: Some(quest.to_string()),
        block: Some(BlockInput { start: "18:00".into(), minutes: 60 }),
        ..new_task(" Run ")
    });
    let snapshot = world.snapshot();
    let task = snapshot.task(id).unwrap();
    assert_eq!((task.title.as_str(), task.description.as_str()), ("Run", "3 sets"));
    assert_eq!((task.day, task.quest_id), (Some(day(24)), Some(quest)));
    assert_eq!(task.block.unwrap().start, NaiveTime::from_hms_opt(18, 0, 0).unwrap());
}

#[test]
fn create_task_refuses_a_block_in_the_backlog_and_unknown_quests() {
    let world = World::new();
    let backlog_block = CreateTaskInput {
        day: Some("someday".into()),
        block: Some(BlockInput { start: "09:00".into(), minutes: 30 }),
        ..new_task("x")
    };
    assert_eq!(
        create_task(&world.snapshot(), &backlog_block, &world.clock, &mut EntityId::new).unwrap_err().code,
        ToolCode::InvalidInput
    );
    let unknown = CreateTaskInput { quest_id: Some(EntityId::new().to_string()), ..new_task("x") };
    assert_eq!(
        create_task(&world.snapshot(), &unknown, &world.clock, &mut EntityId::new).unwrap_err().code,
        ToolCode::QuestNotFound
    );
    let empty = new_task("  ");
    assert_eq!(
        create_task(&world.snapshot(), &empty, &world.clock, &mut EntityId::new).unwrap_err().code,
        ToolCode::EmptyTitle
    );
}

#[test]
fn update_task_changes_only_given_fields_and_needs_one() {
    let mut world = World::new();
    let id = world.task(new_task("Write report"));
    let decision =
        update_task(&world.snapshot(), &UpdateTaskInput { day: Some("2026-11-26".into()), ..update(id) }, &world.clock);
    world.decide(decision);
    let snapshot = world.snapshot();
    let task = snapshot.task(id).unwrap();
    assert_eq!((task.title.as_str(), task.day), ("Write report", Some(day(26))));
    assert_eq!(update_task(&snapshot, &update(id), &world.clock).unwrap_err().code, ToolCode::InvalidInput);
}

#[test]
fn editing_a_planned_repeat_saves_it_first() {
    let mut world = World::new();
    let series = world.task(CreateTaskInput { repeat: Some(daily()), ..new_task("Stretch") });
    let snapshot = world.snapshot();
    let planned = snapshot.planned_occurrences(day(25), world.clock.now).next().expect("tomorrow's repeat");
    assert_eq!(planned.series, Some(series));
    let decision = complete_task(&snapshot, &planned.id.to_string(), &world.clock);
    world.decide(decision);
    let snapshot = world.snapshot();
    let saved = snapshot.task(planned.id).expect("the repeat is saved");
    assert!(saved.is_done());
    // Done ahead of time: it moves to the day it was done on, as in the apps.
    assert_eq!(saved.day, Some(day(24)));
}

#[test]
fn repeat_on_an_occurrence_edits_its_series() {
    let mut world = World::new();
    let series = world.task(CreateTaskInput { repeat: Some(daily()), ..new_task("Stretch") });
    let snapshot = world.snapshot();
    let planned = snapshot.planned_occurrences(day(25), world.clock.now).next().unwrap();
    let weekly = RepeatInput { frequency: FrequencyInput::Week, weekdays: vec!["mon".into(), "thu".into()], ..daily() };
    let (commands, _) =
        update_task(&snapshot, &UpdateTaskInput { repeat: Some(weekly), ..update(planned.id) }, &world.clock).unwrap();
    assert!(
        matches!(&commands[..], [Command::SetTaskRepeat { id, rule: Some(rule), .. }] if *id == series && rule.frequency == Frequency::Week)
    );
}

#[test]
fn repeat_rules_are_checked() {
    let clock = World::new().clock;
    let bad_weekday = RepeatInput { frequency: FrequencyInput::Week, weekdays: vec!["funday".into()], ..daily() };
    assert_eq!(repeat_rule(&bad_weekday, None, None, &clock).unwrap_err().code, ToolCode::InvalidRepeatRule);
    let weekdays_on_daily = RepeatInput { weekdays: vec!["mon".into()], ..daily() };
    assert_eq!(repeat_rule(&weekdays_on_daily, None, None, &clock).unwrap_err().code, ToolCode::InvalidRepeatRule);
    let last = RepeatInput { frequency: FrequencyInput::Month, month_day: Some("last".into()), ..daily() };
    assert_eq!(repeat_rule(&last, Some(day(30)), None, &clock).unwrap().month_day, Some(MonthDay::Last));
    // Starts on the task's day, but not in the past.
    assert_eq!(repeat_rule(&daily(), Some(day(1)), None, &clock).unwrap().starts, day(24));
}

#[test]
fn complete_and_reopen_are_idempotent() {
    let mut world = World::new();
    let id = world.task(new_task("Call mum"));
    let decision = complete_task(&world.snapshot(), &id.to_string(), &world.clock);
    world.decide(decision);
    assert!(complete_task(&world.snapshot(), &id.to_string(), &world.clock).unwrap().0.is_empty());
    let decision = reopen_task(&world.snapshot(), &id.to_string(), &world.clock);
    world.decide(decision);
    assert!(!world.snapshot().task(id).unwrap().is_done());
    assert!(reopen_task(&world.snapshot(), &id.to_string(), &world.clock).unwrap().0.is_empty());
}

#[test]
fn archive_and_restore_a_task() {
    let mut world = World::new();
    let id = world.task(new_task("Old idea"));
    let decision = archive_task(&world.snapshot(), &id.to_string(), &world.clock);
    world.decide(decision);
    assert!(world.snapshot().task(id).is_none());
    let (commands, _) =
        restore(&RestoreInput { kind: ItemKind::Task, id: id.to_string(), context: CallContext::default() }).unwrap();
    world.run(&commands);
    assert!(find_task(&world.snapshot(), id, &world.clock).is_some());
}

#[test]
fn log_work_needs_a_past_range_and_one_task() {
    let mut world = World::new();
    let input = |start: &str, end: &str| LogWorkInput {
        task_id: None,
        new_task_title: Some("Review PR".into()),
        quest_id: None,
        day: Some("2026-11-24".into()),
        start: start.into(),
        end: end.into(),
        context: CallContext::default(),
    };
    let snapshot = world.snapshot();
    assert_eq!(
        log_work(&snapshot, &input("09:00", "08:00"), &world.clock, &mut EntityId::new).unwrap_err().code,
        ToolCode::InvalidTimeRange
    );
    // 11:00 Berlin is after "now" (09:00 UTC = 10:00 Berlin).
    assert_eq!(
        log_work(&snapshot, &input("09:00", "11:00"), &world.clock, &mut EntityId::new).unwrap_err().code,
        ToolCode::InvalidTimeRange
    );
    let entry = world.decide(log_work(&snapshot, &input("08:00", "09:30"), &world.clock, &mut EntityId::new));
    let snapshot = world.snapshot();
    let entry = snapshot.time_entries.iter().find(|e| e.id == entry).unwrap();
    assert_eq!(entry.tracked_ms(world.clock.now), 90 * 60_000);
    assert!(snapshot.task(entry.task_id.unwrap()).unwrap().is_done());
}

#[test]
fn log_work_on_a_planned_repeat_saves_it_first() {
    let mut world = World::new();
    world.task(CreateTaskInput { repeat: Some(daily()), ..new_task("Stretch") });
    let snapshot = world.snapshot();
    let planned = snapshot.planned_occurrences(day(25), world.clock.now).next().unwrap();
    let input = LogWorkInput {
        task_id: Some(planned.id.to_string()),
        new_task_title: None,
        quest_id: None,
        day: None,
        start: "08:00".into(),
        end: "08:30".into(),
        context: CallContext::default(),
    };
    let entry = world.decide(log_work(&snapshot, &input, &world.clock, &mut EntityId::new));
    let snapshot = world.snapshot();
    assert!(snapshot.task(planned.id).is_some(), "the repeat is saved");
    let entry = snapshot.time_entries.iter().find(|e| e.id == entry).unwrap();
    assert_eq!(entry.task_id, Some(planned.id));
}

#[test]
fn timer_starts_on_a_task_and_stopping_twice_is_fine() {
    let mut world = World::new();
    let id = world.task(new_task("Deep work"));
    let decision = start_timer(&world.snapshot(), &id.to_string(), &world.clock, &mut EntityId::new);
    world.decide(decision);
    assert_eq!(world.snapshot().running_entry().unwrap().task_id, Some(id));
    let (commands, stopped) = stop_timer(&world.snapshot(), &world.clock);
    assert!(stopped.is_some());
    world.run(&commands);
    assert_eq!(stop_timer(&world.snapshot(), &world.clock), (Vec::new(), None));
}

#[test]
fn quests_get_updated_and_completed() {
    let mut world = World::new();
    let quest = world.quest("Learn Rust");
    let input = UpdateQuestInput {
        quest_id: quest.to_string(),
        title: None,
        description: Some("Ownership first".into()),
        color: Some(ColorInput::Teal),
        status: None,
        context: CallContext::default(),
    };
    let decision = update_quest(&world.snapshot(), &input);
    world.decide(decision);
    let decision = complete_quest(&world.snapshot(), &quest.to_string());
    world.decide(decision);
    let snapshot = world.snapshot();
    let saved = snapshot.quest(quest).unwrap();
    assert_eq!((saved.description.as_str(), saved.status), ("Ownership first", QuestStatus::Completed));
    assert!(complete_quest(&snapshot, &quest.to_string()).unwrap().0.is_empty());
}

#[test]
fn milestones_are_added_renamed_and_checked() {
    let mut world = World::new();
    let quest = world.quest("Marathon");
    let add = AddMilestoneInput {
        quest_id: quest.to_string(),
        title: "10 km".into(),
        achieved_on: None,
        context: CallContext::default(),
    };
    let decision = add_milestone(&world.snapshot(), &add, &world.clock, &mut EntityId::new);
    let id = world.decide(decision);
    let rename = UpdateMilestoneInput {
        milestone_id: id.to_string(),
        title: Some("First 10 km".into()),
        achieved_on: None,
        context: CallContext::default(),
    };
    let decision = update_milestone(&world.snapshot(), &rename);
    world.decide(decision);
    assert_eq!(world.snapshot().milestones[0].title, "First 10 km");
    let unknown = UpdateMilestoneInput { milestone_id: EntityId::new().to_string(), ..rename };
    assert_eq!(update_milestone(&world.snapshot(), &unknown).unwrap_err().code, ToolCode::MilestoneNotFound);
}
