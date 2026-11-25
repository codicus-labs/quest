//! Drags on the calendar's grid, pure: where a drag lands (the desktop's `dnd::drop::place` for
//! grid payloads, on the mobile grid's 00:00–24:00: snapped to quarter hours, at least a quarter
//! hour long, inside the day) and what a drop does. Blocks are read from the day's `day_grid`,
//! so a drag takes exactly what the grid showed. Mobile adds a top handle to planned blocks.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use quest_domain::{Command, EntityId, Snapshot, TimeBlock};
use quest_view::format;
use quest_view::timescale::{SNAP_MINUTES, Span, TimeScale, snap};

use crate::calendar_types::{GridDrag, GridDragKind, GridPlacement};
use crate::day_grid::{self, DAY_MINUTES};
use crate::error::MobileError;
use crate::ids::{iso_day, parse_id, parse_iso_day};
use crate::task_types::{GridBlock, GridBlockKind};
use crate::time::Now;
use crate::undo::PutBack;

/// The mobile grid: the whole day. Only minutes matter here, so an hour is 60 units tall.
const SCALE: TimeScale = TimeScale { first_hour: 0, last_hour: 24, hour_height: 60.0 };

/// Where a drop lands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Landing {
    /// A session's new times on `day`; a running one keeps running.
    Entry { id: EntityId, day: NaiveDate, span: Span, running: bool },
    /// A task's block, planned on `day` for `span`.
    Block { id: EntityId, day: NaiveDate, span: Span },
    /// Time to log: nothing is saved, Log work opens with it.
    NewSpan { day: NaiveDate, span: Span },
}

impl Landing {
    pub fn day(&self) -> NaiveDate {
        match *self {
            Landing::Entry { day, .. } | Landing::Block { day, .. } | Landing::NewSpan { day, .. } => day,
        }
    }

    pub fn span(&self) -> Span {
        match *self {
            Landing::Entry { span, .. } | Landing::Block { span, .. } | Landing::NewSpan { span, .. } => span,
        }
    }

    pub fn placement(&self) -> GridPlacement {
        let span = self.span();
        GridPlacement {
            day: iso_day(self.day()),
            start_minute: span.start as u32,
            end_minute: span.end as u32,
            range_label: range_label(span),
        }
    }
}

/// "09:15 – 10:00".
pub(crate) fn range_label(span: Span) -> String {
    format!("{} – {}", format::hh_mm(span.start.max(0) as u32), format::hh_mm(span.end.max(0) as u32))
}

/// Where `drag` lands with the finger over `day` at the unsnapped `minute`; None where it can't
/// land (it snaps back): a block that is gone or doesn't take this drag, or time to log shorter
/// than a quarter hour.
pub(crate) fn place(
    snapshot: &Snapshot,
    drag: &GridDrag,
    day: NaiveDate,
    minute: f32,
    now: Now,
) -> Result<Option<Landing>, MobileError> {
    let from_day = parse_iso_day(&drag.from_day)?;
    let at = SCALE.clamp(snap(minute));
    if drag.kind == GridDragKind::NewSpan {
        let anchor = SCALE.clamp(snap(drag.grab_minute));
        let span = Span::new(anchor.min(at), anchor.max(at));
        return Ok((span.len() >= SNAP_MINUTES).then_some(Landing::NewSpan { day: from_day, span }));
    }
    let Some(block) = grabbed(snapshot, drag, from_day, now) else { return Ok(None) };
    let id = parse_id(&block.target_id)?;
    let span = match block.kind {
        // A block past midnight shows cut off; it keeps its whole length.
        GridBlockKind::Planned => snapshot.task(id).and_then(|t| t.block).map(day_grid::block_span),
        GridBlockKind::Session | GridBlockKind::Running => None,
    }
    .unwrap_or(Span::new(block.start_minute as i32, block.end_minute as i32));
    let (day, span) = match drag.kind {
        GridDragKind::Move if block.can_move => {
            let grab = (drag.grab_minute - span.start as f32).round();
            (day, span.moved_to(SCALE.clamp_start(snap(minute - grab), span.len())))
        }
        GridDragKind::StartEdge if block.start_handle => {
            (from_day, Span::new(at.min(span.end - SNAP_MINUTES), span.end))
        }
        GridDragKind::EndEdge if block.end_handle => {
            (from_day, Span::new(span.start, at.max(span.start + SNAP_MINUTES)))
        }
        _ => return Ok(None),
    };
    Ok(Some(match block.kind {
        GridBlockKind::Planned => Landing::Block { id, day, span },
        GridBlockKind::Session => Landing::Entry { id, day, span, running: false },
        GridBlockKind::Running => Landing::Entry { id, day, span, running: true },
    }))
}

/// The block `drag` holds, as the grid of its day shows it.
pub(crate) fn grabbed(snapshot: &Snapshot, drag: &GridDrag, from_day: NaiveDate, now: Now) -> Option<GridBlock> {
    let id = drag.block_id.as_deref()?;
    day_grid::day_grid(snapshot, from_day, now).blocks.into_iter().find(|block| block.id == id)
}

/// What a drop saves, with where the block was before (for Undo); None when nothing changes or
/// the landing is time to log.
pub(crate) fn command(snapshot: &Snapshot, landing: Landing, now: Now) -> Option<(Command, PutBack)> {
    let at = |day: NaiveDate, minute: i32| now.instant(local(day, minute));
    match landing {
        Landing::Entry { id, day, span, running } => {
            let entry = snapshot.time_entries.iter().find(|e| e.id == id)?;
            // An edge left on its minute keeps its seconds (as the entry editor does).
            let keep =
                |stored: DateTime<Utc>, shown: DateTime<Utc>| if same_minute(stored, shown) { stored } else { shown };
            let started_at = keep(entry.started_at, at(day, span.start));
            let ended_at = (!running).then(|| {
                let shown = at(day, span.end);
                entry.ended_at.map_or(shown, |stored| keep(stored, shown))
            });
            if started_at == entry.started_at && ended_at == entry.ended_at {
                return None;
            }
            let before = PutBack::Entry { id, started_at: entry.started_at, ended_at: entry.ended_at };
            Some((Command::SetEntryTimes { id, started_at, ended_at }, before))
        }
        Landing::Block { id, day, span } => {
            let task = snapshot.task(id)?;
            let (old_day, old_block) = (task.day?, task.block?);
            let block = block(span);
            if old_day == day && old_block == block {
                return None;
            }
            Some((Command::ScheduleTask { id, day, block }, PutBack::Block { id, day: old_day, block: old_block }))
        }
        Landing::NewSpan { .. } => None,
    }
}

/// `minute` minutes after midnight on `day` (24:00 is the next midnight).
pub(crate) fn local(day: NaiveDate, minute: i32) -> chrono::NaiveDateTime {
    day.and_time(NaiveTime::MIN) + chrono::Duration::minutes(i64::from(minute))
}

fn block(span: Span) -> TimeBlock {
    let start = local(NaiveDate::MIN, span.start.clamp(0, DAY_MINUTES - 1)).time();
    TimeBlock { start, minutes: u32::try_from(span.len()).unwrap_or(0) }
}

fn same_minute(a: DateTime<Utc>, b: DateTime<Utc>) -> bool {
    a.timestamp().div_euclid(60) == b.timestamp().div_euclid(60)
}

#[cfg(test)]
#[path = "grid_drag_tests.rs"]
mod tests;
