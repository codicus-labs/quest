//! The keyboard shortcuts sheet (`?`): what `keys::shortcut` binds.

use rune::{Color, El, col, el, row, spacer, text};

use crate::state::{Overlay, State};
use crate::theme::{Pal, ty};
use crate::ui::chrome::keycap;
use crate::ui::parts;

type Group = (&'static str, &'static [(&'static [&'static str], &'static str)]);

const LEFT: [Group; 2] = [
    (
        "Go to",
        &[
            (&["⌘", "1"], "Inbox"),
            (&["⌘", "2"], "Today"),
            (&["⌘", "3"], "Plan"),
            (&["⌘", "4"], "Review"),
            (&["⌘", "5"], "Logbook"),
            (&["⌘", "6…"], "Quests"),
            (&["⌘", "K"], "Command bar"),
            (&["⌘", ","], "Settings"),
        ],
    ),
    (
        "View",
        &[
            (&["T"], "Tracked time"),
            (&["⌥", "⌘", "I"], "Tracked time"),
            (&["V"], "Non-quest tasks in Plan"),
            (&["?"], "Shortcuts"),
            (&["Esc"], "Back"),
        ],
    ),
];

const RIGHT: [Group; 1] = [(
    "Tasks",
    &[
        (&["J", "K"], "Next / previous"),
        (&["↓", "↑"], "Next / previous"),
        (&["↵"], "Open"),
        (&["X"], "Done / open"),
        (&["Space"], "Start / stop timer"),
        (&["F"], "Focus"),
        (&["N"], "New task"),
        (&["D"], "Next day"),
        (&["W"], "Next week"),
        (&["⌘", "⌫"], "Delete"),
        (&["⌘", "Z"], "Undo"),
    ],
)];

pub fn layer(p: &Pal, layer: &mut Vec<El>) {
    let header = row()
        .w_full()
        .center_y()
        .child(text("Keyboard shortcuts").font(rune::TextStyle::new(24.0, 500.0).lh(30.0)).color(p.text))
        .child(spacer());
    let columns = row().w_full().gap(48.0).mt(28.0).child(column(&LEFT, p)).child(column(&RIGHT, p));
    let panel = col()
        .key("shortcuts")
        .role("dialog")
        .label("Keyboard shortcuts")
        .w(760.0)
        .padding(28.0)
        .radius(14.0)
        .fill(p.card_raised)
        .border(1.0, p.border_strong)
        .shadow(20.0, 60.0, Color::BLACK.alpha(0.45))
        .child(header)
        .child(columns)
        .modal(true, |s: &mut State| s.overlay = Overlay::None);
    layer.push(el().key("shortcuts-scrim").absolute(0.0, 0.0).w_full().h_full().fill(p.scrim).a11y_hidden());
    layer.push(col().key("shortcuts-layer").absolute(0.0, 0.0).w_full().h_full().items_center().pt(60.0).child(panel));
}

/// A table per group: the keys, then what they do, one row each with a hairline under it.
fn column(groups: &[Group], p: &Pal) -> El {
    let mut c = col().grow(1.0).w(0.0);
    for (title, keys) in groups {
        c = c.child(parts::caps(*title, p).mt(14.0).mb(6.0));
        for (caps, what) in *keys {
            let mut combo = row().w(120.0).gap(4.0);
            for cap in *caps {
                combo = combo.child(keycap(cap, p));
            }
            c = c.child(
                row()
                    .role("row")
                    .w_full()
                    .h(32.0)
                    .center_y()
                    .border_sides([0.0, 0.0, 1.0, 0.0], p.border)
                    .child(combo)
                    .child(text(*what).font(ty::BODY).color(p.text)),
            );
        }
    }
    c
}
