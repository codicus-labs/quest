use quest_domain::Snapshot;

use super::*;
use crate::test_support::{berlin, block, day, entry, quest, task};

/// Wednesday, 25 Nov 2026, 10:30:30 in Berlin.
fn now() -> Now {
    berlin("2026-11-25T10:30:30+01:00")
}

fn snapshot() -> Snapshot {
    let mut planned = task(10, "Write", Some(1), Some("2026-11-25"));
    planned.block = block("09:30", 60);
    let mut done = task(11, "Done plan", None, Some("2026-11-25"));
    done.block = block("13:00", 30);
    done.completed_at = Some(now().at);
    Snapshot {
        quests: vec![quest(1, "Book")],
        tasks: vec![planned, done],
        time_entries: vec![
            // From the evening before, over midnight.
            entry(20, Some(10), Some(1), "2026-11-24T23:00:00+01:00", Some("2026-11-25T01:00:00+01:00")),
            entry(21, None, None, "2026-11-25T09:00:00+01:00", Some("2026-11-25T10:00:00+01:00")),
            entry(22, Some(10), Some(1), "2026-11-25T10:15:00+01:00", None),
        ],
        ..Snapshot::default()
    }
}

#[test]
fn sessions_then_planned_blocks_with_minutes_labels_and_kinds() {
    let grid = day_grid(&snapshot(), day("2026-11-25"), now());
    let blocks: Vec<_> = grid
        .blocks
        .iter()
        .map(|b| (b.kind, b.start_minute, b.end_minute, b.title.as_str(), b.range_label.as_str()))
        .collect();
    assert_eq!(
        blocks,
        [
            (GridBlockKind::Session, 0, 60, "Write", "00:00 – 01:00"),
            (GridBlockKind::Session, 540, 600, "Other work", "09:00 – 10:00"),
            (GridBlockKind::Running, 615, 630, "Write", "10:15 – 10:30"),
            (GridBlockKind::Planned, 570, 630, "Write", "09:30 – 10:30"),
            (GridBlockKind::Planned, 780, 810, "Done plan", "13:00 – 13:30"),
        ]
    );
    let session = &grid.blocks[0];
    assert_eq!(session.id, format!("{}-2026-11-25", crate::test_support::id(20)), "a segment after midnight");
    assert_eq!(session.color_hex.as_deref(), Some("#5c8fd9"));
    assert_eq!(grid.blocks[1].color_hex, None);
    assert!(grid.blocks[4].done);
}

#[test]
fn overlapping_items_share_lanes_like_the_desktop() {
    let grid = day_grid(&snapshot(), day("2026-11-25"), now());
    let lanes: Vec<_> = grid.blocks.iter().map(|b| (b.lane, b.lane_count)).collect();
    // 09:00–10:00 and 09:30–10:30 overlap, and 10:15–10:30 runs beside the planned block.
    assert_eq!(lanes, [(0, 1), (0, 2), (0, 2), (1, 2), (0, 1)]);
}

#[test]
fn the_now_line_and_hours_show_on_today_only() {
    let grid = day_grid(&snapshot(), day("2026-11-25"), now());
    assert_eq!(grid.now_minute, Some(630.5));
    assert_eq!(grid.day_label, "Wednesday, November 25");
    assert_eq!(grid.hours.len(), 23);
    assert_eq!((grid.hours[0].minute, grid.hours[0].label.as_str()), (60, "01:00"));
    let yesterday = day_grid(&snapshot(), day("2026-11-24"), now());
    assert_eq!(yesterday.now_minute, None);
    let spans: Vec<_> = yesterday.blocks.iter().map(|b| (b.id.clone(), b.start_minute, b.end_minute)).collect();
    assert_eq!(spans, [(crate::test_support::id(20).to_string(), 1380, 1440)]);
}
