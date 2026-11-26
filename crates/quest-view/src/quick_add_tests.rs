use super::*;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 23).unwrap()
}

#[test]
fn ordered_dates_take_the_other_boundary_along() {
    let start = today();
    let parse = |text| parse(text, Fields::default(), start, &[]);
    let fri = resolve_day("fri", start);
    let wed = resolve_day("we", start);
    assert_eq!(parse("Draft @fri !we").fields.dates, TaskDates { planned_on: wed, due_on: wed });
    assert_eq!(parse("Draft !we @fri").fields.dates, TaskDates { planned_on: fri, due_on: fri });
    assert_eq!(parse("Draft !bad @tod").title, "Draft !bad");
    assert_eq!(resolve_day("MON", start), Some(start));
    assert_eq!(resolve_day("tomorrow", start), start.succ_opt());
}

#[test]
fn identifies_the_active_quest_token_without_consuming_title_text() {
    let input = "Café #reading by Friday";
    let (range, query) = quest_token(input, "Café #rea".len()).unwrap();
    assert_eq!(query, "rea");
    assert_eq!(&input[range], "#reading");
    assert!(quest_token("Word#embedded", 13).is_none());
    assert!(quest_token(input, 4).is_none());
}

#[test]
fn known_quest_tokens_strip_and_keep_recent_order_for_prefix_matches() {
    use quest_domain::{QuestColor, QuestStatus};
    let quest = |title: &str| Quest {
        id: EntityId::new(),
        title: title.into(),
        color: QuestColor::Blue,
        status: QuestStatus::Active,
        sort_key: String::new(),
    };
    let recent = quest("Read books");
    let older = quest("Reading room");
    assert_eq!(quest_matches(&[&recent, &older], "Rea"), [&recent, &older]);
    let result = parse("Buy books #rea @tom", Fields::default(), today(), &[&recent, &older]);
    assert_eq!(result.title, "Buy books");
    assert_eq!(result.fields.quest, Some(recent.id));
    assert_eq!(result.fields.dates.planned_on, today().succ_opt());
}

#[test]
fn a_recurring_draft_creates_the_task_note_and_rule_in_one_batch() {
    use quest_domain::{FieldStore, Frequency, HlcClock, NodeId, RepeatPin, Weekdays, plan_all};

    let id = EntityId::new();
    let now = today().and_hms_opt(9, 0, 0).unwrap().and_utc();
    let rule = RepeatRule {
        every: 1,
        frequency: Frequency::Week,
        starts: today(),
        ends: None,
        time_zone: chrono_tz::UTC,
        pin: RepeatPin::None,
        weekdays: Weekdays::default(),
        month_day: None,
    };
    let draft = parse("Write a sketch", Fields::default(), today(), &[]);
    let workspace = EntityId::DEFAULT_WORKSPACE;
    let store = FieldStore::default();
    let clock = HlcClock::new(NodeId(1));
    let commands = draft.clone().commands_with_repeat(id, Position::Bottom, "## Idea", Some(rule), now);
    let batch = plan_all(&commands, &store, workspace, &clock, 1).unwrap();
    let snapshot = Snapshot::from_store(&batch.store, workspace);
    let task = snapshot.task(id).unwrap();
    assert_eq!(task.description, "## Idea");
    assert_eq!(task.repeat.unwrap().rule, rule);
    assert_eq!(task.planned_on, Some(today()));

    let invalid = RepeatRule { every: 0, ..rule };
    let commands = draft.commands_with_repeat(id, Position::Bottom, "", Some(invalid), now);
    assert!(matches!(
        plan_all(&commands, &store, workspace, &clock, 1),
        Err(quest_domain::CommandError::InvalidRepeatRule)
    ));
    assert!(Snapshot::from_store(&store, workspace).task(id).is_none());
    assert!(
        parse("", Fields::default(), today(), &[])
            .commands_with_repeat(id, Position::Bottom, "", Some(rule), now)
            .is_empty()
    );
}

#[test]
fn landing_uses_today_then_plan_then_quest_and_keeps_future_deadline_only_in_inbox() {
    use quest_domain::{FieldStore, HlcClock, NodeId, QuestColor, plan};
    let quest = EntityId::new();
    let mut store = FieldStore::default();
    let mut clock = HlcClock::new(NodeId(1));
    let mut snap = Snapshot::default();
    let mut apply = |command| {
        for write in plan(&command, &snap).unwrap() {
            store.apply(&write.stamp(clock.now(1)));
        }
        snap = Snapshot::from_store(&store, EntityId::DEFAULT_WORKSPACE);
        snap.clone()
    };
    let quests = apply(Command::CreateQuest { id: quest, title: "Writing".into(), color: QuestColor::Blue });
    let quests = quests.active_quests().collect::<Vec<_>>();
    for (input, label) in [
        ("Book dentist @tod", "Today"),
        ("Draft #Writing @fri", "Plan · Fri 27"),
        ("Draft #Writing !tod", "Today"),
        ("Draft #Writing", "Writing"),
        ("Book dentist !fri", "Inbox"),
        ("Book dentist", "Inbox"),
    ] {
        let id = EntityId::new();
        let draft = parse(input, Fields::default(), today(), &quests);
        let snapshot = apply(draft.command(id, Position::Bottom).unwrap());
        assert_eq!(landing(snapshot.task(id).unwrap(), &snapshot, today()).label(), label, "{input}");
    }
}
