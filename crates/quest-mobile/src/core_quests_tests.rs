use std::sync::Arc;

use quest_domain::{Command, EntityId, QuestStatus};

use super::*;
use crate::core::ChangeListener;
use crate::ids::parse_id;
use crate::test_support::moment;

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

fn create(core: &QuestCore, title: &str) -> String {
    core.create_quest(title.into(), String::new(), None).unwrap()
}

#[test]
fn a_new_quest_needs_a_title_and_takes_a_free_colour_or_the_picked_one() {
    let (_dir, core) = open();
    assert_eq!(core.create_quest("  ".into(), "Notes".into(), None), Err(MobileError::EmptyTitle));
    assert!(core.quests(now()).unwrap().active.is_empty(), "nothing without a title");
    let photo = create(&core, " Photo series ");
    assert_eq!(core.quests(now()).unwrap().new_quest_hues.iter().find(|h| h.selected).unwrap().hue, QuestHue::Orange);
    create(&core, "Garden");
    let teal = core.create_quest("Reading".into(), "Twelve books\n**by spring**".into(), Some(QuestHue::Teal)).unwrap();
    let data = core.quests(now()).unwrap();
    assert_eq!(data.active.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(), ["Photo series", "Garden", "Reading"]);
    assert_eq!(data.active[1].color_hex, "#d98a4e", "red is taken, so orange");
    let reading = core.quest_detail(teal, now()).unwrap();
    assert_eq!((reading.color_hex.as_str(), reading.description.as_str()), ("#4cb5a8", "Twelve books\n**by spring**"));
    assert_eq!(core.quest_detail(photo, now()).unwrap().description, "");
    assert_eq!(data.in_progress, "03");
}

#[test]
fn completing_and_reopening_moves_tabs_keeps_hours_and_undoes() {
    let (_dir, core) = open();
    let photo = create(&core, "Photo");
    let garden = create(&core, "Garden");
    let task = core.add_quest_task(garden.clone(), "Dig".into(), false, AddPosition::Bottom, now()).unwrap();
    core.start_timer(task, moment("2026-11-25T09:00:00+01:00")).unwrap();
    core.stop_timer(moment("2026-11-25T09:30:00+01:00")).unwrap();
    let before = core.quests(now()).unwrap();
    assert_eq!((before.in_progress.as_str(), before.completed_count.as_str()), ("02", "00"));
    assert_eq!(before.all_time_hours, "0.5h");

    core.toggle_quest_status(garden.clone(), now()).unwrap();
    let data = core.quests(now()).unwrap();
    assert_eq!((data.active_label.as_str(), data.completed_label.as_str()), ("Active · 1", "Completed · 1"));
    assert_eq!((data.in_progress.as_str(), data.completed_count.as_str()), ("01", "01"));
    assert_eq!(data.all_time_hours, "0.5h", "completing keeps the tracked time");
    assert_eq!((data.completed[0].week_hours.as_str(), data.completed[0].total_hours.as_str()), ("0.5h", "0.5h total"));
    assert_eq!(data.toast.map(|t| t.text), Some("Completed quest “Garden”".into()));
    let detail = core.quest_detail(garden.clone(), now()).unwrap();
    assert_eq!((detail.status_action.as_str(), detail.badge.as_deref()), ("Reopen quest", Some("Completed")));

    core.undo(now()).unwrap();
    let data = core.quests(now()).unwrap();
    assert_eq!((data.active.len(), data.completed.len(), data.toast), (2, 0, None));

    core.toggle_quest_status(garden.clone(), now()).unwrap();
    core.toggle_quest_status(garden.clone(), now()).unwrap();
    let data = core.quests(now()).unwrap();
    assert_eq!(data.toast.map(|t| t.text), Some("Reopened quest “Garden”".into()));
    assert_eq!((data.active.len(), data.all_time_hours.as_str()), (2, "0.5h"));
    core.undo(now()).unwrap();
    assert_eq!(core.quests(now()).unwrap().completed.len(), 1, "undoing the reopen completes it again");
    assert_eq!(core.quest_detail(photo, now()).unwrap().status_action, "Complete quest");
}

#[test]
fn the_page_edits_title_description_and_colour() {
    let (_dir, core) = open();
    let id = create(&core, "Photo");
    core.rename_quest(id.clone(), " Photo\nseries ".into()).unwrap();
    core.rename_quest(id.clone(), "   ".into()).unwrap();
    core.set_quest_description(id.clone(), "Twelve prints\nby spring".into()).unwrap();
    core.set_quest_hue(id.clone(), QuestHue::Teal).unwrap();
    let detail = core.quest_detail(id, now()).unwrap();
    assert_eq!(detail.title, "Photo series");
    assert_eq!(detail.description, "Twelve prints\nby spring");
    assert_eq!(detail.color_hex, "#4cb5a8");
}

#[test]
fn tasks_added_on_the_page_go_to_today_or_the_backlog() {
    let (_dir, core) = open();
    let id = create(&core, "Photo");
    core.add_quest_task(id.clone(), "Shoot".into(), false, AddPosition::Bottom, now()).unwrap();
    core.add_quest_task(id.clone(), "Scout".into(), true, AddPosition::Bottom, now()).unwrap();
    assert_eq!(
        core.add_quest_task(id.clone(), " ".into(), false, AddPosition::Top, now()),
        Err(MobileError::EmptyTitle)
    );
    let detail = core.quest_detail(id, now()).unwrap();
    let groups: Vec<&str> = detail.task_groups.iter().map(|g| g.label.as_str()).collect();
    assert_eq!(groups, ["Today", "Backlog"]);
    assert_eq!(detail.open_count, 2);
}

#[test]
fn an_archived_quest_is_in_the_archive_and_unarchives_to_active() {
    let (_dir, core) = open();
    let old = create(&core, "Old plan");
    let id = parse_id(&old).unwrap();
    core.dispatch(Command::SetQuestStatus { id, status: QuestStatus::Archived }).unwrap();
    let data = core.quests(now()).unwrap();
    assert_eq!((data.active.len(), data.completed.len(), data.completed_count.as_str()), (0, 0, "00"));
    let archive = core.archive(now()).unwrap();
    assert_eq!(
        archive.archived_quests.iter().map(|q| (q.title.as_str(), q.meta.as_str())).collect::<Vec<_>>(),
        [("Old plan", "Archived quest")]
    );
    let detail = core.quest_detail(old.clone(), now()).unwrap();
    assert_eq!((detail.status_action.as_str(), detail.badge.as_deref()), ("Unarchive", Some("Archived")));

    core.unarchive_quest(old.clone()).unwrap();
    assert!(core.archive(now()).unwrap().archived_quests.is_empty());
    assert_eq!(core.quests(now()).unwrap().active.len(), 1);
    core.unarchive_quest(old).unwrap();
    assert_eq!(core.quests(now()).unwrap().active.len(), 1, "an active quest stays active");
}

#[test]
fn unarchive_on_the_quest_page_reopens_and_undo_archives_again() {
    let (_dir, core) = open();
    let old = create(&core, "Old plan");
    let id = parse_id(&old).unwrap();
    core.dispatch(Command::SetQuestStatus { id, status: QuestStatus::Archived }).unwrap();

    core.toggle_quest_status(old.clone(), now()).unwrap();
    let data = core.quests(now()).unwrap();
    assert_eq!(data.active.len(), 1);
    assert_eq!(data.toast.map(|t| t.text), Some("Reopened quest “Old plan”".into()));

    core.undo(now()).unwrap();
    assert_eq!(core.archive(now()).unwrap().archived_quests.len(), 1);
    assert!(core.quests(now()).unwrap().active.is_empty());
}

#[test]
fn a_deleted_quest_closes_its_page_and_comes_back_from_the_archive() {
    let (_dir, core) = open();
    let quest = create(&core, "Photo");
    let quest_id = parse_id(&quest).unwrap();
    let achieved_on = chrono::NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
    core.dispatch(Command::AddMilestone { id: EntityId::new(), quest_id, title: "First roll".into(), achieved_on })
        .unwrap();
    core.delete_quest(quest.clone(), now()).unwrap();
    assert!(matches!(core.quest_detail(quest.clone(), now()), Err(MobileError::UnknownQuest { .. })));
    let quests = core.quests(now()).unwrap();
    assert_eq!(quests.toast.map(|t| t.text), Some("Deleted “Photo”".into()));

    let archive = core.archive(now()).unwrap();
    assert_eq!(archive.groups[0].title, "Deleted quests");
    let item = archive.groups[0].items[0].clone();
    core.restore(item.kind, item.id, now()).unwrap();
    assert_eq!(core.quests(now()).unwrap().toast, None, "the delete toast is gone");
    core.undo(now()).unwrap();
    let detail = core.quest_detail(quest, now()).unwrap();
    assert_eq!(detail.milestones.len(), 1, "its milestones come back with it");
    assert_eq!(detail.milestones[0].date_label, "Nov 20, 2026");
    assert!(core.archive(now()).unwrap().groups.is_empty());
    assert!(matches!(core.restore(TrashKind::Task, detail.id, now()), Err(MobileError::Rejected { .. })));
}

#[test]
fn unknown_ids_are_reported() {
    let (_dir, core) = open();
    assert!(matches!(core.quest_detail("nope".into(), now()), Err(MobileError::InvalidId { .. })));
    let gone = quest_domain::EntityId::new().to_string();
    assert!(matches!(core.toggle_quest_status(gone.clone(), now()), Err(MobileError::UnknownQuest { .. })));
    assert!(matches!(core.unarchive_quest(gone), Err(MobileError::UnknownQuest { .. })));
}
