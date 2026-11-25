//! Small pieces the views share: chips, checks, buttons, cards, the add-task field.

use quest_domain::{EntityId, Quest, Task};
use rune::{Align, Color, Cursor, El, Icon, TextEdit, TextStyle, Tx, col, el, icon, row, text, text_input};

use crate::icons;
use crate::state::State;
use crate::theme::{self, Pal, quest_color, ty};

pub fn quest_of<'a>(s: &'a State, task: &Task) -> Option<&'a Quest> {
    task.quest_id.and_then(|id| s.snap.quest(id))
}

pub fn accent_of(quest: Option<&Quest>, p: &Pal) -> Color {
    quest.map_or(p.faint, |q| quest_color(q.color))
}

/// The text between a period's back and forward buttons, as wide for every period ("Sep 28 – Oct 4"
/// as "Oct 5 – 11"), so the buttons around it stay put while one steps through the weeks.
pub fn period_label(key: &str, label: String, font: TextStyle, color: Color) -> El {
    text(label).key(key).font(font).color(color).single_line().w(PERIOD_LABEL_W).align(Align::Center)
}

/// Fits the widest week range of `format::week_range` with room to spare.
const PERIOD_LABEL_W: f32 = 130.0;

pub fn caps(label: impl Into<String>, p: &Pal) -> El {
    text(label.into().to_uppercase()).font(ty::CAPS).color(p.muted)
}

pub fn dot(color: Color, size: f32) -> El {
    el().size(size, size).radius(size / 2.0).fill(color)
}

pub fn icon_button(
    key: impl Into<String>,
    label: &str,
    glyph: &'static Icon,
    ink: Color,
    p: &Pal,
    on_press: impl Fn(&mut State, &mut Tx) + 'static,
) -> El {
    el().key(key.into())
        .role("button")
        .label(label)
        .focusable(true)
        .size(28.0, 28.0)
        .radius(6.0)
        .center()
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .child(icon(glyph, 16.0).color(ink))
        .on_press_tx(on_press)
}

/// A bordered button with an optional leading icon.
pub fn button(
    key: impl Into<String>,
    label: &str,
    glyph: Option<&'static Icon>,
    p: &Pal,
    on_press: impl Fn(&mut State, &mut Tx) + 'static,
) -> El {
    let mut b = row()
        .key(key.into())
        .role("button")
        .label(label)
        .focusable(true)
        .h(36.0)
        .px(14.0)
        .gap(8.0)
        .center_y()
        .radius(7.0)
        .fill(p.card)
        .border(1.0, p.border_strong)
        .cursor(Cursor::Pointer)
        .hover(p.hover);
    if let Some(glyph) = glyph {
        b = b.child(icon(glyph, 15.0).color(p.text));
    }
    b.child(text(label).font(ty::BODY_MEDIUM).color(p.text)).on_press_tx(on_press)
}

pub fn card(p: &Pal) -> El {
    col().radius(12.0).fill(p.card).border(1.0, p.border)
}

/// A single-line field. `edit` picks the `TextEdit` it binds; `submit` runs on Return.
pub fn field(
    key: &str,
    edit: &TextEdit,
    placeholder: &str,
    style: TextStyle,
    p: &Pal,
    bind: impl for<'a> Fn(&'a mut State) -> &'a mut TextEdit + 'static,
    submit: impl Fn(&mut State, &mut Tx) + 'static,
) -> El {
    text_input(edit, style)
        .key(key.to_owned())
        .label(placeholder)
        .single_line()
        .grow(1.0)
        .color(p.text)
        .placeholder(placeholder, p.faint)
        .on_edit(bind)
        .on_submit(submit)
}

pub(super) fn timer_button(key: String, id: EntityId, running: bool, p: &Pal) -> El {
    let (glyph, size, label, ink) = if running {
        (&icons::STOP, 18.0, "Stop timer", theme::on_accent_fill())
    } else {
        (&icons::PLAY, 14.0, "Start timer", p.muted)
    };
    el().key(key)
        .role("button")
        .label(label)
        .help(label)
        .focusable(true)
        .size(26.0, 26.0)
        .radius(13.0)
        .fill(if running { p.accent_fill } else { Color::TRANSPARENT })
        .hover(if running { p.accent_fill } else { p.hover })
        .center()
        .cursor(Cursor::Pointer)
        .child(icon(glyph, size).color(ink))
        .on_press(move |s: &mut State| s.toggle_focus(id))
}

/// A thin horizontal progress bar; `fraction` is clamped to 0..=1.
pub fn progress_bar(key: &str, width: f32, height: f32, fraction: f32, fill: Color, track: Color) -> El {
    let filled = (width * fraction.clamp(0.0, 1.0)).max(if fraction > 0.0 { height } else { 0.0 });
    el().key(key.to_owned())
        .w(width)
        .h(height)
        .radius(height / 2.0)
        .fill(track)
        .child(el().w(filled).h(height).radius(height / 2.0).fill(fill))
}

pub fn empty_note(message: &str, p: &Pal) -> El {
    row().w_full().py(18.0).center().child(text(message).font(ty::BODY).color(p.faint))
}
