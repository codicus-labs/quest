//! The top bar: logo, workspace switcher, view tabs, a timer running in another workspace, the
//! "Search or jump…" button and the account avatar with its sync dot.

use quest_store::SyncStatus;
use rune::{Color, Cursor, Cx, El, Tx, el, icon, row, spacer, text};

use crate::icons;
use crate::state::{Overlay, State, View};
use crate::theme::{Pal, ty};
use crate::ui::workspaces;

/// The bar is the window's titlebar band too.
pub const HEIGHT: f32 = 56.0;

/// The least room between the last tab and what follows it (the timer pill or the search).
const TABS_ROOM: f32 = 20.0;

/// The bar's left padding: past the window's controls (`leading`, the traffic lights) when it has
/// them. Focus, which hides the bar, starts its own top row there too.
pub fn leading_pad(leading: f32) -> f32 {
    if leading > 0.0 { leading + 16.0 } else { 22.0 }
}

/// The search field's least width: its text and ⌘K whole. With less room it is a magnifier.
const SEARCH_MIN_W: f32 = 210.0;

/// Room after the timer pill and after the search.
const ITEM_GAP: f32 = 14.0;

pub fn topbar(s: &State, p: &Pal, cx: &Cx<State>) -> El {
    let leading = cx.chrome_insets().leading;
    let logo = row()
        .gap(10.0)
        .center_y()
        .mr(14.0)
        .child(icon(&icons::LOGO, 20.0).color(p.text))
        .child(text("Quest").font(ty::NAV.lh(22.0)).color(p.text));
    let mut tabs = row().gap(4.0).center_y();
    for view in View::ALL {
        let on = s.view.tab() == view;
        tabs = tabs.child(
            row()
                .key(format!("tab-{}", view.title().to_lowercase()))
                .role("tab")
                .label(view.title())
                .checked(on)
                .focusable(true)
                .h(36.0)
                .px(14.0)
                .center_y()
                .radius(7.0)
                .cursor(Cursor::Pointer)
                .fill(if on { p.selected } else { Color::TRANSPARENT })
                .hover(p.hover)
                .child(text(view.title()).font(ty::NAV).color(if on { p.text } else { p.muted }))
                .on_press_tx(move |s: &mut State, tx: &mut Tx| s.go_with(view, tx)),
        );
    }
    let search = if search_fits(cx, leading) { search_field(p) } else { search_icon(p) };
    let mut bar = row()
        .key("topbar")
        .w_full()
        .h(HEIGHT)
        .pl(leading_pad(leading))
        .pr(22.0)
        .window_drag()
        .center_y()
        .fill(p.bar)
        .child(row().key("topbar-lead").center_y().child(logo).child(workspaces::switcher(s, p).mr(14.0)).child(tabs))
        .child(el().w(TABS_ROOM))
        .child(spacer());
    if let Some(note) = workspaces::timer_note(s, p) {
        bar = bar.child(note).child(el().w(ITEM_GAP));
    }
    bar.child(search).child(el().w(ITEM_GAP)).child(avatar(s, p))
}

/// Whether the whole search field fits beside the rest of the bar. Reads only what keeps its
/// size whichever way the search shows, so the choice settles in one frame.
fn search_fits(cx: &Cx<State>, leading: f32) -> bool {
    let width = |key| cx.layout_size(key).map_or(0.0, |[w, _]| w);
    let Some([bar_w, _]) = cx.layout_size("topbar") else { return true };
    let pill = cx.layout_size("timer-elsewhere").map_or(0.0, |[w, _]| w + ITEM_GAP);
    let taken =
        leading_pad(leading) + 22.0 + width("topbar-lead") + TABS_ROOM + pill + ITEM_GAP + width("topbar-account");
    bar_w - taken >= SEARCH_MIN_W
}

fn search_field(p: &Pal) -> El {
    row()
        .key("search")
        .role("button")
        .label("Search or jump")
        .focusable(true)
        .w(330.0)
        .min_w(SEARCH_MIN_W)
        .shrink()
        .h(36.0)
        .pl(12.0)
        .pr(6.0)
        .gap(10.0)
        .center_y()
        .radius(8.0)
        .fill(p.bar)
        .border(1.0, p.border_strong)
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .child(icon(&icons::SEARCH, 16.0).color(p.muted))
        .child(text("Search or jump…").font(ty::BODY).color(p.muted).grow(1.0).single_line())
        .child(keycap("⌘K", p))
        .on_press_tx(open_palette)
}

/// The search where the whole field doesn't fit: a magnifier that opens the same palette.
fn search_icon(p: &Pal) -> El {
    row()
        .key("search")
        .role("button")
        .label("Search or jump")
        .focusable(true)
        .size(36.0, 36.0)
        .center()
        .radius(8.0)
        .fill(p.bar)
        .border(1.0, p.border_strong)
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .help("Search or jump  ⌘K")
        .child(icon(&icons::SEARCH, 16.0).color(p.muted))
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

fn avatar(s: &State, p: &Pal) -> El {
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
    let mut wrap = row().key("topbar-account").gap(8.0).center_y();
    if let Some(pending) = pending {
        wrap = wrap.child(text(format!("{pending} pending")).key("sync-pending").font(ty::SMALL).color(p.muted));
    }
    let face = el()
        .key("avatar")
        .role("button")
        .label(format!("Account – {label}"))
        .focusable(true)
        .size(38.0, 38.0)
        .radius(19.0)
        .center()
        .fill(p.bar)
        .border(1.0, p.border_strong)
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .help(label)
        .child(text(initial.to_string()).font(ty::BODY_MEDIUM).color(p.text))
        .child(
            el().key("sync-dot")
                .absolute(28.0, 28.0)
                .size(10.0, 10.0)
                .radius(5.0)
                .fill(tone_color(tone, p))
                .border(2.0, p.bar),
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
