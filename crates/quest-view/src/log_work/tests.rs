use chrono::{NaiveDate, NaiveDateTime};
use quest_domain::{Command, EntityId, Quest, QuestColor, QuestStatus, Snapshot, Task, WorkTask};

use super::*;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 23).unwrap()
}

fn utc(t: NaiveDateTime) -> DateTime<Utc> {
    t.and_utc()
}

fn snapshot() -> (Snapshot, EntityId, EntityId) {
    let quest = Quest {
        id: EntityId::new(),
        title: "Photo series".into(),
        color: QuestColor::Red,
        status: QuestStatus::Active,
        sort_key: "a".into(),
    };
    let task = |title: &str, done: bool| Task {
        id: EntityId::new(),
        title: title.into(),
        quest_id: Some(quest.id),
        planned_on: Some(today()),
        due_on: None,
        sort_key: "a".into(),
        completed_at: done.then(|| "2026-10-15T14:40:00Z".parse().unwrap()),
        description: String::new(),
        repeat: None,
        day_order: String::new(),
        series: None,
    };
    let (open, done) = (task("Motif ideas", false), task("Pick three motifs", true));
    let ids = (quest.id, done.id);
    (Snapshot { tasks: vec![done, open], quests: vec![quest], ..Snapshot::default() }, ids.0, ids.1)
}

fn draft<'a>(task: Option<EntityId>, title: &'a str, start: &'a str, end: &'a str) -> Draft<'a> {
    Draft { task, title, date: "Nov 23, 2026", start, end, quest: None }
}

#[test]
fn suggestions_include_done_tasks_after_open_ones() {
    let (snap, _, done) = snapshot();
    let titles: Vec<&str> = suggestions(&snap, "MOTIF").iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["Motif ideas", "Pick three motifs"]);
    assert_eq!(suggestions(&snap, "three")[0].id, done);
    assert!(suggestions(&snap, "  ").is_empty());
}

#[test]
fn a_picked_task_brings_its_quest() {
    let (snap, quest, done) = snapshot();
    assert_eq!(super::quest(&snap, Some(done), None), Some(quest));
    assert_eq!(super::quest(&snap, None, None), None, "a new task has the quest chosen in the dialog");
}

#[test]
fn saving_logs_on_the_picked_task_or_creates_a_done_one() {
    let (_, quest, done) = snapshot();
    let (entry, new) = (EntityId::new(), EntityId::new());
    let at = |h, m| utc(today().and_hms_opt(h, m, 0).unwrap());
    assert_eq!(
        decide(&draft(Some(done), "Pick three motifs", "14:50", "15:05"), today(), utc, entry, new),
        Ok(Command::LogWork {
            entry_id: entry,
            task: WorkTask::Existing(done),
            started_at: at(14, 50),
            ended_at: at(15, 5)
        })
    );
    let fresh = Draft { quest: Some(quest), ..draft(None, " Contact sheet ", "9", "9:30") };
    assert_eq!(
        decide(&fresh, today(), utc, entry, new),
        Ok(Command::LogWork {
            entry_id: entry,
            task: WorkTask::New { id: new, title: "Contact sheet".into(), quest_id: Some(quest), planned_on: today() },
            started_at: at(9, 0),
            ended_at: at(9, 30)
        })
    );
}

#[test]
fn a_wrong_field_refuses_to_save() {
    let (e, n) = (EntityId::new(), EntityId::new());
    let refuse = |d: Draft| decide(&d, today(), utc, e, n).unwrap_err();
    assert_eq!(refuse(draft(None, " ", "9", "10")), FieldError::Task);
    assert_eq!(refuse(Draft { date: "someday", ..draft(None, "x", "9", "10") }), FieldError::Date);
    assert_eq!(refuse(draft(None, "x", "soon", "10")), FieldError::Start);
    assert_eq!(refuse(draft(None, "x", "9", "")), FieldError::End);
    assert_eq!(refuse(draft(None, "x", "10:00", "10:00")), FieldError::EndNotAfterStart);
    assert_eq!(refuse(draft(None, "x", "10:00", "9:45")), FieldError::EndNotAfterStart);
}

#[test]
fn dates_read_in_the_field_format_or_iso() {
    let accepts = |date: &str| {
        decide(&Draft { date, ..draft(None, "x", "9", "10") }, today(), utc, EntityId::new(), EntityId::new()).is_ok()
    };
    assert!(accepts("Nov 23, 2026") && accepts("2026-11-20") && accepts("Nov 2"));
    assert_eq!(date_label(today()), "Nov 23, 2026");
}
