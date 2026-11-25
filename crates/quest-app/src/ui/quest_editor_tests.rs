//! The Quests page and the quest dialog over it: cards open the dialog, Esc and Close return to
//! the page as it was (tab, scroll position, focus); completing and reopening move a quest
//! between the tabs with their counts and keep its hours, with Undo; New quest; and the archive,
//! the quest page and delete-undo stay reachable.

use quest_domain::{Command, EntityId, QuestColor, QuestStatus};
use rune::{ElementInfo, Key, Mods, Runtime};

use super::dnd_tests::rect;
use super::tests::{TempDir, click, demo_app, go, press, type_text};
use crate::state::{Overlay, QuestsTab, State, View};

fn quests_page() -> (Runtime<State>, TempDir) {
    let (mut rt, dir) = demo_app();
    go(&mut rt, View::Quests);
    settle(&mut rt);
    (rt, dir)
}

fn settle(rt: &mut Runtime<State>) {
    rt.frame(0.0);
    rt.settle_all();
    rt.frame(0.0);
}

fn element(rt: &Runtime<State>, key: &str) -> ElementInfo {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
        .unwrap_or_else(|| panic!("`{key}` is shown"))
}

fn shown(rt: &Runtime<State>, key: &str) -> String {
    let e = element(rt, key);
    e.text.or(e.label).unwrap_or_default()
}

/// Quest hours, In progress, Quests completed, and the two tabs' labels.
fn figures(rt: &Runtime<State>) -> [String; 5] {
    ["quests-stat-hours", "quests-stat-progress", "quests-stat-completed", "quests-tab-active", "quests-tab-completed"]
        .map(|key| shown(rt, key))
}

fn first_quest(rt: &Runtime<State>) -> EntityId {
    rt.state.snap.active_quests().next().map(|q| q.id).expect("a demo quest")
}

fn undo(rt: &mut Runtime<State>) {
    press(rt, Key::Command('z'), Mods { cmd: true, ..Mods::default() });
}

#[test]
fn the_page_shows_its_figures_tabs_and_cards_with_hours() {
    let (rt, _dir) = quests_page();
    let quest = first_quest(&rt);
    assert_eq!(figures(&rt)[1..], ["04", "00", "Active · 4", "Completed · 0"].map(String::from));
    assert!(shown(&rt, &format!("quest-hours-{quest}")).ends_with('h'), "this week's hours");
    assert!(shown(&rt, &format!("quest-total-{quest}")).ends_with("h total"));
    assert!(rt.find("quests-new").is_some() && rt.find("quests-new-card").is_some());
}

#[test]
fn a_card_opens_the_dialog_over_the_page_and_escape_returns_with_scroll_and_focus() {
    let (mut rt, _dir) = quests_page();
    // Enough quests that the page scrolls.
    for n in 0..6 {
        rt.state.dispatch(Command::CreateQuest {
            id: EntityId::new(),
            title: format!("Extra {n}"),
            color: QuestColor::Teal,
        });
    }
    settle(&mut rt);
    let last = rt.state.snap.active_quests().last().map(|q| q.id).unwrap();
    let card = format!("quest-{last}");
    let (x, y) = rt.find("quests-scroll").unwrap();
    rt.wheel(x, y, -600.0);
    settle(&mut rt);
    let scrolled = element(&rt, &card).scrolled;
    assert!(scrolled[1] > 0.0, "the page scrolled");

    click(&mut rt, &card);
    assert_eq!((rt.state.overlay, rt.state.view), (Overlay::Quest, View::Quests), "over the page");
    assert_eq!(rt.state.quest_editor.as_ref().and_then(|f| f.quest), Some(last));
    assert!(rt.find("quest-editor-title").is_some());
    press(&mut rt, Key::Escape, Mods::default());
    settle(&mut rt);
    assert_eq!((rt.state.overlay, rt.state.view), (Overlay::None, View::Quests));
    let card_now = element(&rt, &card);
    assert_eq!(card_now.scrolled, scrolled, "the scroll position stays");
    assert!(card_now.focused, "focus is back on the card");

    click(&mut rt, &card);
    click(&mut rt, "quest-editor-close");
    settle(&mut rt);
    assert_eq!(rt.state.overlay, Overlay::None);
    assert_eq!(element(&rt, &card).scrolled, scrolled);
    assert!(element(&rt, &card).focused, "Close also returns focus to the card");
}

#[test]
fn completing_moves_a_quest_to_completed_keeps_its_hours_and_undo_and_reopen_bring_it_back() {
    let (mut rt, _dir) = quests_page();
    let quest = first_quest(&rt);
    let card = format!("quest-{quest}");
    let before = figures(&rt);
    let (week, total) = (shown(&rt, &format!("quest-hours-{quest}")), shown(&rt, &format!("quest-total-{quest}")));

    click(&mut rt, &card);
    assert_eq!(shown(&rt, "quest-editor-status-action"), "Complete quest");
    click(&mut rt, "quest-editor-status-action");
    assert_eq!(rt.state.snap.quest(quest).unwrap().status, QuestStatus::Completed);
    assert_eq!(rt.state.overlay, Overlay::None, "the dialog closes");
    assert_eq!(figures(&rt)[0], before[0], "all-time hours keep the completed quest's time");
    assert_eq!(figures(&rt)[1..], ["03", "01", "Active · 3", "Completed · 1"].map(String::from));
    assert!(rt.find(&card).is_none(), "gone from Active");
    assert!(shown(&rt, "toast-message").starts_with("Completed quest"));

    undo(&mut rt);
    assert_eq!(rt.state.snap.quest(quest).unwrap().status, QuestStatus::Active, "⌘Z reopens it");
    assert_eq!(figures(&rt), before);

    click(&mut rt, &card);
    click(&mut rt, "quest-editor-status-action");
    click(&mut rt, "quests-tab-completed");
    settle(&mut rt);
    assert_eq!(rt.state.quests_tab, QuestsTab::Completed);
    assert_eq!(
        (shown(&rt, &format!("quest-hours-{quest}")), shown(&rt, &format!("quest-total-{quest}"))),
        (week.clone(), total.clone()),
        "its hours stay"
    );
    click(&mut rt, &card);
    assert_eq!(shown(&rt, "quest-editor-status-action"), "Reopen quest");
    click(&mut rt, "quest-editor-status-action");
    assert_eq!(rt.state.snap.quest(quest).unwrap().status, QuestStatus::Active);
    assert_eq!(rt.state.quests_tab, QuestsTab::Completed, "the tab stays");
    assert!(rt.find("quests-empty-completed").is_some(), "Completed's own empty state");
    assert_eq!(figures(&rt), before);
    click(&mut rt, "toast-action");
    assert_eq!(rt.state.snap.quest(quest).unwrap().status, QuestStatus::Completed, "the toast's Undo");
}

#[test]
fn new_quest_opens_the_dialog_and_return_creates_the_quest() {
    let (mut rt, _dir) = quests_page();
    let before = rt.state.snap.quests.len();
    // The dashed card comes after the quests, below the fold.
    let (x, y) = rt.find("quests-scroll").unwrap();
    rt.wheel(x, y, -600.0);
    settle(&mut rt);
    click(&mut rt, "quests-new-card");
    assert_eq!(rt.state.overlay, Overlay::Quest, "the dashed card opens the dialog");
    press(&mut rt, Key::Escape, Mods::default());
    assert_eq!((rt.state.overlay, rt.state.snap.quests.len()), (Overlay::None, before), "nothing without a title");
    rt.wheel(x, y, 600.0);
    settle(&mut rt);
    click(&mut rt, "quests-new");
    assert_eq!(rt.state.overlay, Overlay::Quest);
    assert!(element(&rt, "quest-editor-title").focused, "the title takes typing");
    type_text(&mut rt, "Learn the cello");
    assert_eq!(rt.state.snap.quests.len(), before, "nothing is created while typing");
    press(&mut rt, Key::Enter, Mods::default());
    click(&mut rt, "quest-editor-color-pink");
    let quest = rt.state.snap.quests.iter().find(|q| q.title == "Learn the cello").expect("created");
    assert_eq!((quest.color, quest.status), (QuestColor::Pink, QuestStatus::Active));
    let id = quest.id;
    assert_eq!(rt.state.quest_editor.as_ref().and_then(|f| f.quest), Some(id), "the dialog edits it now");
    press(&mut rt, Key::Escape, Mods::default());
    settle(&mut rt);
    assert!(rt.find(&format!("quest-{id}")).is_some(), "its card shows");
    assert_eq!(figures(&rt)[3], "Active · 5");
}

#[test]
fn archive_sits_right_in_the_tab_row_and_new_quest_stays_in_the_header() {
    let (rt, _dir) = quests_page();
    let [tabs_x, tabs_y, tabs_w, tabs_h] = rect(&rt, "quests-tabs");
    let [archive_x, archive_y, archive_w, archive_h] = rect(&rt, "quests-archive");
    let [content_x, _, content_w, _] = rect(&rt, "quests-scroll");
    let middle = |y: f32, h: f32| (y + h / 2.0).round();
    assert_eq!(middle(archive_y, archive_h), middle(tabs_y, tabs_h), "on the tabs' line");
    assert!(archive_x > tabs_x + tabs_w, "right of the tabs");
    assert_eq!((archive_x + archive_w).round(), (content_x + content_w - 30.0).round(), "at the right edge");
    assert!(rect(&rt, "quests-new")[1] < rect(&rt, "quests-stats")[1], "New quest stays above the figures");
    assert!(!rt.elements().iter().any(|e| e.text.as_deref() == Some("Small steps. Meaningful outcomes.")));
}

#[test]
fn archive_quest_page_and_delete_undo_stay_reachable_and_old_archives_are_not_completed() {
    let (mut rt, _dir) = quests_page();
    let quest = first_quest(&rt);
    let old = rt.state.snap.active_quests().nth(1).map(|q| q.id).unwrap();
    rt.state.dispatch(Command::SetQuestStatus { id: old, status: QuestStatus::Archived });
    settle(&mut rt);
    assert_eq!(figures(&rt)[1..], ["03", "00", "Active · 3", "Completed · 0"].map(String::from));
    assert_eq!(shown(&rt, "quests-archive"), "Archive", "no count");
    click(&mut rt, "quests-archive");
    assert_eq!(rt.state.view, View::Archive);
    assert!(rt.find(&format!("archive-quest-{old}")).is_some(), "the old archive is listed there");
    click(&mut rt, "archive-back");

    click(&mut rt, &format!("quest-{quest}"));
    click(&mut rt, "quest-editor-page");
    assert_eq!((rt.state.view, rt.state.overlay), (View::Quest(quest), Overlay::None), "the full quest page");
    assert!(rt.find("quick-add").is_some(), "with its tasks");
    go(&mut rt, View::Quests);

    click(&mut rt, &format!("quest-{quest}"));
    click(&mut rt, "quest-editor-delete");
    assert!(rt.state.snap.quest(quest).is_none());
    assert_eq!(rt.state.overlay, Overlay::None);
    undo(&mut rt);
    assert!(rt.state.snap.quest(quest).is_some(), "⌘Z brings it back");
}
