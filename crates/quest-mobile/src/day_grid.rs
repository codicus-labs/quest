//! A day's time grid, laid out as the desktop's `ui/timegrid` does: tracked sessions (the
//! running one growing to now), then planned task blocks, overlapping items side by side in
//! lanes (`quest_view::timescale::lanes`). The whole day shows (00:00 to 24:00). Each block says
//! which drags it takes (`grid_drag` decides where they land). Pure.

use chrono::{DateTime, NaiveDate, NaiveTime, Timelike, Utc};
use quest_domain::{Snapshot, Task, TimeBlock, TimeEntry};
use quest_view::format;
use quest_view::timescale::{Span, lanes};

use crate::home::color_hex;
use crate::ids::iso_day;
use crate::task_types::{DayGrid, GridBlock, GridBlockKind, HourMark};
use crate::time::Now;

pub(crate) const DAY_MINUTES: i32 = 24 * 60;

pub(crate) fn day_grid(snapshot: &Snapshot, day: NaiveDate, now: Now) -> DayGrid {
    let items = items(snapshot, day, now);
    let spans: Vec<Span> = items.iter().map(|item| item.span).collect();
    let blocks = items.into_iter().zip(lanes(&spans)).map(|(item, lane)| GridBlock {
        lane: lane.index as u32,
        lane_count: lane.count as u32,
        ..item.block
    });
    DayGrid {
        day: iso_day(day),
        day_label: format::long_day(day),
        hours: (1..24).map(|hour| HourMark { minute: hour * 60, label: format::hh_mm(hour * 60) }).collect(),
        blocks: blocks.collect(),
        now_minute: (day == now.today()).then(|| now_minute(now)),
    }
}

/// A block before its lane is known, with the span lanes are made from.
struct Item {
    span: Span,
    block: GridBlock,
}

fn items(snapshot: &Snapshot, day: NaiveDate, now: Now) -> Vec<Item> {
    let sessions = snapshot.time_entries.iter().filter_map(|entry| session(snapshot, entry, day, now));
    let planned = snapshot.tasks_on(day).filter_map(|task| planned(snapshot, task));
    sessions.chain(planned).collect()
}

fn session(snapshot: &Snapshot, entry: &TimeEntry, day: NaiveDate, now: Now) -> Option<Item> {
    let span = entry_span(entry, day, now)?;
    let span = visible(Span::new(span.start, span.end.max(span.start + 1)))?;
    let task = entry.task_id.and_then(|id| snapshot.task(id));
    let ends = ends_on(entry, day, now);
    let running = entry.is_running();
    let title = task.map_or_else(|| "Other work".to_owned(), |t| t.title.clone());
    let base = block_at(span);
    let block = GridBlock {
        id: segment_id(entry, day, now),
        kind: if running { GridBlockKind::Running } else { GridBlockKind::Session },
        color_hex: entry.quest_id.and_then(|id| snapshot.quest(id)).map(|q| color_hex(q.color).to_owned()),
        done: false,
        target_id: entry.id.to_string(),
        a11y_label: format!("Session {title}, {}", base.range_label),
        can_move: !running && ends.start && ends.end,
        start_handle: ends.start,
        end_handle: ends.end && !running,
        title,
        ..base
    };
    Some(Item { span, block })
}

fn planned(snapshot: &Snapshot, task: &Task) -> Option<Item> {
    let whole = block_span(task.block?);
    let span = visible(whole)?;
    let base = block_at(span);
    let block = GridBlock {
        id: task.id.to_string(),
        kind: GridBlockKind::Planned,
        title: task.title.clone(),
        color_hex: task.quest_id.and_then(|id| snapshot.quest(id)).map(|q| color_hex(q.color).to_owned()),
        done: task.is_done(),
        target_id: task.id.to_string(),
        a11y_label: format!("Planned {}, {}", task.title, base.range_label),
        can_move: true,
        start_handle: true,
        // A block past midnight shows cut off: its end is not here to drag.
        end_handle: whole.end <= DAY_MINUTES,
        ..base
    };
    Some(Item { span, block })
}

/// A block's place and times; the rest is filled in by its kind.
fn block_at(span: Span) -> GridBlock {
    GridBlock {
        id: String::new(),
        kind: GridBlockKind::Session,
        start_minute: span.start as u32,
        end_minute: span.end as u32,
        title: String::new(),
        range_label: format!("{} – {}", format::hh_mm(span.start as u32), format::hh_mm(span.end as u32)),
        color_hex: None,
        done: false,
        lane: 0,
        lane_count: 1,
        target_id: String::new(),
        a11y_label: String::new(),
        can_move: false,
        start_handle: false,
        end_handle: false,
    }
}

/// Which ends of a session lie on `day` (desktop `timegrid::ends_on`): a segment cut at midnight
/// has no handle there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ends {
    pub start: bool,
    pub end: bool,
}

pub(crate) fn ends_on(entry: &TimeEntry, day: NaiveDate, now: Now) -> Ends {
    let next = day.succ_opt().unwrap_or(day);
    Ends {
        start: entry.started_at >= now.midnight(day),
        end: entry.ended_at.is_none_or(|end| end <= now.midnight(next)),
    }
}

/// A session's block id on `day`: later segments past midnight add their day, so ids stay unique.
fn segment_id(entry: &TimeEntry, day: NaiveDate, now: Now) -> String {
    if ends_on(entry, day, now).start { entry.id.to_string() } else { format!("{}-{day}", entry.id) }
}

/// A session's span on `day` in minutes after local midnight (a running one ends now), if it
/// touches that day (desktop `timegrid::entry_span`).
pub(crate) fn entry_span(entry: &TimeEntry, day: NaiveDate, now: Now) -> Option<Span> {
    // Wall-clock minutes, as blocks and drops use them (on DST days elapsed minutes would differ).
    let midnight = day.and_time(NaiveTime::MIN);
    let minutes = |at: DateTime<Utc>| (now.local(at) - midnight).num_minutes().clamp(0, i64::from(DAY_MINUTES)) as i32;
    let span = Span::new(minutes(entry.started_at), minutes(entry.ended_at.unwrap_or(now.at)));
    let starts_on_day = entry.started_at.with_timezone(&now.tz).date_naive() == day;
    (span.end > span.start || (entry.is_running() && starts_on_day)).then_some(span)
}

pub(crate) fn block_span(block: TimeBlock) -> Span {
    let start = (block.start.hour() * 60 + block.start.minute()) as i32;
    Span::new(start, start + block.minutes as i32)
}

/// The part of `span` within the day.
fn visible(span: Span) -> Option<Span> {
    let span = Span::new(span.start.max(0), span.end.min(DAY_MINUTES));
    (span.end > span.start).then_some(span)
}

fn now_minute(now: Now) -> f32 {
    let local = now.at.with_timezone(&now.tz);
    (local.hour() * 60 + local.minute()) as f32 + local.second() as f32 / 60.0
}

#[cfg(test)]
#[path = "day_grid_tests.rs"]
mod tests;
