//! The quest picker: a quest chip opens a search field over the quests it can choose (most
//! recently used first), drawn in the overlay layer and anchored under the chip. Typing filters,
//! ↑/↓ move, Return takes the row, Escape or a press outside closes without a change; focus goes
//! back to the chip.

use rune::{Color, Cursor, Cx, El, Key, KeyEvent, Tx, col, el, icon, row, text, text_input};
use rune_kit::{Placement, popover};

use crate::icons;
use crate::state::quest_picker::{Pick, PickFor, PickRow};
use crate::state::{CaptureStep, Overlay, State, View};
use crate::theme::{Pal, quest_color, ty};
use crate::ui::parts;

const WIDTH: f32 = 300.0;
/// About eight rows; more scroll.
const LIST_MAX_H: f32 = 288.0;
/// The search row and the line under it.
const HEADER_H: f32 = 41.0;
const GAP: f32 = 4.0;
/// How far the picker stays inside the dialog (or window) it opens in.
const MARGIN: f32 = 12.0;
/// Narrower than this beside the chip, the picker lines up with the chip's end instead.
const MIN_WIDTH: f32 = 220.0;

/// Where the picker goes so it stays inside its dialog or the window.
#[derive(Debug, PartialEq)]
pub(super) struct Fit {
    pub placement: Placement,
    pub width: f32,
    pub list_max_h: f32,
}

impl Fit {
    const DEFAULT: Fit = Fit { placement: Placement::BottomStart, width: WIDTH, list_max_h: LIST_MAX_H };
}

/// Fits the picker under (or, with more room there, above) the chip `chip`, inside `bounds`;
/// both are `[x, y, w, h]`. It starts at the chip and narrows to the bounds' right edge, or ends
/// at the chip's end when that leaves too little; its list shrinks to the room.
pub(super) fn fit(chip: [f32; 4], bounds: [f32; 4]) -> Fit {
    let [cx, cy, cw, ch] = chip;
    let [bx, by, bw, bh] = bounds;
    let (left, right, top, bottom) = (bx + MARGIN, bx + bw - MARGIN, by + MARGIN, by + bh - MARGIN);
    let room_right = right - cx;
    let room_left = cx + cw - left;
    let start = room_right >= MIN_WIDTH || room_right >= room_left;
    let width = WIDTH.min(if start { room_right } else { room_left }).max(0.0);
    let below = bottom - (cy + ch + GAP);
    let above = cy - GAP - top;
    let down = below >= HEADER_H + LIST_MAX_H || below >= above;
    let list_max_h = ((if down { below } else { above }) - HEADER_H).clamp(0.0, LIST_MAX_H);
    let placement = match (down, start) {
        (true, true) => Placement::BottomStart,
        (true, false) => Placement::BottomEnd,
        (false, true) => Placement::TopStart,
        (false, false) => Placement::TopEnd,
    };
    Fit { placement, width, list_max_h }
}

/// Makes `chip` the picker's trigger for `target`: a button named `label` that opens and closes it.
pub fn trigger(chip: El, target: PickFor, label: impl Into<String>, s: &State, p: &Pal) -> El {
    let open = s.quest_picker.as_ref().is_some_and(|picker| picker.target == target);
    chip.key(target.trigger_key())
        .role("combobox")
        .label(label)
        .expanded(open)
        .focusable(true)
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .on_press_tx(move |s: &mut State, tx: &mut Tx| {
            s.toggle_quest_picker(target.clone());
            if s.quest_picker.is_some() {
                tx.focus_when_shown("quest-picker-search", false);
            }
        })
}

/// The open picker, if its chip shows, kept inside the dialog it opens in (where both were laid
/// out the frame before).
pub fn layer(s: &State, p: &Pal, cx: &Cx<State>, layer: &mut Vec<El>) {
    let Some(picker) = &s.quest_picker else { return };
    if !hosted(s, &picker.target) {
        return;
    }
    let anchor = picker.target.trigger_key();
    let fit = match (cx.window_layout(&anchor), cx.window_layout(bounds_key(&picker.target))) {
        (Some(chip), Some(bounds)) => fit(chip, bounds),
        _ => Fit::DEFAULT,
    };
    let rows = s.quest_picker_rows();
    let active = s.quest_picker_active(&rows);
    let current = s.current_pick(&picker.target);
    let search = text_input(&picker.search, ty::BODY)
        .key("quest-picker-search")
        .label("Search quests")
        .single_line()
        .grow(1.0)
        .min_w(0.0)
        .color(p.text)
        .placeholder("Search quests…", p.faint)
        .on_edit(|s: &mut State| s.quest_picker_search())
        .on_input_change(|s: &mut State, _: &mut Tx| s.quest_search_changed())
        .on_input_key(keys)
        .on_submit(|s: &mut State, tx: &mut Tx| choose(s, None, tx));
    let header = row()
        .w_full()
        .h(40.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .child(icon(&icons::SEARCH, 14.0).color(p.muted))
        .child(search);
    let list = if rows.is_empty() {
        col()
            .key("quest-picker-empty")
            .w_full()
            .padding(14.0)
            .child(text("No quests match").font(ty::SMALL).color(p.faint))
    } else {
        let mut list = col().key("quest-picker-list").role("listbox").label("Quests").w_full().padding(4.0).gap(1.0);
        for (i, row) in rows.iter().enumerate() {
            list = list.child(option(row, Some(i) == active, current.as_ref() == Some(&row.pick), p));
        }
        popover::fit_scroll(list).max_h(fit.list_max_h)
    };
    let surface = col()
        .key("quest-picker")
        .role("dialog")
        .label("Choose quest")
        .w(fit.width)
        .shrink()
        .min_h(0.0)
        .child(header)
        .child(el().w_full().h(1.0).fill(p.border).a11y_hidden())
        .child(list)
        .focus_return(return_key(&picker.target))
        .modal(true, |s: &mut State| s.quest_picker = None);
    let surface = popover::raised(&p.kit, surface, 10.0).clip_rounded(10.0);
    layer.push(popover::anchored(&p.kit, surface, &anchor, fit.placement, GAP));
}

fn option(row_data: &PickRow, active: bool, current: bool, p: &Pal) -> El {
    let pick = row_data.pick.clone();
    let dot = row_data.color.map_or(p.faint, quest_color);
    let mut line = row()
        .key(pick.key())
        .role("option")
        .label(row_data.label.clone())
        .checked(current)
        .w_full()
        .h(32.0)
        .px(10.0)
        .gap(10.0)
        .center_y()
        .radius(6.0)
        .focus_selected(active)
        .fill(if active { p.kit.c.hover } else { Color::TRANSPARENT })
        .hover(p.kit.c.hover)
        .cursor(Cursor::Pointer)
        .child(parts::dot(dot, 8.0))
        .child(text(row_data.label.clone()).font(ty::BODY).color(p.text).single_line().truncate().grow(1.0).min_w(0.0));
    if current {
        line = line.child(icon(&icons::CHECK, 14.0).color(p.muted));
    }
    line.on_press_tx(move |s: &mut State, tx: &mut Tx| choose(s, Some(pick.clone()), tx))
}

/// ↑/↓ move the highlight, Escape closes without a change.
fn keys(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let step = match ev.key {
        Key::Down => 1,
        Key::Up => -1,
        Key::Escape => {
            let anchor = s.quest_picker.take().map(|picker| return_key(&picker.target));
            if let Some(anchor) = anchor {
                tx.focus(anchor, true);
            }
            return true;
        }
        _ => return false,
    };
    if let Some(key) = s.move_quest_highlight(step) {
        tx.scroll_into_view(key);
    }
    true
}

/// Takes `pick` (the highlighted row without one), closes and focuses the chip again.
fn choose(s: &mut State, pick: Option<Pick>, tx: &mut Tx) {
    let Some(anchor) = s.quest_picker.as_ref().map(|picker| return_key(&picker.target)) else { return };
    match pick {
        Some(pick) => s.choose_quest(pick),
        None => s.choose_highlighted_quest(),
    }
    tx.focus(anchor, tx.focus_visible());
}

/// The key of the dialog the picker opens in, or of the whole window.
fn return_key(target: &PickFor) -> String {
    if *target == PickFor::QuickAdd { "quick-add".into() } else { target.trigger_key() }
}

fn bounds_key(target: &PickFor) -> &'static str {
    match target {
        PickFor::TaskModal => "app",
        PickFor::LogWork => "log-work-dialog",
        PickFor::CaptureTask(_) => "capture-dialog",
        PickFor::QuickAdd | PickFor::Focus(_) => "app",
    }
}

/// Whether the chip that opened the picker is on screen.
fn hosted(s: &State, target: &PickFor) -> bool {
    match target {
        PickFor::TaskModal => s.task_modal.is_some() && s.overlay == Overlay::None,
        PickFor::LogWork => s.overlay == Overlay::LogWork && s.log_work.task.is_none(),
        PickFor::CaptureTask(_) => s.overlay == Overlay::Capture && matches!(s.capture.step, CaptureStep::Review(_)),
        PickFor::QuickAdd => s.quick_hosted(),
        // A session started elsewhere may change the task the chip belongs to.
        PickFor::Focus(task) => {
            s.view == View::Focus
                && s.overlay == Overlay::None
                && s.focus_target().task().is_some_and(|t| t.id == *task)
        }
    }
}
