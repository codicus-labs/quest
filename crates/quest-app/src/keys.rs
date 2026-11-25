//! Window shortcuts, as the shortcuts sheet (`?`) lists them: Tab / ⇧Tab and H P F Q R views,
//! ⌘K palette, ⌘, settings, N new task, V other work, and Space X T D W ⌘⌫ on the task under the
//! pointer (else the selected one; in Focus, its task). Also ⌘1–5, ⌘Z undo, Esc back and ⌫
//! deletes the selected task. And the keys a focused task card takes: the keyboard way to do
//! what dragging does.

use chrono::{Datelike, Days, NaiveDate};
use quest_domain::{Command, EntityId, Position};
use rune::{Key, KeyEvent, Tx};

use crate::state::{Overlay, State, View};
use crate::ui::topbar::open_palette;
use crate::undo::Deletable;

/// What a key press asks for, decided without touching state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    TogglePalette,
    Go(View),
    /// Tab / ⇧Tab: the next or previous view.
    Cycle {
        back: bool,
    },
    Settings,
    Sheet,
    NewTask,
    ToggleOther,
    Back,
    DeleteSelection,
    Undo,
    Task(TaskKey),
}

/// A shortcut on one task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskKey {
    /// Space: start its session, or pause it.
    StartPause,
    /// X: done, or open again.
    ToggleDone,
    /// ⌘↵ in Focus: done, session over, back Home. In Log work: save.
    Complete,
    /// T, D, W: today, the next day, next week.
    Move(MoveTo),
    /// ⌘⌫: delete, with an undo toast.
    Delete,
    /// ↵: the task's dialog (the card under the pointer only, in Focus too).
    Open,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveTo {
    Today,
    NextDay,
    NextWeek,
}

/// The day a task on `day` (None: backlog) moves to: the next day counts from its day, or
/// from today for a backlog task; next week is the coming Monday.
pub fn moved_day(to: MoveTo, day: Option<NaiveDate>, today: NaiveDate) -> NaiveDate {
    match to {
        MoveTo::Today => today,
        MoveTo::NextDay => day.unwrap_or(today) + Days::new(1),
        MoveTo::NextWeek => today + Days::new(u64::from(7 - today.weekday().num_days_from_monday())),
    }
}

/// What a focused task card does with a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKey {
    /// ⌥↑ / ⌥↓: one place up or down.
    Nudge { up: bool },
    /// ⌥← / ⌥→: to the previous or next day.
    Day { later: bool },
}

pub fn card_key(ev: &KeyEvent) -> Option<CardKey> {
    if !ev.mods.alt || ev.mods.cmd || ev.mods.ctrl {
        return None;
    }
    match ev.key {
        Key::Up => Some(CardKey::Nudge { up: true }),
        Key::Down => Some(CardKey::Nudge { up: false }),
        Key::Left => Some(CardKey::Day { later: false }),
        Key::Right => Some(CardKey::Day { later: true }),
        _ => None,
    }
}

/// Plain letters only arrive here when no text field took them (Rune's key routing gives a
/// focused field's typing to the field first).
pub fn shortcut(ev: &KeyEvent) -> Option<Shortcut> {
    let plain = !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt;
    match &ev.key {
        Key::Command('k') => Some(Shortcut::TogglePalette),
        Key::Command('z') if !ev.mods.shift => Some(Shortcut::Undo),
        Key::Command(',') => Some(Shortcut::Settings),
        Key::Command(c @ '1'..='5') => {
            let index = (*c as usize) - ('1' as usize);
            View::ALL.get(index).map(|v| Shortcut::Go(*v))
        }
        Key::Tab if plain => Some(Shortcut::Cycle { back: ev.mods.shift }),
        Key::Escape => Some(Shortcut::Back),
        Key::Enter if ev.mods.cmd => Some(Shortcut::Task(TaskKey::Complete)),
        Key::Enter if plain && !ev.mods.shift => Some(Shortcut::Task(TaskKey::Open)),
        Key::Backspace | Key::Delete if ev.mods.cmd => Some(Shortcut::Task(TaskKey::Delete)),
        Key::Backspace | Key::Delete => Some(Shortcut::DeleteSelection),
        Key::Text(t) if plain => match t.to_lowercase().as_str() {
            "h" => Some(Shortcut::Go(View::Home)),
            "p" => Some(Shortcut::Go(View::Plan)),
            "f" => Some(Shortcut::Go(View::Focus)),
            "q" => Some(Shortcut::Go(View::Quests)),
            "r" => Some(Shortcut::Go(View::Review)),
            "?" => Some(Shortcut::Sheet),
            "n" => Some(Shortcut::NewTask),
            "v" => Some(Shortcut::ToggleOther),
            " " => Some(Shortcut::Task(TaskKey::StartPause)),
            "x" => Some(Shortcut::Task(TaskKey::ToggleDone)),
            "t" => Some(Shortcut::Task(TaskKey::Move(MoveTo::Today))),
            "d" => Some(Shortcut::Task(TaskKey::Move(MoveTo::NextDay))),
            "w" => Some(Shortcut::Task(TaskKey::Move(MoveTo::NextWeek))),
            _ => None,
        },
        _ => None,
    }
}

pub fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let Some(shortcut) = shortcut(ev) else { return false };
    // With a dialog or sheet open, only the keys that close it act.
    if s.overlay == Overlay::LogWork && shortcut == Shortcut::Task(TaskKey::Complete) {
        s.save_log_work();
        return true;
    }
    // Signing out: nothing may change the account's data or leave its dialog until it is done.
    if s.auth.locks_edits() {
        return true;
    }
    let overlay_keys = matches!(shortcut, Shortcut::TogglePalette | Shortcut::Back | Shortcut::Sheet);
    if s.overlay != Overlay::None && !overlay_keys {
        return false;
    }
    match shortcut {
        Shortcut::TogglePalette if s.overlay == Overlay::Palette => s.overlay = Overlay::None,
        Shortcut::TogglePalette => {
            s.close_task_modal();
            open_palette(s, tx);
        }
        Shortcut::Go(view) => s.go_with(view, tx),
        Shortcut::Cycle { back } => s.go_with(s.view.cycled(back), tx),
        Shortcut::Settings => s.overlay = Overlay::Account,
        Shortcut::Sheet if s.overlay == Overlay::Shortcuts => s.overlay = Overlay::None,
        Shortcut::Sheet if s.overlay == Overlay::None => s.overlay = Overlay::Shortcuts,
        Shortcut::Sheet => return false,
        Shortcut::NewTask => {
            s.go_with(View::Home, tx);
            tx.focus_when_shown("home-add", true);
        }
        Shortcut::ToggleOther => s.show_other = !s.show_other,
        Shortcut::Back if s.date_picker.is_some() => s.date_picker = None,
        Shortcut::Back if s.overlay == Overlay::LogWork => s.close_log_work(),
        Shortcut::Back if s.overlay == Overlay::RepeatEditor => s.close_repeat_editor(),
        Shortcut::Back if s.overlay == Overlay::Capture => s.capture_back(),
        Shortcut::Back if s.overlay == Overlay::GuestImport => s.keep_guest_data(),
        Shortcut::Back if s.overlay == Overlay::AiNotice => s.decline_ai_notice(),
        Shortcut::Back if s.overlay == Overlay::Task => {
            tx.blur();
            s.close_task_modal();
        }
        Shortcut::Back if s.overlay == Overlay::Quest => {
            tx.blur();
            s.close_quest_editor();
        }
        Shortcut::Back if s.overlay != Overlay::None => s.overlay = Overlay::None,
        Shortcut::Back if s.view == View::Focus => s.go_with(View::Home, tx),
        Shortcut::Back if s.menu.is_some() => s.menu = None,
        Shortcut::Back if matches!(s.view, View::Archive | View::Quest(_)) => s.go(View::Quests),
        Shortcut::Back => return false,
        Shortcut::DeleteSelection => {
            let Some(task) = s.shortcut_task() else { return false };
            tx.blur();
            s.delete(Deletable::Task(task));
        }
        Shortcut::Undo => s.undo_latest(),
        Shortcut::Task(key) => {
            let target = if s.view == View::Focus && key != TaskKey::Open {
                s.focus_subject().map(|t| t.id)
            } else {
                s.shortcut_task()
            };
            let Some(task) = target else { return false };
            on_task_key(s, task, key, tx);
        }
    }
    true
}

fn on_task_key(s: &mut State, task: EntityId, key: TaskKey, tx: &mut Tx) {
    match key {
        TaskKey::StartPause => s.toggle_focus(task),
        TaskKey::ToggleDone => s.toggle_done(task),
        TaskKey::Complete if s.view == View::Focus => s.complete_focus(task, tx),
        TaskKey::Complete => {}
        TaskKey::Open => s.open_task_modal(task),
        TaskKey::Move(to) => {
            let current = s.snap.task(task).and_then(|t| t.planned_on);
            let day = moved_day(to, current, s.today());
            let at = s.clock.now();
            s.dispatch(Command::MoveTask { id: task, planned_on: Some(day), position: Position::Bottom, at });
        }
        TaskKey::Delete => {
            tx.blur();
            s.hovered_task = None;
            s.delete(Deletable::Task(task));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::Mods;

    fn key(key: Key) -> KeyEvent {
        KeyEvent { key, mods: Mods::default() }
    }

    fn text(t: &str) -> KeyEvent {
        key(Key::Text(t.into()))
    }

    #[test]
    fn command_digits_pick_views_in_tab_order() {
        assert_eq!(shortcut(&key(Key::Command('1'))), Some(Shortcut::Go(View::Home)));
        assert_eq!(shortcut(&key(Key::Command('5'))), Some(Shortcut::Go(View::Review)));
        assert_eq!(shortcut(&key(Key::Command('6'))), None);
    }

    #[test]
    fn letters_jump_to_views_and_tab_cycles() {
        assert_eq!(shortcut(&text("h")), Some(Shortcut::Go(View::Home)));
        assert_eq!(shortcut(&text("P")), Some(Shortcut::Go(View::Plan)));
        assert_eq!(shortcut(&text("f")), Some(Shortcut::Go(View::Focus)));
        assert_eq!(shortcut(&key(Key::Tab)), Some(Shortcut::Cycle { back: false }));
        let shift_tab = KeyEvent { key: Key::Tab, mods: Mods { shift: true, ..Mods::default() } };
        assert_eq!(shortcut(&shift_tab), Some(Shortcut::Cycle { back: true }));
        assert_eq!(View::Home.cycled(true), View::Review);
        assert_eq!(View::Review.cycled(false), View::Home);
    }

    #[test]
    fn letters_create_toggle_and_act_on_tasks() {
        assert_eq!(shortcut(&text("n")), Some(Shortcut::NewTask));
        assert_eq!(shortcut(&text("v")), Some(Shortcut::ToggleOther));
        assert_eq!(shortcut(&text("?")), Some(Shortcut::Sheet));
        assert_eq!(shortcut(&text(" ")), Some(Shortcut::Task(TaskKey::StartPause)));
        assert_eq!(shortcut(&text("x")), Some(Shortcut::Task(TaskKey::ToggleDone)));
        assert_eq!(shortcut(&text("w")), Some(Shortcut::Task(TaskKey::Move(MoveTo::NextWeek))));
        let cmd = Mods { cmd: true, ..Mods::default() };
        assert_eq!(shortcut(&KeyEvent { key: Key::Backspace, mods: cmd }), Some(Shortcut::Task(TaskKey::Delete)));
        assert_eq!(shortcut(&KeyEvent { key: Key::Enter, mods: cmd }), Some(Shortcut::Task(TaskKey::Complete)));
        assert_eq!(shortcut(&KeyEvent { key: Key::Enter, mods: Mods::default() }), Some(Shortcut::Task(TaskKey::Open)));
    }

    #[test]
    fn command_k_escape_and_settings() {
        assert_eq!(shortcut(&key(Key::Command('k'))), Some(Shortcut::TogglePalette));
        assert_eq!(shortcut(&key(Key::Command(','))), Some(Shortcut::Settings));
        assert_eq!(shortcut(&key(Key::Escape)), Some(Shortcut::Back));
        assert_eq!(shortcut(&key(Key::Command('z'))), Some(Shortcut::Undo));
    }

    #[test]
    fn moves_count_from_the_task_and_week_from_today() {
        let thursday = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        let saturday = NaiveDate::from_ymd_opt(2026, 10, 17).unwrap();
        let monday = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        assert_eq!(moved_day(MoveTo::Today, Some(saturday), thursday), thursday);
        assert_eq!(moved_day(MoveTo::NextDay, Some(saturday), thursday), saturday + Days::new(1));
        assert_eq!(moved_day(MoveTo::NextDay, None, thursday), thursday + Days::new(1));
        assert_eq!(moved_day(MoveTo::NextWeek, None, thursday), monday);
        assert_eq!(moved_day(MoveTo::NextWeek, None, monday), monday + Days::new(7));
    }
    fn with(key: Key, alt: bool, shift: bool) -> KeyEvent {
        KeyEvent { key, mods: Mods { alt, shift, ..Mods::default() } }
    }

    #[test]
    fn option_arrows_move_cards() {
        assert_eq!(card_key(&with(Key::Up, true, false)), Some(CardKey::Nudge { up: true }));
        assert_eq!(card_key(&with(Key::Right, true, false)), Some(CardKey::Day { later: true }));
        assert_eq!(card_key(&with(Key::Up, false, false)), None, "plain arrows stay focus navigation");
    }
}
