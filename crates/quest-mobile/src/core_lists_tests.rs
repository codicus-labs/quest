use std::sync::Arc;

use super::*;
use crate::core::ChangeListener;
use crate::list_types::MarkTone;
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

fn titles(list: &TaskList, section: usize) -> Vec<&str> {
    list.sections[section].rows.iter().map(|r| r.title.as_str()).collect()
}

#[test]
fn lists_add_with_their_defaults() {
    let (_dir, core) = open();
    let quest = core.create_quest("Launch".into(), None).unwrap();
    core.add_to_list(ListKind::Today, "Call the landlord".into(), now()).unwrap();
    core.add_to_list(ListKind::Inbox, "Pick up shoes".into(), now()).unwrap();
    core.add_to_list(ListKind::Quest { quest_id: quest.clone() }, "Sketch pricing".into(), now()).unwrap();

    let today = core.task_list(ListKind::Today, now()).unwrap();
    assert_eq!(titles(&today, 0), ["Call the landlord"]);
    assert!(today.subtitle.ends_with("0h tracked"), "{}", today.subtitle);
    assert_eq!(titles(&core.task_list(ListKind::Inbox, now()).unwrap(), 0), ["Pick up shoes"]);
    let quest_list = core.task_list(ListKind::Quest { quest_id: quest }, now()).unwrap();
    assert_eq!((quest_list.title.as_str(), titles(&quest_list, 0)), ("Launch", vec!["Sketch pricing"]));
    assert!(quest_list.sections[0].rows[0].quest.is_none(), "no dot on the quest's own list");
}

#[test]
fn today_marks_overdue_rows_and_lists_what_was_done() {
    let (_dir, core) = open();
    let overdue = core.add_to_list(ListKind::Inbox, "File taxes".into(), now()).unwrap();
    core.set_task_due(overdue, Some("2026-11-23".into()), now()).unwrap();
    let done = core.add_to_list(ListKind::Today, "Read".into(), now()).unwrap();
    core.toggle_task(done, now()).unwrap();

    let today = core.task_list(ListKind::Today, now()).unwrap();
    let row = &today.sections[0].rows[0];
    assert_eq!((row.title.as_str(), row.marks[0].tone), ("File taxes", MarkTone::Overdue));
    assert_eq!(today.sections[1].label.as_deref(), Some("1 done"));
    assert_eq!(titles(&today, 1), ["Read"]);
    assert!(today.sections[1].rows[0].marks.is_empty());
}

#[test]
fn the_player_and_the_row_show_the_running_timer() {
    let (_dir, core) = open();
    assert_eq!(core.player(now()).unwrap(), PlayerCard::Empty);
    let task = core.add_to_list(ListKind::Today, "Write".into(), now()).unwrap();
    core.start_timer(task.clone(), now()).unwrap();
    let later = moment("2026-11-25T10:36:13+01:00");

    let PlayerCard::Running { task_id, timer, .. } = core.player(later.clone()).unwrap() else { panic!("not running") };
    assert_eq!((task_id.as_str(), timer.as_str()), (task.as_str(), "36:13"));
    let row = core.task_list(ListKind::Today, later.clone()).unwrap().sections[0].rows[0].clone();
    assert_eq!((row.timer_running, row.timer.as_deref()), (true, Some("36:13")));

    core.stop_timer(later.clone()).unwrap();
    assert!(matches!(core.player(later).unwrap(), PlayerCard::Resume { task_id, .. } if task_id == task));
}

#[test]
fn list_completion_lingers_in_place_and_undo_outlives_the_toast() {
    let (_dir, core) = open();
    core.add_to_list(ListKind::Today, "First".into(), now()).unwrap();
    let id = core.add_to_list(ListKind::Today, "Second".into(), now()).unwrap();
    core.add_to_list(ListKind::Today, "Third".into(), now()).unwrap();

    core.toggle_list_task(id, now()).unwrap();
    let during = core.task_list(ListKind::Today, moment("2026-11-25T10:00:03+01:00")).unwrap();
    assert_eq!(titles(&during, 0), ["First", "Second", "Third"]);
    assert!(during.sections[0].rows[1].done);
    assert_eq!(during.sections.len(), 1);
    assert_eq!(during.toast.unwrap().text, "Done");
    assert_eq!(core.next_wake_ms(now()).unwrap(), now().epoch_ms + crate::completion_linger_ms() + 5);

    let after = moment("2026-11-25T10:00:04+01:00");
    core.tick(after.clone()).unwrap();
    let list = core.task_list(ListKind::Today, after).unwrap();
    assert_eq!(titles(&list, 0), ["First", "Third"]);
    assert_eq!(titles(&list, 1), ["Second"]);
    assert!(list.toast.is_some());

    let expired = moment("2026-11-25T10:00:05+01:00");
    core.tick(expired.clone()).unwrap();
    assert!(core.task_list(ListKind::Today, expired.clone()).unwrap().toast.is_none());
    core.undo(expired.clone()).unwrap();
    assert_eq!(titles(&core.task_list(ListKind::Today, expired).unwrap(), 0), ["First", "Second", "Third"]);
}

#[test]
fn organizing_shows_back_to_its_task() {
    let (_dir, core) = open();
    let task = core.add_to_list(ListKind::Today, "Write".into(), now()).unwrap();
    core.start_organizing(Some(task.clone()), now()).unwrap();
    let card = core.player(moment("2026-11-25T10:02:00+01:00")).unwrap();
    assert_eq!(
        card,
        PlayerCard::Organizing {
            timer: "02:00".into(),
            back_to_task_id: Some(task),
            back_to_title: Some("Write".into())
        }
    );
}

#[test]
fn tracked_lists_today_s_blocks_and_the_week() {
    let (_dir, core) = open();
    let quest = core.create_quest("Launch".into(), None).unwrap();
    let task = core.add_to_list(ListKind::Quest { quest_id: quest }, "Sketch".into(), now()).unwrap();
    core.start_timer(task, moment("2026-11-25T09:00:00+01:00")).unwrap();
    core.stop_timer(moment("2026-11-25T09:30:00+01:00")).unwrap();

    let tracked = core.tracked(now()).unwrap();
    let block = &tracked.blocks[0];
    assert_eq!((block.title.as_str(), block.range.as_str(), block.start_minute), ("Sketch", "09:00–09:30", 540));
    assert!(block.color_hex.is_some() && !block.organizing);
    assert_eq!(tracked.total, "0.5h");
    assert_eq!(tracked.week.quests[0].hours, "0.5h");

    let nav = core.quest_nav(now()).unwrap();
    assert_eq!((nav.active[0].title.as_str(), nav.active[0].week_hours.as_str()), ("Launch", "0.5h"));
}

#[test]
fn a_gone_quest_s_list_is_unknown() {
    let (_dir, core) = open();
    let missing = "00000000-0000-7000-8000-000000000001".to_owned();
    let error = core.task_list(ListKind::Quest { quest_id: missing.clone() }, now()).unwrap_err();
    assert_eq!(error, MobileError::UnknownQuest { id: missing });
}
