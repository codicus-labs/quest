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
