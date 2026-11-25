use quest_domain::{ChecklistItem, DateField, Postponement, Snapshot};

use super::*;
use crate::test_support::{at, berlin, day, entry, id, quest, task};

fn now() -> Now {
    berlin("2026-11-25T10:30:00+01:00")
}

fn snapshot() -> Snapshot {
    let mut draft = task(10, "Draft", Some(1), Some("2026-11-24"));
    draft.description = "Line one\nLine two".into();
    draft.due_on = Some(day("2026-11-24"));
    Snapshot {
        quests: vec![quest(1, "Book"), quest(2, "Garden")],
        tasks: vec![draft, task(11, "Someday", None, None)],
        checklist: vec![ChecklistItem {
            id: id(30),
            task_id: id(10),
            text: "Outline".into(),
            done: true,
            sort_key: "a".into(),
        }],
        postponements: vec![Postponement {
            id: id(40),
            task_id: id(10),
            field: DateField::Plan,
            from: day("2026-11-23"),
            to: day("2026-11-24"),
            at: at("2026-11-23T18:00:00+01:00"),
        }],
        time_entries: vec![
            entry(20, Some(10), Some(1), "2026-11-23T09:00:00+01:00", Some("2026-11-23T10:30:00+01:00")),
            entry(21, Some(10), Some(1), "2026-11-25T10:00:00+01:00", None),
            entry(22, Some(11), None, "2026-11-25T08:00:00+01:00", Some("2026-11-25T08:45:00+01:00")),
        ],
        ..Snapshot::default()
    }
}

#[test]
fn shows_the_fields_the_desktop_dialog_shows() {
    let snapshot = snapshot();
    let detail = task_detail(&snapshot, snapshot.task(id(10)).unwrap(), now());
    assert_eq!(
        (detail.title.as_str(), detail.description.as_str(), detail.done),
        ("Draft", "Line one\nLine two", false)
    );
    assert_eq!(detail.quest.map(|q| q.name), Some("Book".into()));
    assert_eq!(detail.quest_options.iter().map(|q| q.name.as_str()).collect::<Vec<_>>(), ["Book", "Garden"]);
    let nov_24 = DayValue { iso: "2026-11-24".into(), label: "Nov 24, 2026".into() };
    assert_eq!((detail.planned_on, detail.due_on), (Some(nov_24.clone()), Some(nov_24)));
    assert!(detail.overdue);
    assert_eq!(detail.postponed, PostponedCount { plan: 1, due: 0, total: 1 });
    let checklist: Vec<_> = detail.checklist.iter().map(|r| (r.text.as_str(), r.done)).collect();
    assert_eq!(checklist, [("Outline", true)]);
    assert_eq!((detail.repeat_label.as_str(), detail.repeats), ("Does not repeat", false));
}

#[test]
fn logged_time_lists_sessions_newest_first_with_a_total() {
    let snapshot = snapshot();
    let detail = task_detail(&snapshot, snapshot.task(id(10)).unwrap(), now());
    assert_eq!(detail.logged_total, "2h total");
    let logged: Vec<_> = detail.logged.iter().map(|e| (e.when.as_str(), e.duration.as_str(), e.running)).collect();
    assert_eq!(logged, [("Nov 25 · 10:00–now", "30m", true), ("Nov 23 · 09:00–10:30", "1h 30m", false)]);
}

#[test]
fn only_the_newest_four_sessions_show_but_all_count() {
    let mut snapshot = snapshot();
    for n in 0..5 {
        let from = format!("2026-11-2{n}T07:00:00+01:00");
        let to = format!("2026-11-2{n}T07:30:00+01:00");
        snapshot.time_entries.push(entry(30 + n as u128, Some(10), Some(1), &from, Some(&to)));
    }
    let detail = task_detail(&snapshot, snapshot.task(id(10)).unwrap(), now());
    let when: Vec<_> = detail.logged.iter().map(|e| e.when.as_str()).collect();
    assert_eq!(when, ["Nov 25 · 10:00–now", "Nov 24 · 07:00–07:30", "Nov 23 · 09:00–10:30", "Nov 23 · 07:00–07:30"]);
    assert_eq!(detail.logged_total, "4h 30m total");
}

#[test]
fn an_unplanned_task_has_no_dates() {
    let snapshot = snapshot();
    let detail = task_detail(&snapshot, snapshot.task(id(11)).unwrap(), now());
    assert_eq!((detail.planned_on, detail.due_on, detail.overdue), (None, None, false));
    assert!(detail.checklist.is_empty());
    assert_eq!(detail.quest, None);
    assert_eq!(detail.logged_total, "45m total");
}

#[test]
fn short_sessions_and_their_total_read_in_the_same_minutes() {
    let mut snapshot = snapshot();
    snapshot.time_entries =
        vec![entry(40, Some(11), None, "2026-11-25T10:00:00+01:00", Some("2026-11-25T10:01:26+01:00"))];
    let detail = task_detail(&snapshot, snapshot.task(id(11)).unwrap(), now());
    assert_eq!(detail.logged_total, "1m total", "not \"0h total\"");
    assert_eq!(detail.logged[0].duration, "1m");
    let snapshot = Snapshot { time_entries: Vec::new(), ..snapshot };
    assert_eq!(task_detail(&snapshot, snapshot.task(id(11)).unwrap(), now()).logged_total, "0m total");
}
