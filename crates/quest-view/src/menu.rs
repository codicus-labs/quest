//! A task's "…" menu, pure: which actions it offers (`task_actions`) and what each one does
//! (`task_step`). The view lists them; the app carries out the `Step`.

#[cfg(test)]
mod tests;

use quest_domain::{EntityId, Task};

use crate::palette::Mode;
use crate::undo::Deletable;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskAction {
    Open,
    Focus,
    ToggleDone,
    MoveTo,
    ScheduleAt,
    AssignQuest,
    Repeat,
    OpenQuest,
    Delete,
}

impl TaskAction {
    pub fn label(self, task: &Task) -> &'static str {
        match self {
            TaskAction::Open => "Open",
            TaskAction::Focus => "Start focus",
            TaskAction::ToggleDone if task.is_done() => "Reopen",
            TaskAction::ToggleDone => "Complete",
            TaskAction::MoveTo => "Move to…",
            TaskAction::ScheduleAt => "Schedule at…",
            TaskAction::AssignQuest => "Assign quest…",
            TaskAction::Repeat => "Repeat…",
            TaskAction::OpenQuest => "Open quest",
            TaskAction::Delete => "Delete",
        }
    }

    /// The keyboard way to do the same, shown on the right.
    pub fn hint(self) -> Option<&'static str> {
        match self {
            TaskAction::MoveTo => Some("⌥← ⌥→"),
            TaskAction::Open => Some("↵"),
            TaskAction::Delete => Some("⌫"),
            _ => None,
        }
    }
}

/// The menu's rows, top to bottom; `None` is a separator. `has_quest` says whether the task's
/// quest exists (a deleted quest's tasks count as other work).
pub fn task_actions(task: &Task, has_quest: bool) -> Vec<Option<TaskAction>> {
    let mut rows = vec![Some(TaskAction::Open), None];
    if !task.is_done() {
        rows.push(Some(TaskAction::Focus));
    }
    rows.extend([Some(TaskAction::ToggleDone), None, Some(TaskAction::MoveTo)]);
    if !task.is_done() {
        rows.push(Some(TaskAction::ScheduleAt));
    }
    rows.extend([Some(TaskAction::Repeat), Some(TaskAction::AssignQuest)]);
    if has_quest {
        rows.push(Some(TaskAction::OpenQuest));
    }
    rows.extend([None, Some(TaskAction::Delete)]);
    rows
}

/// What picking an action asks the app to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Open(EntityId),
    Repeat(EntityId),
    Focus(EntityId),
    ToggleDone(EntityId),
    Palette(Mode),
    OpenQuest(EntityId),
    Delete(Deletable),
}

pub fn task_step(action: TaskAction, task: &Task) -> Option<Step> {
    let id = task.id;
    Some(match action {
        TaskAction::Open => Step::Open(id),
        TaskAction::Repeat => Step::Repeat(id),
        TaskAction::Focus => Step::Focus(id),
        TaskAction::ToggleDone => Step::ToggleDone(id),
        TaskAction::MoveTo => Step::Palette(Mode::MoveTo(id)),
        TaskAction::ScheduleAt => Step::Palette(Mode::ScheduleAt(id)),
        TaskAction::AssignQuest => Step::Palette(Mode::AssignQuest(id)),
        TaskAction::OpenQuest => Step::OpenQuest(task.quest_id?),
        TaskAction::Delete => Step::Delete(Deletable::Task(id)),
    })
}
