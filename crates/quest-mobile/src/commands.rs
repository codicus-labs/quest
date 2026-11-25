//! Turns UI intents into domain commands, deciding as the desktop does. Pure: ids and time come
//! in as arguments. `None` means there is nothing to do (e.g. an unchanged title).

use chrono::NaiveDate;
use quest_domain::{Command, EntityId, Position, RepeatRule, Snapshot, Task};
use quest_view::home::{one_line, rename};
use quest_view::repeat::series_of;

use crate::error::MobileError;
use crate::home::Choice;
use crate::ids::parse_id;
use crate::time::Now;
use crate::types::{AddPosition, TaskSection};

/// A new task for today in one of Home's lists (desktop `home::add_today`).
pub(crate) fn add_task(
    snapshot: &Snapshot,
    id: EntityId,
    title: &str,
    section: TaskSection,
    position: AddPosition,
    choice: Choice,
    now: Now,
) -> Result<Command, MobileError> {
    let title = one_line(title);
    let title = title.trim();
    if title.is_empty() {
        return Err(MobileError::EmptyTitle);
    }
    let position = match position {
        AddPosition::Top => Position::Top,
        AddPosition::Bottom => Position::Bottom,
    };
    let quest_id = choice.known_in(snapshot).quest_for(snapshot, section);
    Ok(Command::CreateTask {
        id,
        title: title.to_owned(),
        quest_id,
        planned_on: Some(now.today()),
        due_on: None,
        position,
    })
}

/// Completes an open task or reopens a done one. Completing goes the plan's way (desktop
/// `toggle_done` on the plan): a repeating task done ahead of its day moves to today; any other
/// completes as on Home.
pub(crate) fn toggle_task(task: &Task, now: Now) -> Command {
    if task.is_done() {
        return Command::ReopenTask { id: task.id };
    }
    Command::CompleteTaskFromPlan { id: task.id, at: now.at, today: now.today() }
}

/// The desktop's title rule: line breaks become spaces; empty or unchanged saves nothing.
pub(crate) fn rename_task(task: &Task, title: &str) -> Option<Command> {
    rename(task, title)
}

/// Starts the task's timer; one running elsewhere stops (the domain does that). Already running
/// on this task: nothing, so a double tap does not split the session.
pub(crate) fn start_timer(
    snapshot: &Snapshot,
    task: &Task,
    entry_id: EntityId,
    now: Now,
) -> Result<Option<Command>, MobileError> {
    if task.is_done() {
        return Err(MobileError::TaskIsDone { id: task.id.to_string() });
    }
    if snapshot.running_entry().is_some_and(|e| e.task_id == Some(task.id)) {
        return Ok(None);
    }
    Ok(Some(Command::StartFocus { entry_id, task_id: task.id, at: now.at }))
}

pub(crate) fn set_description(task: &Task, description: &str) -> Option<Command> {
    (task.description != description)
        .then(|| Command::SetTaskDescription { id: task.id, description: description.to_owned() })
}

pub(crate) fn set_quest(task: &Task, quest_id: Option<&str>) -> Result<Option<Command>, MobileError> {
    let quest_id = quest_id.map(parse_id).transpose()?;
    Ok((task.quest_id != quest_id).then_some(Command::AssignTask { id: task.id, quest_id }))
}

/// Plans the task for `planned_on` (None: unplanned) at the bottom of that day's list, as the
/// task dialog's date field does; nothing when it is planned so already.
pub(crate) fn set_planned(task: &Task, planned_on: Option<NaiveDate>, now: Now) -> Option<Command> {
    (task.planned_on != planned_on).then_some(Command::MoveTask {
        id: task.id,
        planned_on,
        position: Position::Bottom,
        at: now.at,
    })
}

/// Sets or clears the deadline; nothing when it is so already.
pub(crate) fn set_due(task: &Task, due_on: Option<NaiveDate>, now: Now) -> Option<Command> {
    (task.due_on != due_on).then_some(Command::SetTaskDue { id: task.id, due_on, at: now.at })
}

/// Sets or (with `None`) stops the repeat of the task's series.
pub(crate) fn set_repeat(snapshot: &Snapshot, task: &Task, rule: Option<RepeatRule>, now: Now) -> Command {
    Command::SetTaskRepeat { id: series_of(snapshot, task).id, rule, now: now.at }
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
