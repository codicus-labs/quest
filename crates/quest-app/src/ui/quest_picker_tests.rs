//! The quest picker in the task dialog and the AI capture: the chip opens it with the search
//! focused; typing filters, ↑/↓ and Return choose, Escape changes nothing, and focus goes back to
//! the chip.

use quest_domain::EntityId;
use quest_protocol::{QuestRef, TempKey};
use rune::{Key, Mods, Runtime};

use super::capture_tests::{capture_app, structure, wait_for_answer};
use super::dnd_tests::{settle, task};
use super::tests::{TempDir, click, command, demo_app, press, type_text};
use crate::state::State;

const TASK: &str = "Design the next chapter";

fn focused(rt: &Runtime<State>) -> Option<String> {
    rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key)
}

fn shown(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

/// The keys of the rows the picker shows, in order.
fn rows(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| e.role == "option" && !e.exiting).filter_map(|e| e.key).collect()
}

fn quest_id(rt: &Runtime<State>, title: &str) -> EntityId {
    rt.state.snap.quests.iter().find(|q| q.title == title).expect("demo quest").id
}

/// The demo with the task dialog open on `TASK`.
fn task_dialog() -> (Runtime<State>, TempDir, EntityId) {
    let (mut rt, dir) = demo_app();
    rt.state.stop_focus();
    command(&mut rt, '2');
    let id = task(&rt, TASK).id;
    rt.state.open_task_modal(id);
    settle(&mut rt);
    (rt, dir, id)
}

fn open(rt: &mut Runtime<State>, chip: &str) {
    click(rt, chip);
    settle(rt);
    assert!(shown(rt, "quest-picker"), "the picker opens");
    assert_eq!(focused(rt).as_deref(), Some("quest-picker-search"), "with the search focused");
}

fn task_quest(rt: &Runtime<State>, id: EntityId) -> Option<EntityId> {
    rt.state.snap.task(id).unwrap().quest_id
}

#[test]
fn the_task_dialog_lists_the_recent_quests_then_no_quest() {
    let (mut rt, _dir, _) = task_dialog();
    open(&mut rt, "task-modal-quest");
    let recent = quest_view::quest_picker::recent_quests(&rt.state.snap, rt.state.now);
    let mut expected: Vec<String> = recent.iter().map(|q| format!("quest-picker-{}", q.id)).collect();
    expected.push("quest-picker-none".into());
    assert_eq!(rows(&rt), expected);
}

#[test]
fn typing_filters_and_return_takes_the_first_row() {
    let (mut rt, _dir, id) = task_dialog();
    open(&mut rt, "task-modal-quest");
    type_text(&mut rt, "MARATHON");
    let running = quest_id(&rt, "Run a half marathon");
    assert_eq!(rows(&rt), [format!("quest-picker-{running}")], "a substring, any case");

    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(task_quest(&rt, id), Some(running));
    settle(&mut rt);
    assert!(!shown(&rt, "quest-picker"), "it closes");
    assert_eq!(focused(&rt).as_deref(), Some("task-modal-quest"), "focus is back on the chip");
}

#[test]
fn no_match_shows_the_empty_state_and_return_changes_nothing() {
    let (mut rt, _dir, id) = task_dialog();
    let before = task_quest(&rt, id);
    open(&mut rt, "task-modal-quest");
    type_text(&mut rt, "zzz");
    assert!(rows(&rt).is_empty() && shown(&rt, "quest-picker-empty"));
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(task_quest(&rt, id), before);
}

#[test]
fn arrows_move_from_the_chosen_quest_and_return_takes_it() {
    let (mut rt, _dir, id) = task_dialog();
    open(&mut rt, "task-modal-quest");
    let keys = rows(&rt);
    let current = keys.iter().position(|k| *k == format!("quest-picker-{}", task_quest(&rt, id).unwrap())).unwrap();
    press(&mut rt, Key::Down, Mods::default());
    press(&mut rt, Key::Down, Mods::default());
    press(&mut rt, Key::Up, Mods::default());
    press(&mut rt, Key::Enter, Mods::default());
    let chosen = &keys[(current + 1) % keys.len()];
    assert_eq!(task_quest(&rt, id).map(|q| format!("quest-picker-{q}")).as_ref(), Some(chosen));
}

#[test]
fn up_from_the_first_row_wraps_to_no_quest() {
    let (mut rt, _dir, id) = task_dialog();
    open(&mut rt, "task-modal-quest");
    type_text(&mut rt, "e");
    press(&mut rt, Key::Up, Mods::default());
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(task_quest(&rt, id), None, "No quest is the last row");
}

#[test]
fn escape_closes_without_a_change_and_focuses_the_chip() {
    let (mut rt, _dir, id) = task_dialog();
    let before = task_quest(&rt, id);
    open(&mut rt, "task-modal-quest");
    type_text(&mut rt, "marathon");
    press(&mut rt, Key::Down, Mods::default());
    press(&mut rt, Key::Escape, Mods::default());
    settle(&mut rt);
    assert!(rt.state.quest_picker.is_none() && !shown(&rt, "quest-picker"));
    assert!(rt.state.task_modal.is_some(), "the dialog stays open");
    assert_eq!(task_quest(&rt, id), before);
    assert_eq!(focused(&rt).as_deref(), Some("task-modal-quest"));
}

#[test]
fn no_quest_takes_the_task_off_its_quest() {
    let (mut rt, _dir, id) = task_dialog();
    open(&mut rt, "task-modal-quest");
    click(&mut rt, "quest-picker-none");
    assert_eq!(task_quest(&rt, id), None);
}

fn capture_review() -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = capture_app();
    structure(&mut rt, "Tomorrow write the offer");
    wait_for_answer(&mut rt);
    (rt, dir)
}

fn capture_quest(rt: &Runtime<State>, task: &str) -> Option<QuestRef> {
    let review = rt.state.capture.review().expect("the review shows");
    let task = review.draft.proposal.tasks.iter().find(|t| t.key.0 == task).unwrap();
    review.draft.effective_quest(&task.quest).cloned()
}

#[test]
fn the_capture_offers_its_new_quests_and_takes_one_by_search() {
    let (mut rt, _dir) = capture_review();
    open(&mut rt, "capture-item-t1-quest");
    let keys = rows(&rt);
    assert!(keys.contains(&"quest-picker-new-q1".to_owned()), "the proposal's new quest: {keys:?}");
    assert_eq!(keys.last().map(String::as_str), Some("quest-picker-none"));

    type_text(&mut rt, "tax");
    assert_eq!(rows(&rt), ["quest-picker-new-q1"]);
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(capture_quest(&rt, "t1"), Some(QuestRef::New { key: TempKey("q1".into()) }));
    settle(&mut rt);
    assert_eq!(focused(&rt).as_deref(), Some("capture-item-t1-quest"));
}

#[test]
fn the_capture_picker_chooses_an_existing_quest_with_the_arrows() {
    let (mut rt, _dir) = capture_review();
    let running = quest_id(&rt, "Run a half marathon");
    open(&mut rt, "capture-item-t1-quest");
    type_text(&mut rt, "run");
    press(&mut rt, Key::Down, Mods::default());
    press(&mut rt, Key::Up, Mods::default());
    press(&mut rt, Key::Enter, Mods::default());
    assert_eq!(capture_quest(&rt, "t1"), Some(QuestRef::Existing { id: running }));
}

#[test]
fn escape_and_no_quest_in_the_capture() {
    let (mut rt, _dir) = capture_review();
    let before = capture_quest(&rt, "t1");
    assert!(before.is_some(), "the demo guesses a quest");
    open(&mut rt, "capture-item-t1-quest");
    press(&mut rt, Key::Down, Mods::default());
    press(&mut rt, Key::Escape, Mods::default());
    settle(&mut rt);
    assert_eq!(capture_quest(&rt, "t1"), before, "Escape changes nothing");
    assert!(rt.state.capture.review().is_some(), "and the capture stays open");
    assert_eq!(focused(&rt).as_deref(), Some("capture-item-t1-quest"));

    open(&mut rt, "capture-item-t1-quest");
    click(&mut rt, "quest-picker-none");
    assert_eq!(capture_quest(&rt, "t1"), None);
}

mod fitting {
    use rune_kit::Placement;

    use crate::ui::quest_picker::{Fit, fit};

    const DIALOG: [f32; 4] = [100.0, 60.0, 800.0, 560.0];

    #[test]
    fn with_room_it_opens_full_size_under_the_chip() {
        let chip = [140.0, 100.0, 120.0, 30.0];
        assert_eq!(fit(chip, DIALOG), Fit { placement: Placement::BottomStart, width: 300.0, list_max_h: 288.0 });
    }

    #[test]
    fn near_the_right_edge_it_narrows_to_the_dialog() {
        let chip = [655.0, 300.0, 116.0, 30.0];
        let fitted = fit(chip, DIALOG);
        assert_eq!((fitted.placement, fitted.width), (Placement::BottomStart, 900.0 - 12.0 - 655.0));
        assert!(655.0 + fitted.width <= 900.0 - 12.0, "inside the dialog's right edge");
    }

    #[test]
    fn with_too_little_room_right_it_ends_at_the_chip() {
        let chip = [800.0, 100.0, 60.0, 30.0];
        let fitted = fit(chip, DIALOG);
        assert_eq!((fitted.placement, fitted.width), (Placement::BottomEnd, 300.0));
        assert!(860.0 - fitted.width >= 100.0 + 12.0, "inside the dialog's left edge");
    }

    #[test]
    fn near_the_bottom_it_opens_above_and_its_list_shrinks_to_the_room() {
        let chip = [140.0, 520.0, 120.0, 30.0];
        assert_eq!(fit(chip, DIALOG).placement, Placement::TopStart);

        // A short dialog: a little more room above, and the list only as tall as that.
        let short = [100.0, 60.0, 800.0, 300.0];
        let fitted = fit([140.0, 200.0, 120.0, 30.0], short);
        let above = 200.0 - 4.0 - (60.0 + 12.0);
        assert_eq!((fitted.placement, fitted.list_max_h), (Placement::TopStart, above - 41.0));
    }
}
