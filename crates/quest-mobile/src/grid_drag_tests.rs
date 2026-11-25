use quest_domain::Snapshot;

use super::*;
use crate::test_support::{at, berlin, block, day, entry, id, quest, task};

/// Wednesday, 25 Nov 2026, 10:30 in Berlin.
fn now() -> Now {
    berlin("2026-11-25T10:30:00+01:00")
}

fn snapshot() -> Snapshot {
    let mut planned = task(10, "Write", Some(1), Some("2026-11-25"));
    planned.block = block("13:00", 60);
    Snapshot {
        quests: vec![quest(1, "Book")],
        tasks: vec![planned],
        time_entries: vec![
            entry(20, Some(10), Some(1), "2026-11-25T08:00:00+01:00", Some("2026-11-25T09:00:00+01:00")),
            entry(21, None, None, "2026-11-24T23:00:00+01:00", Some("2026-11-25T01:00:00+01:00")),
            entry(22, Some(10), Some(1), "2026-11-25T10:00:00+01:00", None),
        ],
        ..Snapshot::default()
    }
}

fn drag(kind: GridDragKind, block: Option<String>, grab: f32) -> GridDrag {
    GridDrag { kind, block_id: block, from_day: "2026-11-25".into(), grab_minute: grab }
}

fn on(block: u128) -> Option<String> {
    Some(id(block).to_string())
}

fn land(drag: &GridDrag, on_day: &str, minute: f32) -> Option<Landing> {
    place(&snapshot(), drag, day(on_day), minute, now()).unwrap()
}

fn span_of(landing: Option<Landing>) -> Option<(String, i32, i32)> {
    landing.map(|l| (iso_day(l.day()), l.span().start, l.span().end))
}

#[test]
fn new_spans_snap_to_quarter_hours_either_way_and_need_a_quarter_hour() {
    let new = drag(GridDragKind::NewSpan, None, 14.0 * 60.0 + 7.0);
    assert_eq!(span_of(land(&new, "2026-11-25", 15.0 * 60.0 + 22.0)), Some(("2026-11-25".into(), 840, 915)));
    assert_eq!(span_of(land(&new, "2026-11-25", 13.0 * 60.0 + 10.0)), Some(("2026-11-25".into(), 795, 840)));
    assert_eq!(land(&new, "2026-11-25", 14.0 * 60.0 + 4.0), None, "shorter than a quarter hour");
    // The finger over another day still logs on the day it started (desktop).
    assert_eq!(span_of(land(&new, "2026-11-26", 15.0 * 60.0)).unwrap().0, "2026-11-25");
    // Past the grid's ends it clamps to the day.
    assert_eq!(span_of(land(&new, "2026-11-25", 2000.0)), Some(("2026-11-25".into(), 840, 1440)));
}

#[test]
fn a_moved_session_keeps_its_length_and_the_grab_offset_within_the_day() {
    let session = drag(GridDragKind::Move, on(20), 8.0 * 60.0 + 20.0);
    // Grabbed 20 min into it, dropped at 11:23 → starts 11:00 (snapped).
    let landing = land(&session, "2026-11-25", 11.0 * 60.0 + 23.0).unwrap();
    assert_eq!(
        landing,
        Landing::Entry { id: id(20), day: day("2026-11-25"), span: Span::new(660, 720), running: false }
    );
    // Onto another day, and clamped at the day's end.
    assert_eq!(span_of(land(&session, "2026-11-26", 1430.0)), Some(("2026-11-26".into(), 1380, 1440)));
    assert_eq!(span_of(land(&session, "2026-11-25", -50.0)), Some(("2026-11-25".into(), 0, 60)));
}

#[test]
fn handles_change_one_end_by_at_least_a_quarter_hour() {
    let start = drag(GridDragKind::StartEdge, on(20), 480.0);
    assert_eq!(span_of(land(&start, "2026-11-25", 7.0 * 60.0 + 40.0)), Some(("2026-11-25".into(), 465, 540)));
    assert_eq!(span_of(land(&start, "2026-11-25", 600.0)), Some(("2026-11-25".into(), 525, 540)), "stops 15 min short");
    let end = drag(GridDragKind::EndEdge, on(20), 540.0);
    assert_eq!(span_of(land(&end, "2026-11-25", 9.0 * 60.0 + 53.0)), Some(("2026-11-25".into(), 480, 600)));
    assert_eq!(span_of(land(&end, "2026-11-25", 100.0)), Some(("2026-11-25".into(), 480, 495)));
    // A handle ignores the day under the finger.
    assert_eq!(span_of(land(&end, "2026-11-27", 600.0)).unwrap().0, "2026-11-25");
}

#[test]
fn planned_blocks_move_to_other_days_and_take_both_handles() {
    let moved = land(&drag(GridDragKind::Move, on(10), 13.0 * 60.0 + 30.0), "2026-11-27", 9.0 * 60.0 + 30.0);
    assert_eq!(moved, Some(Landing::Block { id: id(10), day: day("2026-11-27"), span: Span::new(540, 600) }));
    let top = land(&drag(GridDragKind::StartEdge, on(10), 780.0), "2026-11-25", 12.0 * 60.0 + 10.0);
    assert_eq!(span_of(top), Some(("2026-11-25".into(), 735, 840)));
}

#[test]
fn the_running_session_and_midnight_segments_only_take_what_the_grid_offers() {
    let running = id(22).to_string();
    assert_eq!(land(&drag(GridDragKind::Move, Some(running.clone()), 610.0), "2026-11-25", 700.0), None);
    assert_eq!(land(&drag(GridDragKind::EndEdge, Some(running.clone()), 630.0), "2026-11-25", 700.0), None);
    let start = land(&drag(GridDragKind::StartEdge, Some(running), 600.0), "2026-11-25", 9.0 * 60.0 + 44.0).unwrap();
    assert_eq!(start, Landing::Entry { id: id(22), day: day("2026-11-25"), span: Span::new(585, 630), running: true });
    // The part after midnight of last night's session: no start, so no move and no top handle.
    let segment = Some(format!("{}-2026-11-25", id(21)));
    assert_eq!(land(&drag(GridDragKind::Move, segment.clone(), 30.0), "2026-11-25", 120.0), None);
    assert_eq!(land(&drag(GridDragKind::StartEdge, segment.clone(), 0.0), "2026-11-25", 30.0), None);
    let end = land(&drag(GridDragKind::EndEdge, segment, 60.0), "2026-11-25", 90.0).unwrap();
    assert_eq!(end, Landing::Entry { id: id(21), day: day("2026-11-25"), span: Span::new(0, 90), running: false });
    // A block that is gone snaps back.
    assert_eq!(land(&drag(GridDragKind::Move, on(99), 0.0), "2026-11-25", 60.0), None);
}

#[test]
fn drops_send_new_local_times_with_what_to_put_back() {
    let snapshot = snapshot();
    let session = Landing::Entry { id: id(20), day: day("2026-11-26"), span: Span::new(660, 720), running: false };
    let (command, before) = command(&snapshot, session, now()).unwrap();
    assert_eq!(
        command,
        Command::SetEntryTimes {
            id: id(20),
            started_at: at("2026-11-26T11:00:00+01:00"),
            ended_at: Some(at("2026-11-26T12:00:00+01:00")),
        }
    );
    assert_eq!(
        before,
        PutBack::Entry {
            id: id(20),
            started_at: at("2026-11-25T08:00:00+01:00"),
            ended_at: Some(at("2026-11-25T09:00:00+01:00"))
        }
    );
    let running = Landing::Entry { id: id(22), day: day("2026-11-25"), span: Span::new(585, 630), running: true };
    let (command, _) = super::command(&snapshot, running, now()).unwrap();
    assert_eq!(
        command,
        Command::SetEntryTimes { id: id(22), started_at: at("2026-11-25T09:45:00+01:00"), ended_at: None },
        "a running session keeps running"
    );
    let planned = Landing::Block { id: id(10), day: day("2026-11-25"), span: Span::new(1380, 1440) };
    let (command, before) = super::command(&snapshot, planned, now()).unwrap();
    assert_eq!(
        command,
        Command::ScheduleTask { id: id(10), day: day("2026-11-25"), block: block("23:00", 60).unwrap() }
    );
    assert_eq!(before, PutBack::Block { id: id(10), day: day("2026-11-25"), block: block("13:00", 60).unwrap() });
}

#[test]
fn a_drop_where_it_was_changes_nothing() {
    let snapshot = snapshot();
    let session = Landing::Entry { id: id(20), day: day("2026-11-25"), span: Span::new(480, 540), running: false };
    assert_eq!(command(&snapshot, session, now()), None);
    let planned = Landing::Block { id: id(10), day: day("2026-11-25"), span: Span::new(780, 840) };
    assert_eq!(command(&snapshot, planned, now()), None);
    let new = Landing::NewSpan { day: day("2026-11-25"), span: Span::new(0, 15) };
    assert_eq!(command(&snapshot, new, now()), None);
}

#[test]
fn placements_carry_the_ghost_label() {
    let landing = Landing::NewSpan { day: day("2026-11-25"), span: Span::new(555, 600) };
    assert_eq!(
        landing.placement(),
        GridPlacement {
            day: "2026-11-25".into(),
            start_minute: 555,
            end_minute: 600,
            range_label: "09:15 – 10:00".into()
        }
    );
}

#[test]
fn a_session_moved_into_a_dst_gap_starts_an_hour_later_like_a_wall_clock() {
    // 29 Mar 2026: Berlin skips 02:00–03:00; 02:15 reads as 03:15 summer time.
    let spring = berlin("2026-03-29T12:00:00+02:00");
    let snapshot = Snapshot {
        time_entries: vec![entry(30, None, None, "2026-03-29T08:00:00+02:00", Some("2026-03-29T09:00:00+02:00"))],
        ..Snapshot::default()
    };
    let landing = Landing::Entry { id: id(30), day: day("2026-03-29"), span: Span::new(135, 240), running: false };
    let (command, _) = command(&snapshot, landing, spring).unwrap();
    assert_eq!(
        command,
        Command::SetEntryTimes {
            id: id(30),
            started_at: at("2026-03-29T03:15:00+02:00"),
            ended_at: Some(at("2026-03-29T04:00:00+02:00")),
        }
    );
}

#[test]
fn a_handle_drag_keeps_the_seconds_of_the_edge_it_left() {
    let mut snapshot = snapshot();
    snapshot.time_entries[0].started_at = at("2026-11-25T08:00:30+01:00");
    let landing = Landing::Entry { id: id(20), day: day("2026-11-25"), span: Span::new(480, 600), running: false };
    let (command, _) = command(&snapshot, landing, now()).unwrap();
    assert_eq!(
        command,
        Command::SetEntryTimes {
            id: id(20),
            started_at: at("2026-11-25T08:00:30+01:00"),
            ended_at: Some(at("2026-11-25T10:00:00+01:00")),
        }
    );
}

#[test]
fn a_planned_block_past_midnight_keeps_its_length_and_has_no_end_handle() {
    let mut snapshot = snapshot();
    snapshot.tasks[0].block = block("23:00", 120);
    let grid = day_grid::day_grid(&snapshot, day("2026-11-25"), now());
    let shown = grid.blocks.iter().find(|b| b.target_id == id(10).to_string()).unwrap();
    assert_eq!((shown.end_minute, shown.end_handle), (1440, false));
    let moved = place(&snapshot, &drag(GridDragKind::Move, on(10), 1390.0), day("2026-11-25"), 610.0, now()).unwrap();
    assert_eq!(moved, Some(Landing::Block { id: id(10), day: day("2026-11-25"), span: Span::new(600, 720) }));
}

#[test]
fn on_a_dst_day_sessions_read_and_write_wall_clock_minutes() {
    // 29 Mar 2026: Berlin skips 02:00–03:00. 10:00–11:00 summer time stays 10:00–11:00.
    let spring = berlin("2026-03-29T12:00:00+02:00");
    let snapshot = Snapshot {
        time_entries: vec![entry(30, None, None, "2026-03-29T10:00:00+02:00", Some("2026-03-29T11:00:00+02:00"))],
        ..Snapshot::default()
    };
    let grid = day_grid::day_grid(&snapshot, day("2026-03-29"), spring);
    assert_eq!(grid.blocks[0].range_label, "10:00 – 11:00");
    let end =
        GridDrag { kind: GridDragKind::EndEdge, block_id: on(30), from_day: "2026-03-29".into(), grab_minute: 660.0 };
    let landing = place(&snapshot, &end, day("2026-03-29"), 690.0, spring).unwrap().unwrap();
    let (command, _) = command(&snapshot, landing, spring).unwrap();
    assert_eq!(
        command,
        Command::SetEntryTimes {
            id: id(30),
            started_at: at("2026-03-29T10:00:00+02:00"),
            ended_at: Some(at("2026-03-29T11:30:00+02:00")),
        }
    );
}
