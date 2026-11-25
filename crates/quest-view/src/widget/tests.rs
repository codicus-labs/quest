use chrono::{TimeDelta, TimeZone};
use quest_domain::{FieldStore, HlcClock, NodeId, Position, plan};

use super::*;

#[test]
fn a_timer_only_shows_outside_the_main_window_and_if_enabled() {
    for mode in [Mode::Card, Mode::Pill] {
        assert!(visible(mode, false, true));
        assert!(!visible(mode, true, true));
        assert!(!visible(mode, false, false));
    }
    assert!(!visible(Mode::Hidden, false, true));
}

#[test]
fn position_fits_a_whole_widget_on_one_display_in_physical_pixels() {
    let laptop = Screen { origin: (0, 0), size: (2000, 1200), scale: 2.0 };
    let left = Screen { origin: (-1920, -200), size: (1920, 1080), scale: 1.0 };
    let widget = (200.0, 100.0);
    assert_eq!(clamp_position((-300, 100), widget, &[laptop, left]), (-300, 100));
    assert_eq!(clamp_position((1950, 1180), widget, &[laptop, left]), (1600, 1000));
    assert_eq!(clamp_position((-300, -725), widget, &[laptop, left]), (-300, -200));
    assert_eq!(clamp_position((-300, 100), widget, &[laptop]), (0, 100));
}

#[test]
fn pause_ends_an_entry_and_only_resumes_its_original_open_task() {
    let task = EntityId::new();
    let entry = EntityId::new();
    let resumed = EntityId::new();
    let start = Utc.with_ymd_and_hms(2026, 11, 25, 10, 0, 0).unwrap();
    let at = start + TimeDelta::seconds(37);
    let mut store = FieldStore::default();
    let mut clock = HlcClock::new(NodeId(1));
    let mut run = |command| {
        let snap = Snapshot::from_store(&store, EntityId::DEFAULT_WORKSPACE);
        for write in plan(&command, &snap).unwrap() {
            store.apply(&write.stamp(clock.now(1)));
        }
        Snapshot::from_store(&store, EntityId::DEFAULT_WORKSPACE)
    };
    run(Command::CreateTask {
        id: task,
        title: "Write".into(),
        quest_id: None,
        planned_on: None,
        due_on: None,
        position: Position::Bottom,
    });
    let running = run(Command::StartFocus { entry_id: entry, task_id: task, at: start });
    let (pause, stop) = Pause::stop(running.running_focus().unwrap(), at).unwrap();
    assert_eq!(pause.elapsed_ms, 37_000);
    assert!(!pause.valid(&running));
    let stopped = run(stop);
    assert!(stopped.running_focus().is_none());
    assert_eq!(stopped.time_entries[0].ended_at, Some(at));
    let resume_at = at + TimeDelta::minutes(5);
    let new = pause.resume(&stopped, resumed, resume_at).unwrap();
    let resuming = run(new);
    let entry = &resuming.running_focus().unwrap().entry;
    assert_eq!((entry.id, entry.task_id, entry.started_at), (resumed, Some(task), resume_at));
    assert!(!pause.valid(&resuming));
    let organizing =
        run(Command::StartOrganizing { entry_id: EntityId::new(), origin_task_id: Some(task), at: resume_at });
    assert!(Pause::stop(organizing.running_focus().unwrap(), resume_at).is_none());
    let completed = run(Command::CompleteTask { id: task, at: resume_at });
    assert!(pause.resume(&completed, EntityId::new(), resume_at).is_none());
}
