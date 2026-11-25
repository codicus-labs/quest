use quest_domain::Snapshot;

use super::*;
use crate::test_support::{at, berlin, entry, id, quest, task};

/// Wednesday, 25 Nov 2026, 10:40:30 in Berlin.
fn now() -> Now {
    berlin("2026-11-25T10:40:30+01:00")
}

fn snapshot() -> Snapshot {
    let mut done = task(11, "Pick motifs", Some(1), Some("2026-11-20"));
    done.completed_at = Some(at("2026-11-20T12:00:00+01:00"));
    Snapshot {
        quests: vec![quest(1, "Book"), quest(2, "Photos")],
        tasks: vec![done, task(10, "Write chapter", Some(1), Some("2026-11-25")), task(12, "Mail", None, None)],
        time_entries: vec![
            entry(20, Some(10), Some(1), "2026-11-25T08:00:20+01:00", Some("2026-11-25T09:30:40+01:00")),
            entry(21, None, None, "2026-11-24T23:00:00+01:00", Some("2026-11-25T01:00:00+01:00")),
            entry(22, Some(12), None, "2026-11-25T10:00:00+01:00", None),
        ],
        ..Snapshot::default()
    }
}

fn log(task: Option<u128>, title: &str, start: u32, end: u32) -> LogWorkDraft {
    LogWorkDraft {
        task_id: task.map(|n| id(n).to_string()),
        new_title: title.into(),
        quest_id: None,
        day: "2026-11-25".into(),
        start_minute: start,
        end_minute: end,
    }
}

fn field(error: Option<FormError>) -> Option<(FormField, String)> {
    error.map(|e| (e.field, e.message))
}

#[test]
fn task_choices_list_open_then_done_and_offer_to_create_what_matches_nothing() {
    let all = task_choices(&snapshot(), "  ");
    let titles: Vec<_> = all.options.iter().map(|o| (o.title.as_str(), o.status.as_str())).collect();
    assert_eq!(titles, [("Write chapter", "Open"), ("Mail", "Open"), ("Pick motifs", "Done")]);
    assert_eq!(all.options[2].quest.as_ref().map(|q| q.name.as_str()), Some("Book"));
    assert_eq!(all.create_label, None);
    let hits = task_choices(&snapshot(), "pick");
    assert_eq!(hits.options.len(), 1);
    assert!(hits.options[0].done, "done tasks can be picked");
    let none = task_choices(&snapshot(), "Invoice");
    assert!(none.options.is_empty());
    assert_eq!(none.create_label.as_deref(), Some("Create “Invoice”"));
}

#[test]
fn log_work_now_is_the_quarter_hour_that_just_ended() {
    assert_eq!(log_work_now(now()), log_work_for(now().today(), 625, 640));
    let early = log_work_now(berlin("2026-11-25T00:07:00+01:00"));
    assert_eq!((early.start_minute, early.end_minute), (0, 7));
}

#[test]
fn a_quest_is_chosen_only_for_a_new_task() {
    let mut empty = log(None, "  ", 540, 600);
    empty.quest_id = Some(id(2).to_string());
    let check = check_log_work(&snapshot(), &empty, now()).unwrap();
    assert!(!check.can_choose_quest, "no task, no title: nothing to choose a quest for");
    assert_eq!(check.quest, None);

    let mut picked = log(Some(11), "", 540, 600);
    picked.quest_id = Some(id(2).to_string());
    let check = check_log_work(&snapshot(), &picked, now()).unwrap();
    assert!(!check.can_choose_quest);
    assert_eq!(check.quest.map(|q| q.name), Some("Book".into()), "the task's own quest, not the chosen one");

    let check = check_log_work(&snapshot(), &log(None, "Invoice", 540, 600), now()).unwrap();
    assert!(check.can_choose_quest);
}

#[test]
fn a_picked_task_brings_its_quest_and_a_new_one_takes_the_chosen_quest() {
    let picked = check_log_work(&snapshot(), &log(Some(11), "", 540, 615), now()).unwrap();
    assert_eq!(picked.task_title.as_deref(), Some("Pick motifs"));
    assert_eq!(picked.quest.map(|q| q.name), Some("Book".into()));
    assert!(!picked.can_choose_quest && !picked.is_new_task);
    assert_eq!(
        (picked.start_label.as_str(), picked.end_label.as_str(), picked.duration.as_str()),
        ("09:00", "10:15", "1h 15m")
    );
    assert_eq!(picked.date.map(|d| d.label), Some("Nov 25, 2026".into()));
    assert_eq!(picked.error, None);

    let mut new = log(None, " Invoice ", 540, 600);
    new.quest_id = Some(id(2).to_string());
    let check = check_log_work(&snapshot(), &new, now()).unwrap();
    assert_eq!((check.task_title.as_deref(), check.is_new_task), (Some("Invoice"), true));
    assert_eq!(check.quest.map(|q| q.name), Some("Photos".into()));
    assert_eq!(check.quest_options.len(), 2);
}

#[test]
fn log_work_refuses_like_the_desktop() {
    let s = snapshot();
    let error = |draft: &LogWorkDraft| field(check_log_work(&s, draft, now()).unwrap().error);
    assert_eq!(
        error(&log(None, "  ", 540, 600)),
        Some((FormField::Task, "Choose a task or type a title for a new one.".into()))
    );
    assert_eq!(error(&log(Some(10), "", 600, 600)), Some((FormField::End, "The end must be after the start.".into())));
    assert_eq!(check_log_work(&s, &log(Some(10), "", 600, 540), now()).unwrap().duration, "–");
    assert_eq!(error(&log(Some(10), "", 1440, 1500)).map(|e| e.0), Some(FormField::Start));
    let mut bad_day = log(Some(10), "", 540, 600);
    bad_day.day = "someday".into();
    assert_eq!(error(&bad_day).map(|e| e.0), Some(FormField::Date));
    assert_eq!(check_log_work(&s, &bad_day, now()).unwrap().date, None);
    assert_eq!(error(&log(Some(99), "", 540, 600)).map(|e| e.0), Some(FormField::Task), "a task deleted meanwhile");
}

#[test]
fn saving_log_work_is_one_log_work_command() {
    let command = decide_log_work(&snapshot(), &log(Some(11), "", 540, 615), id(40), now()).unwrap();
    let Command::LogWork { entry_id, task, started_at, ended_at } = command else { panic!("{command:?}") };
    assert_eq!((entry_id, task), (id(40), WorkTask::Existing(id(11))));
    assert_eq!((started_at, ended_at), (at("2026-11-25T09:00:00+01:00"), at("2026-11-25T10:15:00+01:00")));
    let command = decide_log_work(&snapshot(), &log(None, "Invoice", 540, 600), id(41), now()).unwrap();
    let Command::LogWork { task: WorkTask::New { title, quest_id, day, .. }, .. } = command else { panic!() };
    assert_eq!((title.as_str(), quest_id, day), ("Invoice", None, now().today()));
}

#[test]
fn the_entry_editor_opens_with_local_times_and_midnight_ends_on_another_day() {
    let draft = entry_draft(&snapshot(), &id(20).to_string(), now()).unwrap();
    assert_eq!((draft.start_day.as_str(), draft.start_minute, draft.end_minute), ("2026-11-25", 480, 570));
    assert!(!draft.ends_other_day);
    let overnight = entry_draft(&snapshot(), &id(21).to_string(), now()).unwrap();
    assert_eq!((overnight.start_day.as_str(), overnight.end_day.as_str()), ("2026-11-24", "2026-11-25"));
    assert!(overnight.ends_other_day);
    assert!(matches!(entry_draft(&snapshot(), &id(99).to_string(), now()), Err(MobileError::UnknownEntry { .. })));
}

#[test]
fn the_entry_editors_quest_follows_its_task() {
    let mut draft = entry_draft(&snapshot(), &id(20).to_string(), now()).unwrap();
    let check = check_entry(&snapshot(), &draft, now()).unwrap();
    assert_eq!(
        (check.task_title.as_deref(), check.quest.map(|q| q.name)),
        (Some("Write chapter"), Some("Book".into()))
    );
    assert!(check.can_open_task);
    assert_eq!(check.duration, "1h 30m");
    draft.task_id = Some(id(12).to_string());
    let check = check_entry(&snapshot(), &draft, now()).unwrap();
    assert_eq!(check.quest, None, "Mail has no quest");
    let Command::EditEntry { task, quest_id, .. } = decide_entry(&snapshot(), &draft, now()).unwrap() else { panic!() };
    assert_eq!((task, quest_id), (Some(WorkTask::Existing(id(12))), None));
    draft.task_id = Some(id(11).to_string());
    let Command::EditEntry { quest_id, .. } = decide_entry(&snapshot(), &draft, now()).unwrap() else { panic!() };
    assert_eq!(quest_id, Some(id(1)));
    draft.task_id = None;
    let check = check_entry(&snapshot(), &draft, now()).unwrap();
    assert_eq!((check.task_title, check.can_open_task), (None, false));
    let Command::EditEntry { task, quest_id, .. } = decide_entry(&snapshot(), &draft, now()).unwrap() else { panic!() };
    assert_eq!((task, quest_id), (None, None));
}

#[test]
fn the_entry_editor_keeps_seconds_crosses_midnight_and_refuses_like_the_desktop() {
    let mut draft = entry_draft(&snapshot(), &id(20).to_string(), now()).unwrap();
    let Command::EditEntry { started_at, ended_at, .. } = decide_entry(&snapshot(), &draft, now()).unwrap() else {
        panic!()
    };
    assert_eq!((started_at, ended_at), (at("2026-11-25T08:00:20+01:00"), Some(at("2026-11-25T09:30:40+01:00"))));
    draft.end_minute = 60;
    assert_eq!(field(check_entry(&snapshot(), &draft, now()).unwrap().error).map(|e| e.0), Some(FormField::End));
    assert!(matches!(decide_entry(&snapshot(), &draft, now()), Err(MobileError::InvalidForm { .. })));
    draft.ends_other_day = true;
    draft.end_day = "2026-11-26".into();
    let check = check_entry(&snapshot(), &draft, now()).unwrap();
    assert_eq!((check.error, check.duration.as_str()), (None, "16h 59m"), "seconds kept: 08:00:20 to 01:00");
    let Command::EditEntry { ended_at, .. } = decide_entry(&snapshot(), &draft, now()).unwrap() else { panic!() };
    assert_eq!(ended_at, Some(at("2026-11-26T01:00:00+01:00")));
    draft.end_day = "never".into();
    assert_eq!(field(check_entry(&snapshot(), &draft, now()).unwrap().error).map(|e| e.0), Some(FormField::EndDate));
    // Without "ends on another day" the end date follows the start.
    draft.ends_other_day = false;
    draft.end_minute = 600;
    assert_eq!(check_entry(&snapshot(), &draft, now()).unwrap().end_date.map(|d| d.iso), Some("2026-11-25".into()));
}

#[test]
fn a_running_entry_has_no_end_and_counts_to_now() {
    let draft = entry_draft(&snapshot(), &id(22).to_string(), now()).unwrap();
    let check = check_entry(&snapshot(), &draft, now()).unwrap();
    assert!(check.running);
    assert_eq!(check.duration, "40m");
    let Command::EditEntry { ended_at, .. } = decide_entry(&snapshot(), &draft, now()).unwrap() else { panic!() };
    assert_eq!(ended_at, None);
}

#[test]
fn deleting_names_the_task_or_session() {
    assert_eq!(delete_entry(&snapshot(), &id(20).to_string()).unwrap().1, "Write chapter");
    assert_eq!(delete_entry(&snapshot(), &id(21).to_string()).unwrap(), (Deletable::Entry(id(21)), "Session".into()));
}

#[test]
fn time_dragged_to_midnight_logs_until_23_59() {
    let draft = log_work_for(now().today(), 1380, 1440);
    assert_eq!(draft.end_minute, 1439);
    let check =
        check_log_work(&snapshot(), &LogWorkDraft { task_id: Some(id(10).to_string()), ..draft }, now()).unwrap();
    assert_eq!((check.error, check.end_label.as_str(), check.duration.as_str()), (None, "23:59", "59m"));
}
