//! The shell: the sidebar's views, quests and player card, and the entry editor a timeline block
//! opens.

use chrono::{Duration, Local};
use quest_domain::{Command, EntityId, EntryKind, TimeEntry};
use quest_view::tracked;
use rune::{Key, Mods, Runtime};

use super::dnd_tests::{rect, settle, task};
use super::log_work_tests::{fill, text_of};
use super::tests::{TempDir, click, demo_app, press, type_text};
use crate::clock;
use crate::state::{Overlay, QuestsTab, State, View};

fn app() -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = demo_app();
    settle(&mut rt);
    (rt, dir)
}

fn entry(rt: &Runtime<State>, id: EntityId) -> Option<&TimeEntry> {
    rt.state.snap.time_entries.iter().find(|e| e.id == id)
}

/// Today's first session on the timeline: done, booked on a task.
fn first_block(rt: &Runtime<State>) -> EntityId {
    let s = &rt.state;
    let day = tracked::day_timeline(&s.snap, s.today(), &Local, s.now);
    let block = day.blocks.first().expect("the demo tracked today");
    assert!(!block.running);
    block.entry
}

#[test]
fn the_sidebar_goes_to_views_quests_and_completed_quests() {
    let (mut rt, _dir) = app();
    for (key, view) in [("nav-inbox", View::Inbox), ("nav-plan", View::Plan), ("nav-today", View::Today)] {
        click(&mut rt, key);
        assert_eq!(rt.state.view, view, "{key}");
    }
    let quest = rt.state.snap.active_quests().next().map(|q| q.id).expect("a demo quest");
    click(&mut rt, &format!("nav-quest-{quest}"));
    assert_eq!(rt.state.view, View::Quest(quest));
    click(&mut rt, "nav-completed");
    assert_eq!((rt.state.view, rt.state.quests_tab), (View::Quests, QuestsTab::Completed));
    click(&mut rt, "nav-new-quest");
    assert_eq!(rt.state.overlay, Overlay::Quest);
}

#[test]
fn the_player_stops_resumes_and_opens_focus() {
    let (mut rt, _dir) = app();
    let design = task(&rt, "Design the next chapter").id;
    assert_eq!(text_of(&rt, "player-title"), "Design the next chapter");
    click(&mut rt, "player-focus");
    assert_eq!(rt.state.view, View::Focus);
    press(&mut rt, Key::Escape, Mods::default());
    click(&mut rt, "player-stop");
    assert!(rt.state.snap.running_entry().is_none());
    click(&mut rt, "player-resume");
    assert_eq!(rt.state.snap.running_entry().and_then(|e| e.task_id), Some(design), "Resume picks it up again");
}

#[test]
fn long_timer_keeps_the_stop_button_inside_the_player() {
    let (mut rt, _dir) = app();
    let start = rt.state.snap.running_entry().unwrap().started_at;
    rt.state.clock = clock::Clock::Fixed(start + Duration::minutes(99 * 60 + 59));
    settle(&mut rt);
    assert_eq!(text_of(&rt, "player-time"), "99h 59m");
    let [x, _, w, _] = rect(&rt, "player");
    let [stop_x, _, stop_w, _] = rect(&rt, "player-stop");
    assert!(stop_x + stop_w <= x + w - 11.0, "Stop overflows the card padding");
    click(&mut rt, "player-stop");
    assert!(rt.state.snap.running_entry().is_none());
}

#[test]
fn while_organizing_the_player_goes_back_to_the_task() {
    let (mut rt, _dir) = app();
    let design = task(&rt, "Design the next chapter").id;
    let now = rt.state.clock.now();
    rt.state.dispatch(Command::RebookAsOrganizing {
        expected_task_entry: None,
        entry_id: EntityId::new(),
        since: now - Duration::minutes(12),
        now,
    });
    settle(&mut rt);
    assert_eq!(rt.state.snap.running_entry().map(|e| e.kind), Some(EntryKind::Organizing));
    assert!(rt.find("player-focus").is_none(), "Organizing has no focus");
    click(&mut rt, "player-back-to");
    let running = rt.state.snap.running_entry().expect("a session runs");
    assert_eq!((running.kind, running.task_id), (EntryKind::Task, Some(design)));
}

#[test]
fn a_short_timeline_block_only_shows_its_title_when_it_fits() {
    let (mut rt, _dir) = app();
    let now = rt.state.now;
    let organizing = rt.state.snap.time_entries.iter().find(|e| e.kind == EntryKind::Organizing).unwrap().id;
    let long_title = EntityId::new();
    rt.state.dispatch(Command::CreateTask {
        id: long_title,
        title: "A task title far too wide for a single line in the time column".to_owned(),
        quest_id: None,
        planned_on: None,
        due_on: None,
        position: quest_domain::Position::Bottom,
    });
    let wide = EntityId::new();
    rt.state.dispatch(Command::StartFocus { entry_id: wide, task_id: long_title, at: now - Duration::minutes(390) });
    rt.state.dispatch(Command::StopFocus { at: now - Duration::minutes(365) });
    settle(&mut rt);
    assert_eq!(rt.presented(&format!("tl-short-title-{organizing}")).unwrap().1, 1.0);
    assert_eq!(rt.presented(&format!("tl-short-title-{wide}")).unwrap().1, 0.0);
}

#[test]
fn a_timeline_block_opens_its_entry_editor_which_moves_the_end_and_deletes() {
    let (mut rt, _dir) = app();
    let id = first_block(&rt);
    click(&mut rt, &format!("tl-block-{id}"));
    assert_eq!(rt.state.overlay, Overlay::EntryEditor);
    let started_at = entry(&rt, id).unwrap().started_at;

    fill(&mut rt, "entry-editor-end", "08:00");
    assert!(rt.find("entry-editor-error").is_some(), "an end before the start says so");
    fill(&mut rt, "entry-editor-end", "10:00");
    click(&mut rt, "entry-editor-save");
    assert_eq!(rt.state.overlay, Overlay::None);
    let saved = entry(&rt, id).unwrap();
    assert_eq!(saved.started_at, started_at, "the start stays");
    assert_eq!(saved.ended_at.map(|end| clock::local(end).format("%H:%M").to_string()).as_deref(), Some("10:00"));

    click(&mut rt, &format!("tl-block-{id}"));
    click(&mut rt, "entry-editor-delete");
    assert!(entry(&rt, id).is_none(), "deleted");
    assert_eq!(rt.state.overlay, Overlay::None);
    click(&mut rt, "toast-action");
    assert!(entry(&rt, id).is_some(), "Undo brings it back");
}

#[test]
fn the_entry_editor_books_a_session_as_organizing() {
    let (mut rt, _dir) = app();
    let id = first_block(&rt);
    let task_before = entry(&rt, id).unwrap().task_id;
    click(&mut rt, &format!("tl-block-{id}"));
    click(&mut rt, "entry-editor-booking-value");
    settle(&mut rt);
    // The list opens scrolled to the current task; searching puts Organizing first.
    type_text(&mut rt, "organ");
    settle(&mut rt);
    press(&mut rt, Key::Enter, Mods::default());
    click(&mut rt, "entry-editor-save");
    let saved = entry(&rt, id).unwrap();
    assert_eq!((saved.kind, saved.task_id), (EntryKind::Organizing, None));
    assert_eq!(saved.origin_task_id, task_before, "it keeps the task it came from");
}
