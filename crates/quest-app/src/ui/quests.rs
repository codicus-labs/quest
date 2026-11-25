//! Quests: Quest hours of all time, how many quests are in progress and completed; Active and
//! Completed tabs with their counts, and Archive beside them; and a card per quest with its colour, hours this week and
//! in total. A card opens the quest's dialog (`quest_editor`) over this page, which stays as it
//! was (tab, scroll position, focus). "New quest" opens the dialog empty; the archive holds
//! archived and deleted things.

use chrono::Days;
use quest_domain::{EntityId, Quest};
use rune::{Color, Cursor, El, ScrollSpec, Tx, col, el, icon, row, spacer, text};

use crate::clock;
use crate::format;
use crate::icons;
use crate::state::{QuestsTab, State, View};
use crate::theme::{Pal, quest_color, ty};
use crate::ui::parts;
use quest_view::quests::{count_label, listed, overview};

/// A card's width: three to a row in the default window.
const CARD_W: f32 = 322.0;
const CARD_H: f32 = 196.0;

pub fn view(s: &State, p: &Pal) -> El {
    let figures = overview(&s.snap, s.now);
    let mut cards = row().key("quest-list").w_full().flex_wrap().gap(18.0);
    let listed: Vec<&Quest> = listed(&s.snap, s.quests_tab).collect();
    for quest in &listed {
        cards = cards.child(quest_card(s, quest, p));
    }
    if s.quests_tab == QuestsTab::Active {
        cards = cards.child(new_quest_card(p));
    }
    let mut content = col()
        .key("quests-content")
        .w_full()
        .px(30.0)
        .pt(36.0)
        .pb(32.0)
        .gap(26.0)
        .child(header(p))
        .child(stats(&figures, p))
        .child(tabs(s, figures.in_progress, figures.completed, p))
        .child(text(hint(s.quests_tab)).font(ty::BODY).color(p.muted));
    if listed.is_empty() {
        content = content.child(empty_state(s.quests_tab, p));
    }
    col().key("quests").w_full().h_full().child(
        content.child(cards).key("quests-scroll").scroll(ScrollSpec { stick_bottom: false }).grow(1.0).min_h(0.0),
    )
}

fn header(p: &Pal) -> El {
    let title = col()
        .gap(8.0)
        .child(text("Quests").font(ty::HEADING).color(p.text))
        .child(text("Give your time a purpose. Make room for what comes next.").font(ty::BODY).color(p.muted));
    row().w_full().center_y().gap(12.0).child(title).child(spacer()).child(new_quest_button(p))
}

/// Opens the archive: archived quests and deleted things.
fn archive_button(p: &Pal) -> El {
    parts::button("quests-archive", "Archive", Some(&icons::ARCHIVE), p, |s: &mut State, _: &mut Tx| {
        s.go(View::Archive)
    })
    .h(42.0)
}

/// The filled "+ New quest" button.
fn new_quest_button(p: &Pal) -> El {
    row()
        .key("quests-new")
        .role("button")
        .label("New quest")
        .focusable(true)
        .h(40.0)
        .px(16.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.accent)
        .cursor(Cursor::Pointer)
        .hover(p.accent.alpha(0.85))
        .child(icon(&icons::PLUS, 15.0).color(p.bg))
        .child(text("New quest").font(ty::BODY_MEDIUM).color(p.bg))
        .on_press(State::open_new_quest)
}

/// Quest hours · all time, In progress, Quests completed.
fn stats(figures: &quest_view::quests::Overview, p: &Pal) -> El {
    let cell = |key: &str, label: El, value: String, divider: bool| {
        let cell = col()
            .grow(1.0)
            .min_w(0.0)
            .padding(22.0)
            .gap(12.0)
            .child(label)
            .child(text(value).key(key.to_owned()).font(ty::BIG_STAT).color(p.text));
        if divider { cell.border_sides([0.0, 0.0, 0.0, 1.0], p.border) } else { cell }
    };
    let label = |words: &str| text(words.to_owned()).font(ty::BODY).color(p.muted);
    let trophy =
        row().gap(8.0).center_y().child(icon(&icons::TROPHY, 15.0).color(p.accent)).child(label("Quests completed"));
    row()
        .key("quests-stats")
        .w_full()
        .radius(14.0)
        .fill(p.card)
        .border(1.0, p.border)
        .child(cell("quests-stat-hours", label("Quest hours · all time"), format::hours(figures.all_time_ms), false))
        .child(cell("quests-stat-progress", label("In progress"), count_label(figures.in_progress), true))
        .child(cell("quests-stat-completed", trophy, count_label(figures.completed), true))
}

/// Active · n | Completed · n, and Archive on the right.
fn tabs(s: &State, active: usize, completed: usize, p: &Pal) -> El {
    let tab = |key: &str, tab: QuestsTab, label: String| {
        let on = s.quests_tab == tab;
        row()
            .key(key.to_owned())
            .role("tab")
            .label(label.clone())
            .checked(on)
            .focusable(true)
            .h(34.0)
            .px(14.0)
            .center_y()
            .radius(7.0)
            .fill(if on { p.selected } else { Color::TRANSPARENT })
            .cursor(Cursor::Pointer)
            .hover(p.hover)
            .child(text(label).font(ty::BODY_MEDIUM).color(if on { p.text } else { p.muted }))
            .on_press(move |s: &mut State| s.quests_tab = tab)
    };
    let switch = row()
        .key("quests-tabs")
        .role("tablist")
        .padding(4.0)
        .gap(2.0)
        .radius(10.0)
        .border(1.0, p.border)
        .child(tab("quests-tab-active", QuestsTab::Active, format!("Active · {active}")))
        .child(tab("quests-tab-completed", QuestsTab::Completed, format!("Completed · {completed}")));
    col()
        .w_full()
        .gap(18.0)
        .child(row().w_full().center_y().child(switch).child(spacer()).child(archive_button(p)))
        .child(el().w_full().h(1.0).fill(p.border))
}

fn hint(tab: QuestsTab) -> &'static str {
    match tab {
        QuestsTab::Active => "Click a quest to edit or complete it.",
        QuestsTab::Completed => "Click a quest to see it or reopen it.",
    }
}

fn empty_state(tab: QuestsTab, p: &Pal) -> El {
    let (key, message) = match tab {
        QuestsTab::Active => ("quests-empty-active", "No active quests. Start one with New quest."),
        QuestsTab::Completed => ("quests-empty-completed", "No completed quests yet. Complete a quest to see it here."),
    };
    parts::empty_note(message, p).key(key)
}

/// Time tracked on `quest`'s work this week.
pub fn week_ms(s: &State, quest: EntityId) -> i64 {
    let monday = s.this_week();
    let (from, to) = (clock::midnight(monday), clock::midnight(monday + Days::new(7)));
    quest_view::quests::quest_ms(&s.snap, quest, from, to, s.now)
}

/// Time tracked on `quest`'s work ever.
pub fn total_ms(s: &State, quest: EntityId) -> i64 {
    quest_view::quests::total_ms(&s.snap, quest, s.now)
}

/// A quest's card: colour, name, open tasks, hours this week and in total. A click opens its
/// dialog.
fn quest_card(s: &State, quest: &Quest, p: &Pal) -> El {
    let id = quest.id;
    let color = quest_color(quest.color);
    let open = s.snap.tasks.iter().filter(|t| t.quest_id == Some(id) && !t.is_done()).count();
    let open_label = format!("{open} open {}", if open == 1 { "task" } else { "tasks" });
    let week = format::hours(week_ms(s, id));
    let total = format!("{} total", format::hours(total_ms(s, id)));
    let badge = row().size(36.0, 36.0).radius(9.0).fill(p.chip).center().child(parts::dot(color, 12.0));
    let name = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(4.0)
        .child(text(quest.title.clone()).font(ty::STAT).color(p.text).wrap().max_lines(2))
        .child(text(open_label.clone()).font(ty::SMALL).color(p.muted));
    let hours = row()
        .w_full()
        .items_end()
        .child(
            col()
                .gap(2.0)
                .child(text(week.clone()).key(format!("quest-hours-{id}")).font(ty::BIG_STAT).color(p.text))
                .child(parts::caps("This week", p)),
        )
        .child(spacer())
        .child(text(total.clone()).key(format!("quest-total-{id}")).font(ty::SMALL).color(p.muted));
    col()
        .key(format!("quest-{id}"))
        .role("button")
        .label(format!("{}, {week} this week, {total}, {open_label}", quest.title))
        .focusable(true)
        .w(CARD_W)
        .h(CARD_H)
        .padding(22.0)
        .radius(14.0)
        .fill(p.card)
        .border(1.0, p.border)
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .child(row().w_full().gap(14.0).items_start().child(badge).child(name))
        .child(spacer())
        .child(hours)
        .on_press(move |s: &mut State| s.open_quest_editor(id))
}

/// The dashed "+ New quest" card after the active quests.
fn new_quest_card(p: &Pal) -> El {
    col()
        .key("quests-new-card")
        .role("button")
        .label("New quest")
        .focusable(true)
        .w(CARD_W)
        .h(CARD_H)
        .gap(10.0)
        .center()
        .radius(14.0)
        .border(1.0, p.border_strong)
        .border_dash(4.0, 3.0)
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .child(icon(&icons::PLUS, 22.0).color(p.muted))
        .child(text("New quest").font(ty::BODY).color(p.muted))
        .on_press(State::open_new_quest)
}
