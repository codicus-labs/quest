//! Plan: day columns with their tasks (QUEST WORK first, then OTHER, which V hides), an add
//! field per day and an optional backlog column. Five days fill the board; it scrolls sideways
//! through the days without end (trackpad, Earlier / Later, Today), building only the days
//! around what shows (`plan_strip`). Cards drag between and within columns (or ⌥ + arrows on a
//! focused card); a drag resting at the board's edge scrolls it on.

use chrono::{Datelike, Days};
use quest_domain::{Command, EntityId, Position};
use rune::{Align, Color, Cursor, Cx, El, ScrollSpec, TextEdit, Transition, Tx, col, el, row, spacer, text};

use crate::dnd::Rect;
use crate::dnd::{ListId, Section};
use crate::icons;
use crate::palette::Mode;
use crate::plan_strip::{GAP, Strip, VISIBLE_DAYS};
use crate::state::{AddAt, PLAN_BOARD, State};
use crate::theme::{Pal, ty};
use crate::ui::drag::{self, Zone};
use crate::ui::parts;
use crate::ui::topbar::open_palette_in;

/// Fits the widest month ("September 2026").
const MONTH_W: f32 = 170.0;

pub fn view(s: &State, p: &Pal, cx: &mut Cx<State>) -> El {
    let filter_ink = if s.quest_filter.is_some() { p.accent } else { p.muted };
    let mut header = row()
        .w_full()
        .center_y()
        .gap(16.0)
        .px(18.0)
        .pb(30.0)
        .child(text("Plan the week").font(ty::HEADING).color(p.text))
        .child(
            parts::icon_button(
                "plan-filter",
                "Filter by quest",
                &icons::FILTER,
                filter_ink,
                p,
                |s: &mut State, tx: &mut Tx| open_palette_in(s, Mode::FilterQuest, tx),
            )
            .size(34.0, 34.0)
            .border(1.0, p.border),
        );
    if let Some(quest) = s.quest_filter.and_then(|q| s.snap.quest(q)) {
        header = header.child(
            parts::chip(Some(quest), p)
                .key("plan-filter-chip")
                .role("button")
                .label("Clear the quest filter")
                .cursor(Cursor::Pointer)
                .hover(p.hover)
                .on_press(|s: &mut State| s.quest_filter = None),
        );
    }
    let (other_glyph, other_label) =
        if s.show_other { (&icons::EYE, "Hide other tasks") } else { (&icons::EYE_OFF, "Show other tasks") };
    let header = header
        .child(spacer())
        .child(parts::button(
            "plan-recurring",
            "Recurring tasks",
            Some(&icons::REPEAT),
            p,
            |s: &mut State, tx: &mut Tx| {
                tx.blur();
                s.overlay = crate::state::Overlay::RecurringTasks;
            },
        ))
        // As wide for every month, so the buttons before it stay put while the strip glides.
        .child(text(month(s)).key("plan-month").font(ty::STAT.lh(24.0)).color(p.muted).w(MONTH_W).align(Align::End))
        .child(parts::icon_button(
            "plan-prev",
            "Earlier",
            &icons::CHEVRON_LEFT,
            p.muted,
            p,
            |s: &mut State, tx: &mut Tx| s.glide_plan_by(-1, tx),
        ))
        .child(parts::button("plan-today", "Today", None, p, |s: &mut State, tx: &mut Tx| {
            s.glide_plan_to(s.today() - Days::new(2), tx)
        }))
        .child(parts::icon_button(
            "plan-next",
            "Later",
            &icons::CHEVRON_RIGHT,
            p.muted,
            p,
            |s: &mut State, tx: &mut Tx| s.glide_plan_by(1, tx),
        ))
        .child(
            parts::icon_button(
                "plan-backlog",
                "Show backlog",
                &icons::ARCHIVE,
                if s.show_backlog { p.accent } else { p.muted },
                p,
                |s: &mut State, _: &mut Tx| s.show_backlog = !s.show_backlog,
            )
            .size(34.0, 34.0)
            .border(1.0, p.border),
        )
        .child(
            parts::icon_button(
                "plan-other-toggle",
                other_label,
                other_glyph,
                p.muted,
                p,
                |s: &mut State, _: &mut Tx| s.show_other = !s.show_other,
            )
            .size(34.0, 34.0)
            .border(1.0, p.border),
        );
    let total_w = cx.layout_size("plan-columns").map_or(s.plan.strip.viewport_w, |[w, _]| w);
    // With the backlog, six columns share the width alike.
    let backlog_w = (total_w - GAP * VISIBLE_DAYS as f32) / (VISIBLE_DAYS + 1) as f32;
    let board_w = if s.show_backlog { total_w - backlog_w - GAP } else { total_w };
    let mut columns = row().key("plan-columns").w_full().grow(1.0).min_h(0.0).gap(GAP).child(board(s, p, board_w));
    if s.show_backlog {
        columns =
            columns.child(drag::zone(column(s, p, ListId::Backlog).w(backlog_w), Zone::PlanColumn(ListId::Backlog)));
    }
    col().key("plan").w_full().h_full().px(12.0).pt(40.0).pb(16.0).child(header).child(columns)
}

/// The month the middle of the board shows.
fn month(s: &State) -> String {
    s.plan.strip.month_at(s.plan.offset()).format("%B %Y").to_string()
}

/// The day strip in its horizontal scroll viewport. Only the days around the offset are built,
/// after a spacer as wide as the days before them, so every day sits where the whole strip has
/// it and days coming and going move nothing on screen.
fn board(s: &State, p: &Pal, board_w: f32) -> El {
    let strip = Strip { viewport_w: board_w, ..s.plan.strip };
    let (_, offset) = s.plan.strip.resized(s.plan.offset(), board_w);
    let window = strip.next_window(s.plan.built, offset, drag::dragged_day(s));
    let mut days = row().key("plan-days").w(strip.content_w()).h_full().child(
        el().key("plan-days-lead")
            .w(strip.x_of(window.first))
            .h(1.0)
            .on_layout_tx(|s: &mut State, _, tx: &mut Tx| s.plan_strip_moved(tx)),
    );
    for day in window.days() {
        days = days.child(column(s, p, ListId::Day(day)).w(strip.column_w()).mr(GAP));
    }
    row()
        .key(PLAN_BOARD)
        .w(board_w)
        .h_full()
        .min_h(0.0)
        .child(days)
        .scroll_x()
        .on_scroll_offset(s.plan.on_scroll())
        .on_layout_tx(|s: &mut State, rect, tx: &mut Tx| s.plan_board_laid_out(Rect::from(rect), tx))
}

/// What keys a column's parts: its day, or "backlog".
fn column_name(list: ListId) -> String {
    match list {
        ListId::Day(day) => day.to_string(),
        ListId::Backlog => "backlog".to_owned(),
        ListId::Home => "home".to_owned(),
    }
}

fn column(s: &State, p: &Pal, list_id: ListId) -> El {
    let name_key = column_name(list_id);
    let today = s.today();
    let day = list_id.day(today);
    let is_today = day == Some(today);
    let past = day.is_some_and(|d| d < today);
    let (name, number) = match day {
        Some(d) => (d.format("%a").to_string(), d.day().to_string()),
        None => ("Someday".to_owned(), String::new()),
    };
    let head = row()
        .w_full()
        .gap(8.0)
        .items_end()
        .pb(12.0)
        .mb(14.0)
        .border_sides([0.0, 0.0, 2.0, 0.0], if is_today { p.accent } else { p.border })
        .child(text(name).font(ty::STAT.lh(24.0)).color(if is_today { p.text } else { p.muted }))
        .child(text(number).font(ty::BODY_MEDIUM.lh(22.0)).color(if is_today { p.accent } else { p.faint }));
    let mut list = col().key(format!("plan-list-{name_key}")).w_full().gap(drag::LIST_GAP);
    if !past {
        list = list.child(add_field(s, list_id, &name_key));
    }
    let options = drag::list_card_options(list_id);
    list = list.children(drag::list_cards(s, p, list_id, Section::Quest, options));
    let others = if s.show_other { drag::list_cards(s, p, list_id, Section::Other, options) } else { Vec::new() };
    if !others.is_empty() {
        let divider = row()
            .key(format!("plan-other-{name_key}"))
            .w_full()
            .gap(10.0)
            .center_y()
            .pt(6.0)
            .child(parts::caps("Other", p))
            .child(el().grow(1.0).h(1.0).fill(p.border));
        list = list.child(divider).children(others);
    }
    col()
        .key(format!("plan-col-{name_key}"))
        .h_full()
        .min_h(0.0)
        .px(10.0)
        .pt(14.0)
        .radius(10.0)
        .fill(if is_today { p.card.alpha(0.4) } else { Color::TRANSPARENT })
        .border(1.0, p.border.alpha(0.5))
        .opacity(if past { 0.8 } else { 1.0 })
        .child(head)
        .child(list.pb(24.0).scroll(ScrollSpec { stick_bottom: false }).grow(1.0).min_h(0.0))
        // Days scrolling in show as they are, without fading in.
        .transition(Transition::none())
}

fn add_field(s: &State, list: ListId, name: &str) -> El {
    let key = format!("plan-add-{name}");
    let empty = TextEdit::new("");
    parts::add_box(
        &key,
        s.plan_add.get(&list).unwrap_or(&empty),
        "Add a task…",
        s,
        move |s: &mut State| s.plan_add.entry(list).or_insert_with(|| TextEdit::new("")),
        move |s: &mut State, _: &mut Tx| add_task(s, list),
        true,
    )
}

fn add_task(s: &mut State, list: ListId) {
    let Some(title) = s.plan_add.get(&list).map(|edit| edit.text.trim().to_owned()) else { return };
    if title.is_empty() {
        return;
    }
    let planned_on = list.day(s.today());
    let position = match s.add_at {
        AddAt::Top => Position::Top,
        AddAt::Bottom => Position::Bottom,
    };
    s.dispatch(Command::CreateTask { id: EntityId::new(), title, quest_id: None, planned_on, due_on: None, position });
    s.plan_add.remove(&list);
}
