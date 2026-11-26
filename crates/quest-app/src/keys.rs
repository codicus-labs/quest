//! Window shortcuts, as the shortcuts sheet (`?`) lists them: ⌘1–5 the sidebar's views and ⌘6…
//! its quests, J/K and ↑/↓ move the selection, ↵ opens it, Space X L P D W ⌘⌫ act on it (else on the
//! task under the pointer; in Focus, its task), F Focus, T and ⌥⌘I the tracked column, N quick
//! add, V other work, ⌘K palette, ⌘, settings, ⌘Z undo, Esc back and ⌫ deletes. Single keys only
//! arrive without text focus. And the keys a focused task card takes: the keyboard way to do what
//! dragging does.

use quest_domain::{Command, EntityId, Position};
use rune::{Key, KeyEvent, Tx};

use crate::state::{Overlay, State, View};
use crate::ui::chrome::open_palette;
use crate::ui::task_fields::{self, Picker};
use crate::undo::Deletable;

/// What a key press asks for, decided without touching state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    TogglePalette,
    Go(View),
    /// ⌘ and a digit past the sidebar's views: the active quest at this index.
    GoQuest(usize),
    Settings,
    Sheet,
    NewTask,
    /// T, ⌥⌘I: show or hide the tracked-time column.
    ToggleTracked,
    /// J / ↓ and K / ↑: the next or previous row.
    JumpToday,
    ReviewWeek(i32),
    Nudge {
        up: bool,
    },
    Select {
        next: bool,
    },
    Back,
    DeleteSelection,
    DeleteTask,
    Undo,
    EditNotes,
    Repeat,
    Organize,
    BookOrganizing,
    Pick(Picker),
    QuickPick(Picker),
    Task(TaskKey),
}

/// A shortcut on one task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskKey {
    /// Space: start or stop its session.
    StartPause,
    /// X: done, or open again.
    ToggleDone,
    /// ⌘↵ in Focus: done, session over, back Home. In Log work: save.
    Complete,
    MoveToNextMonday,
    /// ↵: expand or collapse the selected row or the card under the pointer.
    Open,
}

/// Plain letters only arrive here when no text field took them (Rune's key routing gives a
/// focused field's typing to the field first).
pub fn shortcut(ev: &KeyEvent) -> Option<Shortcut> {
    let plain = !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt;
    let reorder = ev.mods.alt && !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.shift;
    if let Key::Command(letter) = ev.key
        && ev.mods.shift
        && !ev.mods.alt
        && !ev.mods.ctrl
    {
        if letter.eq_ignore_ascii_case(&'r') {
            return Some(Shortcut::Repeat);
        }
        if let Some(picker) = Picker::from_letter(letter) {
            return Some(Shortcut::QuickPick(picker));
        }
    }
    match &ev.key {
        Key::Command('k') => Some(Shortcut::TogglePalette),
        Key::Command('z') if !ev.mods.shift => Some(Shortcut::Undo),
        Key::Command(',') => Some(Shortcut::Settings),
        // ⌥⌘I; with ⌥ held a layout may report the dead key's character instead.
        Key::Command('i' | 'ˆ') if ev.mods.alt => Some(Shortcut::ToggleTracked),
        Key::Command(c @ '1'..='9') if !ev.mods.alt && !ev.mods.shift => {
            let index = (*c as usize) - ('1' as usize);
            Some(match View::SIDEBAR.get(index) {
                Some(view) => Shortcut::Go(*view),
                None => Shortcut::GoQuest(index - View::SIDEBAR.len()),
            })
        }
        Key::Up if reorder => Some(Shortcut::Nudge { up: true }),
        Key::Down if reorder => Some(Shortcut::Nudge { up: false }),
        Key::Down if plain => Some(Shortcut::Select { next: true }),
        Key::Up if plain => Some(Shortcut::Select { next: false }),
        Key::Escape => Some(Shortcut::Back),
        Key::Enter if ev.mods.cmd => Some(Shortcut::Task(TaskKey::Complete)),
        Key::Enter if plain && !ev.mods.shift => Some(Shortcut::Task(TaskKey::Open)),
        Key::Backspace | Key::Delete if ev.mods.cmd => Some(Shortcut::DeleteTask),
        Key::Backspace | Key::Delete => Some(Shortcut::DeleteSelection),
        Key::Text(t) if plain => match t.to_lowercase().as_str() {
            "f" => Some(Shortcut::Go(View::Focus)),
            "o" => Some(Shortcut::Organize),
            "b" => Some(Shortcut::BookOrganizing),
            "e" if !ev.mods.shift => Some(Shortcut::EditNotes),
            "r" if !ev.mods.shift => Some(Shortcut::Repeat),
            "g" => Some(Shortcut::JumpToday),
            "[" => Some(Shortcut::ReviewWeek(-1)),
            "]" => Some(Shortcut::ReviewWeek(1)),
            "j" => Some(Shortcut::Select { next: true }),
            "k" => Some(Shortcut::Select { next: false }),
            "t" => Some(Shortcut::ToggleTracked),
            "?" => Some(Shortcut::Sheet),
            "n" => Some(Shortcut::NewTask),
            " " => Some(Shortcut::Task(TaskKey::StartPause)),
            "x" => Some(Shortcut::Task(TaskKey::ToggleDone)),
            "l" | "p" | "d" => t.chars().next().and_then(Picker::from_letter).map(Shortcut::Pick),
            "w" => Some(Shortcut::Task(TaskKey::MoveToNextMonday)),
            _ => None,
        },
        _ => None,
    }
}

pub fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if ev.key == Key::Function(12)
        && let crate::clock::Clock::Fixed(at) = s.clock
    {
        s.clock = crate::clock::Clock::Fixed(at + chrono::TimeDelta::minutes(2));
        return true;
    }
    let Some(shortcut) = shortcut(ev) else { return false };
    if s.view == View::Focus
        && tx.focused().is_some_and(|key| key.starts_with("checklist-"))
        && matches!(
            shortcut,
            Shortcut::Back
                | Shortcut::Organize
                | Shortcut::BookOrganizing
                | Shortcut::Task(TaskKey::StartPause | TaskKey::ToggleDone)
        )
    {
        return true;
    }
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
        Shortcut::Go(View::Focus) if s.snap.running_entry().is_none() && s.shortcut_task().is_some() => {
            if let Some(task) = s.shortcut_task() {
                s.start_focus(task);
            }
        }
        Shortcut::Go(view) => s.go_with(view, tx),
        Shortcut::Organize => return s.organize(tx),
        Shortcut::BookOrganizing => return s.book_organizing(),
        Shortcut::GoQuest(index) => {
            let Some(quest) = s.snap.active_quests().nth(index).map(|q| q.id) else { return false };
            s.go_with(View::Quest(quest), tx);
        }
        Shortcut::ToggleTracked => toggle_tracked(s),
        Shortcut::JumpToday if s.view == View::Plan => crate::ui::plan::jump_today(s, tx),
        Shortcut::JumpToday => return false,
        Shortcut::ReviewWeek(offset) if s.view == View::Review => crate::ui::review::browse(s, offset, tx),
        Shortcut::ReviewWeek(_) => return false,
        Shortcut::Nudge { up } => return crate::ui::drag::nudge(s, up, tx),
        Shortcut::Select { next } => {
            if !s.select_next_row(next) {
                return false;
            }
            if let Some(task) = s.selected_task {
                tx.scroll_into_view(format!("row-{task}"));
            }
        }
        Shortcut::Settings => s.overlay = Overlay::Account,
        Shortcut::Sheet if s.overlay == Overlay::Shortcuts => s.overlay = Overlay::None,
        Shortcut::Sheet if s.overlay == Overlay::None => s.overlay = Overlay::Shortcuts,
        Shortcut::Sheet => return false,
        Shortcut::NewTask => {
            if s.view == View::Plan {
                crate::ui::plan::open_add(s, s.today(), tx);
                return true;
            }
            if !matches!(s.view, View::Today | View::Inbox | View::Quest(_)) {
                s.go_with(View::Today, tx);
            }
            tx.focus_when_shown("quick-add", true);
        }
        Shortcut::Back if s.task_calendar.is_some() => s.close_calendar(tx),
        Shortcut::Back if s.date_picker.is_some() => s.date_picker = None,
        Shortcut::Back if s.overlay == Overlay::LogWork => s.close_log_work(),
        Shortcut::Back if s.overlay == Overlay::RepeatEditor => s.close_repeat_editor(),
        Shortcut::Back if s.overlay == Overlay::Capture => s.capture_back(),
        Shortcut::Back if s.overlay == Overlay::GuestImport => s.keep_guest_data(),
        Shortcut::Back if s.overlay == Overlay::AiNotice => s.decline_ai_notice(),
        Shortcut::Back if s.overlay == Overlay::Quest => {
            tx.blur();
            s.close_quest_editor();
        }
        Shortcut::Back if s.overlay == Overlay::EntryEditor => s.close_entry_editor(),
        Shortcut::Back if s.overlay != Overlay::None => s.overlay = Overlay::None,
        Shortcut::Back if s.view == View::Focus => s.leave_focus(tx),
        Shortcut::Back if s.task_modal.is_some() => {
            tx.blur();
            s.close_task_modal();
            s.selected_task = None;
        }
        Shortcut::Back if s.selected_task.is_some() => s.selected_task = None,
        Shortcut::Back if s.menu.is_some() => s.menu = None,
        Shortcut::Back if matches!(s.view, View::Archive | View::Quest(_)) => s.go(View::Quests),
        Shortcut::Back => return false,
        Shortcut::DeleteSelection => {
            let Some(task) = s.shortcut_task() else { return false };
            tx.blur();
            s.delete(Deletable::Task(task));
        }
        Shortcut::Undo => s.undo_latest(),
        Shortcut::EditNotes => return s.edit_notes(tx),
        Shortcut::Repeat => return repeat(s, tx),
        Shortcut::DeleteTask => return delete_task(s, tx),
        Shortcut::Pick(picker) => {
            let Some(task) = s.shortcut_task() else { return false };
            task_fields::open(s, Some(task), picker, tx);
        }
        Shortcut::QuickPick(picker) => quick_picker(s, picker, tx),
        Shortcut::Task(key) => {
            let target = if s.view == View::Focus && key != TaskKey::Open {
                s.snap.running_entry().and_then(|e| e.task_id)
            } else {
                s.shortcut_task()
            };
            let Some(task) = target else { return false };
            on_task_key(s, task, key, tx);
        }
    }
    true
}

pub fn quick_picker(s: &mut State, picker: Picker, tx: &mut Tx) {
    if s.quick_hosted() {
        task_fields::open(s, None, picker, tx);
    }
}

pub fn repeat(s: &mut State, tx: &mut Tx) -> bool {
    if s.overlay != Overlay::None || s.auth.locks_edits() {
        return false;
    }
    let Some(task) = s.task_modal.as_ref().map(|form| form.task).or_else(|| s.shortcut_task()) else {
        return false;
    };
    tx.blur();
    s.open_repeat_editor(task);
    true
}

pub fn delete_task(s: &mut State, tx: &mut Tx) -> bool {
    if s.overlay != Overlay::None
        || s.auth.locks_edits()
        || matches!(tx.focused(), Some("task-modal-title" | "focus-title" | "quick-add" | "quick-add-note"))
    {
        return false;
    }
    let target = if s.view == View::Focus {
        s.snap.running_entry().and_then(|e| e.task_id)
    } else {
        s.task_modal.as_ref().map(|form| form.task).or_else(|| s.shortcut_task())
    };
    let Some(task) = target else { return false };
    tx.blur();
    s.hovered_task = None;
    s.selected_task = None;
    s.delete(Deletable::Task(task));
    true
}

pub fn toggle_tracked(s: &mut State) {
    if s.view != View::Review {
        s.right_column = !s.right_column;
    }
}

fn on_task_key(s: &mut State, task: EntityId, key: TaskKey, tx: &mut Tx) {
    match key {
        TaskKey::StartPause => s.toggle_focus(task),
        TaskKey::ToggleDone | TaskKey::Complete if s.view == View::Focus => s.complete_focus(task, tx),
        TaskKey::ToggleDone => s.toggle_done(task),
        TaskKey::Complete => {}
        TaskKey::Open => {
            s.toggle_task_detail(task);
            tx.scroll_into_view("task-modal");
        }
        TaskKey::MoveToNextMonday => {
            let planned_on = quest_view::calendar::QuickDate::NextMonday.date(s.today());
            let at = s.clock.now();
            s.dispatch(Command::MoveTask { id: task, planned_on, position: Position::Bottom, at });
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
    fn command_digits_pick_the_sidebars_views_then_its_quests() {
        assert_eq!(shortcut(&key(Key::Command('1'))), Some(Shortcut::Go(View::Inbox)));
        assert_eq!(shortcut(&key(Key::Command('2'))), Some(Shortcut::Go(View::Today)));
        assert_eq!(shortcut(&key(Key::Command('4'))), Some(Shortcut::Go(View::Logbook)));
        assert_eq!(shortcut(&key(Key::Command('5'))), Some(Shortcut::Go(View::Review)));
        assert_eq!(shortcut(&key(Key::Command('6'))), Some(Shortcut::GoQuest(0)));
        assert_eq!(shortcut(&key(Key::Command('9'))), Some(Shortcut::GoQuest(3)));
    }

    #[test]
    fn letters_and_arrows_select_focus_and_toggle_the_tracked_column() {
        assert_eq!(shortcut(&text("j")), Some(Shortcut::Select { next: true }));
        assert_eq!(shortcut(&text("K")), Some(Shortcut::Select { next: false }));
        assert_eq!(shortcut(&key(Key::Down)), Some(Shortcut::Select { next: true }));
        assert_eq!(shortcut(&key(Key::Up)), Some(Shortcut::Select { next: false }));
        assert_eq!(shortcut(&text("f")), Some(Shortcut::Go(View::Focus)));
        assert_eq!(shortcut(&text("t")), Some(Shortcut::ToggleTracked));
        let option_command_i =
            KeyEvent { key: Key::Command('i'), mods: Mods { alt: true, cmd: true, ..Mods::default() } };
        assert_eq!(shortcut(&option_command_i), Some(Shortcut::ToggleTracked));
        assert_eq!(shortcut(&key(Key::Tab)), None, "Tab stays focus navigation");
        assert_eq!(shortcut(&text("h")), None);
    }

    #[test]
    fn letters_create_toggle_and_act_on_tasks() {
        assert_eq!(shortcut(&text("n")), Some(Shortcut::NewTask));
        assert_eq!(shortcut(&text("g")), Some(Shortcut::JumpToday));
        assert_eq!(shortcut(&text("?")), Some(Shortcut::Sheet));
        assert_eq!(shortcut(&text(" ")), Some(Shortcut::Task(TaskKey::StartPause)));
        assert_eq!(shortcut(&text("x")), Some(Shortcut::Task(TaskKey::ToggleDone)));
        assert_eq!(shortcut(&text("w")), Some(Shortcut::Task(TaskKey::MoveToNextMonday)));
        let cmd = Mods { cmd: true, ..Mods::default() };
        assert_eq!(shortcut(&KeyEvent { key: Key::Backspace, mods: cmd }), Some(Shortcut::DeleteTask));
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
    fn option_arrows_sort_tasks() {
        let with_alt = |key| KeyEvent { key, mods: Mods { alt: true, ..Mods::default() } };
        assert_eq!(shortcut(&with_alt(Key::Up)), Some(Shortcut::Nudge { up: true }));
        assert_eq!(shortcut(&with_alt(Key::Down)), Some(Shortcut::Nudge { up: false }));
    }
}
