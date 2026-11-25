use std::sync::Arc;

use super::*;
use crate::calendar_types::GridDragKind;
use crate::core::ChangeListener;
use crate::task_types::GridBlockKind;
use crate::test_support::moment;
use crate::types::{AddPosition, HomeQuery, TaskSection};

struct Quiet;

impl ChangeListener for Quiet {
    fn on_change(&self) {}
}

fn open() -> (tempfile::TempDir, Arc<QuestCore>) {
    let dir = tempfile::tempdir().unwrap();
    let core =
        QuestCore::open(dir.path().display().to_string(), crate::test_support::tokens(), Arc::new(Quiet)).unwrap();
    (dir, core)
}

/// Wednesday, 25 Nov 2026, 10:00 in Berlin.
fn now() -> Moment {
    moment("2026-11-25T10:00:00+01:00")
}

fn today(core: &QuestCore) -> crate::calendar_types::CalendarDay {
    let query = CalendarQuery { first_day: Some("2026-11-25".into()), day_count: 1 };
    core.calendar(query, now()).unwrap().days.remove(0)
}

fn new_span(from: f32) -> GridDrag {
    GridDrag { kind: GridDragKind::NewSpan, block_id: None, from_day: "2026-11-25".into(), grab_minute: from }
}

/// Logs 08:00–09:00 on a new task "Draft" and returns the entry id.
fn logged(core: &QuestCore) -> String {
    let draft = core.grid_drop(new_span(480.0), "2026-11-25".into(), 540.0, now()).unwrap().unwrap();
    let draft = LogWorkDraft { new_title: "Draft".into(), ..draft };
    core.log_work(draft, now()).unwrap()
}

#[test]
fn time_dragged_on_empty_space_is_saved_only_from_log_work() {
    let (_dir, core) = open();
    let draft = core.grid_drop(new_span(480.0), "2026-11-25".into(), 545.0, now()).unwrap().unwrap();
    assert_eq!((draft.day.as_str(), draft.start_minute, draft.end_minute), ("2026-11-25", 480, 540));
    assert!(today(&core).grid.blocks.is_empty(), "nothing saved yet");
    let error = core.log_work(draft.clone(), now()).unwrap_err();
    assert_eq!(error, MobileError::InvalidForm { reason: "Choose a task or type a title for a new one.".into() });

    let entry = core.log_work(LogWorkDraft { new_title: "Draft".into(), ..draft }, now()).unwrap();
    let day = today(&core);
    assert_eq!(day.total, "1h");
    let block = &day.grid.blocks[0];
    assert_eq!(
        (block.target_id.as_str(), block.title.as_str(), block.kind),
        (entry.as_str(), "Draft", GridBlockKind::Session)
    );
    assert!(block.can_move && block.start_handle && block.end_handle);
    assert_eq!(block.a11y_label, "Session Draft, 08:00 – 09:00");
    let draft_task = core.task_choices("Draft".into()).unwrap().options.remove(0);
    assert!(draft_task.done, "a new task is created done");
}

#[test]
fn a_moved_block_can_be_put_back_with_undo() {
    let (_dir, core) = open();
    let entry = logged(&core);
    let drag = GridDrag {
        kind: GridDragKind::Move,
        block_id: Some(entry.clone()),
        from_day: "2026-11-25".into(),
        grab_minute: 490.0,
    };
    let ghost = core.grid_drag_preview(drag.clone(), "2026-11-25".into(), 615.0, now()).unwrap().unwrap();
    assert_eq!(ghost.range_label, "10:00 – 11:00");
    assert_eq!(core.grid_drop(drag, "2026-11-25".into(), 615.0, now()).unwrap(), None);
    assert_eq!(today(&core).grid.blocks[0].range_label, "10:00 – 11:00");
    let toast = core.calendar(CalendarQuery { first_day: None, day_count: 1 }, now()).unwrap().toast.unwrap();
    assert_eq!(toast.text, "Moved “Draft”");

    core.undo(now()).unwrap();
    assert_eq!(today(&core).grid.blocks[0].range_label, "08:00 – 09:00");

    let end = GridDrag {
        kind: GridDragKind::EndEdge,
        block_id: Some(entry),
        from_day: "2026-11-25".into(),
        grab_minute: 540.0,
    };
    core.grid_drop(end, "2026-11-25".into(), 570.0, now()).unwrap();
    assert_eq!(today(&core).grid.blocks[0].range_label, "08:00 – 09:30");
    assert_eq!(
        core.calendar(CalendarQuery { first_day: None, day_count: 1 }, now()).unwrap().toast.unwrap().text,
        "Changed “Draft”"
    );
}

#[test]
fn a_planned_block_moves_to_another_day_and_back() {
    let (_dir, core) = open();
    let id =
        core.add_task("Plan".into(), TaskSection::Other, AddPosition::Bottom, HomeQuery::default(), now()).unwrap();
    let span = core.grid_drop(new_span(600.0), "2026-11-25".into(), 660.0, now()).unwrap();
    assert!(span.is_some());
    // Give it a block through the store, as the desktop's planner would.
    core.dispatch(quest_domain::Command::ScheduleTask {
        id: id.parse().unwrap(),
        day: "2026-11-25".parse().unwrap(),
        block: quest_domain::TimeBlock { start: chrono::NaiveTime::from_hms_opt(13, 0, 0).unwrap(), minutes: 30 },
    })
    .unwrap();
    let drag = GridDrag {
        kind: GridDragKind::Move,
        block_id: Some(id.clone()),
        from_day: "2026-11-25".into(),
        grab_minute: 780.0,
    };
    core.grid_drop(drag, "2026-11-26".into(), 600.0, now()).unwrap();
    let detail = core.task_detail(id.clone(), now()).unwrap();
    assert_eq!(detail.date.map(|d| d.iso), Some("2026-11-26".into()));
    core.undo(now()).unwrap();
    let block = today(&core).grid.blocks.into_iter().find(|b| b.target_id == id).unwrap();
    assert_eq!(block.range_label, "13:00 – 13:30");
}

#[test]
fn the_entry_editor_saves_changes_and_deletes_with_undo() {
    let (_dir, core) = open();
    let entry = logged(&core);
    let mut draft = core.entry_draft(entry.clone(), now()).unwrap();
    draft.start_minute = 450;
    assert_eq!(core.check_entry(draft.clone(), now()).unwrap().duration, "1h 30m");
    core.save_entry(draft, now()).unwrap();
    assert_eq!(today(&core).grid.blocks[0].range_label, "07:30 – 09:00");

    core.delete_entry(entry.clone(), now()).unwrap();
    assert!(today(&core).grid.blocks.is_empty());
    assert_eq!(today(&core).total, "0h");
    core.undo(now()).unwrap();
    assert_eq!(today(&core).grid.blocks.len(), 1);
    core.delete_entry(entry.clone(), now()).unwrap();
    let gone = core.entry_draft(entry.clone(), now()).unwrap_err();
    assert_eq!(gone, MobileError::UnknownEntry { id: entry });
}

#[test]
fn undoing_a_start_drag_of_a_timer_stopped_since_keeps_its_end() {
    let (_dir, core) = open();
    let id = core.add_task("Run".into(), TaskSection::Other, AddPosition::Bottom, HomeQuery::default(), now()).unwrap();
    core.start_timer(id, moment("2026-11-25T09:00:00+01:00")).unwrap();
    let running = today(&core).grid.blocks[0].target_id.clone();
    let start = GridDrag {
        kind: GridDragKind::StartEdge,
        block_id: Some(running),
        from_day: "2026-11-25".into(),
        grab_minute: 540.0,
    };
    core.grid_drop(start, "2026-11-25".into(), 510.0, now()).unwrap();
    assert_eq!(today(&core).grid.blocks[0].range_label, "08:30 – 10:00");
    core.stop_timer(now()).unwrap();
    core.undo(now()).unwrap();
    let block = &today(&core).grid.blocks[0];
    assert_eq!((block.kind, block.range_label.as_str()), (GridBlockKind::Session, "09:00 – 10:00"));
}

#[test]
fn undoing_a_move_to_another_day_puts_the_task_back_at_the_bottom_of_its_day() {
    let (_dir, core) = open();
    let add = |title: &str| {
        core.add_task(title.into(), TaskSection::Other, AddPosition::Bottom, HomeQuery::default(), now()).unwrap()
    };
    let (first, _second) = (add("First"), add("Second"));
    core.dispatch(quest_domain::Command::ScheduleTask {
        id: first.parse().unwrap(),
        day: "2026-11-25".parse().unwrap(),
        block: quest_domain::TimeBlock { start: chrono::NaiveTime::from_hms_opt(13, 0, 0).unwrap(), minutes: 30 },
    })
    .unwrap();
    let drag =
        GridDrag { kind: GridDragKind::Move, block_id: Some(first), from_day: "2026-11-25".into(), grab_minute: 780.0 };
    core.grid_drop(drag, "2026-11-26".into(), 780.0, now()).unwrap();
    core.undo(now()).unwrap();
    let titles: Vec<_> = core.home(now(), HomeQuery::default()).unwrap().other.into_iter().map(|t| t.title).collect();
    assert_eq!(titles, ["Second", "First"], "ScheduleTask puts it at the bottom (accepted)");
}
