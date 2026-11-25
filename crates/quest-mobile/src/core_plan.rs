//! `QuestCore`'s Plan: the days around what shows, adding on a day, and moving cards (drop,
//! "Move to…", the accessibility actions). Decisions live in `plan`.

use quest_domain::{Command, EntityId, Position};
use quest_view::home::one_line;

use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::home::Choice;
use crate::ids::parse_iso_day;
use crate::plan::{self, DayWindow, PlanMemory};
use crate::plan_types::{MoveTarget, PlanData, PlanWindow};
use crate::time::Now;
use crate::types::{AddPosition, HomeQuery, Moment};

/// The plan's days to build: `visible_days` show from `first_visible` (None: today), `built`
/// are built (None: none yet). Returns `built` while it still reaches a few days beyond what
/// shows, else a fresh window of whole weeks around it. Pure and cheap: call it whenever the
/// visible days change.
#[uniffi::export]
pub fn plan_window(
    first_visible: Option<String>,
    visible_days: u32,
    built: Option<PlanWindow>,
    now: Moment,
) -> Result<PlanWindow, MobileError> {
    guard(|| {
        let today = Now::parse(&now)?.today();
        let first = first_visible.as_deref().map(parse_iso_day).transpose()?.unwrap_or(today);
        let built = built.as_ref().map(DayWindow::parse).transpose()?;
        Ok(plan::window(first, visible_days, built, today).record())
    })
}

/// How fast a drag resting at `pos` scrolls a board spanning `start..end` (dp, dp a second):
/// negative towards earlier days in the start's edge zone, positive in the end's, faster nearer
/// the edge, else 0 (the desktop's `plan_strip::edge_speed`). Pure.
#[uniffi::export]
pub fn plan_edge_speed(pos: f32, start: f32, end: f32) -> f32 {
    quest_view::plan_strip::edge_speed(pos, start, end)
}

#[uniffi::export]
impl QuestCore {
    /// The days of `window`; `quest_filter` narrows QUEST WORK to one quest. Edits on the
    /// repeats it previews work by their ids until the next `plan`.
    pub fn plan(&self, window: PlanWindow, quest_filter: Option<String>, now: Moment) -> Result<PlanData, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let window = DayWindow::parse(&window)?;
            let choice = Choice::parse(&HomeQuery { quest_filter, quest_pick: None })?;
            self.remember_plan(PlanMemory { window, at: now.at, choice });
            let toast = self.undo_state().toast();
            Ok(plan::plan(&self.store().snapshot(), window, now, choice, toast))
        })
    }

    /// Call when the Plan screen goes (not for a sheet over it): edits no longer reach the
    /// repeats it previewed, and moves sort as on Home again (desktop: only while on the plan).
    pub fn leave_plan(&self) {
        self.forget_plan();
    }

    /// Adds a task without a quest to `day` ("2026-11-24"), at its top or bottom, as the
    /// desktop's add field on a day. Returns its id.
    pub fn plan_add_task(
        &self,
        day: String,
        title: String,
        position: AddPosition,
        now: Moment,
    ) -> Result<String, MobileError> {
        guard(|| {
            Now::parse(&now)?;
            let day = parse_iso_day(&day)?;
            let title = one_line(&title).trim().to_owned();
            if title.is_empty() {
                return Err(MobileError::EmptyTitle);
            }
            let position = match position {
                AddPosition::Top => Position::Top,
                AddPosition::Bottom => Position::Bottom,
            };
            let id = EntityId::new();
            let task = Command::CreateTask { id, title, quest_id: None, planned_on: Some(day), due_on: None, position };
            self.dispatch(task)?;
            Ok(id.to_string())
        })
    }

    /// A drop: plans the task for `day` (None: unplanned), right before `before_task_id` (a
    /// card of the same day and section) or, with None, at the end of the day.
    pub fn plan_move_task(
        &self,
        task_id: String,
        day: Option<String>,
        before_task_id: Option<String>,
        now: Moment,
    ) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let task = self.task(&task_id)?;
            let day = day.as_deref().map(parse_iso_day).transpose()?;
            // An anchor gone meanwhile (deleted, synced away) leaves the end of the day.
            let before = before_task_id.as_deref().and_then(|id| self.task(id).ok());
            plan::drop_move(&task, day, before.as_ref(), now.at).map_or(Ok(()), |command| self.dispatch(command))
        })
    }

    /// "Move up" / "Move down": one place within the task's day and section. Nothing at
    /// either end.
    pub fn plan_nudge_task(&self, task_id: String, up: bool, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let task = self.task(&task_id)?;
            let (at, choice) = self.plan_memory().map_or((now.at, Choice::default()), |m| (m.at, m.choice));
            let command = plan::nudge_move(&self.store().snapshot(), at, choice, &task, up, now.at);
            command.map_or(Ok(()), |command| self.dispatch(command))
        })
    }

    /// "Move to previous day" / "Move to next day": to the bottom of the day before or after.
    pub fn plan_shift_task(&self, task_id: String, later: bool, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let task = self.task(&task_id)?;
            self.dispatch(plan::shift_move(&task, later, now))
        })
    }

    /// The entries of "Move to…".
    pub fn move_targets(&self, now: Moment) -> Result<Vec<MoveTarget>, MobileError> {
        guard(|| Ok(plan::move_targets(Now::parse(&now)?.today())))
    }
}

#[cfg(test)]
#[path = "core_plan_tests.rs"]
mod tests;
