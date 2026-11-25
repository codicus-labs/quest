//! Focus and the running session, mapped from the snapshot as the desktop's Focus view shows
//! them (`quest-app/src/ui/focus.rs`). Pure.

use chrono::TimeDelta;
use quest_domain::{EntityId, EntryKind, Snapshot, Task};
use quest_view::focus::{FocusTarget, target};
use quest_view::format;

use crate::home::{chip, color_hex};
use crate::time::Now;
use crate::types::{FocusPickTask, FocusSession, FocusView, RunningSession};
use crate::work_types::SessionKind;

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

/// No session: Today's tasks to start one, in Today's order.
fn pick(snapshot: &Snapshot, now: Now) -> Vec<FocusPickTask> {
    quest_view::lists::today(snapshot, now.today())
        .into_iter()
        .map(|t| FocusPickTask {
            id: t.id.to_string(),
            title: t.title.clone(),
            quest: t.quest_id.and_then(|id| snapshot.quest(id)).map(chip),
        })
        .collect()
}

/// The running timer: a task's, with the chronometer base that makes the notification show the
/// Focus timer's value (all of the task's tracked time), or organizing, counting this session.
/// `snapshot` shows the timer's workspace; `elsewhere` names it when that is not the shown one.
pub(crate) fn running_session(snapshot: &Snapshot, now: Now, elsewhere: Option<String>) -> Option<RunningSession> {
    let entry = snapshot.running_entry()?;
    if entry.kind == EntryKind::Organizing {
        let origin = entry.origin_task_id.and_then(|id| snapshot.task(id)).filter(|t| !t.is_done());
        return Some(RunningSession {
            kind: SessionKind::Organizing,
            task_id: None,
            title: "Organizing".to_owned(),
            quest_name: None,
            quest_color_hex: None,
            chronometer_base_ms: entry.started_at.timestamp_millis(),
            origin_task_id: origin.map(|t| t.id.to_string()),
            origin_title: origin.map(|t| t.title.clone()),
            workspace_name: elsewhere,
        });
    }
    let task: &Task = entry.task_id.and_then(|id| snapshot.task(id))?;
    let quest = task.quest_id.and_then(|id| snapshot.quest(id));
    let tracked = TimeDelta::milliseconds(snapshot.task_tracked_ms(task.id, now.at));
    Some(RunningSession {
        kind: SessionKind::Task,
        task_id: Some(task.id.to_string()),
        title: task.title.clone(),
        quest_name: quest.map(|q| q.title.clone()),
        quest_color_hex: quest.map(|q| color_hex(q.color).to_owned()),
        chronometer_base_ms: (now.at - tracked).timestamp_millis(),
        origin_task_id: None,
        origin_title: None,
        workspace_name: elsewhere,
    })
}

#[cfg(test)]
#[path = "focus_tests.rs"]
mod tests;
