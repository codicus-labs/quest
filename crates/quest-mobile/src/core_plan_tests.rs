use std::sync::Arc;

use super::*;
use crate::core::ChangeListener;
use crate::plan_types::PlanDay;
use crate::task_types::RepeatFrequency;
use crate::test_support::moment;
use crate::types::{HomeTask, TaskSection};

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

/// Mon 23 Nov to Sun 29 Nov.
fn week() -> PlanWindow {
    PlanWindow { first_day: "2026-11-23".into(), count: 7 }
}

fn plan_days(core: &QuestCore) -> Vec<PlanDay> {
    core.plan(week(), None, now()).unwrap().days
}

fn day<'a>(days: &'a [PlanDay], iso: &str) -> &'a PlanDay {
    days.iter().find(|d| d.day_iso == iso).unwrap()
}

fn titles(cards: &[HomeTask]) -> Vec<&str> {
    cards.iter().map(|c| c.title.as_str()).collect()
}

fn add(core: &QuestCore, on: &str, title: &str) -> String {
    core.plan_add_task(on.into(), title.into(), AddPosition::Bottom, now()).unwrap()
}

/// "Water plants" today, repeating daily: the plan previews it on every later day.
fn daily(core: &QuestCore) -> String {
    let id = add(core, "2026-11-25", "Water plants");
    let draft = core.repeat_draft(id.clone(), now()).unwrap();
    core.set_task_repeat(
        id.clone(),
        crate::task_types::RepeatDraft { frequency: RepeatFrequency::Day, ..draft },
        now(),
    )
    .unwrap();
    id
}

#[test]
fn the_window_and_edge_speed_are_exported() {
    let window = plan_window(None, 5, None, now()).unwrap();
    assert_eq!(window, PlanWindow { first_day: "2026-11-09".into(), count: 35 });
    assert_eq!(plan_window(Some("2026-11-20".into()), 5, Some(window.clone()), now()).unwrap(), window);
    assert!(matches!(plan_window(Some("soon".into()), 5, None, now()), Err(MobileError::InvalidDate { .. })));
    assert_eq!(plan_edge_speed(500.0, 0.0, 1000.0), 0.0);
    assert!(plan_edge_speed(10.0, 0.0, 1000.0) < -1000.0);
    assert!(plan_edge_speed(990.0, 0.0, 1000.0) > 1000.0);
}

#[test]
fn adds_land_on_their_day_without_a_quest() {
    let (_dir, core) = open();
    add(&core, "2026-11-27", "Second");
    core.plan_add_task("2026-11-27".into(), "First\nline".into(), AddPosition::Top, now()).unwrap();
    let days = plan_days(&core);
    assert_eq!(titles(&day(&days, "2026-11-27").other), ["First line", "Second"]);
    assert!(day(&days, "2026-11-27").quest_work.is_empty());
    let blank = core.plan_add_task("2026-11-27".into(), "  ".into(), AddPosition::Top, now());
    assert_eq!(blank, Err(MobileError::EmptyTitle));
}

#[test]
fn a_drop_moves_a_card_to_another_day_before_a_card_or_to_the_end() {
    let (_dir, core) = open();
    let a = add(&core, "2026-11-25", "A");
    let b = add(&core, "2026-11-26", "B");
    add(&core, "2026-11-26", "C");
    core.plan_move_task(a.clone(), Some("2026-11-26".into()), Some(b), now()).unwrap();
    assert_eq!(titles(&day(&plan_days(&core), "2026-11-26").other), ["A", "B", "C"]);
    core.plan_move_task(a.clone(), Some("2026-11-26".into()), None, now()).unwrap();
    assert_eq!(titles(&day(&plan_days(&core), "2026-11-26").other), ["B", "C", "A"]);
    core.plan_move_task(a.clone(), None, None, now()).unwrap();
    assert!(!titles(&day(&plan_days(&core), "2026-11-26").other).contains(&"A"), "unplanned");
    assert!(core.task_detail(a, now()).is_ok());
}

#[test]
fn accessibility_moves_step_a_place_or_a_day() {
    let (_dir, core) = open();
    add(&core, "2026-11-25", "A");
    let b = add(&core, "2026-11-25", "B");
    plan_days(&core);
    core.plan_nudge_task(b.clone(), true, now()).unwrap();
    assert_eq!(titles(&day(&plan_days(&core), "2026-11-25").other), ["B", "A"]);
    core.plan_nudge_task(b.clone(), true, now()).unwrap();
    assert_eq!(titles(&day(&plan_days(&core), "2026-11-25").other), ["B", "A"], "the first stays");
    core.plan_shift_task(b.clone(), true, now()).unwrap();
    assert_eq!(titles(&day(&plan_days(&core), "2026-11-26").other), ["B"]);
    core.plan_shift_task(b, false, now()).unwrap();
    assert_eq!(titles(&day(&plan_days(&core), "2026-11-25").other), ["A", "B"], "to the bottom");
    assert_eq!(core.move_targets(now()).unwrap()[0].day_iso.as_deref(), Some("2026-11-25"));
}

#[test]
fn the_plan_previews_repeats_and_edits_save_them_first() {
    let (_dir, core) = open();
    daily(&core);
    let days = plan_days(&core);
    let preview = day(&days, "2026-11-27").other[0].clone();
    assert_eq!(preview.title, "Water plants");
    assert!(day(&days, "2026-11-24").other.is_empty(), "not before it starts");

    core.rename_task(preview.id.clone(), "Water the ferns".into()).unwrap();
    let days = plan_days(&core);
    assert_eq!(titles(&day(&days, "2026-11-27").other), ["Water the ferns"], "saved under the same id");
    assert_eq!(day(&days, "2026-11-27").other[0].id, preview.id);
    assert_eq!(titles(&day(&days, "2026-11-28").other), ["Water plants"], "the series keeps its title");
}

#[test]
fn a_previewed_repeat_drags_and_completes_like_a_task() {
    let (_dir, core) = open();
    daily(&core);
    let days = plan_days(&core);
    let friday = day(&days, "2026-11-27").other[0].id.clone();
    let saturday = day(&days, "2026-11-28").other[0].id.clone();
    let errand = add(&core, "2026-11-28", "Errand");
    plan_days(&core);

    core.plan_move_task(errand, Some("2026-11-28".into()), Some(saturday.clone()), now()).unwrap();
    assert_eq!(titles(&day(&plan_days(&core), "2026-11-28").other), ["Errand", "Water plants"], "a preview anchors");

    core.toggle_task(friday.clone(), now()).unwrap();
    let days = plan_days(&core);
    assert!(day(&days, "2026-11-27").other.is_empty());
    let done_today = day(&days, "2026-11-25").other.iter().find(|c| c.id == friday).unwrap();
    assert!(done_today.done, "completed ahead of its day, it moves to today as on the desktop plan");
    assert_eq!(
        core.home(now(), HomeQuery::default()).unwrap().toast.map(|t| t.text),
        Some("Completed “Water plants”".into())
    );
}

#[test]
fn a_preview_outside_the_last_plan_is_unknown() {
    let (_dir, core) = open();
    daily(&core);
    let preview = day(&plan_days(&core), "2026-11-27").other[0].id.clone();
    core.plan(PlanWindow { first_day: "2026-12-07".into(), count: 7 }, None, now()).unwrap();
    assert!(matches!(core.toggle_task(preview, now()), Err(MobileError::UnknownTask { .. })));
}

#[test]
fn home_adds_still_complete_as_before() {
    let (_dir, core) = open();
    let id =
        core.add_task("Today".into(), TaskSection::Other, AddPosition::Bottom, HomeQuery::default(), now()).unwrap();
    core.toggle_task(id.clone(), now()).unwrap();
    let home = core.home(now(), HomeQuery::default()).unwrap();
    assert!(home.other.iter().any(|c| c.id == id && c.done));
}

#[test]
fn undoing_a_completion_that_moved_a_repeat_puts_it_back_on_its_day() {
    let (_dir, core) = open();
    daily(&core);
    let friday = day(&plan_days(&core), "2026-11-27").other[0].id.clone();
    core.toggle_task(friday.clone(), now()).unwrap();
    core.undo(now()).unwrap();
    let days = plan_days(&core);
    let back = day(&days, "2026-11-27").other.iter().find(|c| c.id == friday).unwrap();
    assert!(!back.done, "open again on Friday");
    assert!(day(&days, "2026-11-25").other.iter().all(|c| c.id != friday));
}

#[test]
fn previews_can_be_deleted_restored_timed_and_shifted() {
    let (_dir, core) = open();
    daily(&core);
    let days = plan_days(&core);
    let friday = day(&days, "2026-11-27").other[0].id.clone();
    let saturday = day(&days, "2026-11-28").other[0].id.clone();

    core.delete_task(friday.clone(), now()).unwrap();
    assert!(day(&plan_days(&core), "2026-11-27").other.is_empty(), "a deleted preview stays gone");
    core.undo(now()).unwrap();
    assert_eq!(day(&plan_days(&core), "2026-11-27").other[0].id, friday, "restored");

    core.start_timer(saturday.clone(), now()).unwrap();
    assert!(day(&plan_days(&core), "2026-11-28").other[0].timer_running);

    core.plan_shift_task(saturday.clone(), true, now()).unwrap();
    let days = plan_days(&core);
    assert!(day(&days, "2026-11-29").other.iter().any(|c| c.id == saturday), "moved to Sunday");
    assert!(day(&days, "2026-11-28").other.is_empty());
}

#[test]
fn after_leaving_the_plan_previews_are_unknown() {
    let (_dir, core) = open();
    daily(&core);
    let preview = day(&plan_days(&core), "2026-11-27").other[0].id.clone();
    core.leave_plan();
    assert!(matches!(core.rename_task(preview, "x".into()), Err(MobileError::UnknownTask { .. })));
}

#[test]
fn a_drop_back_onto_itself_changes_nothing() {
    let (_dir, core) = open();
    let a = add(&core, "2026-11-25", "A");
    add(&core, "2026-11-25", "B");
    core.plan_move_task(a.clone(), Some("2026-11-25".into()), Some(a), now()).unwrap();
    assert_eq!(titles(&day(&plan_days(&core), "2026-11-25").other), ["A", "B"]);
}

#[test]
fn any_visible_count_gives_a_window() {
    let window = plan_window(None, u32::MAX, None, now()).unwrap();
    assert!(window.count > 0);
}

#[test]
fn an_open_card_dropped_before_a_done_one_lands_after_the_open_cards() {
    let (_dir, core) = open();
    let outline = add(&core, "2026-11-25", "Outline");
    let research = add(&core, "2026-11-25", "Research");
    add(&core, "2026-11-25", "Draft");
    add(&core, "2026-11-25", "Revise");
    core.toggle_task(outline.clone(), now()).unwrap();
    assert_eq!(titles(&day(&plan_days(&core), "2026-11-25").other), ["Research", "Draft", "Revise", "Outline"]);
    core.plan_move_task(research, Some("2026-11-25".into()), Some(outline), now()).unwrap();
    assert_eq!(titles(&day(&plan_days(&core), "2026-11-25").other), ["Draft", "Revise", "Research", "Outline"]);
}
