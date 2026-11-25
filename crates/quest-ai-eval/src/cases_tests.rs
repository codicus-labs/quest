use std::collections::BTreeSet;

use super::*;
use crate::cases::ExpectWorkTask;

#[test]
fn the_eval_set_loads_and_every_request_passes_the_servers_checks() {
    let set = EvalSet::load().unwrap();
    assert!(set.cases.len() >= 30, "about 30 cases");
    for case in &set.cases {
        quest_capture::check_request(&set.request(case)).unwrap_or_else(|error| panic!("{}: {error}", case.id));
    }
}

#[test]
fn case_ids_are_unique() {
    let set = EvalSet::load().unwrap();
    let ids: BTreeSet<&str> = set.cases.iter().map(|case| case.id.as_str()).collect();
    assert_eq!(ids.len(), set.cases.len());
}

#[test]
fn expectations_only_name_fixture_quests_and_tasks() {
    let set = EvalSet::load().unwrap();
    for case in &set.cases {
        for task in case.expect.tasks.iter().flatten() {
            if let Some(Some(quest)) = &task.quest {
                assert!(quest == "new" || set.quests.contains_key(quest), "{}: unknown quest {quest}", case.id);
            }
        }
        for work in case.expect.work.iter().flatten() {
            if let ExpectWorkTask::Existing(key) = &work.task {
                assert!(set.tasks.contains_key(key), "{}: unknown task {key}", case.id);
            }
        }
    }
    for task in set.tasks.values() {
        if let Some(quest) = &task.quest {
            assert!(set.quests.contains_key(quest), "fixture task names unknown quest {quest}");
        }
    }
}

#[test]
fn left_out_and_null_fields_are_told_apart() {
    let expect: Expect =
        serde_json::from_str(r#"{ "tasks": [{ "title": ["a"], "due_on": null, "quest": "relaunch" }] }"#).unwrap();
    let task = &expect.tasks.unwrap()[0];
    assert!(matches!(task.due_on, Some(None)), "null: must have no deadline");
    assert!(task.planned_on.is_none(), "left out: not checked");
    assert_eq!(task.quest, Some(Some("relaunch".to_owned())));
}

#[test]
fn fixture_ids_are_stable_and_distinct() {
    assert_eq!(EvalSet::id("relaunch"), EvalSet::id("relaunch"));
    assert_ne!(EvalSet::id("relaunch"), EvalSet::id("marathon"));
}

#[test]
fn typos_in_repeat_expectations_are_refused() {
    let repeat = |json: &str| {
        serde_json::from_str::<Expect>(&format!(r#"{{ "tasks": [{{ "title": ["a"], "repeat": {json} }}] }}"#))
    };
    assert!(repeat(r#"{ "every": 1, "frequency": "week", "weekdays": ["mon"] }"#).is_ok());
    assert!(repeat(r#"{ "every": 1, "frequency": "weekly" }"#).is_err());
    assert!(repeat(r#"{ "every": 1, "frequency": "week", "weekdays": ["mond"] }"#).is_err());
}
