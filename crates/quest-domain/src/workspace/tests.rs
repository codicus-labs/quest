use chrono::{DateTime, NaiveDate, TimeZone, Utc};

use crate::change::{EntityId, FieldStore};
use crate::command::{Command, CommandError, Position};
use crate::model::{QuestColor, Workspace, fields};
use crate::recurrence::{Frequency, RepeatPin, RepeatRule, Weekdays};
use crate::review::WeeklyReview;
use crate::snapshot::Snapshot;
use crate::test_support::Replica;

const DEFAULT: EntityId = EntityId::DEFAULT_WORKSPACE;

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, d).unwrap()
}

fn at(d: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, d, hour, 0, 0).unwrap()
}

fn create_quest(r: &mut Replica, title: &str) -> EntityId {
    let id = EntityId::new();
    r.run(Command::CreateQuest { id, title: title.into(), color: QuestColor::Blue });
    id
}

fn create_task(r: &mut Replica, title: &str, quest_id: Option<EntityId>) -> EntityId {
    let id = EntityId::new();
    r.run(Command::CreateTask {
        id,
        title: title.into(),
        quest_id,
        planned_on: Some(day(24)),
        due_on: None,
        position: Position::Bottom,
    });
    id
}

fn new_workspace(r: &mut Replica, name: &str) -> EntityId {
    let id = EntityId::new();
    r.run(Command::CreateWorkspace { id, name: name.into() });
    id
}

fn titles(snapshot: &Snapshot) -> Vec<&str> {
    snapshot.quests.iter().map(|q| q.title.as_str()).chain(snapshot.tasks.iter().map(|t| t.title.as_str())).collect()
}

fn daily(starts: NaiveDate) -> RepeatRule {
    RepeatRule {
        every: 1,
        frequency: Frequency::Day,
        starts,
        ends: None,
        time_zone: chrono_tz::UTC,
        pin: RepeatPin::None,
        weekdays: Weekdays::default(),
        month_day: None,
    }
}

/// The default workspace with a quest, a repeating task with a session, a deleted task and a
/// weekly target, written by another device.
fn default_store() -> (FieldStore, [EntityId; 4]) {
    let mut other = Replica::on_node(9);
    let quest = create_quest(&mut other, "Launch");
    let task = create_task(&mut other, "Write", Some(quest));
    other.run(Command::SetTaskRepeat { id: task, rule: Some(daily(day(24))), now: at(23, 8) });
    let entry = EntityId::new();
    other.run(Command::LogTime { entry_id: entry, task_id: Some(task), started_at: at(23, 9), ended_at: at(23, 10) });
    let deleted = create_task(&mut other, "Old", None);
    other.run(Command::DeleteTask { id: deleted, at: at(23, 11) });
    other.run(Command::SetWeeklyTarget { minutes: 300 });
    (other.store, [quest, task, entry, deleted])
}

#[test]
fn the_default_workspace_exists_without_a_record_and_holds_its_data() {
    let (store, [quest, task, entry, deleted]) = default_store();
    let snapshot = Snapshot::from_store(&store, DEFAULT);

    assert_eq!(snapshot.workspaces.len(), 1);
    assert_eq!(snapshot.active_workspace().name, Workspace::DEFAULT_NAME);
    assert_eq!(snapshot.quest(quest).unwrap().title, "Launch");
    assert_eq!(snapshot.task(task).unwrap().quest_id, Some(quest));
    assert!(snapshot.task(task).unwrap().repeat.is_some(), "series");
    assert_eq!(snapshot.time_entries[0].id, entry);
    assert_eq!(snapshot.trash[0].id, deleted, "archive and trash");
    assert_eq!(snapshot.settings.weekly_target_minutes, 300);
    let content = snapshot.active_workspace().content;
    assert_eq!((content.quests, content.tasks, content.tracked_ms), (1, 1, 3_600_000));
    assert_eq!(Snapshot::from_store(&store, EntityId::new()), snapshot, "unknown workspace falls back");
}

#[test]
fn a_new_workspace_starts_empty_and_keeps_its_own_data_and_weekly_target() {
    let mut r = Replica::new();
    let (store, _) = default_store();
    r.store = store;
    r.switch(DEFAULT);
    let private = new_workspace(&mut r, "Private");
    r.switch(private);
    assert_eq!(r.snapshot.workspace, private);
    assert!(r.snapshot.quests.is_empty() && r.snapshot.tasks.is_empty() && r.snapshot.trash.is_empty());
    assert_eq!(r.snapshot.settings.weekly_target_minutes, 20 * 60, "a new workspace has the default target");

    let quest = create_quest(&mut r, "Garden");
    let task = create_task(&mut r, "Dig", Some(quest));
    r.run(Command::StartFocus { entry_id: EntityId::new(), task_id: task, at: at(24, 9) });
    r.run(Command::StopFocus { at: at(24, 11) });
    r.run(Command::SetWeeklyTarget { minutes: 120 });
    assert_eq!(titles(&r.snapshot), ["Garden", "Dig"]);

    r.switch(DEFAULT);
    assert_eq!(titles(&r.snapshot), ["Launch", "Write"]);
    assert_eq!(r.snapshot.settings.weekly_target_minutes, 300);
    let review = WeeklyReview::build(&r.snapshot, day(24), &Utc, at(24, 12));
    assert_eq!(review.quest_ms, 3_600_000, "only this workspace's session");
    assert_eq!(review.target_ms, 300 * 60_000);
    assert_eq!(r.snapshot.workspace(private).unwrap().content.tracked_ms, 2 * 3_600_000);
}

#[test]
fn commands_cannot_reach_into_another_workspace() {
    let mut r = Replica::new();
    let task = create_task(&mut r, "Default task", None);
    let quest = create_quest(&mut r, "Default quest");
    let other = new_workspace(&mut r, "Other");
    r.switch(other);
    assert_eq!(r.try_run(Command::RenameTask { id: task, title: "x".into() }), Err(CommandError::UnknownTask(task)));
    let create = Command::CreateTask {
        id: EntityId::new(),
        title: "t".into(),
        quest_id: Some(quest),
        planned_on: None,
        due_on: None,
        position: Position::Bottom,
    };
    assert_eq!(r.try_run(create), Err(CommandError::UnknownQuest(quest)));
    let start = Command::StartFocus { entry_id: EntityId::new(), task_id: task, at: at(24, 9) };
    assert_eq!(r.try_run(start), Err(CommandError::UnknownTask(task)));
}

#[test]
fn copies_a_repeat_makes_follow_their_series_into_its_workspace() {
    let mut r = Replica::new();
    let work = new_workspace(&mut r, "Work");
    r.switch(work);
    let task = create_task(&mut r, "Code", None);
    r.run(Command::AddChecklistItem {
        id: EntityId::new(),
        task_id: task,
        text: "Test".into(),
        position: Position::Top,
    });
    r.run(Command::SetTaskRepeat { id: task, rule: Some(daily(day(23))), now: at(23, 8) });
    r.run(Command::GenerateDueTasks { now: at(25, 9) });
    assert!(r.changes.iter().any(|c| c.field == fields::TASK_OCCURRENCE), "occurrences carry no workspace");

    let occurrences: Vec<EntityId> = r.snapshot.tasks.iter().filter(|t| t.series == Some(task)).map(|t| t.id).collect();
    assert_eq!(occurrences.len(), 2);
    assert!(occurrences.iter().all(|id| r.snapshot.checklist_of(*id).count() == 1));
    r.switch(DEFAULT);
    assert!(r.snapshot.tasks.is_empty() && r.snapshot.checklist.is_empty());
}

#[test]
fn deleting_a_workspace_hides_its_content_until_it_is_restored() {
    let mut r = Replica::new();
    let side = new_workspace(&mut r, "Side");
    r.switch(side);
    create_task(&mut r, "Side task", None);
    r.run(Command::DeleteWorkspace { id: side, at: at(24, 12) });

    assert_eq!(r.snapshot.workspace, DEFAULT, "the active workspace falls back to the default one");
    assert!(r.snapshot.tasks.is_empty());
    assert_eq!(r.snapshot.live_workspaces().count(), 1);
    let deleted = r.snapshot.deleted_workspaces();
    assert_eq!((deleted[0].id, deleted[0].content.tasks), (side, 1));
    assert_eq!(
        r.try_run(Command::RenameWorkspace { id: side, name: "x".into() }),
        Err(CommandError::UnknownWorkspace(side))
    );

    r.run(Command::RestoreWorkspace { id: side });
    r.switch(side);
    assert_eq!(titles(&r.snapshot), ["Side task"]);
    assert_eq!(r.try_run(Command::RestoreWorkspace { id: side }), Err(CommandError::WorkspaceNotDeleted(side)));
}

#[test]
fn the_default_workspace_can_be_renamed_and_deleted_while_another_one_remains() {
    let mut r = Replica::new();
    assert_eq!(
        r.try_run(Command::DeleteWorkspace { id: DEFAULT, at: at(24, 9) }),
        Err(CommandError::CannotDeleteLastWorkspace)
    );
    create_task(&mut r, "Default task", None);
    let other = new_workspace(&mut r, "Other");
    r.run(Command::RenameWorkspace { id: DEFAULT, name: "  Work ".into() });
    assert_eq!(r.snapshot.active_workspace().name, "Work");
    assert_eq!(
        r.try_run(Command::CreateWorkspace { id: EntityId::new(), name: " ".into() }),
        Err(CommandError::EmptyTitle)
    );
    let reuse = Command::CreateWorkspace { id: other, name: "Again".into() };
    assert_eq!(r.try_run(reuse), Err(CommandError::WorkspaceExists(other)));
    let names: Vec<&str> = r.snapshot.workspaces.iter().map(|w| w.name.as_str()).collect();
    assert_eq!(names, ["Work", "Other"], "default first, then by creation");

    r.run(Command::DeleteWorkspace { id: DEFAULT, at: at(24, 10) });
    assert_eq!(r.snapshot.workspace, other, "the remaining workspace is shown");
    assert!(r.snapshot.tasks.is_empty());
    assert_eq!(r.snapshot.check_deletable(other), Err(CommandError::CannotDeleteLastWorkspace));
    r.switch(DEFAULT);
    assert_eq!(r.snapshot.workspace, other, "a deleted default is not shown");

    r.run(Command::RestoreWorkspace { id: DEFAULT });
    r.switch(DEFAULT);
    assert_eq!(titles(&r.snapshot), ["Default task"]);
}

#[test]
fn devices_deleting_every_workspace_offline_keep_the_default_one() {
    let mut laptop = Replica::on_node(1);
    let side = new_workspace(&mut laptop, "Side");
    let mut phone = Replica::on_node(2);
    phone.merge(&laptop);

    laptop.run(Command::DeleteWorkspace { id: DEFAULT, at: at(24, 9) });
    phone.run(Command::DeleteWorkspace { id: side, at: at(24, 9) });
    laptop.merge(&phone);
    phone.merge(&laptop);

    for device in [&laptop, &phone] {
        let live: Vec<EntityId> = device.snapshot.live_workspaces().map(|w| w.id).collect();
        assert_eq!(live, [DEFAULT]);
        assert_eq!(device.snapshot.workspace, DEFAULT);
    }

    laptop.run(Command::RestoreWorkspace { id: side });
    let live: Vec<EntityId> = laptop.snapshot.live_workspaces().map(|w| w.id).collect();
    assert_eq!(live, [DEFAULT, side], "the revived default stays when another one is live again");
    assert_eq!(laptop.snapshot.workspace, DEFAULT);
}

#[test]
fn one_focus_timer_runs_across_all_workspaces() {
    let mut r = Replica::new();
    let write = create_task(&mut r, "Write", None);
    let first = EntityId::new();
    r.run(Command::StartFocus { entry_id: first, task_id: write, at: at(24, 9) });
    let side = new_workspace(&mut r, "Side");
    r.switch(side);

    assert_eq!(r.snapshot.running_entry(), None, "the timer belongs to another workspace");
    let running = r.snapshot.running_focus().expect("still visible after the switch");
    assert_eq!((running.entry.id, running.workspace, running.task_title.as_deref()), (first, DEFAULT, Some("Write")));

    let read = create_task(&mut r, "Read", None);
    let second = EntityId::new();
    r.run(Command::StartFocus { entry_id: second, task_id: read, at: at(24, 10) });
    assert_eq!(r.snapshot.running.len(), 1, "starting here stops the other workspace's timer");
    assert_eq!(r.snapshot.running_entry().map(|e| e.id), Some(second));
    r.switch(DEFAULT);
    assert_eq!(r.snapshot.time_entries.iter().find(|e| e.id == first).and_then(|e| e.ended_at), Some(at(24, 10)));

    r.run(Command::StopFocus { at: at(24, 11) });
    assert!(r.snapshot.running.is_empty(), "stopping works from any workspace");

    r.switch(side);
    r.run(Command::StartFocus { entry_id: EntityId::new(), task_id: read, at: at(24, 12) });
    r.run(Command::DeleteWorkspace { id: side, at: at(24, 13) });
    assert!(r.snapshot.running.is_empty(), "deleting a workspace stops its timer");
}

#[test]
fn a_timer_in_a_workspace_deleted_elsewhere_hides_with_it() {
    let mut laptop = Replica::on_node(1);
    let side = new_workspace(&mut laptop, "Side");
    let mut phone = Replica::on_node(2);
    phone.merge(&laptop);
    phone.switch(side);
    let task = create_task(&mut phone, "Side task", None);
    laptop.merge(&phone);
    laptop.run(Command::DeleteWorkspace { id: side, at: at(24, 9) });
    phone.run(Command::StartFocus { entry_id: EntityId::new(), task_id: task, at: at(24, 10) });
    phone.merge(&laptop);
    laptop.merge(&phone);

    assert!(laptop.snapshot.running_focus().is_none(), "no timer that can't be shown");
    laptop.run(Command::RestoreWorkspace { id: side });
    assert_eq!(laptop.snapshot.running_focus().map(|r| r.workspace), Some(side));
}

#[test]
fn two_devices_offline_share_one_default_workspace() {
    let (store, _) = default_store();
    let mut laptop = Replica::on_node(1);
    let mut phone = Replica::on_node(2);
    for device in [&mut laptop, &mut phone] {
        device.store = store.clone();
        device.switch(DEFAULT);
    }
    let a = new_workspace(&mut laptop, "Laptop");
    laptop.run(Command::RenameWorkspace { id: DEFAULT, name: "Home".into() });
    let b = new_workspace(&mut phone, "Phone");
    create_task(&mut phone, "Phone default task", None);

    laptop.merge(&phone);
    phone.merge(&laptop);
    assert_eq!(laptop.snapshot, phone.snapshot);
    let ids: Vec<EntityId> = laptop.snapshot.workspaces.iter().map(|w| w.id).collect();
    assert_eq!(ids, [DEFAULT, a, b]);
    assert_eq!(laptop.snapshot.active_workspace().name, "Home");
    assert_eq!(titles(&laptop.snapshot), ["Launch", "Write", "Phone default task"]);
}

#[test]
fn offline_edits_in_a_workspace_deleted_elsewhere_reappear_with_it() {
    let mut laptop = Replica::on_node(1);
    let side = new_workspace(&mut laptop, "Side");
    laptop.switch(side);
    let task = create_task(&mut laptop, "Draft", None);
    let mut phone = Replica::on_node(2);
    phone.merge(&laptop);
    phone.switch(side);

    laptop.run(Command::DeleteWorkspace { id: side, at: at(24, 12) });
    phone.run(Command::RenameTask { id: task, title: "Final".into() });
    let added = create_task(&mut phone, "Added offline", None);
    laptop.merge(&phone);
    phone.merge(&laptop);

    assert_eq!(phone.snapshot.workspace, DEFAULT, "the phone leaves the deleted workspace");
    assert!(phone.snapshot.task(added).is_none(), "nothing leaks into the default workspace");
    laptop.run(Command::RestoreWorkspace { id: side });
    laptop.switch(side);
    assert_eq!(titles(&laptop.snapshot), ["Final", "Added offline"]);
}
