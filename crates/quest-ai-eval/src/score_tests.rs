use chrono::{NaiveDate, NaiveTime, Weekday};
use quest_domain::{Frequency, MonthDay, TimeBlock};
use quest_protocol::{
    CaptureProposal, ProposedQuest, ProposedRepeat, ProposedTask, ProposedWork, QuestRef, TempKey, WorkTarget,
};

use super::*;
use crate::cases::{EvalSet, Expect};

fn expect(json: &str) -> Expect {
    serde_json::from_str(json).unwrap()
}

fn day(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn at(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
}

fn task(title: &str, on: Option<&str>) -> ProposedTask {
    ProposedTask {
        key: TempKey("t1".into()),
        title: title.into(),
        description: String::new(),
        quest: None,
        quest_uncertain: false,
        day: on.map(day),
        block: None,
        repeat: None,
    }
}

fn proposal(tasks: Vec<ProposedTask>) -> CaptureProposal {
    CaptureProposal { quests: vec![], tasks, work: vec![], dropped: vec![] }
}

#[test]
fn a_matching_task_is_a_hit() {
    let expected = expect(r#"{ "tasks": [{ "title": ["rechnung"], "day": "2026-11-25", "block": null }] }"#);
    let result = score(&expected, &proposal(vec![task("Rechnung Müller schreiben", Some("2026-11-25"))]));
    assert!(result.is_hit(), "{result:?}");
    assert_eq!(result.total, 5, "found, day, block, no extras, clean titles");
}

#[test]
fn wrong_fields_extra_items_and_filler_cost_points() {
    let expected = expect(r#"{ "tasks": [{ "title": ["rechnung"], "day": "2026-11-25" }] }"#);
    let actual = proposal(vec![task("Äh Rechnung", Some("2026-11-26")), task("Test", None)]);
    let result = score(&expected, &actual);
    assert_eq!((result.passed, result.total), (1, 4), "{result:?}");
    assert_eq!(result.failures.len(), 3);
}

#[test]
fn a_missing_task_fails_all_its_checks() {
    let expected = expect(r#"{ "tasks": [{ "title": ["blumen"], "day": "2026-11-25", "quest": null }] }"#);
    let result = score(&expected, &proposal(vec![]));
    assert_eq!((result.passed, result.total), (2, 5), "missing item and its two fields; no extras; clean");
}

#[test]
fn alternatives_and_null_days_are_accepted() {
    let expected = expect(
        r#"{ "tasks": [{ "title": ["a"], "day": ["2026-11-25", "2026-12-02"] }, { "title": ["b"], "day": null }] }"#,
    );
    let result = score(&expected, &proposal(vec![task("A", Some("2026-12-02")), task("B", None)]));
    assert!(result.is_hit(), "{result:?}");
}

#[test]
fn quests_are_checked_by_fixture_key_or_as_new() {
    let expected =
        expect(r#"{ "tasks": [{ "title": ["a"], "quest": "relaunch" }, { "title": ["b"], "quest": "new" }] }"#);
    let mut a = task("A", None);
    a.quest = Some(QuestRef::Existing { id: EvalSet::id("relaunch") });
    let mut b = task("B", None);
    b.quest = Some(QuestRef::New { key: TempKey("q1".into()) });
    let mut actual = proposal(vec![a, b]);
    actual.quests.push(ProposedQuest {
        key: TempKey("q1".into()),
        title: "Umzug".into(),
        description: String::new(),
        milestones: vec![],
    });
    assert!(score(&expected, &actual).is_hit());
    actual.tasks[0].quest = Some(QuestRef::Existing { id: EvalSet::id("marathon") });
    assert!(!score(&expected, &actual).is_hit());
}

#[test]
fn blocks_and_repeats_compare_their_fields() {
    let expected = expect(
        r#"{ "tasks": [{ "title": ["planung"], "block": { "start": "08:00", "minutes": 30 },
             "repeat": { "every": 1, "frequency": "week", "weekdays": ["wed", "mon"] } }] }"#,
    );
    let mut planning = task("Wochenplanung", Some("2026-11-30"));
    planning.block = Some(TimeBlock { start: at(8, 0), minutes: 30 });
    planning.repeat = Some(ProposedRepeat {
        every: 1,
        frequency: Frequency::Week,
        weekdays: vec![Weekday::Mon, Weekday::Wed],
        month_day: None,
        ends: None,
    });
    assert!(score(&expected, &proposal(vec![planning.clone()])).is_hit());
    planning.repeat.as_mut().unwrap().weekdays = vec![Weekday::Mon];
    assert!(!score(&expected, &proposal(vec![planning])).is_hit());
}

#[test]
fn last_day_of_month_repeats_match() {
    let expected = expect(
        r#"{ "tasks": [{ "title": ["rechnung"], "repeat": { "every": 1, "frequency": "month", "month_day": "last" } }] }"#,
    );
    let mut bills = task("Rechnungen", Some("2026-11-30"));
    bills.repeat = Some(ProposedRepeat {
        every: 1,
        frequency: Frequency::Month,
        weekdays: vec![],
        month_day: Some(MonthDay::Last),
        ends: None,
    });
    assert!(score(&expected, &proposal(vec![bills])).is_hit());
}

#[test]
fn work_matches_existing_or_new_tasks_and_compares_times() {
    let expected = expect(
        r#"{ "work": [
            { "task": "kapitel3", "day": "2026-11-23", "start": "20:00", "end": "00:00" },
            { "task": { "new": ["mail"] }, "day": "2026-11-24", "start": "10:00", "end": "10:30" }
        ], "tasks": [] }"#,
    );
    let work = |task, start, end| ProposedWork {
        key: TempKey("w1".into()),
        task,
        uncertain: false,
        day: day("2026-11-23"),
        start,
        end,
    };
    let chapter = work(WorkTarget::Existing { id: EvalSet::id("kapitel3") }, at(20, 0), at(0, 0));
    let mut mails = work(WorkTarget::New { title: "Mails beantworten".into(), quest: None }, at(10, 0), at(10, 30));
    mails.day = day("2026-11-24");
    let mut actual = proposal(vec![]);
    actual.work = vec![chapter, mails];
    assert!(score(&expected, &actual).is_hit(), "{:?}", score(&expected, &actual));
    actual.work[1].end = at(11, 0);
    assert_eq!(score(&expected, &actual).failures.len(), 1);
}

#[test]
fn an_empty_expectation_wants_nothing() {
    let expected = expect(r#"{ "quests": [], "tasks": [], "work": [] }"#);
    assert!(score(&expected, &proposal(vec![])).is_hit());
    assert!(!score(&expected, &proposal(vec![task("Erfunden", None)])).is_hit());
}

#[test]
fn filler_words_are_whole_words_only() {
    assert!(has_filler("Ähm Rechnung"));
    assert!(has_filler("Rechnung, ne"));
    assert!(!has_filler("Nebenkosten prüfen"));
    assert!(!has_filler("Kalender halten"));
}

#[test]
fn repeat_end_dates_are_checked_when_expected() {
    let expected = expect(
        r#"{ "tasks": [{ "title": ["jogg"], "repeat": { "every": 1, "frequency": "week", "until": ["2026-12-24", "2026-12-25"] } }] }"#,
    );
    let mut jogging = task("Joggen", Some("2026-11-25"));
    jogging.repeat = Some(ProposedRepeat {
        every: 1,
        frequency: Frequency::Week,
        weekdays: vec![Weekday::Wed, Weekday::Fri],
        month_day: None,
        ends: Some(day("2026-12-24")),
    });
    assert!(score(&expected, &proposal(vec![jogging.clone()])).is_hit());
    jogging.repeat.as_mut().unwrap().ends = None;
    assert!(!score(&expected, &proposal(vec![jogging])).is_hit());
}
