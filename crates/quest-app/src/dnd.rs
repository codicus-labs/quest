//! Drag and drop, pure: what is being dragged (`Payload`), where it would land (`Target`, found
//! by `hit` from the pointer and the element rects the views reported), and the gesture as a
//! state machine (`step`). `drop` turns a finished drop into domain commands. The views only
//! feed events in and draw the state; nothing here touches Rune or the store.

pub mod drop;

#[cfg(test)]
mod tests;

use chrono::NaiveDate;
use quest_domain::EntityId;

use crate::timescale::{Span, TimeScale};

/// How far (in points) a press travels before it becomes a drag rather than a click.
pub const DRAG_THRESHOLD: f32 = 4.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Point {
        Point { x, y }
    }
}

/// A rect in window points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    #[cfg(test)]
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }

    pub fn center_y(&self) -> f32 {
        self.y + self.h / 2.0
    }

    pub fn offset(&self, dx: f32, dy: f32) -> Rect {
        Rect { x: self.x + dx, y: self.y + dy, ..*self }
    }
}

impl From<[f32; 4]> for Rect {
    fn from([x, y, w, h]: [f32; 4]) -> Rect {
        Rect { x, y, w, h }
    }
}

/// A list's two kinds of task, shown apart: QUEST WORK (tasks with a quest) and OTHER. A card
/// only drags among cards of its own kind; giving or taking its quest moves it across.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Quest,
    Other,
}

/// A task list that takes drops: a place for tasks, shown as a QUEST WORK and an OTHER section.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ListId {
    /// Home's lists: today.
    Home,
    /// A planner column.
    Day(NaiveDate),
    /// The planner's "Someday" column.
    Backlog,
}

impl ListId {
    /// The day a task dropped here is planned for.
    pub fn day(self, today: NaiveDate) -> Option<NaiveDate> {
        match self {
            ListId::Home => Some(today),
            ListId::Day(day) => Some(day),
            ListId::Backlog => None,
        }
    }
}

/// A time grid that takes drops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GridId {
    Home,
    Calendar,
}

/// Which edge of a tracked session is being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Start,
    End,
}

/// What a drag carries. Grid payloads remember their span and where in it the pointer grabbed
/// (`grab`, minutes from the start), so the item moves with the pointer rather than jumping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Payload {
    /// A task card (≡ handle) or a calendar picker row. `block_minutes` is its planned length.
    Task { id: EntityId, block_minutes: Option<i32> },
    /// A planned block, moved.
    Block { id: EntityId, day: NaiveDate, span: Span, grab: i32 },
    /// A planned block's bottom edge.
    BlockEnd { id: EntityId, day: NaiveDate, span: Span },
    /// A finished tracked session, moved.
    Session { id: EntityId, day: NaiveDate, span: Span, grab: i32 },
    /// A tracked session's edge; a running session only offers its start.
    SessionEdge { id: EntityId, day: NaiveDate, span: Span, edge: Edge, running: bool },
    /// A drag over empty grid space, from `anchor` (a snapped minute): a new manual session.
    NewSession { day: NaiveDate, anchor: i32 },
}

impl Payload {
    /// The task a list drag carries; lists only take tasks.
    pub fn task(&self) -> Option<EntityId> {
        match self {
            Payload::Task { id, .. } => Some(*id),
            _ => None,
        }
    }

    /// The planned block or session the payload came from, so the grid can fade it.
    pub fn source(&self) -> Option<EntityId> {
        match self {
            Payload::Task { id, .. }
            | Payload::Block { id, .. }
            | Payload::BlockEnd { id, .. }
            | Payload::Session { id, .. }
            | Payload::SessionEdge { id, .. } => Some(*id),
            Payload::NewSession { .. } => None,
        }
    }
}

/// Where a drop would land.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// In a list, before `before` (or at the end).
    List { list: ListId, before: Option<EntityId> },
    /// On a grid's day column at `minute` (unsnapped, inside the grid's hours).
    Grid { grid: GridId, day: NaiveDate, minute: f32 },
}

/// A list's drop area and its cards, top to bottom, as laid out, `spacing` apart.
#[derive(Clone, Debug, PartialEq)]
pub struct ListGeom {
    pub list: ListId,
    pub rect: Rect,
    pub cards: Vec<(EntityId, Rect)>,
    pub spacing: f32,
}

/// A grid's visible viewport and its day columns (whose rects move as the grid scrolls).
#[derive(Clone, Debug, PartialEq)]
pub struct GridGeom {
    pub grid: GridId,
    pub viewport: Rect,
    pub scale: TimeScale,
    pub days: Vec<(NaiveDate, Rect)>,
}

/// Where a list has opened room for the dragged card (`Slot::Room`): before `before`, or at
/// the end. In another list than the card's own, the room is a ghost as tall as the card.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Room {
    pub list: ListId,
    pub before: Option<EntityId>,
}

/// Everything that takes drops on screen right now, with the room a drag has opened.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geometry {
    pub lists: Vec<ListGeom>,
    pub grids: Vec<GridGeom>,
    pub room: Option<Room>,
}

/// The drop target under `at` for `payload`: tasks go into lists or onto grids; grid items
/// only onto grids. `lifted` is the dragged card where it follows the pointer, if it shows.
/// All rects and `at` are window points, as laid out after scrolling.
pub fn hit(geometry: &Geometry, payload: &Payload, at: Point, lifted: Option<Rect>) -> Option<Target> {
    if let Some(dragged) = payload.task()
        && let Some(list) = geometry.lists.iter().find(|l| l.rect.contains(at))
    {
        let room = geometry.room.filter(|r| r.list == list.list);
        let probe = lifted.unwrap_or(Rect { x: at.x, y: at.y, w: 0.0, h: 0.0 });
        return Some(Target::List { list: list.list, before: slot_at(list, dragged, room, probe) });
    }
    let grid = geometry.grids.iter().find(|g| g.viewport.contains(at))?;
    let (day, column) = grid.days.iter().find(|(_, r)| at.x >= r.x && at.x < r.x + r.w)?;
    let minute = grid.scale.minute_at(at.y - column.y);
    let minute = minute.clamp(grid.scale.first_minute() as f32, grid.scale.last_minute() as f32);
    Some(Target::Grid { grid: grid.grid, day: *day, minute })
}

/// The place in `list` whose middle is nearest the middle of `card` (the lifted card): before
/// one of the other cards, or at the end. Places are measured where the other cards rest with
/// the dragged card and the open `room` taken out, so the target holds still while the gap
/// moves and the siblings spring, and a card dragged a little stays where it was.
fn slot_at(list: &ListGeom, dragged: EntityId, room: Option<Room>, card: Rect) -> Option<EntityId> {
    let others: Vec<(EntityId, Rect)> = list.cards.iter().copied().filter(|(id, _)| *id != dragged).collect();
    // The gap's index among the others and its height: the dragged card's own element (at its
    // origin or moved into the room), or a ghost as tall as the lifted card.
    let own = list.cards.iter().find(|(id, _)| *id == dragged).map(|c| c.1);
    let gap = match (own, room) {
        (Some(own), _) => Some((others.iter().position(|c| c.1.y > own.y).unwrap_or(others.len()), own.h)),
        (None, Some(room)) => {
            let at = room.before.and_then(|b| others.iter().position(|c| c.0 == b)).unwrap_or(others.len());
            Some((at, card.h))
        }
        (None, None) => None,
    };
    let rest_top = |i: usize, rect: &Rect| match gap {
        Some((at, h)) if i >= at => rect.y - h - list.spacing,
        _ => rect.y,
    };
    let mut tops: Vec<f32> = others.iter().enumerate().map(|(i, (_, rect))| rest_top(i, rect)).collect();
    if let Some((_, last)) = others.last() {
        tops.push(rest_top(others.len() - 1, last) + last.h + list.spacing);
    }
    let middle = card.center_y();
    let distance = |top: f32| (top + card.h / 2.0 - middle).abs();
    let nearest = (0..tops.len()).min_by(|a, b| distance(tops[*a]).total_cmp(&distance(tops[*b])))?;
    others.get(nearest).map(|c| c.0)
}

/// One row of a list while a card may be dragged: a card, or the room the dragged card would
/// drop into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Card(EntityId),
    Room,
}

/// The rows of a list showing `order`. With a `room` open (before that card, or at the end)
/// the dragged card leaves its own place, so the list shows one gap, not two.
pub fn slots(order: &[EntityId], dragged: Option<EntityId>, room: Option<Option<EntityId>>) -> Vec<Slot> {
    let Some(before) = room else { return order.iter().map(|id| Slot::Card(*id)).collect() };
    let mut rows: Vec<Slot> = order.iter().filter(|id| Some(**id) != dragged).map(|id| Slot::Card(*id)).collect();
    let at = before.and_then(|b| rows.iter().position(|r| *r == Slot::Card(b))).unwrap_or(rows.len());
    rows.insert(at, Slot::Room);
    rows
}

/// Whether moving `id` before `before` (or to the end) changes `order`, the list as shown.
pub fn changes_order(order: &[EntityId], id: EntityId, before: Option<EntityId>) -> bool {
    match order.iter().position(|t| *t == id) {
        Some(index) => order.get(index + 1).copied() != before,
        None => true,
    }
}

/// The gesture. `lift` is the source card's rect at the press, for drawing it under the pointer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum DragState {
    #[default]
    Idle,
    Pressed {
        payload: Payload,
        origin: Point,
        lift: Option<Rect>,
    },
    Dragging {
        payload: Payload,
        origin: Point,
        pointer: Point,
        lift: Option<Rect>,
        target: Option<Target>,
    },
}

impl DragState {
    /// The payload of a drag past the threshold.
    pub fn dragging(&self) -> Option<&Payload> {
        match self {
            DragState::Dragging { payload, .. } => Some(payload),
            _ => None,
        }
    }

    pub fn target(&self) -> Option<Target> {
        match self {
            DragState::Dragging { target, .. } => *target,
            _ => None,
        }
    }
}

/// Pointer events, with the target the caller found under the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragEvent {
    Down {
        payload: Payload,
        at: Point,
        lift: Option<Rect>,
    },
    Move {
        at: Point,
        target: Option<Target>,
    },
    Up {
        at: Point,
        target: Option<Target>,
    },
    /// Esc, a lost window, or a pointer taken away.
    Cancel,
}

/// What a step asks the app to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    None,
    /// Released without travelling: a click on the source.
    Click(Payload),
    /// Released over a target.
    Drop(Payload, Target),
    /// Released over nothing that takes it, or cancelled: the item goes back.
    SnapBack(Payload),
}

pub fn step(state: DragState, event: DragEvent) -> (DragState, Effect) {
    match (state, event) {
        (_, DragEvent::Down { payload, at, lift }) => (DragState::Pressed { payload, origin: at, lift }, Effect::None),
        (DragState::Pressed { payload, origin, lift }, DragEvent::Move { at, target }) => {
            if (at.x - origin.x).hypot(at.y - origin.y) < DRAG_THRESHOLD {
                return (state, Effect::None);
            }
            (DragState::Dragging { payload, origin, pointer: at, lift, target }, Effect::None)
        }
        (DragState::Dragging { payload, origin, lift, .. }, DragEvent::Move { at, target }) => {
            (DragState::Dragging { payload, origin, pointer: at, lift, target }, Effect::None)
        }
        (DragState::Pressed { payload, .. }, DragEvent::Up { .. }) => (DragState::Idle, Effect::Click(payload)),
        (DragState::Dragging { payload, .. }, DragEvent::Up { target, .. }) => match target {
            Some(target) => (DragState::Idle, Effect::Drop(payload, target)),
            None => (DragState::Idle, Effect::SnapBack(payload)),
        },
        (DragState::Dragging { payload, .. }, DragEvent::Cancel) => (DragState::Idle, Effect::SnapBack(payload)),
        (_, DragEvent::Cancel) | (DragState::Idle, _) => (DragState::Idle, Effect::None),
    }
}

pub use quest_view::reorder::nudge;
