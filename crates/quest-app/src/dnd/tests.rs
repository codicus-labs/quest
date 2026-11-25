use chrono::{NaiveDate, NaiveTime};
use quest_domain::{Command, EntityId, Position, TimeBlock};

use super::drop::{Placement, commands, place};
use super::*;

const SCALE: TimeScale = TimeScale { first_hour: 7, last_hour: 23, hour_height: 48.0 };

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
}

fn task_payload(id: EntityId) -> Payload {
    Payload::Task { id, block_minutes: None }
}

// --- the state machine -------------------------------------------------------------------

#[test]
fn a_press_without_travel_is_a_click() {
    let payload = task_payload(EntityId::new());
    let (state, _) = step(DragState::Idle, DragEvent::Down { payload, at: Point::new(10.0, 10.0), lift: None });
    let (state, effect) = step(state, DragEvent::Move { at: Point::new(12.0, 11.0), target: None });
    assert!(matches!(state, DragState::Pressed { .. }), "2 pt is still a press");
    assert_eq!(effect, Effect::None);
    let (state, effect) = step(state, DragEvent::Up { at: Point::new(12.0, 11.0), target: None });
    assert_eq!(state, DragState::Idle);
    assert_eq!(effect, Effect::Click(payload));
}

#[test]
fn travel_past_the_threshold_drags_and_a_release_over_a_target_drops() {
    let payload = task_payload(EntityId::new());
    let target = Target::List { list: ListId::Home, before: None };
    let (state, _) = step(DragState::Idle, DragEvent::Down { payload, at: Point::new(10.0, 10.0), lift: None });
    let (state, _) = step(state, DragEvent::Move { at: Point::new(10.0, 30.0), target: Some(target) });
    assert_eq!(state.dragging(), Some(&payload));
    assert_eq!(state.target(), Some(target));
    let (state, effect) = step(state, DragEvent::Up { at: Point::new(10.0, 30.0), target: Some(target) });
    assert_eq!(state, DragState::Idle);
    assert_eq!(effect, Effect::Drop(payload, target));
}

#[test]
fn a_release_over_nothing_or_a_cancel_snaps_back() {
    let payload = task_payload(EntityId::new());
    let dragging = |target| {
        let (s, _) = step(DragState::Idle, DragEvent::Down { payload, at: Point::default(), lift: None });
        step(s, DragEvent::Move { at: Point::new(50.0, 0.0), target }).0
    };
    let target = Some(Target::List { list: ListId::Home, before: None });
    assert_eq!(
        step(dragging(None), DragEvent::Up { at: Point::new(50.0, 0.0), target: None }).1,
        Effect::SnapBack(payload)
    );
    assert_eq!(step(dragging(target), DragEvent::Cancel), (DragState::Idle, Effect::SnapBack(payload)));
    assert_eq!(step(DragState::Idle, DragEvent::Cancel), (DragState::Idle, Effect::None));
}

#[test]
fn the_lift_rect_rides_along() {
    let lift = Some(Rect::new(1.0, 2.0, 300.0, 80.0));
    let payload = task_payload(EntityId::new());
    let (s, _) = step(DragState::Idle, DragEvent::Down { payload, at: Point::default(), lift });
    let (s, _) = step(s, DragEvent::Move { at: Point::new(0.0, 40.0), target: None });
    assert!(matches!(s, DragState::Dragging { lift: l, .. } if l == lift));
}

// --- hit-testing -------------------------------------------------------------------------

fn geometry(a: EntityId, b: EntityId) -> Geometry {
    Geometry {
        lists: vec![ListGeom {
            list: ListId::Home,
            rect: Rect::new(0.0, 100.0, 400.0, 500.0),
            cards: vec![(a, Rect::new(0.0, 100.0, 400.0, 80.0)), (b, Rect::new(0.0, 190.0, 400.0, 80.0))],
            spacing: 10.0,
        }],
        grids: vec![GridGeom {
            grid: GridId::Calendar,
            viewport: Rect::new(500.0, 100.0, 700.0, 400.0),
            scale: SCALE,
            // Scrolled by 96 pt: the column's top is above the viewport.
            days: vec![(day(15), Rect::new(500.0, 4.0, 100.0, 768.0)), (day(16), Rect::new(600.0, 4.0, 100.0, 768.0))],
        }],
        room: None,
    }
}

#[test]
fn over_a_list_the_drop_goes_before_the_first_card_below_the_pointer() {
    let (a, b, dragged) = (EntityId::new(), EntityId::new(), EntityId::new());
    let geo = geometry(a, b);
    let p = task_payload(dragged);
    assert_eq!(
        hit(&geo, &p, Point::new(10.0, 120.0), None),
        Some(Target::List { list: ListId::Home, before: Some(a) })
    );
    assert_eq!(
        hit(&geo, &p, Point::new(10.0, 150.0), None),
        Some(Target::List { list: ListId::Home, before: Some(b) })
    );
    assert_eq!(hit(&geo, &p, Point::new(10.0, 400.0), None), Some(Target::List { list: ListId::Home, before: None }));
}

#[test]
fn a_card_is_never_dropped_before_itself() {
    let (a, b) = (EntityId::new(), EntityId::new());
    let geo = geometry(a, b);
    assert_eq!(
        hit(&geo, &task_payload(a), Point::new(10.0, 120.0), None),
        Some(Target::List { list: ListId::Home, before: Some(b) })
    );
}

#[test]
fn over_a_grid_the_minute_follows_the_scrolled_column() {
    let geo = geometry(EntityId::new(), EntityId::new());
    // y 148 is 144 pt below the column top: three hours after 07:00.
    let target = hit(&geo, &task_payload(EntityId::new()), Point::new(650.0, 148.0), None);
    assert_eq!(target, Some(Target::Grid { grid: GridId::Calendar, day: day(16), minute: 600.0 }));
}

#[test]
fn grid_items_ignore_lists_and_nothing_outside_the_viewport_hits() {
    let geo = geometry(EntityId::new(), EntityId::new());
    let block = Payload::Block { id: EntityId::new(), day: day(15), span: Span::new(600, 660), grab: 0 };
    assert_eq!(hit(&geo, &block, Point::new(10.0, 120.0), None), None);
    assert_eq!(
        hit(&geo, &block, Point::new(650.0, 60.0), None),
        None,
        "above the viewport, though the column is there"
    );
}

#[test]
fn a_scrolled_list_measures_cards_where_they_show() {
    let (a, b, c) = (EntityId::new(), EntityId::new(), EntityId::new());
    // The list shows from y 100 and has scrolled 150 pt: `a` is out of view above.
    let geo = Geometry {
        lists: vec![ListGeom {
            list: ListId::Home,
            rect: Rect::new(0.0, 100.0, 400.0, 300.0),
            cards: vec![
                (a, Rect::new(0.0, -50.0, 400.0, 80.0)),
                (b, Rect::new(0.0, 40.0, 400.0, 80.0)),
                (c, Rect::new(0.0, 130.0, 400.0, 80.0)),
            ],
            spacing: 10.0,
        }],
        ..Geometry::default()
    };
    let p = task_payload(EntityId::new());
    assert_eq!(
        hit(&geo, &p, Point::new(10.0, 110.0), None),
        Some(Target::List { list: ListId::Home, before: Some(c) })
    );
    assert_eq!(hit(&geo, &p, Point::new(10.0, 90.0), None), None, "above the list's viewport");
}

/// Home's list as drawn while `dragged` (from this list) is dragged: cards 80 pt tall, 10
/// apart from y 100, with the dragged card moved into its room before `room_before`.
fn list_with_room(order: &[EntityId], dragged: EntityId, room_before: Option<Option<EntityId>>) -> Geometry {
    let mut y = 100.0;
    let mut cards = Vec::new();
    for slot in slots(order, Some(dragged), room_before) {
        let id = match slot {
            Slot::Card(id) => id,
            Slot::Room => dragged,
        };
        cards.push((id, Rect::new(0.0, y, 400.0, 80.0)));
        y += 90.0;
    }
    Geometry {
        lists: vec![ListGeom { list: ListId::Home, rect: Rect::new(0.0, 100.0, 400.0, 600.0), cards, spacing: 10.0 }],
        room: room_before.map(|before| Room { list: ListId::Home, before }),
        ..Geometry::default()
    }
}

/// The lifted card with its top at `y`.
fn lifted(y: f32) -> Option<Rect> {
    Some(Rect::new(0.0, y, 400.0, 80.0))
}

fn before_at(geo: &Geometry, dragged: EntityId, y: f32) -> Option<Option<EntityId>> {
    match hit(geo, &task_payload(dragged), Point::new(10.0, y + 20.0), lifted(y))? {
        Target::List { before, .. } => Some(before),
        Target::Grid { .. } => None,
    }
}

#[test]
fn the_open_room_does_not_move_the_target() {
    let order: Vec<EntityId> = (0..4).map(|_| EntityId::new()).collect();
    let dragged = order[1];
    let rooms = [None, Some(Some(order[0])), Some(Some(order[3])), Some(None)];
    let rest = list_with_room(&order, dragged, None);
    for room in rooms {
        let open = list_with_room(&order, dragged, room);
        for y in (80..420).step_by(5) {
            let y = y as f32;
            assert_eq!(before_at(&open, dragged, y), before_at(&rest, dragged, y), "y {y}, room before {room:?}");
        }
    }
}

#[test]
fn the_room_follows_the_lifted_card_and_a_small_drag_keeps_it_in_place() {
    let order: Vec<EntityId> = (0..3).map(|_| EntityId::new()).collect();
    let (a, b, c) = (order[0], order[1], order[2]);
    let rest = list_with_room(&order, a, None);
    assert_eq!(before_at(&rest, a, 100.0), Some(Some(b)), "where it was");
    assert_eq!(before_at(&rest, a, 135.0), Some(Some(b)), "less than half a place down");
    assert_eq!(before_at(&rest, a, 150.0), Some(Some(c)), "more than half a place down");
    assert_eq!(before_at(&rest, a, 280.0), Some(None), "at the end");
}

/// Home's list with `cards` 80 pt tall, 10 apart from y 100, and a ghost's room before the
/// card at `ghost` (if any), as when a card from another list is dragged over it.
fn foreign_list(cards: &[EntityId], ghost: Option<usize>) -> Geometry {
    let top = |i: usize| 100.0 + 90.0 * i as f32 + if ghost.is_some_and(|g| i >= g) { 90.0 } else { 0.0 };
    let cards = cards.iter().enumerate().map(|(i, id)| (*id, Rect::new(0.0, top(i), 400.0, 80.0))).collect();
    Geometry {
        lists: vec![ListGeom { list: ListId::Home, rect: Rect::new(0.0, 100.0, 400.0, 600.0), cards, spacing: 10.0 }],
        room: ghost.map(|_| Room { list: ListId::Home, before: None }),
        ..Geometry::default()
    }
}

#[test]
fn a_ghost_from_another_list_does_not_move_the_target() {
    let cards: Vec<EntityId> = (0..3).map(|_| EntityId::new()).collect();
    let other = EntityId::new();
    let rest = foreign_list(&cards, None);
    assert_eq!(before_at(&rest, other, 150.0), Some(Some(cards[1])));
    for ghost in 0..cards.len() {
        let mut open = foreign_list(&cards, Some(ghost));
        open.room = Some(Room { list: ListId::Home, before: Some(cards[ghost]) });
        for y in (80..400).step_by(5) {
            let y = y as f32;
            assert_eq!(before_at(&open, other, y), before_at(&rest, other, y), "y {y}, ghost before card {ghost}");
        }
    }
}

#[test]
fn slots_move_the_dragged_card_into_its_room() {
    let (a, b, c) = (EntityId::new(), EntityId::new(), EntityId::new());
    let order = [a, b, c];
    let cards = |ids: &[EntityId]| ids.iter().map(|id| Slot::Card(*id)).collect::<Vec<_>>();
    assert_eq!(slots(&order, Some(a), None), cards(&order), "no room: everything in place");
    assert_eq!(slots(&order, Some(a), Some(Some(c))), vec![Slot::Card(b), Slot::Room, Slot::Card(c)]);
    assert_eq!(slots(&order, Some(c), Some(None)), vec![Slot::Card(a), Slot::Card(b), Slot::Room]);
    let other = EntityId::new();
    assert_eq!(
        slots(&order, Some(other), Some(Some(a))),
        vec![Slot::Room, Slot::Card(a), Slot::Card(b), Slot::Card(c)]
    );
}

#[test]
fn a_drop_back_into_its_own_place_changes_nothing() {
    let (a, b, c) = (EntityId::new(), EntityId::new(), EntityId::new());
    let order = [a, b, c];
    assert!(!changes_order(&order, a, Some(b)));
    assert!(!changes_order(&order, c, None));
    assert!(changes_order(&order, a, Some(c)));
    assert!(changes_order(&order, c, Some(a)));
    assert!(changes_order(&order, EntityId::new(), None), "from another list");
}

// --- placement and commands --------------------------------------------------------------

fn grid_at(day: NaiveDate, minute: f32) -> Target {
    Target::Grid { grid: GridId::Calendar, day, minute }
}

#[test]
fn a_task_on_the_grid_gets_a_snapped_half_hour_or_its_own_length() {
    let id = EntityId::new();
    let placed = place(&task_payload(id), &grid_at(day(15), 9.0 * 60.0 + 37.0), &SCALE);
    assert_eq!(placed, Some(Placement::Schedule { id, day: day(15), span: Span::new(570, 600) }));
    let long = Payload::Task { id, block_minutes: Some(90) };
    let placed = place(&long, &grid_at(day(15), 22.0 * 60.0 + 50.0), &SCALE);
    assert_eq!(
        placed,
        Some(Placement::Schedule { id, day: day(15), span: Span::new(21 * 60 + 30, 23 * 60) }),
        "kept inside the grid"
    );
}

#[test]
fn a_moved_block_keeps_its_grab_offset_and_length_across_days() {
    let id = EntityId::new();
    let block = Payload::Block { id, day: day(15), span: Span::new(600, 660), grab: 20 };
    let placed = place(&block, &grid_at(day(16), 700.0), &SCALE);
    assert_eq!(placed, Some(Placement::Schedule { id, day: day(16), span: Span::new(675, 735) }));
}

#[test]
fn a_resized_block_ends_at_least_a_quarter_hour_after_it_starts() {
    let id = EntityId::new();
    let end = Payload::BlockEnd { id, day: day(15), span: Span::new(600, 660) };
    assert_eq!(
        place(&end, &grid_at(day(15), 728.0), &SCALE),
        Some(Placement::Resize { id, day: day(15), span: Span::new(600, 735) })
    );
    assert_eq!(
        place(&end, &grid_at(day(15), 500.0), &SCALE),
        Some(Placement::Resize { id, day: day(15), span: Span::new(600, 615) })
    );
}

#[test]
fn session_edges_move_one_side_and_a_running_session_only_its_start() {
    let id = EntityId::new();
    let edge = |edge, running| Payload::SessionEdge { id, day: day(15), span: Span::new(600, 660), edge, running };
    assert_eq!(
        place(&edge(Edge::Start, false), &grid_at(day(15), 571.0), &SCALE),
        Some(Placement::EntryTimes { id, day: day(15), span: Span::new(570, 660), running: false })
    );
    assert_eq!(
        place(&edge(Edge::Start, true), &grid_at(day(15), 700.0), &SCALE),
        Some(Placement::EntryTimes { id, day: day(15), span: Span::new(645, 660), running: true }),
        "the start stays a quarter hour before the end"
    );
    assert_eq!(place(&edge(Edge::End, true), &grid_at(day(15), 700.0), &SCALE), None);
    assert_eq!(
        place(&edge(Edge::End, false), &grid_at(day(15), 690.0), &SCALE),
        Some(Placement::EntryTimes { id, day: day(15), span: Span::new(600, 690), running: false })
    );
}

#[test]
fn a_new_session_spans_from_the_press_either_way_and_needs_a_quarter_hour() {
    let new = Payload::NewSession { day: day(15), anchor: 600 };
    assert_eq!(
        place(&new, &grid_at(day(15), 541.0), &SCALE),
        Some(Placement::NewEntry { day: day(15), span: Span::new(540, 600) })
    );
    assert_eq!(place(&new, &grid_at(day(15), 604.0), &SCALE), None);
}

#[test]
fn grid_items_do_not_land_in_lists() {
    let new = Payload::NewSession { day: day(15), anchor: 600 };
    assert_eq!(place(&new, &Target::List { list: ListId::Home, before: None }, &SCALE), None);
}

#[test]
fn placements_become_domain_commands() {
    let (id, other) = (EntityId::new(), EntityId::new());
    let utc = |t: chrono::NaiveDateTime| t.and_utc();
    let today = day(15);
    let run = |p: Placement| commands(&p, today, utc);

    assert_eq!(
        run(Placement::Reorder { id, list: ListId::Home, before: Some(other) }),
        vec![Command::MoveTask { id, day: Some(today), position: Position::Before(other) }]
    );
    assert_eq!(
        run(Placement::Reorder { id, list: ListId::Backlog, before: None }),
        vec![Command::MoveTask { id, day: None, position: Position::Bottom }]
    );
    let nine_thirty = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
    assert_eq!(
        run(Placement::Schedule { id, day: day(16), span: Span::new(570, 600) }),
        vec![Command::ScheduleTask { id, day: day(16), block: TimeBlock { start: nine_thirty, minutes: 30 } }]
    );
    assert_eq!(
        run(Placement::Resize { id, day: today, span: Span::new(570, 660) }),
        vec![Command::SetTaskBlock { id, block: Some(TimeBlock { start: nine_thirty, minutes: 90 }) }]
    );
    let at = |m: i32| drop::local(today, m).and_utc();
    assert_eq!(
        run(Placement::EntryTimes { id, day: today, span: Span::new(570, 600), running: true }),
        vec![Command::SetEntryTimes { id, started_at: at(570), ended_at: None }]
    );
    assert_eq!(
        run(Placement::EntryTimes { id, day: today, span: Span::new(570, 600), running: false }),
        vec![Command::SetEntryTimes { id, started_at: at(570), ended_at: Some(at(600)) }]
    );
    assert_eq!(run(Placement::NewEntry { day: today, span: Span::new(570, 600) }), Vec::new(), "the dialog logs it");
}

#[test]
fn keyboard_nudges_move_one_place() {
    let ids: Vec<EntityId> = (0..3).map(|_| EntityId::new()).collect();
    assert_eq!(nudge(&ids, ids[1], true), Some(Position::Before(ids[0])));
    assert_eq!(nudge(&ids, ids[0], true), None);
    assert_eq!(nudge(&ids, ids[0], false), Some(Position::Before(ids[2])));
    assert_eq!(nudge(&ids, ids[1], false), Some(Position::Bottom));
    assert_eq!(nudge(&ids, ids[2], false), None);
    assert_eq!(nudge(&ids, EntityId::new(), false), None);
}
