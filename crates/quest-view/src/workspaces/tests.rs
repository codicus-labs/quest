use quest_domain::{EntityId, EntryKind, RunningFocus, Snapshot, TimeEntry, Workspace, WorkspaceContent};

use super::*;

fn workspace(id: EntityId, name: &str, content: WorkspaceContent) -> Workspace {
    Workspace { id, name: name.into(), deleted_at: None, content }
}

fn snapshot() -> (Snapshot, EntityId) {
    let work = EntityId::new();
    let snapshot = Snapshot {
        workspaces: vec![
            workspace(EntityId::DEFAULT_WORKSPACE, "Home", WorkspaceContent::default()),
            workspace(work, "Work", WorkspaceContent { quests: 1, tasks: 3, tracked_ms: 5_400_000 }),
        ],
        ..Snapshot::default()
    };
    (snapshot, work)
}

#[test]
fn a_summary_counts_quests_tasks_and_tracked_hours() {
    assert_eq!(content_summary(&WorkspaceContent::default()), "Empty");
    let content = WorkspaceContent { quests: 1, tasks: 3, tracked_ms: 5_400_000 };
    assert_eq!(content_summary(&content), "1 quest · 3 tasks · 1.5h tracked");
}

#[test]
fn deleting_asks_with_what_the_workspace_holds() {
    let (snapshot, work) = snapshot();
    assert_eq!(
        delete_question(snapshot.workspace(work).unwrap()),
        "Delete “Work”? It holds 1 quest, 3 tasks, 1.5h tracked. You can restore it under Deleted."
    );
    let home = snapshot.workspace(EntityId::DEFAULT_WORKSPACE).unwrap();
    assert!(delete_question(home).contains("It is empty."));
}

#[test]
fn names_are_trimmed_short_and_unique_among_live_workspaces() {
    let (mut snapshot, work) = snapshot();
    assert_eq!(check_name(&snapshot, "  Side ", None), Ok("Side".into()));
    assert_eq!(check_name(&snapshot, " ", None), Err(NameError::Empty));
    assert_eq!(check_name(&snapshot, &"x".repeat(61), None), Err(NameError::TooLong));
    assert_eq!(check_name(&snapshot, "work", None), Err(NameError::Taken));
    assert_eq!(check_name(&snapshot, "WORK", Some(work)), Ok("WORK".into()), "renaming keeps its own name");
    snapshot.workspaces[1].deleted_at = Some(chrono::Utc::now());
    assert_eq!(check_name(&snapshot, "Work", None), Ok("Work".into()), "deleted names are free");
}

#[test]
fn a_timer_in_another_workspace_names_that_workspace() {
    let (mut snapshot, work) = snapshot();
    assert_eq!(timer_elsewhere(&snapshot), None);
    let entry = TimeEntry {
        id: EntityId::new(),
        kind: EntryKind::Task,
        origin_task_id: None,
        rebooked_from: None,
        task_id: Some(EntityId::new()),
        quest_id: None,
        started_at: chrono::Utc::now(),
        ended_at: None,
    };
    snapshot.running = vec![RunningFocus { entry, workspace: work, task_title: Some("Ship".into()) }];
    assert_eq!(timer_elsewhere(&snapshot), Some((work, "Work".into(), "Ship".into())));
    snapshot.workspace = work;
    assert_eq!(timer_elsewhere(&snapshot), None, "here it is the normal running timer");
}
