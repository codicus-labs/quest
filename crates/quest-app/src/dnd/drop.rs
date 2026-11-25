//! What a drop means: `place` decides where a payload lands on a target (snapped, clamped,
//! or refused), and `commands` turns that into domain commands. The grid draws the same
//! placement as its ghost, so what you see while dragging is what the drop does.

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use quest_domain::{Command, EntityId, Position, TimeBlock};

use super::{Edge, ListId, Payload, Target};
use crate::timescale::{DEFAULT_BLOCK_MINUTES, SNAP_MINUTES, Span, TimeScale, snap};

/// Where a drop puts things.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    /// A task into a list, before `before` or at the end.
    Reorder { id: EntityId, list: ListId, before: Option<EntityId> },
    /// A task planned on `day` for `span` (dropped from a list, or a block moved).
    Schedule { id: EntityId, day: NaiveDate, span: Span },
    /// A planned block made longer or shorter on its own day.
    Resize { id: EntityId, day: NaiveDate, span: Span },
    /// A tracked session's new times; a running one keeps running.
    EntryTimes { id: EntityId, day: NaiveDate, span: Span, running: bool },
    /// Time to log: the Log work dialog opens with it.
    NewEntry { day: NaiveDate, span: Span },
}

impl Placement {
    /// The block a grid draws as the drop's ghost.
    pub fn ghost(&self) -> Option<(NaiveDate, Span)> {
        match *self {
            Placement::Reorder { .. } => None,
            Placement::Schedule { day, span, .. }
            | Placement::Resize { day, span, .. }
            | Placement::EntryTimes { day, span, .. }
            | Placement::NewEntry { day, span } => Some((day, span)),
        }
    }
}

/// Where `payload` lands on `target`, or None when it can't land there (it snaps back).
pub fn place(payload: &Payload, target: &Target, scale: &TimeScale) -> Option<Placement> {
    match (*payload, *target) {
        (Payload::Task { id, .. }, Target::List { list, before }) => Some(Placement::Reorder { id, list, before }),
        (_, Target::List { .. }) => None,
        (Payload::Task { id, block_minutes }, Target::Grid { day, minute, .. }) => {
            let length = block_minutes.unwrap_or(DEFAULT_BLOCK_MINUTES).max(SNAP_MINUTES);
            let start = scale.clamp_start(snap(minute), length);
            Some(Placement::Schedule { id, day, span: Span::new(start, start + length) })
        }
        (Payload::Block { id, span, grab, .. }, Target::Grid { day, minute, .. }) => {
            let start = scale.clamp_start(snap(minute - grab as f32), span.len());
            Some(Placement::Schedule { id, day, span: span.moved_to(start) })
        }
        (Payload::BlockEnd { id, day, span }, Target::Grid { minute, .. }) => {
            let end = scale.clamp(snap(minute)).max(span.start + SNAP_MINUTES);
            Some(Placement::Resize { id, day, span: Span::new(span.start, end) })
        }
        (Payload::Session { id, span, grab, .. }, Target::Grid { day, minute, .. }) => {
            let start = scale.clamp_start(snap(minute - grab as f32), span.len());
            Some(Placement::EntryTimes { id, day, span: span.moved_to(start), running: false })
        }
        (Payload::SessionEdge { id, day, span, edge, running }, Target::Grid { minute, .. }) => {
            let minute = scale.clamp(snap(minute));
            let span = match edge {
                Edge::Start => Span::new(minute.min(span.end - SNAP_MINUTES), span.end),
                Edge::End if running => return None,
                Edge::End => Span::new(span.start, minute.max(span.start + SNAP_MINUTES)),
            };
            Some(Placement::EntryTimes { id, day, span, running })
        }
        (Payload::NewSession { day, anchor }, Target::Grid { minute, .. }) => {
            let minute = scale.clamp(snap(minute));
            let span = Span::new(anchor.min(minute), anchor.max(minute));
            (span.len() >= SNAP_MINUTES).then_some(Placement::NewEntry { day, span })
        }
    }
}

/// The commands that carry out `placement`. `to_utc` reads a local wall-clock time. Time to
/// log (`NewEntry`) has none: it is saved from the Log work dialog, or not at all.
pub fn commands(
    placement: &Placement,
    today: NaiveDate,
    to_utc: impl Fn(NaiveDateTime) -> DateTime<Utc>,
) -> Vec<Command> {
    let at = |day: NaiveDate, minute: i32| to_utc(local(day, minute));
    match *placement {
        Placement::Reorder { id, list, before } => vec![Command::MoveTask {
            id,
            day: list.day(today),
            position: before.map_or(Position::Bottom, Position::Before),
        }],
        Placement::Schedule { id, day, span } => vec![Command::ScheduleTask { id, day, block: block(span) }],
        Placement::Resize { id, span, .. } => vec![Command::SetTaskBlock { id, block: Some(block(span)) }],
        Placement::EntryTimes { id, day, span, running } => vec![Command::SetEntryTimes {
            id,
            started_at: at(day, span.start),
            ended_at: (!running).then(|| at(day, span.end)),
        }],
        Placement::NewEntry { .. } => Vec::new(),
    }
}

/// `minute` minutes after midnight on `day` (24:00 is the next midnight).
pub fn local(day: NaiveDate, minute: i32) -> NaiveDateTime {
    day.and_time(NaiveTime::MIN) + chrono::Duration::minutes(i64::from(minute))
}

fn block(span: Span) -> TimeBlock {
    let start = local(NaiveDate::MIN, span.start).time();
    TimeBlock { start, minutes: u32::try_from(span.len()).unwrap_or(0) }
}
