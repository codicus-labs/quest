//! Focus and the running session, mapped from the snapshot as the desktop's Focus view shows
//! them (`quest-app/src/ui/focus.rs`). Pure.

use chrono::TimeDelta;
use quest_domain::{EntityId, Snapshot, Task};
use quest_view::focus::{FocusTarget, target};
use quest_view::format;

use crate::home::{chip, color_hex};
use crate::time::Now;
use crate::types::{FocusPickTask, FocusSession, FocusView, RunningSession};

pub(crate) fn focus(snapshot: &Snapshot, focus_task: Option<EntityId>, now: Now) -> FocusView {
    let (task, running) = match target(snapshot, focus_task) {
        FocusTarget::Running { task } => (task, true),
        FocusTarget::Paused { task } => (task, false),
        FocusTarget::Nothing => return FocusView::Pick { tasks: pick(snapshot, now) },
    };
    let quest = task.quest_id.and_then(|id| snapshot.quest(id));
    let header = quest.map_or("Focus".to_owned(), |q| format!("Focus · {}", q.title));
    FocusView::Session {
        session: FocusSession {
            task_id: task.id.to_string(),
            title: task.title.clone(),
            description: task.description.clone(),
            quest: quest.map(chip),
            header: header.to_uppercase(),
            timer: format::timer(snapshot.task_tracked_ms(task.id, now.at)),
            running,
            state_label: if running { "RUNNING" } else { "PAUSED" }.to_owned(),
        },
    }
}

/// No session: today's open tasks to start one, as the desktop lists them.
fn pick(snapshot: &Snapshot, now: Now) -> Vec<FocusPickTask> {
    snapshot
        .tasks_on(now.today())
        .filter(|t| !t.is_done())
        .map(|t| FocusPickTask {
            id: t.id.to_string(),
            title: t.title.clone(),
            quest: t.quest_id.and_then(|id| snapshot.quest(id)).map(chip),
        })
        .collect()
}

/// The running timer's task, with the chronometer base that makes the notification show the
/// Focus timer's value (all of the task's tracked time).
/// `snapshot` shows the timer's workspace; `elsewhere` names it when that is not the shown one.
pub(crate) fn running_session(snapshot: &Snapshot, now: Now, elsewhere: Option<String>) -> Option<RunningSession> {
    let task: &Task = snapshot.running_entry()?.task_id.and_then(|id| snapshot.task(id))?;
    let quest = task.quest_id.and_then(|id| snapshot.quest(id));
    let tracked = TimeDelta::milliseconds(snapshot.task_tracked_ms(task.id, now.at));
    Some(RunningSession {
        task_id: task.id.to_string(),
        title: task.title.clone(),
        quest_name: quest.map(|q| q.title.clone()),
        quest_color_hex: quest.map(|q| color_hex(q.color).to_owned()),
        chronometer_base_ms: (now.at - tracked).timestamp_millis(),
        workspace_name: elsewhere,
    })
}

#[cfg(test)]
#[path = "focus_tests.rs"]
mod tests;
