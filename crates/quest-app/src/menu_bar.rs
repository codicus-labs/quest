//! The macOS menu bar's View menu: the sidebar's views (⌘1–5), the tracked-time column (⌥⌘I),
//! the command bar (⌘K) and the shortcuts overview. The window's own key handling sees each
//! chord first (`keys_first`), so `keys::on_key` stays the one place that decides.

use rune::{App, Key, KeyEvent, MenuItem, Mods, Tx};

use crate::keys;
use crate::state::{Overlay, State, View};
use crate::ui::chrome::open_palette;

fn quick_field(s: &mut State, tx: &mut Tx, letter: char) {
    let ev = KeyEvent { key: Key::Command(letter), mods: Mods { cmd: true, shift: true, ..Mods::default() } };
    if crate::desktop::forward_panel_key(s, ev.clone()) {
        return;
    }
    keys::on_key(s, &ev, tx);
}

fn review_week(s: &mut State, tx: &mut Tx, key: &str, offset: i32) {
    if !crate::desktop::forward_panel_key(s, KeyEvent { key: Key::Text(key.into()), mods: Mods::default() }) {
        crate::ui::review::browse(s, offset, tx);
    }
}

pub fn install(app: App<State>) -> App<State> {
    let item = |title: &str, action: fn(&mut State, &mut Tx), chord: Option<&str>| {
        let mut item = MenuItem::new(title, action).menu("View").keys_first();
        if let Some(chord) = chord.and_then(|c| c.parse().ok()) {
            item = item.key_equivalent(chord);
        }
        item
    };
    app.menu_item(item("Inbox", |s, tx| s.go_with(View::Inbox, tx), Some("cmd+1")))
        .menu_item(item("Today", |s, tx| s.go_with(View::Today, tx), Some("cmd+2")))
        .menu_item(item("Plan", |s, tx| s.go_with(View::Plan, tx), Some("cmd+3")))
        .menu_item(item("Logbook", |s, tx| s.go_with(View::Logbook, tx), Some("cmd+4")))
        .menu_item(item("Review", |s, tx| s.go_with(View::Review, tx), Some("cmd+5")))
        .menu_item(item("Previous Review Week", |s, tx| review_week(s, tx, "[", -1), Some("[")))
        .menu_item(item("Next Review Week", |s, tx| review_week(s, tx, "]", 1), Some("]")))
        .menu_item(MenuItem::separator().menu("View"))
        .menu_item(item("Global Quick Add…", crate::desktop::quick_add, Some(crate::desktop::SHORTCUT)))
        .menu_item(item("Show Timer Widget", crate::desktop::show_widget, None))
        .menu_item(item("Recurring Tasks…", crate::ui::recurrence::open_overview, None))
        .menu_item(item("Tracked Time", |s, _| keys::toggle_tracked(s), Some("alt+cmd+i")))
        .menu_item(item("Command Bar", open_palette, Some("cmd+k")))
        .menu_item(item("Quick Add Quest", |s, tx| quick_field(s, tx, 'l'), Some("cmd+shift+l")))
        .menu_item(item("Quick Add Plan", |s, tx| quick_field(s, tx, 'p'), Some("cmd+shift+p")))
        .menu_item(item("Quick Add Deadline", |s, tx| quick_field(s, tx, 'd'), Some("cmd+shift+d")))
        .menu_item(item("Quick Add Markdown Note", |s, tx| quick_field(s, tx, 'n'), Some("cmd+shift+n")))
        .menu_item(item(
            "Jump to Today in Plan",
            |s, tx| {
                if s.view == View::Plan {
                    crate::ui::plan::jump_today(s, tx);
                }
            },
            Some("g"),
        ))
        .menu_item(item(
            "Move Task Up",
            |s, tx| {
                crate::ui::drag::nudge(s, true, tx);
            },
            Some("alt+up"),
        ))
        .menu_item(item(
            "Move Task Down",
            |s, tx| {
                crate::ui::drag::nudge(s, false, tx);
            },
            Some("alt+down"),
        ))
        .menu_item(item(
            "Edit Notes",
            |s, tx| {
                s.edit_notes(tx);
            },
            None,
        ))
        .menu_item(item("Back to Lists (Esc)", State::leave_focus, None))
        .menu_item(item("Stop Timer (Space)", |s, _| s.stop_focus(), None))
        .menu_item(item(
            "Done in Focus (X)",
            |s, tx| {
                if s.view == View::Focus
                    && let Some(task) = s.snap.running_entry().and_then(|e| e.task_id)
                {
                    s.complete_focus(task, tx);
                }
            },
            None,
        ))
        .menu_item(item(
            "Organize (O)",
            |s, tx| {
                s.organize(tx);
            },
            None,
        ))
        .menu_item(item(
            "Book as Organizing (B)",
            |s, _| {
                s.book_organizing();
            },
            None,
        ))
        .menu_item(item("Edit Task Repeat…", |s, tx| quick_field(s, tx, 'r'), Some("cmd+shift+r")))
        .menu_item(item(
            "Delete Task",
            |s, tx| {
                keys::delete_task(s, tx);
            },
            Some("cmd+backspace"),
        ))
        .menu_item(item("Undo Last Change", |s, _| s.undo_latest(), None))
        .menu_item(item("Keyboard Shortcuts", |s, _| s.overlay = Overlay::Shortcuts, None))
}
