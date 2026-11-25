//! The keyboard shortcuts sheet (`?`): what `keys::shortcut` binds.

use rune::{Color, El, col, el, row, spacer, text};

use crate::state::{Overlay, State};
use crate::theme::{Pal, ty};
use crate::ui::parts;
use crate::ui::topbar::keycap;

type Group = (&'static str, &'static [(&'static [&'static str], &'static str)]);

const LEFT: [Group; 3] = [
    (
        "Navigate",
        &[
            (&["Tab"], "Cycle screens (⇧ Tab back)"),
            (&["H"], "Home"),
            (&["P"], "Plan"),
            (&["Q"], "Quests"),
            (&["R"], "Review"),
            (&["⌘", ","], "Settings"),
        ],
    ),
    ("Tasks", &[(&["N"], "New task"), (&["V"], "Show / hide non-quest")]),
    ("View", &[(&["⌘", "K"], "Command bar"), (&["⌘", "Z"], "Undo delete"), (&["?"], "This sheet")]),
];

const RIGHT: [Group; 2] = [
    (
        "Focus",
        &[
            (&["F"], "Focus screen"),
            (&["Space"], "Start / pause"),
            (&["⌘", "↵"], "Complete"),
            (&["Esc"], "Back to home"),
        ],
    ),
    (
        "Tasks (under cursor)",
        &[
            (&["Space"], "Start / pause"),
            (&["X"], "Complete / reopen"),
            (&["T"], "Move to today"),
            (&["D"], "Move to next day"),
            (&["W"], "Move to next week"),
            (&["⌘", "⌫"], "Delete"),
        ],
    ),
];

pub fn layer(p: &Pal, layer: &mut Vec<El>) {
    let header = row()
        .w_full()
        .center_y()
        .child(text("Keyboard shortcuts").font(rune::TextStyle::new(24.0, 500.0).lh(30.0)).color(p.text))
        .child(spacer())
        .child(parts::caps("Press ? anytime", p));
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

fn column(groups: &[Group], p: &Pal) -> El {
    let mut c = col().grow(1.0).gap(4.0);
    for (title, keys) in groups {
        c = c.child(parts::caps(*title, p).mt(14.0).mb(8.0));
        for (caps, what) in *keys {
            let mut combo = row().w(110.0).gap(4.0);
            for cap in *caps {
                combo = combo.child(keycap(cap, p));
            }
            c = c.child(row().h(30.0).center_y().child(combo).child(text(*what).font(ty::BODY).color(p.muted)));
        }
    }
    c
}
