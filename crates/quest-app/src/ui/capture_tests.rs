//! AI capture: ⌘K "Capture with AI" opens the panel (only with AI features on), Structure asks off
//! the UI thread and Esc drops the answer, the review creates only what is selected as one unit,
//! and ⌘Z takes it all back. Proposals come from a fixture, never from the network.

use std::time::{Duration, Instant};

use rune::{Key, Mods, Runtime};

use super::log_work_tests::text_of;
use super::tests::{TempDir, click, command, demo_app, press, signed_in_demo_app, type_text};
use crate::demo;
use crate::palette::Action;
use crate::state::capture::CaptureSource;
use crate::state::{CaptureStep, Overlay, State};

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key))
}

/// A signed-in demo app with AI features on, whose captures answer with the demo proposal.
pub(super) fn capture_app() -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = signed_in_demo_app(true);
    let proposal = demo::capture_proposal(&rt.state.snap, rt.state.today());
    rt.state.capture.source = CaptureSource::Fixture(proposal);
    (rt, dir)
}

fn open_from_palette(rt: &mut Runtime<State>) {
    command(rt, 'k');
    type_text(rt, "capture with");
    press(rt, Key::Enter, Mods::default());
}

/// Steps frames until the answer landed (it comes from a worker thread).
pub(super) fn wait_for_answer(rt: &mut Runtime<State>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while matches!(rt.state.capture.step, CaptureStep::Structuring) {
        assert!(Instant::now() < deadline, "the answer lands");
        std::thread::sleep(Duration::from_millis(5));
        rt.frame(0.0);
    }
    rt.frame(0.0);
}

/// Opens the panel, types `text` and structures it.
pub(super) fn structure(rt: &mut Runtime<State>, text: &str) {
    open_from_palette(rt);
    click(rt, "capture-text");
    type_text(rt, text);
    click(rt, "capture-structure");
}

fn count_tasks(rt: &Runtime<State>) -> usize {
    rt.state.snap.tasks.len()
}

fn offered(rt: &Runtime<State>) -> bool {
    super::palette::current_entries(&rt.state).iter().any(|e| e.action == Action::CaptureWithAi)
}

#[test]
fn the_palette_opens_the_text() {
    let (mut rt, _dir) = capture_app();
    open_from_palette(&mut rt);
    assert_eq!(rt.state.overlay, Overlay::Capture);
    assert!(has(&rt, "capture-text"));
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.overlay, Overlay::None);
}

#[test]
fn without_ai_features_the_palette_does_not_offer_it() {
    let (mut rt, _dir) = demo_app();
    command(&mut rt, 'k');
    type_text(&mut rt, "capture");
    assert!(!offered(&rt), "the guest has no AI features");
    let (mut rt, _dir) = signed_in_demo_app(false);
    command(&mut rt, 'k');
    type_text(&mut rt, "capture");
    assert!(!offered(&rt), "AI features are off by default");
    let (mut rt, _dir) = capture_app();
    command(&mut rt, 'k');
    type_text(&mut rt, "capture");
    assert!(offered(&rt));
}

#[test]
fn turning_ai_features_on_shows_the_privacy_notice_first() {
    let (mut rt, _dir) = signed_in_demo_app(false);
    command(&mut rt, ',');
    click(&mut rt, "account-ai-features");
    assert_eq!(rt.state.overlay, Overlay::AiNotice, "the notice comes before anything is sent");
    let notice = text_of(&rt, "ai-notice-text");
    assert!(notice.contains("7 days ago to 14 days ahead") && notice.contains("no time entries"), "{notice}");
    click(&mut rt, "ai-notice-cancel");
    assert_eq!(rt.state.overlay, Overlay::Account);
    assert!(matches!(rt.state.auth.step, crate::state::AuthStep::Idle), "cancel saves nothing");
    assert!(!rt.state.ai_capture_enabled());
}

#[test]
fn structuring_an_empty_text_says_what_to_do() {
    let (mut rt, _dir) = capture_app();
    structure(&mut rt, "   ");
    assert!(matches!(rt.state.capture.step, CaptureStep::Compose { error: Some(_) }));
    assert!(text_of(&rt, "capture-error").contains("Type or dictate"));
}

#[test]
fn esc_while_structuring_goes_back_and_drops_the_answer() {
    let (mut rt, _dir) = capture_app();
    let (release, gate) = std::sync::mpsc::channel();
    let proposal = demo::capture_proposal(&rt.state.snap, rt.state.today());
    rt.state.capture.source = CaptureSource::Gated(proposal, std::sync::Arc::new(std::sync::Mutex::new(gate)));
    open_from_palette(&mut rt);
    rt.state.capture.text = rune::TextEdit::new("Tomorrow write the offer");
    rt.state.structure_capture();
    assert!(matches!(rt.state.capture.step, CaptureStep::Structuring));
    let tasks = count_tasks(&rt);
    press(&mut rt, Key::Escape, Mods::default());
    assert!(matches!(rt.state.capture.step, CaptureStep::Compose { error: None }));
    release.send(()).expect("the request is still waiting");
    std::thread::sleep(Duration::from_millis(50));
    rt.frame(0.0);
    assert!(matches!(rt.state.capture.step, CaptureStep::Compose { .. }), "a late answer is dropped");
    assert_eq!(count_tasks(&rt), tasks, "nothing was created");
    assert_eq!(rt.state.overlay, Overlay::Capture);
}

#[test]
fn the_review_groups_items_and_marks_guesses() {
    let (mut rt, _dir) = capture_app();
    structure(&mut rt, "Tomorrow write the offer");
    wait_for_answer(&mut rt);
    for key in ["capture-item-q1", "capture-item-t1", "capture-item-t2", "capture-item-w1", "capture-dropped-0"] {
        assert!(has(&rt, key), "`{key}` shows");
    }
    let quest = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("capture-item-t1-quest"));
    assert!(quest.and_then(|e| e.label).is_some_and(|l| l.contains("guess")), "the guessed quest is marked");
    assert_eq!(text_of(&rt, "capture-item-w1-task"), "Design the next chapter");
}

#[test]
fn create_applies_the_selection_as_one_unit_and_undo_takes_it_back() {
    let (mut rt, _dir) = capture_app();
    let before = (rt.state.snap.tasks.len(), rt.state.snap.quests.len(), rt.state.snap.time_entries.len());
    structure(&mut rt, "Tomorrow write the offer");
    wait_for_answer(&mut rt);
    click(&mut rt, "capture-item-t2-check");
    click(&mut rt, "capture-item-t1-title");
    press(&mut rt, Key::Command('a'), Mods { cmd: true, ..Mods::default() });
    type_text(&mut rt, "Offer for Müller");
    click(&mut rt, "capture-create");
    assert_eq!(rt.state.overlay, Overlay::None);
    let snap = rt.state.snap.clone();
    assert!(snap.tasks.iter().any(|t| t.title == "Offer for Müller"), "the edited title is used");
    assert!(!snap.tasks.iter().any(|t| t.title == "Collect receipts"), "a deselected task is not created");
    assert_eq!(snap.quests.len(), before.1 + 1);
    assert_eq!(snap.time_entries.len(), before.2 + 1);
    assert!(text_of(&rt, "toast-message").starts_with("Created 1 quest, 1 task and 1 session"));

    command(&mut rt, 'z');
    let snap = rt.state.snap.clone();
    assert_eq!((snap.tasks.len(), snap.quests.len(), snap.time_entries.len()), before, "⌘Z takes it all back");
    assert!(rt.state.capture_undo.is_none());
}

#[test]
fn deselecting_everything_disables_create() {
    let (mut rt, _dir) = capture_app();
    structure(&mut rt, "Tomorrow write the offer");
    wait_for_answer(&mut rt);
    for key in ["capture-item-q1-check", "capture-item-t1-check", "capture-item-t2-check", "capture-item-w1-check"] {
        click(&mut rt, key);
    }
    let tasks = count_tasks(&rt);
    click(&mut rt, "capture-create");
    assert_eq!(rt.state.overlay, Overlay::Capture, "nothing to create keeps the panel open");
    assert_eq!(count_tasks(&rt), tasks);
}

#[test]
fn a_later_delete_is_what_undo_takes_back() {
    let (mut rt, _dir) = capture_app();
    structure(&mut rt, "Tomorrow write the offer");
    wait_for_answer(&mut rt);
    click(&mut rt, "capture-create");
    let created = rt.state.snap.tasks.len();
    let task = rt.state.snap.tasks_on(rt.state.today()).find(|t| !t.is_done()).map(|t| t.id).expect("an open task");
    rt.state.delete(crate::undo::Deletable::Task(task));
    rt.frame(0.0);
    command(&mut rt, 'z');
    assert_eq!(rt.state.snap.tasks.len(), created, "⌘Z restored the deleted task, the capture stays");
}

#[test]
fn enter_adds_a_line_and_command_return_structures() {
    let (mut rt, _dir) = capture_app();
    open_from_palette(&mut rt);
    click(&mut rt, "capture-text");
    type_text(&mut rt, "one");
    press(&mut rt, Key::Enter, Mods::default());
    type_text(&mut rt, "two");
    assert_eq!(rt.state.capture.text.text, "one\ntwo");
    assert!(matches!(rt.state.capture.step, CaptureStep::Compose { .. }));
    press(&mut rt, Key::Enter, Mods { cmd: true, ..Mods::default() });
    wait_for_answer(&mut rt);
    assert!(rt.state.capture.review().is_some(), "⌘↵ structured the text");
    assert_eq!(rt.state.capture.text.text, "one\ntwo");
}

#[test]
fn the_toast_goes_after_its_time_and_command_z_still_undoes() {
    let (mut rt, _dir) = capture_app();
    let tasks = count_tasks(&rt);
    structure(&mut rt, "Tomorrow write the offer");
    wait_for_answer(&mut rt);
    click(&mut rt, "capture-create");
    assert!(has(&rt, "toast"));
    let later = rt.state.now + chrono::Duration::seconds(crate::undo::TOAST_SECONDS + 1);
    rt.state.clock = crate::clock::Clock::from_local(Some(crate::clock::local(later)));
    rt.frame(0.0);
    rt.settle_all();
    rt.frame(0.0);
    assert!(rt.state.capture_undo.as_ref().is_some_and(|u| u.until.is_none()), "the toast is gone");
    command(&mut rt, 'z');
    assert_eq!(count_tasks(&rt), tasks, "⌘Z still takes the capture back");
}

#[test]
fn a_capture_replaces_an_earlier_delete() {
    let (mut rt, _dir) = capture_app();
    let tasks = count_tasks(&rt);
    // Not today's first task: the demo proposal logs work on that one.
    let task = rt.state.snap.tasks_on(rt.state.today()).filter(|t| !t.is_done()).last().map(|t| t.id);
    rt.state.delete(crate::undo::Deletable::Task(task.expect("an open task")));
    structure(&mut rt, "Tomorrow write the offer");
    wait_for_answer(&mut rt);
    click(&mut rt, "capture-create");
    assert!(rt.state.capture_undo.is_some(), "the capture was created");
    command(&mut rt, 'z');
    assert_eq!(count_tasks(&rt), tasks - 1, "the first ⌘Z takes back the capture");
    command(&mut rt, 'z');
    assert_eq!(count_tasks(&rt), tasks - 1, "the earlier delete is no longer in the slot");
}

#[test]
fn a_completion_replaces_the_capture_undo_slot() {
    let (mut rt, _dir) = capture_app();
    let tasks = count_tasks(&rt);
    structure(&mut rt, "Tomorrow write the offer");
    wait_for_answer(&mut rt);
    click(&mut rt, "capture-create");
    let quest_task = rt.state.snap.tasks_on(rt.state.today()).find(|t| !t.is_done() && t.quest_id.is_some());
    let id = quest_task.expect("an open quest task").id;
    rt.state.toggle_done(id);
    rt.frame(0.0);
    assert!(rt.state.capture_undo.is_none());
    assert!(matches!(rt.state.undo.shown.as_ref().map(|shown| &shown.toast), Some(crate::undo::Toast::Done)));
    command(&mut rt, 'z');
    assert!(!rt.state.snap.task(id).unwrap().is_done());
    assert!(count_tasks(&rt) > tasks, "the capture remains");
}

#[test]
fn edit_text_and_structure_again_shows_a_fresh_review() {
    let (mut rt, _dir) = capture_app();
    structure(&mut rt, "Tomorrow write the offer");
    wait_for_answer(&mut rt);
    click(&mut rt, "capture-item-t2-check");
    click(&mut rt, "capture-edit-text");
    assert!(matches!(rt.state.capture.step, CaptureStep::Compose { .. }));
    click(&mut rt, "capture-structure");
    wait_for_answer(&mut rt);
    let review = rt.state.capture.review().expect("a review");
    assert!(review.draft.deselected.is_empty(), "the new proposal starts all selected");
}

#[test]
fn closing_keeps_the_review_and_cancel_drops_it() {
    let (mut rt, _dir) = capture_app();
    structure(&mut rt, "Tomorrow write the offer");
    wait_for_answer(&mut rt);
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.overlay, Overlay::None);
    open_from_palette(&mut rt);
    assert!(rt.state.capture.review().is_some(), "the proposal is still there");
    click(&mut rt, "capture-cancel");
    open_from_palette(&mut rt);
    assert!(matches!(rt.state.capture.step, CaptureStep::Compose { .. }));
}
