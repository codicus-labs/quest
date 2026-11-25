//! Home: today's heading and Quest Hours, and the QUEST WORK and OTHER lists with their add
//! fields.

use chrono::Days;
use quest_domain::{Command, EntityId, Position};
use rune::{Cursor, El, ScrollSpec, TextEdit, Tx, col, el, row, spacer, text};

use crate::clock;
use crate::dnd::{ListId, Section};
use crate::format;
use crate::icons;
use crate::palette::Mode;
use crate::state::quest_picker::PickFor;
use crate::state::{AddAt, State};
use crate::theme::{Pal, ty};
use crate::ui::drag::{self, Zone};
use crate::ui::topbar::open_palette_in;
use crate::ui::{parts, quest_picker};

pub fn view(s: &State, p: &Pal) -> El {
    let today = s.today();
    let tomorrow = today + Days::new(1);
    let today_ms = s.snap.work_between(clock::midnight(today), clock::midnight(tomorrow), s.now).quest_ms;
    let monday = s.this_week();
    let week_ms = s.snap.work_between(clock::midnight(monday), clock::midnight(monday + Days::new(7)), s.now).quest_ms;
    let target_ms = i64::from(s.snap.settings.weekly_target_minutes) * 60_000;

    let stats = row()
        .gap(26.0)
        .items_end()
        .child(
            row()
                .gap(8.0)
                .items_end()
                .child(text(format::hours(today_ms)).key("today-hours").font(ty::BIG_STAT).color(p.text))
                .child(text("today").font(ty::BODY.lh(28.0)).color(p.muted)),
        )
        .child(
            col()
                .gap(8.0)
                .pb(4.0)
                .child(parts::progress_bar(
                    "week-progress",
                    200.0,
                    6.0,
                    week_ms as f32 / target_ms.max(1) as f32,
                    p.accent,
                    p.bar_track,
                ))
                .child(
                    text(format!("{} this week", format::hours(week_ms)))
                        .key("week-hours")
                        .font(ty::SMALL)
                        .color(p.faint),
                ),
        );
    let header = row()
        .w_full()
        .items_end()
        .pb(28.0)
        .child(text(format::long_day(today)).key("home-date").font(ty::HEADING).color(p.text))
        .child(spacer())
        .child(stats);

    let body = row().w_full().grow(1.0).min_h(0.0).child(work_lists(s, p).grow(1.0).min_w(0.0));
    col().key("home").w_full().h_full().px(30.0).pt(40.0).pb(16.0).child(header).child(body)
}

/// QUEST WORK, then OTHER, scrolling together.
fn work_lists(s: &State, p: &Pal) -> El {
    let lists = col().key("home-lists").w_full().pb(24.0).child(quest_work(s, p)).child(other(s, p));
    let scroller = lists.scroll(ScrollSpec { stick_bottom: false }).grow(1.0).min_h(0.0).scroll_edge_fade(12.0);
    col().h_full().min_h(0.0).child(drag::zone(scroller, Zone::HomeList))
}

fn quest_work(s: &State, p: &Pal) -> El {
    let filter_ink = if s.quest_filter.is_some() { p.accent } else { p.muted };
    let mut head = row().w_full().h(36.0).center_y().gap(10.0).mb(14.0).child(parts::caps("Quest work", p));
    if let Some(quest) = s.quest_filter.and_then(|q| s.snap.quest(q)) {
        head = head.child(
            parts::chip(Some(quest), p)
                .key("home-filter-chip")
                .role("button")
                .label("Clear the quest filter")
                .cursor(Cursor::Pointer)
                .hover(p.hover)
                .on_press(|s: &mut State| s.quest_filter = None),
        );
    }
    let head = head.child(spacer()).child(
        parts::icon_button(
            "home-filter",
            "Filter by quest",
            &icons::FILTER,
            filter_ink,
            p,
            |s: &mut State, tx: &mut Tx| open_palette_in(s, Mode::FilterQuest, tx),
        )
        .size(34.0, 34.0)
        .border(1.0, p.border),
    );
    // Filtered, new quest work gets that quest; otherwise the chip says which one it gets.
    let chip = match (s.quest_filter, s.home_quest().and_then(|q| s.snap.quest(q))) {
        (None, Some(quest)) => Some(quest_picker::trigger(
            parts::chip(Some(quest), p).max_w(180.0),
            PickFor::HomeAdd,
            format!("Quest for new quest work: {}", quest.title),
            s,
            p,
        )),
        _ => None,
    };
    let add = parts::add_box_with(
        "home-add",
        &s.home_add,
        "Add quest work…",
        s,
        |s: &mut State| &mut s.home_add,
        |s: &mut State, _: &mut Tx| add_today(s, Section::Quest),
        false,
        chip,
    );
    let cards = drag::list_cards(s, p, ListId::Home, Section::Quest, drag::list_card_options(ListId::Home));
    let empty = cards.is_empty();
    let mut list = col().key("home-list").w_full().gap(drag::LIST_GAP).mt(14.0).children(cards);
    if empty {
        list = list.child(parts::empty_note("No quest work for today yet. Add some above.", p));
    }
    col().w_full().child(head).child(add).child(list)
}

fn other(s: &State, p: &Pal) -> El {
    let (glyph, label) =
        if s.show_other { (&icons::EYE_OFF, "Hide other tasks") } else { (&icons::EYE, "Show other tasks") };
    let head = row()
        .w_full()
        .h(36.0)
        .center_y()
        .gap(16.0)
        .mt(28.0)
        .mb(14.0)
        .child(parts::caps("Other", p))
        .child(el().grow(1.0).h(1.0).fill(p.border))
        .child(
            parts::icon_button("home-other-toggle", label, glyph, p.muted, p, |s: &mut State, _: &mut Tx| {
                s.show_other = !s.show_other
            })
            .size(34.0, 34.0)
            .border(1.0, p.border),
        );
    let section = col().key("home-other").w_full().child(head);
    if !s.show_other {
        return section;
    }
    let add = parts::add_box(
        "home-other-add",
        &s.home_other_add,
        "Add a task…",
        s,
        |s: &mut State| &mut s.home_other_add,
        |s: &mut State, _: &mut Tx| add_today(s, Section::Other),
        false,
    );
    let cards = drag::list_cards(s, p, ListId::Home, Section::Other, drag::list_card_options(ListId::Home));
    section.child(add).child(col().key("home-other-list").w_full().gap(drag::LIST_GAP).mt(14.0).children(cards))
}

/// Adds today's task from one of Home's fields: quest work gets the field's quest, other
/// tasks none.
pub fn add_today(s: &mut State, section: Section) {
    let edit: &mut TextEdit = match section {
        Section::Quest => &mut s.home_add,
        Section::Other => &mut s.home_other_add,
    };
    let title = edit.text.trim().to_owned();
    if title.is_empty() {
        return;
    }
    let position = match s.add_at {
        AddAt::Top => Position::Top,
        AddAt::Bottom => Position::Bottom,
    };
    let quest_id = match section {
        Section::Quest => s.home_quest(),
        Section::Other => None,
    };
    let planned_on = Some(s.today());
    s.dispatch(Command::CreateTask { id: EntityId::new(), title, quest_id, planned_on, due_on: None, position });
    if s.notice.is_some() {
        return;
    }
    match section {
        Section::Quest => s.home_add.clear(),
        Section::Other => s.home_other_add.clear(),
    }
}
