use chrono::{Days, TimeDelta};
use quest_domain::{Command, EntityId, Position};

use crate::state::State;

pub fn seed(s: &mut State) {
    let now = s.now;
    let today = s.today();
    let monday = s.this_week();
    let Some(quest_id) = s.snap.active_quests().next().map(|q| q.id) else { return };
    let id: EntityId = "00000000-0000-4000-8000-000000000005".parse().expect("valid demo ID");
    if s.snap.task(id).is_some() {
        return;
    }
    let mon = crate::clock::to_utc(monday.and_hms_opt(9, 0, 0).expect("valid local time"));
    s.dispatch(Command::CreateTask {
        id,
        title: "Prepare the launch announcement".into(),
        quest_id: Some(quest_id),
        planned_on: Some(monday - Days::new(4)),
        due_on: Some(today + Days::new(1)),
        position: Position::Bottom,
    });
    for (day, at) in
        [(monday - Days::new(3), mon - TimeDelta::days(5)), (monday - Days::new(2), mon - TimeDelta::days(3))]
    {
        s.dispatch(Command::MoveTask { id, planned_on: Some(day), position: Position::Bottom, at });
    }
    if today >= monday + Days::new(3) {
        for (step, i) in [0, 1, (today - monday).num_days() as u64].into_iter().enumerate() {
            s.dispatch(Command::MoveTask {
                id,
                planned_on: Some(monday + Days::new(i)),
                position: Position::Bottom,
                at: now - TimeDelta::seconds(4 - step as i64),
            });
        }
        s.dispatch(Command::SetTaskDue { id, due_on: Some(today + Days::new(2)), today, at: now });
    } else {
        s.dispatch(Command::MoveTask { id, planned_on: Some(today), position: Position::Bottom, at: now });
    }
    for (title, plan, due, earlier_due) in [
        ("Send the course registration", None, Some(monday + Days::new(1)), Some(monday)),
        ("Review the interview notes", Some(monday + Days::new(1)), None, None),
        ("Share the first draft", Some(monday - Days::new(5)), Some(monday - Days::new(4)), None),
    ] {
        let task = EntityId::new();
        s.dispatch(Command::CreateTask {
            id: task,
            title: title.into(),
            quest_id: Some(quest_id),
            planned_on: plan,
            due_on: earlier_due.or(due),
            position: Position::Bottom,
        });
        let start = mon - TimeDelta::days(5);
        if title == "Share the first draft" {
            s.dispatch(Command::LogTime {
                entry_id: EntityId::new(),
                task_id: Some(task),
                started_at: start,
                ended_at: start + TimeDelta::hours(3),
            });
        } else if earlier_due.is_some() {
            s.dispatch(Command::SetTaskDue { id: task, due_on: due, today: monday, at: now });
        } else if let Some(planned_on) = plan {
            let at = now - TimeDelta::minutes(2);
            s.dispatch(Command::MoveTask { id: task, planned_on: Some(monday), position: Position::Bottom, at });
            s.dispatch(Command::MoveTask {
                id: task,
                planned_on: Some(planned_on),
                position: Position::Bottom,
                at: now,
            });
        }
    }
    s.step_undo(quest_view::undo::UndoEvent::Dismiss);
}
