use quest_domain::{QuestStatus, Snapshot};

use super::*;
use crate::test_support::{berlin, day, id, quest, task};

fn snapshot() -> Snapshot {
    let done = quest_domain::Quest { status: QuestStatus::Archived, ..quest(3, "Done quest") };
    Snapshot {
        quests: vec![quest(1, "Photo series"), quest(2, "Garden"), done],
        // The newest task with a quest works on the garden.
        tasks: vec![task(10, "Shoot", Some(1), None), task(11, "Dig", Some(2), None)],
        ..Snapshot::default()
    }
}

#[test]
fn shared_text_gives_the_title_and_the_rest_the_description() {
    let form = form(
        &snapshot(),
        Some("\n  A good read \n https://example.com/a\n\nmore\n"),
        berlin("2026-11-25T10:00:00+01:00"),
    );
    assert_eq!(form.title, "A good read");
    assert_eq!(form.description, "https://example.com/a\n\nmore");
}

#[test]
fn without_shared_text_the_form_is_empty_for_today_and_the_last_quest() {
    let form = form(&snapshot(), None, berlin("2026-11-25T10:00:00+01:00"));
    assert_eq!((form.title.as_str(), form.description.as_str()), ("", ""));
    assert_eq!(form.quest.map(|q| q.name), Some("Garden".into()));
    assert_eq!(form.quest_options.iter().map(|q| q.name.as_str()).collect::<Vec<_>>(), ["Photo series", "Garden"]);
    assert_eq!((form.today.iso.as_str(), form.today.label.as_str()), ("2026-11-25", "Nov 25, 2026"));
}

#[test]
fn without_an_active_quest_nothing_is_preselected() {
    let form = form(&Snapshot::default(), Some("Call mum"), berlin("2026-11-25T10:00:00+01:00"));
    assert_eq!(form.title, "Call mum");
    assert_eq!(form.quest, None);
}

#[test]
fn a_quick_add_creates_the_task_with_quest_day_and_description() {
    let quest_id = id(1).to_string();
    let commands = commands(&snapshot(), id(50), " Read\nit ", " link ", Some(&quest_id), Some("2026-11-26")).unwrap();
    assert_eq!(
        commands,
        [
            Command::CreateTask {
                id: id(50),
                title: "Read it".into(),
                quest_id: Some(id(1)),
                day: Some(day("2026-11-26")),
                position: Position::Bottom,
            },
            Command::SetTaskDescription { id: id(50), description: "link".into() },
        ]
    );
}

#[test]
fn a_quick_add_without_quest_and_day_goes_to_the_backlog() {
    let commands = commands(&snapshot(), id(50), "Later", "  ", None, None).unwrap();
    assert_eq!(
        commands,
        [Command::CreateTask {
            id: id(50),
            title: "Later".into(),
            quest_id: None,
            day: None,
            position: Position::Bottom
        }]
    );
}

#[test]
fn a_quick_add_rejects_a_blank_title_a_gone_quest_and_a_bad_day() {
    let gone = id(9).to_string();
    assert_eq!(commands(&snapshot(), id(50), " \n ", "", None, None), Err(MobileError::EmptyTitle));
    assert_eq!(
        commands(&snapshot(), id(50), "A", "", Some(&gone), None),
        Err(MobileError::UnknownQuest { id: gone.clone() })
    );
    assert_eq!(
        commands(&snapshot(), id(50), "A", "", None, Some("tomorrow")),
        Err(MobileError::InvalidDate { value: "tomorrow".into() })
    );
}

#[test]
fn a_huge_shared_text_is_cut() {
    let text = format!("Title\n{}", "ä".repeat(50_000));
    let form = form(&snapshot(), Some(&text), berlin("2026-11-25T10:00:00+01:00"));
    assert_eq!(form.title, "Title");
    assert_eq!(form.description.chars().count(), SHARED_TEXT_LIMIT - "Title\n".len());
}
