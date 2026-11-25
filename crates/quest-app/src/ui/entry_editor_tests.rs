//! Edit time entry: a click on a session opens it with the entry's values, Save updates that
//! same entry, Cancel / Esc / Open task drop what was typed, Delete offers undo; drags and
//! resizes on the grid never open it. A session moved over midnight shows on both days.

use chrono::{Days, Duration};
use quest_domain::EntityId;
use rune::{Key, Mods, Runtime};

use super::dnd_tests::{center, drag, entry_of, entry_times, hm, settle, task};
use super::log_work_tests::{fill, search, text_of};
use super::tests::{TempDir, click, command, demo_app, press};
use crate::clock;
use crate::format;
use crate::log_work::date_label;
use crate::state::{Overlay, State};
use crate::ui::timegrid::CALENDAR_SCALE;

const MAP: &str = "Map the ideas worth pursuing";
const READING: &str = "Read a chapter, take a few notes";

fn calendar() -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = demo_app();
    command(&mut rt, '3');
    settle(&mut rt);
    (rt, dir)
}

fn open(rt: &mut Runtime<State>, entry: EntityId) {
    click(rt, &format!("calendar-grid-entry-{entry}"));
    settle(rt);
    assert_eq!(rt.state.overlay, Overlay::EntryEditor, "the click opens the editor");
}

fn entry_count(rt: &Runtime<State>) -> usize {
    rt.state.snap.time_entries.len()
}

fn day_total(rt: &Runtime<State>, index: u64) -> i64 {
    let s = &rt.state;
    let day = s.calendar_week + Days::new(index);
    s.snap.work_between(clock::midnight(day), clock::midnight(day + Days::new(1)), s.now).total_ms()
}

#[test]
fn a_click_opens_the_editor_with_the_entrys_values() {
    let (mut rt, _dir) = calendar();
    let map = entry_of(&rt, MAP);
    open(&mut rt, map);
    let form = &rt.state.entry_form;
    let today = date_label(rt.state.today());
    assert_eq!(form.entry, Some(map));
    assert_eq!((form.start_date.text.as_str(), form.start.text.as_str()), (today.as_str(), "09:00"));
    assert_eq!((form.end_date.text.as_str(), form.end.text.as_str()), (today.as_str(), "10:30"));
    assert_eq!(text_of(&rt, "entry-editor-duration"), "1h 30m");
    let quest = task(&rt, MAP).quest_id.and_then(|q| rt.state.snap.quest(q)).map(|q| q.title.clone());
    assert_eq!(Some(text_of(&rt, "entry-editor-quest-title")), quest);
}

#[test]
fn save_updates_the_same_entry_and_adds_none() {
    let (mut rt, _dir) = calendar();
    let map = entry_of(&rt, MAP);
    let count = entry_count(&rt);
    open(&mut rt, map);
    fill(&mut rt, "entry-editor-start", "8:45");
    fill(&mut rt, "entry-editor-end", "10:00");
    assert_eq!(text_of(&rt, "entry-editor-duration"), "1h 15m", "the duration follows as you type");
    click(&mut rt, "entry-editor-save");
    assert_eq!(rt.state.overlay, Overlay::None);
    assert_eq!(entry_times(&rt, map), (hm(8, 45), Some(hm(10, 0))), "the same id, moved");
    assert_eq!(entry_count(&rt), count, "no new entry");
}

#[test]
fn command_return_saves_like_the_button() {
    let (mut rt, _dir) = calendar();
    let map = entry_of(&rt, MAP);
    open(&mut rt, map);
    fill(&mut rt, "entry-editor-end", "11:00");
    press(&mut rt, Key::Enter, Mods { cmd: true, ..Mods::default() });
    assert_eq!(rt.state.overlay, Overlay::None);
    assert_eq!(entry_times(&rt, map), (hm(9, 0), Some(hm(11, 0))));
}

#[test]
fn cancel_and_escape_drop_what_was_typed() {
    let (mut rt, _dir) = calendar();
    let map = entry_of(&rt, MAP);
    let before = rt.state.snap.time_entries.clone();
    open(&mut rt, map);
    fill(&mut rt, "entry-editor-start", "7:00");
    click(&mut rt, "entry-editor-cancel");
    assert_eq!(rt.state.overlay, Overlay::None);
    assert_eq!(rt.state.snap.time_entries, before);

    open(&mut rt, map);
    fill(&mut rt, "entry-editor-end", "12:00");
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.overlay, Overlay::None);
    assert_eq!(rt.state.snap.time_entries, before);
}

#[test]
fn delete_removes_the_entry_with_undo() {
    let (mut rt, _dir) = calendar();
    let map = entry_of(&rt, MAP);
    open(&mut rt, map);
    click(&mut rt, "entry-editor-delete");
    assert_eq!(rt.state.overlay, Overlay::None);
    assert!(rt.state.snap.time_entries.iter().all(|e| e.id != map));
    assert!(rt.find("toast").is_some(), "the undo toast shows");
    command(&mut rt, 'z');
    assert!(rt.state.snap.time_entries.iter().any(|e| e.id == map), "⌘Z brings it back");
}

#[test]
fn a_done_task_can_take_the_entry_and_the_quest_follows_it() {
    let (mut rt, _dir) = calendar();
    let map = entry_of(&rt, MAP);
    let reading = task(&rt, READING);
    assert!(reading.is_done());
    let quest = reading.quest_id.and_then(|q| rt.state.snap.quest(q)).map(|q| q.title.clone()).expect("a quest");
    open(&mut rt, map);
    search(&mut rt, "entry-editor-task", "read a chapter");
    click(&mut rt, &format!("entry-editor-task-option-{}", reading.id));
    assert_eq!(rt.state.entry_form.task, Some(reading.id), "done tasks are found");
    assert_eq!(text_of(&rt, "entry-editor-quest-title"), quest, "the quest shown follows the task");
    click(&mut rt, "entry-editor-save");
    let entry = rt.state.snap.time_entries.iter().find(|e| e.id == map).expect("same entry");
    assert_eq!((entry.task_id, entry.quest_id), (Some(reading.id), reading.quest_id));
}

#[test]
fn an_emptied_task_field_saves_other_work_with_no_quest() {
    let (mut rt, _dir) = calendar();
    let map = entry_of(&rt, MAP);
    open(&mut rt, map);
    click(&mut rt, "entry-editor-clear-task");
    assert_eq!(text_of(&rt, "entry-editor-quest-title"), "No quest");
    click(&mut rt, "entry-editor-save");
    let entry = rt.state.snap.time_entries.iter().find(|e| e.id == map).expect("same entry");
    assert_eq!((entry.task_id, entry.quest_id), (None, None));
}

#[test]
fn open_task_drops_the_edits_and_opens_the_task_dialog() {
    let (mut rt, _dir) = calendar();
    let map = entry_of(&rt, MAP);
    let before = rt.state.snap.time_entries.clone();
    open(&mut rt, map);
    fill(&mut rt, "entry-editor-start", "7:00");
    click(&mut rt, "entry-editor-open-task");
    assert_eq!(rt.state.overlay, Overlay::Task);
    assert_eq!(rt.state.task_modal.as_ref().map(|m| m.task), Some(task(&rt, MAP).id));
    assert_eq!(rt.state.snap.time_entries, before, "nothing saved");
}

#[test]
fn an_end_not_after_the_start_shows_at_the_field_and_blocks_save() {
    let (mut rt, _dir) = calendar();
    let map = entry_of(&rt, MAP);
    open(&mut rt, map);
    fill(&mut rt, "entry-editor-end", "8:30");
    assert_eq!(text_of(&rt, "entry-editor-error"), "The end must be after the start.");
    click(&mut rt, "entry-editor-save");
    press(&mut rt, Key::Enter, Mods { cmd: true, ..Mods::default() });
    assert_eq!(rt.state.overlay, Overlay::EntryEditor, "still open");
    assert_eq!(entry_times(&rt, map), (hm(9, 0), Some(hm(10, 30))), "nothing saved");
}

#[test]
fn the_end_date_follows_the_start_date_until_it_is_set() {
    let (mut rt, _dir) = calendar();
    let map = entry_of(&rt, MAP);
    open(&mut rt, map);
    let yesterday = date_label(rt.state.today() - Days::new(1));
    fill(&mut rt, "entry-editor-start-date", &yesterday);
    assert_eq!(rt.state.entry_form.end_date.text, yesterday);
    let tomorrow = date_label(rt.state.today() + Days::new(1));
    click(&mut rt, "entry-editor-end-on-different-day");
    fill(&mut rt, "entry-editor-end-date", &tomorrow);
    let today = date_label(rt.state.today());
    fill(&mut rt, "entry-editor-start-date", &today);
    assert_eq!(rt.state.entry_form.end_date.text, tomorrow, "set by hand, it stays");
}

#[test]
fn an_entry_saved_over_midnight_shows_on_both_days_and_opens_again() {
    let (mut rt, _dir) = calendar();
    let map = entry_of(&rt, MAP);
    let (thursday, friday) = (day_total(&rt, 3), day_total(&rt, 4));
    open(&mut rt, map);
    fill(&mut rt, "entry-editor-start", "22:15");
    let tomorrow = rt.state.today() + Days::new(1);
    click(&mut rt, "entry-editor-end-on-different-day");
    fill(&mut rt, "entry-editor-end-date", &date_label(tomorrow));
    fill(&mut rt, "entry-editor-end", "7:30");
    assert_eq!(text_of(&rt, "entry-editor-duration"), "9h 15m");
    click(&mut rt, "entry-editor-save");
    settle(&mut rt);

    let entry = rt.state.snap.time_entries.iter().find(|e| e.id == map).cloned().expect("same entry");
    assert_eq!(clock::local_day(entry.ended_at.expect("finished")), tomorrow);
    let hour = |h: i64| Duration::hours(h).num_milliseconds();
    let minutes = |m: i64| Duration::minutes(m).num_milliseconds();
    assert_eq!(day_total(&rt, 3), thursday - minutes(90) + minutes(105), "Thursday counts 22:15–24:00");
    assert_eq!(day_total(&rt, 4), friday + hour(7) + minutes(30), "Friday counts 00:00–07:30");
    assert_eq!(text_of(&rt, "calendar-day-total-4"), format::hours(friday + minutes(450)));
    assert!(rt.find(&format!("calendar-grid-entry-{map}")).is_some(), "Thursday's segment");
    let next_day = format!("calendar-grid-entry-{map}-{tomorrow}");
    assert!(rt.find(&next_day).is_some(), "Friday's segment");
    assert!(rt.find(&format!("calendar-grid-entry-end-{map}")).is_some(), "the end handle sits on Friday only");

    click(&mut rt, &next_day);
    settle(&mut rt);
    let form = &rt.state.entry_form;
    assert_eq!(rt.state.overlay, Overlay::EntryEditor);
    assert_eq!((form.start_date.text.clone(), form.start.text.as_str()), (date_label(rt.state.today()), "22:15"));
    assert_eq!((form.end_date.text.clone(), form.end.text.as_str()), (date_label(tomorrow), "07:30"));
    fill(&mut rt, "entry-editor-end", "8:00");
    click(&mut rt, "entry-editor-save");
    assert_eq!(entry_times(&rt, map), (hm(22, 15), Some(hm(8, 0))), "edited again");
}

#[test]
fn a_running_entry_shows_running_and_changes_only_its_start() {
    let (mut rt, _dir) = calendar();
    let running = rt.state.snap.running_entry().map(|e| e.id).expect("the demo runs a session");
    open(&mut rt, running);
    assert!(rt.find("entry-editor-end-running").is_some());
    assert!(rt.find("entry-editor-end").is_none(), "no end to edit");
    fill(&mut rt, "entry-editor-start", "14:00");
    click(&mut rt, "entry-editor-save");
    assert_eq!(entry_times(&rt, running), (hm(14, 0), None), "still running");
}

#[test]
fn dragging_a_session_moves_it_without_the_editor() {
    let (mut rt, _dir) = calendar();
    let map = entry_of(&rt, MAP);
    let body = center(&rt, &format!("calendar-grid-entry-{map}"));
    drag(&mut rt, body, (body.0, body.1 + CALENDAR_SCALE.y(7 * 60 + 60)));
    assert_eq!(entry_times(&rt, map), (hm(10, 0), Some(hm(11, 30))));
    assert_eq!(rt.state.overlay, Overlay::None);
}

#[test]
fn resizing_by_either_handle_opens_no_editor() {
    let (mut rt, _dir) = calendar();
    let map = entry_of(&rt, MAP);
    let start = center(&rt, &format!("calendar-grid-entry-start-{map}"));
    drag(&mut rt, start, (start.0, start.1 - CALENDAR_SCALE.y(7 * 60 + 30)));
    assert_eq!(entry_times(&rt, map), (hm(8, 30), Some(hm(10, 30))));
    assert_eq!(rt.state.overlay, Overlay::None);
    let end = center(&rt, &format!("calendar-grid-entry-end-{map}"));
    drag(&mut rt, end, (end.0, end.1 + CALENDAR_SCALE.y(7 * 60 + 30)));
    assert_eq!(entry_times(&rt, map), (hm(8, 30), Some(hm(11, 0))));
    assert_eq!(rt.state.overlay, Overlay::None);

    let running = rt.state.snap.running_entry().map(|e| e.id).expect("the demo runs a session");
    let start = center(&rt, &format!("calendar-grid-entry-start-{running}"));
    drag(&mut rt, start, (start.0, start.1 - CALENDAR_SCALE.y(7 * 60 + 15)));
    assert_eq!(rt.state.overlay, Overlay::None, "nor does the running session's start");
}

#[test]
fn home_opens_the_same_editor() {
    let (mut rt, _dir) = demo_app();
    settle(&mut rt);
    let map = entry_of(&rt, MAP);
    click(&mut rt, &format!("home-grid-entry-{map}"));
    assert_eq!(rt.state.overlay, Overlay::EntryEditor);
    assert_eq!(rt.state.entry_form.entry, Some(map));
}

#[test]
fn a_new_task_is_created_only_on_save_and_the_existing_entry_gets_its_own_quest() {
    let (mut rt, _dir) = calendar();
    let id = entry_of(&rt, MAP);
    let original = task(&rt, MAP).id;
    let count = entry_count(&rt);
    let task_count = rt.state.snap.tasks.len();
    open(&mut rt, id);
    search(&mut rt, "entry-editor-task", "A brand new task");
    click(&mut rt, "entry-editor-task-option-create");
    assert_eq!(rt.state.snap.tasks.len(), task_count);
    click(&mut rt, "entry-editor-quest");
    click(&mut rt, "quest-picker-none");
    click(&mut rt, "entry-editor-save");
    let created = task(&rt, "A brand new task");
    assert!(created.is_done());
    let entry = rt.state.snap.time_entries.iter().find(|e| e.id == id).unwrap();
    assert_eq!((entry.task_id, entry.quest_id), (Some(created.id), None));
    assert_eq!(entry_count(&rt), count);
    assert!(rt.state.snap.task(original).unwrap().quest_id.is_some());
}

#[test]
fn the_start_picker_selects_a_date_and_keeps_same_day_entries_together() {
    let (mut rt, _dir) = calendar();
    let id = entry_of(&rt, MAP);
    open(&mut rt, id);
    let yesterday = rt.state.today() - Days::new(1);
    click(&mut rt, "entry-start-picker-button");
    settle(&mut rt);
    click(&mut rt, &format!("entry-start-picker-day-{yesterday}"));
    assert_eq!(rt.state.entry_form.start_date.text, date_label(yesterday));
    assert_eq!(rt.state.entry_form.end_date.text, date_label(yesterday));
    assert!(rt.state.date_picker.is_none());
}

#[test]
fn escape_closes_only_the_open_picker() {
    let (mut rt, _dir) = calendar();
    let id = entry_of(&rt, MAP);
    open(&mut rt, id);
    click(&mut rt, "entry-start-picker-button");
    settle(&mut rt);
    assert!(rt.state.date_picker.is_some());
    press(&mut rt, Key::Escape, Mods::default());
    assert!(rt.state.date_picker.is_none());
    assert_eq!(rt.state.overlay, Overlay::EntryEditor);
}
