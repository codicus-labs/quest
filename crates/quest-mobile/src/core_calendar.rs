//! `QuestCore`'s Calendar: the page, drags on its grid, Log work and the entry editor.
//! Memory-only like the rest of the core; decisions live in `calendar`, `grid_drag` and
//! `work_forms`.

use quest_domain::EntityId;

use crate::calendar::{self, Page};
use crate::calendar_types::{
    CalendarData, CalendarQuery, EntryCheck, EntryDraft, GridDrag, GridDragKind, GridPlacement, LogWorkCheck,
    LogWorkDraft, TaskChoices,
};
use crate::core::QuestCore;
use crate::error::MobileError;
use crate::grid_drag::{self, Landing};
use crate::guard::guard;
use crate::ids::parse_iso_day;
use crate::time::Now;
use crate::types::Moment;
use crate::work_forms;

#[uniffi::export]
impl QuestCore {
    /// A Calendar page: its days' grids and totals, the pages around it and the week's Quest
    /// Hours. A running timer's block grows to now: read it again on every tick.
    pub fn calendar(&self, query: CalendarQuery, now: Moment) -> Result<CalendarData, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let page = Page::of(&query, now.today())?;
            let toast = self.undo_state().toast();
            calendar::calendar(&self.store().snapshot(), page, now, toast)
        })
    }

    /// Where `drag` would land with the finger over `day` at the unsnapped `minute`: the ghost to
    /// draw. None: it can't land there and snaps back. Pure and cheap: call it on every move.
    pub fn grid_drag_preview(
        &self,
        drag: GridDrag,
        day: String,
        minute: f32,
        now: Moment,
    ) -> Result<Option<GridPlacement>, MobileError> {
        guard(|| {
            let (day, now) = (parse_iso_day(&day)?, Now::parse(&now)?);
            let landing = grid_drag::place(&self.store().snapshot(), &drag, day, minute, now)?;
            Ok(landing.map(|landing| landing.placement()))
        })
    }

    /// Drops `drag`. A block or handle saves the new times (the snackbar offers Undo) and returns
    /// None. Time to log saves nothing and returns the Log work draft to open the sheet with.
    pub fn grid_drop(
        &self,
        drag: GridDrag,
        day: String,
        minute: f32,
        now: Moment,
    ) -> Result<Option<LogWorkDraft>, MobileError> {
        guard(|| {
            let (day, now) = (parse_iso_day(&day)?, Now::parse(&now)?);
            let snapshot = self.store().snapshot();
            let Some(landing) = grid_drag::place(&snapshot, &drag, day, minute, now)? else { return Ok(None) };
            if let Landing::NewSpan { day, span } = landing {
                return Ok(Some(work_forms::log_work_for(day, span.start as u32, span.end as u32)));
            }
            let Some((command, before)) = grid_drag::command(&snapshot, landing, now) else { return Ok(None) };
            let from_day = parse_iso_day(&drag.from_day)?;
            let title = grid_drag::grabbed(&snapshot, &drag, from_day, now).map(|b| b.title).unwrap_or_default();
            self.dispatch(command)?;
            let resized = drag.kind != GridDragKind::Move;
            self.undo_state().moved(before, resized, &title, now.at);
            Ok(None)
        })
    }

    /// Tasks to pick for Log work and the entry editor (`query`: what was typed).
    pub fn task_choices(&self, query: String) -> Result<TaskChoices, MobileError> {
        guard(|| Ok(work_forms::task_choices(&self.store().snapshot(), &query)))
    }

    /// Log work from the button: the quarter hour that just ended, today.
    pub fn log_work_now(&self, now: Moment) -> Result<LogWorkDraft, MobileError> {
        guard(|| Ok(work_forms::log_work_now(Now::parse(&now)?)))
    }

    /// The Log work sheet's labels, quest and what saving would refuse.
    pub fn check_log_work(&self, draft: LogWorkDraft, now: Moment) -> Result<LogWorkCheck, MobileError> {
        guard(|| work_forms::check_log_work(&self.store().snapshot(), &draft, Now::parse(&now)?))
    }

    /// Saves the session (a new title becomes a completed task). A wrong field is
    /// `InvalidForm` with the desktop's message. Returns the new entry's id.
    pub fn log_work(&self, draft: LogWorkDraft, now: Moment) -> Result<String, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let entry_id = EntityId::new();
            let command = work_forms::decide_log_work(&self.store().snapshot(), &draft, entry_id, now)
                .map_err(|e| MobileError::InvalidForm { reason: e.message })?;
            self.dispatch(command)?;
            Ok(entry_id.to_string())
        })
    }

    /// The entry editor's fields as it opens. `UnknownEntry` once the session is gone.
    pub fn entry_draft(&self, entry_id: String, now: Moment) -> Result<EntryDraft, MobileError> {
        guard(|| work_forms::entry_draft(&self.store().snapshot(), &entry_id, Now::parse(&now)?))
    }

    /// The entry editor's labels, the task's quest, the duration and what saving would refuse.
    pub fn check_entry(&self, draft: EntryDraft, now: Moment) -> Result<EntryCheck, MobileError> {
        guard(|| work_forms::check_entry(&self.store().snapshot(), &draft, Now::parse(&now)?))
    }

    /// Saves the times and the task; the session counts towards that task's quest.
    pub fn save_entry(&self, draft: EntryDraft, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let command = work_forms::decide_entry(&self.store().snapshot(), &draft, now)?;
            self.dispatch(command)
        })
    }

    /// Deletes the session softly; the snackbar offers to undo it.
    pub fn delete_entry(&self, entry_id: String, now: Moment) -> Result<(), MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let (item, title) = work_forms::delete_entry(&self.store().snapshot(), &entry_id)?;
            self.dispatch(item.delete(now.at))?;
            self.undo_state().deleted(item, title, now.at);
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "core_calendar_tests.rs"]
mod tests;
