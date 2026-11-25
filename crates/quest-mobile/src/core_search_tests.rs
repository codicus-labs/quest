use std::sync::Arc;

use super::*;
use crate::core::ChangeListener;
use crate::test_support::moment;
use crate::types::{AddPosition, HomeQuery, TaskSection};

struct Quiet;

impl ChangeListener for Quiet {
    fn on_change(&self) {}
}

fn open() -> (tempfile::TempDir, Arc<QuestCore>) {
    let dir = tempfile::tempdir().unwrap();
    let core =
        QuestCore::open(dir.path().display().to_string(), crate::test_support::tokens(), Arc::new(Quiet)).unwrap();
    (dir, core)
}

/// Wednesday, 25 Nov 2026, 10:00 in Berlin.
fn now() -> Moment {
    moment("2026-11-25T10:00:00+01:00")
}

fn query() -> HomeQuery {
    HomeQuery { quest_filter: None, quest_pick: None }
}

fn add(core: &QuestCore, title: &str) -> String {
    core.add_task(title.into(), TaskSection::Other, AddPosition::Bottom, query(), now()).unwrap()
}

#[test]
fn creating_from_the_search_saves_and_says_where_to_go() {
    let (_dir, core) = open();
    let task = core.run_search(SearchToken::CreateTask { title: "Stretch".into() }, now()).unwrap();
    assert_eq!(task, SearchOutcome::Show { view: AppView::Home });
    assert_eq!(core.home(now(), query()).unwrap().other[0].title, "Stretch");

    let SearchOutcome::ShowQuest { quest_id } =
        core.run_search(SearchToken::CreateQuest { title: "Garden".into() }, now()).unwrap()
    else {
        panic!("a new quest opens its page")
    };
    assert_eq!(core.quest_detail(quest_id, now()).unwrap().title, "Garden");
}

#[test]
fn focus_starts_the_timer_and_choose_switches_the_mode() {
    let (_dir, core) = open();
    let task = add(&core, "Draft");
    let outcome = core.run_search(SearchToken::Focus { task_id: task.clone() }, now()).unwrap();
    assert_eq!(outcome, SearchOutcome::Focus { task_id: task.clone() });
    assert!(core.home(now(), query()).unwrap().other[0].timer_running);

    let mode = SearchMode::MoveTo { task_id: task };
    let outcome = core.run_search(SearchToken::Choose { mode: mode.clone() }, now()).unwrap();
    assert_eq!(outcome, SearchOutcome::Choose { mode, placeholder: "Move to which day?".into() });
}

#[test]
fn moving_scheduling_and_deleting_from_the_search() {
    let (_dir, core) = open();
    let task = add(&core, "Draft");
    let results = core.search(SearchMode::MoveTo { task_id: task.clone() }, "tomorrow".into(), now()).unwrap();
    assert_eq!(core.run_search(results[0].token.clone(), now()).unwrap(), SearchOutcome::Done);
    assert_eq!(core.task_detail(task.clone(), now()).unwrap().date.map(|d| d.iso), Some("2026-11-26".into()));

    let at = SearchToken::Schedule { task_id: task.clone(), day: "2026-11-25".into(), start_minute: 570 };
    assert_eq!(core.run_search(at, now()).unwrap(), SearchOutcome::Done);
    let grid = core.day_grid("2026-11-25".into(), now()).unwrap();
    assert_eq!(grid.blocks.len(), 1);

    core.run_search(SearchToken::DeleteTask { task_id: task }, now()).unwrap();
    let home = core.home(now(), query()).unwrap();
    assert!(home.other.is_empty());
    assert_eq!(home.toast.map(|t| t.text), Some("Deleted “Draft”".into()));
}

#[test]
fn search_reads_the_store_and_jumps_pass_through() {
    let (_dir, core) = open();
    add(&core, "Draft the plan");
    let results = core.search(SearchMode::Commands, "plan".into(), now()).unwrap();
    assert!(results.iter().any(|r| r.title == "Focus on “Draft the plan”"));
    assert!(results.iter().any(|r| r.token == SearchToken::Go { view: AppView::Plan }));
    for (token, outcome) in [
        (SearchToken::Go { view: AppView::Review }, SearchOutcome::Show { view: AppView::Review }),
        (SearchToken::LogWork, SearchOutcome::LogWork),
        (SearchToken::ToggleTheme, SearchOutcome::ToggleTheme),
        (SearchToken::Account, SearchOutcome::Account),
    ] {
        assert_eq!(core.run_search(token, now()).unwrap(), outcome);
    }
    assert_eq!(core.search_placeholder(SearchMode::Commands), "Search tasks and quests, or jump to…");
}

#[test]
fn the_review_defaults_to_this_week() {
    let (_dir, core) = open();
    let this = core.review(None, now()).unwrap();
    assert_eq!((this.week_start.as_str(), this.is_this_week), ("2026-11-23", true));
    let before = core.review(Some(this.previous_week), now()).unwrap();
    assert_eq!((before.week_start.as_str(), before.is_this_week), ("2026-11-16", false));
    assert!(matches!(core.review(Some("soon".into()), now()), Err(MobileError::InvalidDate { .. })));
}

#[test]
fn the_search_switches_and_manages_workspaces_like_the_palette() {
    let (_dir, core) = open();
    let work = core.create_workspace("Work".into()).unwrap();
    let choose = core.search(SearchMode::Commands, "switch workspace".into(), now()).unwrap();
    assert_eq!(choose[0].token, SearchToken::Choose { mode: SearchMode::SwitchWorkspace });
    let outcome = core.run_search(choose[0].token.clone(), now()).unwrap();
    assert_eq!(
        outcome,
        SearchOutcome::Choose { mode: SearchMode::SwitchWorkspace, placeholder: "Switch to which workspace?".into() }
    );
    let others = core.search(SearchMode::SwitchWorkspace, String::new(), now()).unwrap();
    let titles: Vec<&str> = others.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(titles, ["Default", "Manage workspaces…"]);
    assert_eq!(others[1].token, SearchToken::ManageWorkspaces);
    assert_eq!(core.run_search(others[1].token.clone(), now()).unwrap(), SearchOutcome::ManageWorkspaces);

    assert_eq!(core.run_search(others[0].token.clone(), now()).unwrap(), SearchOutcome::Done);
    assert_eq!(core.workspaces(now()).unwrap().active.name, "Default");
    let jump = core.search(SearchMode::Commands, "work".into(), now()).unwrap();
    let to_work = jump.iter().find(|r| r.token == SearchToken::SwitchWorkspace { workspace_id: work.clone() });
    assert!(to_work.is_some(), "a workspace's name is a jump too");
}
