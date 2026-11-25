use chrono::NaiveDate;
use quest_domain::{Command, EntityId, Position, Task};
use rune::TextEdit;

use super::{TaskModal, day_label, is_saved, move_to};

fn day(s: &str) -> NaiveDate {
    s.parse().expect("a date")
}

fn task(on: Option<NaiveDate>) -> Task {
    Task {
        id: EntityId::new(),
        title: "Sketch the form".to_owned(),
        quest_id: None,
        day: on,
        sort_key: String::new(),
        block: None,
        completed_at: None,
        description: "Two columns".to_owned(),
        repeat: None,
        series: None,
    }
}

#[test]
fn the_date_field_moves_to_the_day_it_names_or_the_backlog() {
    let today = day("2026-11-23");
    let task = task(Some(today));
    let moved = |day| Some(Command::MoveTask { id: task.id, day, position: Position::Bottom });
    assert_eq!(move_to(&task, "Nov 24, 2026", today), moved(Some(day("2026-11-24"))));
    assert_eq!(move_to(&task, "  ", today), moved(None), "empty is the backlog");
    assert_eq!(move_to(&task, &day_label(Some(today)), today), None, "its own day moves nothing");
    assert_eq!(move_to(&task, "someday soon", today), None, "nor does no day at all");
}

#[test]
fn saved_means_every_field_is_stored() {
    let task = task(Some(day("2026-11-23")));
    let mut form = TaskModal {
        task: task.id,
        title: TextEdit::new(task.title.clone()),
        description: TextEdit::new(task.description.clone()),
        date: TextEdit::new("Nov 23, 2026"),
    };
    assert!(is_saved(&task, &form));
    form.title = TextEdit::new("  ");
    assert!(!is_saved(&task, &form), "an empty title is never saved");
    form.title = TextEdit::new(task.title.clone());
    form.date = TextEdit::new("Nov 24, 2026");
    assert!(!is_saved(&task, &form), "a day typed but not taken yet");
}
