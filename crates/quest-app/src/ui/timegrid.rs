//! A day or week time grid. Tracked sessions come first: filled and hatched in their quest's
//! colour, the running one dashed and growing to the now-line. Planned task blocks are a
//! subtle outline. Items that overlap sit side by side in lanes. Everything on it drags:
//! blocks move and stretch, sessions move and change either edge, and a drag over empty
//! space logs a new session. A click on a session opens its editor.

use chrono::{DateTime, NaiveDate, Timelike, Utc};
use quest_domain::{Command, EntityId, Task, TimeBlock, TimeEntry};
use rune::{Color, Cursor, Drag, El, KeyEvent, LineCap, Shape, Tx, canvas, col, el, row, text};

use crate::clock;
use crate::dnd::{Edge, GridId, Payload};
use crate::format;
use crate::keys::{self, GridKey};
use crate::state::State;
use crate::theme::{Pal, ty};
use crate::timescale::{self, Lane, SNAP_MINUTES, Span, TimeScale};
use crate::ui::drag::{self, Zone};
use crate::ui::parts::{accent_of, quest_of};

pub const HOME_SCALE: TimeScale = TimeScale { first_hour: 7, last_hour: 23, hour_height: 52.0 };
pub const CALENDAR_SCALE: TimeScale = TimeScale { first_hour: 7, last_hour: 23, hour_height: 48.0 };

pub struct GridSpec<'a> {
    pub key: &'a str,
    pub grid: GridId,
    pub days: Vec<NaiveDate>,
    pub scale: TimeScale,
}

const LABEL_W: f32 = 64.0;
/// How tall the grab strip at a block's edge is.
const EDGE: f32 = 6.0;

pub fn grid(s: &State, spec: &GridSpec, p: &Pal) -> El {
    let scale = spec.scale;
    let height = scale.height();
    let mut labels = el().w(LABEL_W).h(height);
    for hour in scale.first_hour + 1..scale.last_hour {
        let y = scale.y(hour as i32 * 60) - 8.0;
        labels = labels.child(
            row()
                .absolute(0.0, y)
                .w(LABEL_W - 12.0)
                .justify_end()
                .child(text(format::hh_mm(hour * 60)).font(ty::SMALL).color(p.faint)),
        );
    }
    let mut columns = row().grow(1.0).h(height);
    for (i, day) in spec.days.iter().enumerate() {
        columns = columns.child(day_column(s, spec, *day, i, p));
    }
    row().key(spec.key.to_owned()).w_full().h(height).fill(p.grid).child(labels.fill(p.grid)).child(columns)
}

/// Something drawn in a day column.
enum Item<'a> {
    Session { entry: &'a TimeEntry, span: Span },
    Planned { task: &'a Task, span: Span },
}

impl Item<'_> {
    /// What the item's keys end in: an entry drawn on more than one day keys its later
    /// segments by their day, so every key stays unique.
    fn key_id(&self, day: NaiveDate) -> String {
        match self {
            Item::Session { entry, .. } => segment_id(entry, day),
            Item::Planned { task, .. } => task.id.to_string(),
        }
    }

    fn span(&self) -> Span {
        match self {
            Item::Session { span, .. } | Item::Planned { span, .. } => *span,
        }
    }
}

/// A session's span on `day` in minutes after midnight (a running one ends now), if it
/// touches that day. A session over midnight has a span on each day it touches.
pub fn entry_span(entry: &TimeEntry, day: NaiveDate, now: DateTime<Utc>) -> Option<Span> {
    let midnight = clock::midnight(day);
    let minutes = |at: DateTime<Utc>| (at - midnight).num_minutes().clamp(0, 24 * 60) as i32;
    let span = Span::new(minutes(entry.started_at), minutes(entry.ended_at.unwrap_or(now)));
    (span.end > span.start || (entry.is_running() && clock::local_day(entry.started_at) == day)).then_some(span)
}

/// Which ends of a session lie on `day`: a segment cut at midnight has no handle there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ends {
    pub start: bool,
    pub end: bool,
}

pub fn ends_on(entry: &TimeEntry, day: NaiveDate) -> Ends {
    let next = day.succ_opt().unwrap_or(day);
    Ends {
        start: entry.started_at >= clock::midnight(day),
        end: entry.ended_at.is_none_or(|end| end <= clock::midnight(next)),
    }
}

fn segment_id(entry: &TimeEntry, day: NaiveDate) -> String {
    if ends_on(entry, day).start { entry.id.to_string() } else { format!("{}-{day}", entry.id) }
}

fn block_span(block: TimeBlock) -> Span {
    let start = (block.start.hour() * 60 + block.start.minute()) as i32;
    Span::new(start, start + block.minutes as i32)
}

fn items<'a>(s: &'a State, day: NaiveDate, scale: &TimeScale) -> Vec<Item<'a>> {
    let sessions = s.snap.time_entries.iter().filter_map(|entry| {
        let span = entry_span(entry, day, s.now)?;
        let span = Span::new(span.start, span.end.max(span.start + 1)).visible(scale)?;
        Some(Item::Session { entry, span })
    });
    let planned = s.snap.tasks_on(day).filter_map(|task| {
        let span = block_span(task.block?).visible(scale)?;
        Some(Item::Planned { task, span })
    });
    sessions.chain(planned).collect()
}

fn day_column(s: &State, spec: &GridSpec, day: NaiveDate, index: usize, p: &Pal) -> El {
    let scale = spec.scale;
    let mut column = el()
        .key(format!("{}-day-{index}", spec.key))
        .grow(1.0)
        .h_full()
        .cursor(Cursor::Crosshair)
        .border_sides([0.0, 0.0, 0.0, 1.0], p.grid_line);
    for hour in scale.first_hour + 1..scale.last_hour {
        column = column
            .child(el().pin(Some(0.0), Some(scale.y(hour as i32 * 60)), Some(0.0), None).h(1.0).fill(p.grid_line));
    }
    let items = items(s, day, &scale);
    let lanes = timescale::lanes(&items.iter().map(Item::span).collect::<Vec<_>>());
    for (item, lane) in items.iter().zip(lanes) {
        let el = match item {
            Item::Session { entry, span } => session_el(s, spec, day, entry, *span, p),
            Item::Planned { task, span } => planned_el(s, spec, day, task, *span, p),
        };
        column =
            column.child(in_lane(&scale, item.span(), lane, el).key(format!("{}-lane-{}", spec.key, item.key_id(day))));
    }
    if let Some((ghost_day, span)) = drag::grid_preview(s, spec.grid).and_then(|placement| placement.ghost())
        && ghost_day == day
    {
        column = column.child(ghost(spec, span, p));
    }
    if day == s.today() {
        let now = clock::local(s.now).time();
        let minutes = (now.hour() * 60 + now.minute()) as i32;
        if (scale.first_minute()..scale.last_minute()).contains(&minutes) {
            let y = scale.y(minutes);
            column = column
                .child(
                    el().key(format!("{}-now", spec.key))
                        .pin(Some(0.0), Some(y - 1.0), Some(0.0), None)
                        .h(2.0)
                        .fill(p.accent)
                        .shadow(0.0, 10.0, p.accent.alpha(0.5)),
                )
                .child(el().absolute(-5.0, y - 5.0).size(10.0, 10.0).radius(5.0).fill(p.accent));
        }
    }
    let column = drag::source(column, None, move |_: &State, d: &Drag| Payload::NewSession {
        day,
        anchor: timescale::snap(scale.minute_at(d.start_y)),
    });
    drag::zone(column, Zone::GridDay(spec.grid, index))
}

/// Places `item` in its lane: a full-width strip at the item's height, with a spacer for
/// the lanes before it.
fn in_lane(scale: &TimeScale, span: Span, lane: Lane, item: El) -> El {
    let top = scale.y(span.start) + 1.0;
    let height = (scale.y(span.end) - scale.y(span.start) - 2.0).max(18.0);
    let share = 1.0 / lane.count as f32;
    row()
        .pin(Some(3.0), Some(top), Some(3.0), None)
        .h(height)
        .child(el().w_frac(lane.index as f32 * share))
        .child(item.w_frac(share).h_full())
}

/// Diagonal stripes for a tracked session (the parent clips them to its rounded rect).
fn hatch(height: f32, color: Color) -> El {
    const WIDTH: f32 = 900.0;
    const PITCH: f32 = 7.0;
    let mut shape = Shape::new();
    let mut x = -height;
    while x < WIDTH {
        shape = shape.line([x, height], [x + height, 0.0], 1.0, LineCap::Butt, color);
        x += PITCH;
    }
    canvas(shape).absolute(0.0, 0.0).size(WIDTH, height).a11y_hidden()
}

fn range_label(span: Span) -> String {
    format!("{} – {}", format::hh_mm(span.start.max(0) as u32), format::hh_mm(span.end.max(0) as u32))
}

fn session_el(s: &State, spec: &GridSpec, day: NaiveDate, entry: &TimeEntry, span: Span, p: &Pal) -> El {
    let id = entry.id;
    let running = entry.is_running();
    let task = entry.task_id.and_then(|t| s.snap.task(t));
    let quest = entry.quest_id.and_then(|q| s.snap.quest(q));
    let color = accent_of(quest, p);
    let ends = ends_on(entry, day);
    let key = segment_id(entry, day);
    let height = (spec.scale.y(span.end) - spec.scale.y(span.start) - 2.0).max(18.0);
    let title = task.map_or_else(|| "Other work".to_owned(), |t| t.title.clone());
    let range = range_label(span);
    let mut body = col().grow(1.0).min_w(0.0).px(8.0).py(4.0).gap(2.0);
    // Tall enough for two lines: the times, then the title; otherwise just the title.
    if height >= 36.0 && !running {
        body = body.child(text(range.clone()).font(ty::SMALL).color(p.muted).single_line());
    }
    body = body.child(
        text(title.clone()).font(ty::SMALL_MEDIUM).color(if running { color } else { p.text }).single_line().truncate(),
    );
    let mut block = row()
        .key(format!("{}-entry-{key}", spec.key))
        .role("button")
        .label(format!("Session {title}, {range}"))
        .focusable(true)
        .radius(4.0)
        .clip_rounded(4.0)
        .cursor(if running { Cursor::Pointer } else { Cursor::Grab })
        .hover_group();
    block = if running {
        block.fill(color.alpha(0.10)).border(1.0, color.alpha(0.85)).border_dash(4.0, 3.0).child(body)
    } else {
        block
            .fill(color.alpha(0.20))
            .child(hatch(height + 2.0, color.alpha(0.28)))
            .child(el().w(3.0).h_full().fill(color))
            .child(body)
    };
    if drag::dragged(s) == Some(id) {
        block = block.opacity(0.3);
    }
    if ends.start {
        block = block.child(edge_handle(format!("{}-entry-start-{id}", spec.key), true, move |_: &State, _: &Drag| {
            Payload::SessionEdge { id, day, span, edge: Edge::Start, running }
        }));
    }
    if ends.end && !running {
        block = block.child(edge_handle(format!("{}-entry-end-{id}", spec.key), false, move |_: &State, _: &Drag| {
            Payload::SessionEdge { id, day, span, edge: Edge::End, running }
        }));
    }
    let scale = spec.scale;
    let block = block.on_key(move |s: &mut State, ev: &KeyEvent, tx: &mut Tx| session_key(s, id, ev, tx));
    // A running session's end is now, and a segment cut at midnight shows only part of its
    // session: neither moves as a whole, so a press just opens the editor.
    if running || !ends.start || !ends.end {
        return block.on_press(move |s: &mut State| s.open_entry_editor(id));
    }
    drag::source(block, None, move |_: &State, d: &Drag| Payload::Session {
        id,
        day,
        span,
        grab: scale.minutes(d.start_y).round() as i32,
    })
}

/// A grab strip along the top or bottom edge of a block.
fn edge_handle(key: String, top: bool, payload: impl Fn(&State, &Drag) -> Payload + 'static) -> El {
    let strip = el()
        .key(key)
        .pin(Some(0.0), if top { Some(0.0) } else { None }, Some(0.0), if top { None } else { Some(0.0) })
        .h(EDGE)
        .cursor(Cursor::ResizeRow);
    drag::source(strip, None, payload)
}

fn planned_el(s: &State, spec: &GridSpec, day: NaiveDate, task: &Task, span: Span, p: &Pal) -> El {
    let id = task.id;
    let color = accent_of(quest_of(s, task), p);
    let picked = s.calendar_pick == Some(id);
    let height = (spec.scale.y(span.end) - spec.scale.y(span.start) - 2.0).max(18.0);
    let range = range_label(span);
    let mut body = col().grow(1.0).min_w(0.0).px(8.0).py(4.0).gap(2.0).child(
        text(task.title.clone())
            .font(ty::SMALL_MEDIUM)
            .color(if task.is_done() { p.faint } else { p.muted })
            .single_line()
            .truncate(),
    );
    if height >= 36.0 {
        body = body.child(text(range.clone()).font(ty::SMALL).color(p.faint).single_line());
    }
    let mut block = row()
        .key(format!("{}-block-{id}", spec.key))
        .role("button")
        .label(format!("Planned {}, {range}", task.title))
        .focusable(true)
        .radius(4.0)
        .border(1.0, if picked { p.accent } else { color.alpha(0.55) })
        .cursor(Cursor::Grab)
        .hover(Color::WHITE.alpha(0.03))
        .child(body)
        .child(edge_handle(format!("{}-block-end-{id}", spec.key), false, move |_: &State, _: &Drag| {
            Payload::BlockEnd { id, day, span }
        }));
    if drag::dragged(s) == Some(id) {
        block = block.opacity(0.3);
    }
    let scale = spec.scale;
    let block = block.on_key(move |s: &mut State, ev: &KeyEvent, _: &mut Tx| block_key(s, id, ev));
    drag::source(block, None, move |_: &State, d: &Drag| Payload::Block {
        id,
        day,
        span,
        grab: scale.minutes(d.start_y).round() as i32,
    })
}

/// Where a drop would land, with its times.
fn ghost(spec: &GridSpec, span: Span, p: &Pal) -> El {
    let scale = spec.scale;
    let top = scale.y(span.start) + 1.0;
    let height = (scale.y(span.end) - scale.y(span.start) - 2.0).max(18.0);
    el().key(format!("{}-ghost", spec.key))
        .pin(Some(3.0), Some(top), Some(3.0), None)
        .h(height)
        .radius(4.0)
        .fill(p.accent.alpha(0.14))
        .border(1.5, p.accent)
        .border_dash(5.0, 3.0)
        .px(8.0)
        .py(4.0)
        .child(text(range_label(span)).key(format!("{}-ghost-time", spec.key)).font(ty::SMALL_MEDIUM).color(p.text))
        .inert()
}

/// ⌫ deletes, ⌥↑/⌥↓ move by a quarter hour, ⌥⇧↑/⌥⇧↓ move the end.
fn session_key(s: &mut State, id: EntityId, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let Some(key) = keys::grid_key(ev) else { return false };
    let Some(entry) = s.snap.time_entries.iter().find(|e| e.id == id).cloned() else { return false };
    let minutes = |m: i32| chrono::Duration::minutes(i64::from(m));
    let command = match key {
        GridKey::Remove => {
            tx.blur();
            drag::delete_entry(s, id);
            return true;
        }
        GridKey::Shift { minutes: m } if entry.is_running() => {
            Command::SetEntryTimes { id, started_at: entry.started_at + minutes(m), ended_at: None }
        }
        GridKey::Shift { minutes: m } => Command::SetEntryTimes {
            id,
            started_at: entry.started_at + minutes(m),
            ended_at: entry.ended_at.map(|end| end + minutes(m)),
        },
        GridKey::Resize { .. } if entry.is_running() => return true,
        GridKey::Resize { minutes: m } => Command::SetEntryTimes {
            id,
            started_at: entry.started_at,
            ended_at: entry.ended_at.map(|end| end + minutes(m)),
        },
    };
    s.dispatch(command);
    true
}

/// ⌫ unplans the block, ⌥↑/⌥↓ move it by a quarter hour, ⌥⇧↑/⌥⇧↓ change its length.
fn block_key(s: &mut State, id: EntityId, ev: &KeyEvent) -> bool {
    let Some(key) = keys::grid_key(ev) else { return false };
    let Some(block) = s.snap.task(id).and_then(|t| t.block) else { return false };
    let span = block_span(block);
    let span = match key {
        GridKey::Remove => {
            s.dispatch(Command::SetTaskBlock { id, block: None });
            return true;
        }
        GridKey::Shift { minutes } => span.moved_to((span.start + minutes).clamp(0, 24 * 60 - span.len())),
        GridKey::Resize { minutes } => Span::new(span.start, (span.end + minutes).max(span.start + SNAP_MINUTES)),
    };
    let start = crate::dnd::drop::local(chrono::NaiveDate::MIN, span.start).time();
    s.dispatch(Command::SetTaskBlock { id, block: Some(TimeBlock { start, minutes: span.len() as u32 }) });
    true
}

#[cfg(test)]
#[path = "timegrid_tests.rs"]
mod tests;
