use quest_domain::Snapshot;

use super::*;
use crate::test_support::{berlin, id, quest, task};

/// Wednesday, 25 Nov 2026, 14:40 in Berlin.
fn now() -> Now {
    berlin("2026-11-25T14:40:00+01:00")
}

fn snapshot() -> Snapshot {
    let mut done = task(11, "Old draft", Some(1), None);
    done.completed_at = Some(crate::test_support::at("2026-11-20T10:00:00Z"));
    Snapshot {
        quests: vec![quest(1, "Photo series")],
        tasks: vec![task(10, "Draft the plan", Some(1), Some("2026-11-25")), done],
        ..Snapshot::default()
    }
}

fn titles(results: &[SearchResult]) -> Vec<&str> {
    results.iter().map(|r| r.title.as_str()).collect()
}

#[test]
fn an_empty_query_lists_jumps_and_actions_in_the_palettes_order() {
    let results = search(&snapshot(), &SearchMode::Commands, "", now()).unwrap();
    assert_eq!(results[0].token, SearchToken::Go { view: AppView::Home });
    assert_eq!((results[0].hint.as_str(), results[0].kind), ("Jump", SearchKind::Jump));
    assert!(results.iter().any(|r| r.token == SearchToken::Go { view: AppView::Archive }));
    let log = results.iter().find(|r| r.token == SearchToken::LogWork).unwrap();
    assert_eq!((log.hint.as_str(), log.kind), ("Time", SearchKind::Action));
    let account = results.iter().find(|r| r.token == SearchToken::Account).unwrap();
    assert_eq!(account.hint, "Account", "no ⌘, on a phone");
    let switch = results.iter().find(|r| r.token == SearchToken::Choose { mode: SearchMode::SwitchWorkspace }).unwrap();
    assert_eq!((switch.title.as_str(), switch.hint.as_str()), ("Switch workspace…", "Workspace"));
    assert!(results.iter().any(|r| r.token == SearchToken::ManageWorkspaces));
    assert_eq!(results.len(), 12, "as many as the desktop lists; tasks follow once a query is typed");
    let typed = search(&snapshot(), &SearchMode::Commands, "plan", now()).unwrap();
    assert!(titles(&typed).contains(&"Focus on “Draft the plan”"));
    assert!(!titles(&typed).iter().any(|t| t.contains("Old draft")), "done tasks are not offered");
}

#[test]
fn a_query_finds_tasks_with_their_actions_then_offers_creating() {
    let results = search(&snapshot(), &SearchMode::Commands, " draft ", now()).unwrap();
    assert_eq!(
        titles(&results),
        [
            "Focus on “Draft the plan”",
            "Move “Draft the plan” to…",
            "Delete task “Draft the plan”",
            "Create task “draft”",
            "Create quest “draft”",
        ]
    );
    let task = id(10).to_string();
    assert_eq!(results[1].token, SearchToken::Choose { mode: SearchMode::MoveTo { task_id: task.clone() } });
    assert_eq!(results[1].hint, "Move");
    assert_eq!(results[2].token, SearchToken::DeleteTask { task_id: task });
    assert_eq!(results[2].hint, "Delete");
    assert_eq!(results[3].kind, SearchKind::Create);
}

#[test]
fn quests_are_found_by_name() {
    let results = search(&snapshot(), &SearchMode::Commands, "photo", now()).unwrap();
    assert_eq!(results[0].token, SearchToken::ShowQuest { quest_id: id(1).to_string() });
    assert_eq!(results[0].kind, SearchKind::Quest);
}

#[test]
fn moving_lists_the_coming_week_and_not_planned() {
    let mode = SearchMode::MoveTo { task_id: id(10).to_string() };
    let results = search(&snapshot(), &mode, "", now()).unwrap();
    assert_eq!(results.len(), 8);
    assert_eq!(results[1].token, SearchToken::MoveTask { task_id: id(10).to_string(), day: Some("2026-11-26".into()) });
    assert_eq!(results[7].token, SearchToken::MoveTask { task_id: id(10).to_string(), day: None });
    assert_eq!(results[7].kind, SearchKind::Choice);
    assert_eq!(placeholder(&mode), "Move to which day?");
}

#[test]
fn saving_from_the_search() {
    let today = crate::test_support::day("2026-11-25");
    assert!(matches!(
        create_task(id(5), " Stretch ", today),
        Ok(Command::CreateTask { title, quest_id: None, planned_on: Some(d), position: Position::Top, .. }) if title == "Stretch" && d == today
    ));
    assert_eq!(create_task(id(5), " ", today), Err(MobileError::EmptyTitle));
    let at = now().at;
    let unplan = Command::MoveTask { id: id(10), planned_on: None, position: Position::Bottom, at };
    assert_eq!(move_task(id(10), None, at), Ok(unplan));
    assert!(search(&snapshot(), &SearchMode::MoveTo { task_id: "x".into() }, "", now()).is_err());
}
