//! The macOS menu bar's View menu: the sidebar's views (⌘1–5), the tracked-time column (⌥⌘I),
//! the command bar (⌘K) and the shortcuts overview. The window's own key handling sees each
//! chord first (`keys_first`), so `keys::on_key` stays the one place that decides.

use rune::{App, MenuItem, Tx};

use crate::keys;
use crate::state::{Overlay, State, View};
use crate::ui::chrome::open_palette;

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
        .menu_item(item("Review", |s, tx| s.go_with(View::Review, tx), Some("cmd+4")))
        .menu_item(item("Logbook", |s, tx| s.go_with(View::Logbook, tx), Some("cmd+5")))
        .menu_item(MenuItem::separator().menu("View"))
        .menu_item(item("Recurring Tasks…", crate::ui::recurrence::open_overview, None))
        .menu_item(item("Tracked Time", |s, _| keys::toggle_tracked(s), Some("alt+cmd+i")))
        .menu_item(item("Command Bar", open_palette, Some("cmd+k")))
        .menu_item(item(
            "Quick Add Quest",
            |s, tx| keys::quick_picker(s, crate::ui::task_fields::Picker::Quest, tx),
            Some("cmd+shift+l"),
        ))
        .menu_item(item(
            "Quick Add Plan",
            |s, tx| keys::quick_picker(s, crate::ui::task_fields::Picker::Date(quest_domain::DateField::Plan), tx),
            Some("cmd+shift+p"),
        ))
        .menu_item(item(
            "Quick Add Deadline",
            |s, tx| keys::quick_picker(s, crate::ui::task_fields::Picker::Date(quest_domain::DateField::Due), tx),
            Some("cmd+shift+d"),
        ))
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
        .menu_item(item("Undo Task Change", |s, _| s.undo_latest(), None))
        .menu_item(item("Keyboard Shortcuts", |s, _| s.overlay = Overlay::Shortcuts, None))
}
