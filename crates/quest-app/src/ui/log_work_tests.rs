//! Log work: the palette's command opens the dialog, nothing is saved before Log work, and
//! logged time shows in every sum (Home, Review).

use chrono::Days;
use rune::{Key, Mods, Runtime};

use super::dnd_tests::{settle, task};
use super::tests::{click, command, demo_app, press, type_text};
use crate::clock;
use crate::format;
use crate::state::{Overlay, State};

const QUARTER_HOUR_MS: i64 = 15 * 60_000;

pub(super) fn text_of(rt: &Runtime<State>, key: &str) -> String {
    let element = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key));
    element.and_then(|e| e.text).unwrap_or_else(|| panic!("`{key}` shows text"))
}

/// Replaces a dialog field's text.
pub(super) fn fill(rt: &mut Runtime<State>, key: &str, value: &str) {
    click(rt, key);
    press(rt, Key::Command('a'), Mods { cmd: true, ..Mods::default() });
    type_text(rt, value);
}

pub(super) fn search(rt: &mut Runtime<State>, key: &str, query: &str) {
    click(rt, &format!("{key}-value"));
    settle(rt);
    type_text(rt, query);
    settle(rt);
}

/// Quest and all work this week, and all work today.
fn sums(rt: &Runtime<State>) -> (i64, i64) {
    let s = &rt.state;
    let monday = s.this_week();
    let week = s.snap.work_between(clock::midnight(monday), clock::midnight(monday + Days::new(7)), s.now);
    let today = s.snap.work_between(clock::midnight(s.today()), clock::midnight(s.today() + Days::new(1)), s.now);
    (week.quest_ms, today.total_ms())
}

/// Runs the palette's "Log work".
fn open_log_work(rt: &mut Runtime<State>) {
    rt.update(|s, tx| super::palette::run(s, crate::palette::Action::LogWork, tx));
    rt.frame(0.0);
}

#[test]
fn fifteen_minutes_on_a_done_quest_task_show_on_today_in_the_week_and_in_review() {
    let (mut rt, _dir) = demo_app();
    let reading = task(&rt, "Read a chapter, take a few notes");
    assert!(reading.is_done() && reading.quest_id.is_some());
    let (week, today) = sums(&rt);

    open_log_work(&mut rt);
    assert_eq!(rt.state.overlay, Overlay::LogWork);
    search(&mut rt, "log-work-task", "read a chapter");
    click(&mut rt, &format!("log-work-task-option-{}", reading.id));
    assert_eq!(rt.state.log_work.task, Some(reading.id), "done tasks are found");
    assert_eq!(crate::log_work::quest(&rt.state.snap, rt.state.log_work.task, None), reading.quest_id);
    fill(&mut rt, "log-work-start", "7:00");
    fill(&mut rt, "log-work-end", "7:15");
    click(&mut rt, "log-work-save");
    assert_eq!(rt.state.overlay, Overlay::None);

    let logged = rt.state.snap.time_entries.iter().find(|e| e.task_id == Some(reading.id)).expect("logged");
    assert_eq!(logged.quest_id, reading.quest_id, "counts towards the task's quest");
    assert_eq!(sums(&rt), (week + QUARTER_HOUR_MS, today + QUARTER_HOUR_MS));

    command(&mut rt, '2');
    let week_total = text_of(&rt, "week-total");
    assert!(week_total.starts_with(&format!("{} / ", format::hours(week + QUARTER_HOUR_MS))), "{week_total}");
    let tracked_today = text_of(&rt, "page-sub");
    assert!(tracked_today.ends_with(&format!("{} tracked", format::hours(today + QUARTER_HOUR_MS))), "{tracked_today}");

    command(&mut rt, '4');
    assert_eq!(text_of(&rt, "review-quest-hours"), format::hours(week + QUARTER_HOUR_MS));
}

#[test]
fn cancel_discards_and_no_sum_moves() {
    let (mut rt, _dir) = demo_app();
    let (tasks, entries, before) = (rt.state.snap.tasks.len(), rt.state.snap.time_entries.len(), sums(&rt));
    open_log_work(&mut rt);
    search(&mut rt, "log-work-task", "A call I forgot");
    press(&mut rt, Key::Enter, Mods::default());
    click(&mut rt, "log-work-cancel");
    assert_eq!(rt.state.overlay, Overlay::None);
    assert_eq!((rt.state.snap.tasks.len(), rt.state.snap.time_entries.len()), (tasks, entries));
    assert_eq!(sums(&rt), before);
}

#[test]
fn a_new_title_becomes_a_completed_task_on_the_chosen_quest() {
    let (mut rt, _dir) = demo_app();
    open_log_work(&mut rt);
    search(&mut rt, "log-work-task", "Fix the bike");
    click(&mut rt, "log-work-task-option-create");
    let third = rt.state.snap.active_quests().nth(2).unwrap().id;
    click(&mut rt, "log-work-quest");
    click(&mut rt, &format!("quest-picker-{third}"));
    fill(&mut rt, "log-work-start", "6:00");
    fill(&mut rt, "log-work-end", "6:30");
    click(&mut rt, "log-work-save");
    let bike = task(&rt, "Fix the bike");
    assert!(bike.is_done(), "logged work is done work");
    assert_eq!(bike.quest_id, Some(third));
    assert!(rt.state.snap.time_entries.iter().any(|e| e.task_id == Some(bike.id)));
}

#[test]
fn an_end_before_the_start_is_refused_at_the_field() {
    let (mut rt, _dir) = demo_app();
    let entries = rt.state.snap.time_entries.len();
    open_log_work(&mut rt);
    search(&mut rt, "log-work-task", "Backwards");
    press(&mut rt, Key::Enter, Mods::default());
    fill(&mut rt, "log-work-start", "9:00");
    fill(&mut rt, "log-work-end", "8:00");
    click(&mut rt, "log-work-save");
    assert_eq!(rt.state.overlay, Overlay::LogWork, "the dialog stays open");
    assert_eq!(text_of(&rt, "log-work-error"), "The end must be after the start.");
    assert_eq!(rt.state.snap.time_entries.len(), entries);
}

#[test]
fn return_in_task_takes_the_first_suggestion_and_only_log_work_saves() {
    let (mut rt, _dir) = demo_app();
    let entries = rt.state.snap.time_entries.len();
    let tempo = task(&rt, "Tempo run");
    open_log_work(&mut rt);
    search(&mut rt, "log-work-task", "tempo");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.log_work.task, Some(tempo.id), "Return picks the first suggestion");
    assert_eq!(rt.state.overlay, Overlay::LogWork, "and saves nothing");
    assert_eq!(rt.state.snap.time_entries.len(), entries);

    fill(&mut rt, "log-work-start", "6:00");
    fill(&mut rt, "log-work-end", "6:15");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.overlay, Overlay::LogWork, "Return in a time field doesn't save either");
    press(&mut rt, Key::Enter, Mods { cmd: true, ..Mods::default() });
    assert_eq!(rt.state.overlay, Overlay::None, "⌘↵ saves");
    assert!(rt.state.snap.time_entries.iter().any(
        |e| e.task_id == Some(tempo.id) && clock::local(e.started_at).time().format("%H:%M").to_string() == "06:00"
    ));
    assert_eq!(rt.state.snap.time_entries.len(), entries + 1);
}

#[test]
fn return_without_a_suggestion_keeps_the_title_for_a_new_completed_task() {
    let (mut rt, _dir) = demo_app();
    let tasks = rt.state.snap.tasks.len();
    open_log_work(&mut rt);
    search(&mut rt, "log-work-task", "Zzz unheard of");
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(rt.state.log_work.task, None);
    assert_eq!(rt.state.snap.tasks.len(), tasks, "nothing created before Log work");
    click(&mut rt, "log-work-save");
    let created = task(&rt, "Zzz unheard of");
    assert!(created.is_done(), "a new completed task");
}
