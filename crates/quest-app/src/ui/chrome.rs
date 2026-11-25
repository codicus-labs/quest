//! Window chrome the views share: the account avatar with its sync dot, the search button that
//! opens the ⌘K palette, and keycaps.

use quest_store::SyncStatus;
use rune::{Color, Cursor, El, Tx, el, icon, row, text};

use crate::icons;
use crate::state::{Overlay, State};
use crate::theme::{Pal, ty};

/// Where a row in the window's top band starts: past the window's controls (`leading`, the
/// traffic lights) when it has them. Focus, which has no sidebar, starts its top row there.
pub fn leading_pad(leading: f32) -> f32 {
    if leading > 0.0 { leading + 16.0 } else { 22.0 }
}

/// The magnifier in the main column's top band: opens the ⌘K palette.
pub fn search_button(p: &Pal) -> El {
    el().key("search")
        .role("button")
        .label("Search or jump")
        .focusable(true)
        .size(32.0, 32.0)
        .center()
        .radius(8.0)
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .help("Search or jump  ⌘K")
        .child(icon(&icons::SEARCH, 16.0).color(p.faint))
        .on_press_tx(open_palette)
}

pub fn keycap(label: &str, p: &Pal) -> El {
    row()
        .h(22.0)
        .px(6.0)
        .center()
        .radius(5.0)
        .border(1.0, p.border_strong)
        .child(text(label).font(ty::SMALL).color(p.muted))
}

pub fn open_palette(s: &mut State, tx: &mut Tx) {
    open_palette_in(s, crate::palette::Mode::Commands, tx);
}

pub fn open_palette_in(s: &mut State, mode: crate::palette::Mode, tx: &mut Tx) {
    s.palette.mode = mode;
    s.palette.query.clear();
    s.palette.picked = 0;
    s.overlay = Overlay::Palette;
    tx.focus_when_shown("palette-input", false);
}

/// The sync state as the dot's colour and a short phrase.
pub fn sync_badge(status: &SyncStatus) -> (SyncTone, String) {
    match status {
        SyncStatus::SignedOut => (SyncTone::Idle, "Signed out · local only".into()),
        SyncStatus::Connecting => (SyncTone::Busy, "Connecting…".into()),
        SyncStatus::Live { pending: 0 } => (SyncTone::Good, "Live".into()),
        SyncStatus::Live { pending } => (SyncTone::Busy, format!("Live · sending {pending}")),
        SyncStatus::Offline { pending, retry_in } => {
            (SyncTone::Warn, format!("Offline · {pending} pending · retry in {}s", retry_in.as_secs()))
        }
        SyncStatus::Failed { error } => (SyncTone::Bad, error.to_string()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncTone {
    Idle,
    Busy,
    Good,
    Warn,
    Bad,
}

pub fn tone_color(tone: SyncTone, p: &Pal) -> Color {
    let code = match tone {
        SyncTone::Idle => return p.faint,
        SyncTone::Bad => return p.error,
        SyncTone::Busy => "#5c8fd9",
        SyncTone::Good => "#62b87a",
        SyncTone::Warn => "#e5b45a",
    };
    rune::color::hex(code).unwrap_or(p.faint)
}

pub fn avatar(s: &State, p: &Pal) -> El {
    let (tone, label) = sync_badge(&s.sync);
    let initial = s
        .account
        .as_ref()
        .and_then(|a| a.info.name.as_deref().or(a.info.email.as_deref()))
        .and_then(|name| name.chars().next())
        .map_or('Q', |c| c.to_ascii_uppercase());
    let pending = match s.sync {
        SyncStatus::Offline { pending, .. } if pending > 0 => Some(pending),
        _ => None,
    };
    let mut wrap = row().key("account-button").gap(8.0).center_y();
    if let Some(pending) = pending {
        wrap = wrap.child(text(format!("{pending} pending")).key("sync-pending").font(ty::SMALL).color(p.muted));
    }
    let face = el()
        .key("avatar")
        .role("button")
        .label(format!("Account – {label}"))
        .focusable(true)
        .size(32.0, 32.0)
        .radius(16.0)
        .center()
        .fill(p.side)
        .border(1.0, p.border_strong)
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .help(label)
        .child(text(initial.to_string()).font(ty::BODY_MEDIUM).color(p.text))
        .child(
            el().key("sync-dot")
                .absolute(23.0, 23.0)
                .size(10.0, 10.0)
                .radius(5.0)
                .fill(tone_color(tone, p))
                .border(2.0, p.side),
        )
        .on_press(|s: &mut State| s.overlay = Overlay::Account);
    wrap.child(face)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn sync_badges_say_what_is_going_on() {
        assert_eq!(sync_badge(&SyncStatus::SignedOut).0, SyncTone::Idle);
        assert_eq!(sync_badge(&SyncStatus::Live { pending: 0 }), (SyncTone::Good, "Live".into()));
        let (tone, label) = sync_badge(&SyncStatus::Offline { pending: 3, retry_in: Duration::from_secs(4) });
        assert_eq!(tone, SyncTone::Warn);
        assert!(label.contains("3 pending"));
    }
}
