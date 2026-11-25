//! The tracked-time column on the right: "Tracked today" (the day's sessions on a timeline, the
//! running one with a dashed edge, a line at now; a block opens its entry editor) and "This week"
//! (quest hours against the weekly goal, per quest, Organizing, No quest, Postponed).

use chrono::Local;
use quest_view::tracked::{self, BlockKind, DayTimeline, TimelineBlock, WeekBalance};
use rune::{Color, Cursor, Cx, El, ScrollSpec, TextStyle, col, el, row, spacer, text};

use crate::format;
use crate::state::State;
use crate::theme::{Pal, quest_color, ty};
use crate::ui::parts;

pub const WIDTH: f32 = 300.0;
const PAD: f32 = 22.0;
const INNER: f32 = WIDTH - 2.0 * PAD;
/// The hour labels' column; blocks start a little right of it.
const LABELS: f32 = 40.0;
const HOUR_PX: f32 = 38.0;
const BLOCK_WIDTH: f32 = INNER - LABELS - 4.0;
const BLOCK_PAD: f32 = 8.0;

const HOUR_LABEL: TextStyle = TextStyle::new(10.5, 400.0).lh(12.0).features("'tnum' 1");
const BLOCK_TITLE: TextStyle = TextStyle::new(11.5, 600.0).lh(15.0);
const SHORT_TITLE: TextStyle = TextStyle::new(10.5, 600.0).lh(11.0);
const BLOCK_RANGE: TextStyle = TextStyle::new(11.0, 400.0).lh(14.0).features("'tnum' 1");
const WEEK_LINE: TextStyle = TextStyle::new(13.0, 400.0).lh(18.0).features("'tnum' 1");

pub fn column(s: &State, p: &Pal, cx: &Cx<State>) -> El {
    let day = tracked::day_timeline_with_focus(&s.snap, s.today(), &Local, s.now, &s.focus_exit);
    let week = tracked::week_balance(&s.snap, s.today(), &Local, s.now);
    let today = col()
        .w_full()
        .child(head("Tracked today", p))
        .child(text(format::hours(day.tracked_ms)).key("tracked-total").font(ty::TOTAL).color(p.text).mt(2.0))
        .child(timeline(&day, p, cx).mt(18.0));
    let content = col().w_full().px(PAD).pt(56.0).pb(60.0).gap(30.0).child(today).child(this_week(&week, p));
    col()
        .key("tracked")
        .w(WIDTH)
        .h_full()
        .fill(p.side)
        .child(content.scroll(ScrollSpec { stick_bottom: false }).grow(1.0).min_h(0.0))
}

fn head(label: &str, p: &Pal) -> El {
    text(label.to_uppercase()).font(ty::CAPS).color(p.faint)
}

fn timeline(day: &DayTimeline, p: &Pal, cx: &Cx<State>) -> El {
    let hours = day.last_hour.saturating_sub(day.first_hour);
    let first_minute = day.first_hour * 60;
    let y = |minute: u32| minute.saturating_sub(first_minute) as f32 / 60.0 * HOUR_PX;
    let mut area = el().key("timeline").w(INNER).h(hours as f32 * HOUR_PX);
    for hour in day.first_hour..=day.last_hour {
        let top = (hour - day.first_hour) as f32 * HOUR_PX;
        area = area
            .child(text(format!("{hour:02}:00")).font(HOUR_LABEL).color(p.faint).absolute(0.0, top - 6.0))
            .child(el().absolute(LABELS, top).w(INNER - LABELS).h(1.0).fill(p.border));
    }
    for block in &day.blocks {
        let top = y(block.start_minute);
        let height = (y(block.end_minute) - top).max(3.0);
        let outside = block.outside_since_minute.map(|minute| (y(minute) - top).min((height - 3.0).max(0.0)));
        area = area.child(block_el(block, top, height, outside, p, cx));
    }
    if let Some(now) = day.now_minute {
        let top = y(now);
        area = area
            .child(
                el().key("timeline-now")
                    .absolute(LABELS - 4.0, top - 0.75)
                    .w(INNER - LABELS + 4.0)
                    .h(1.5)
                    .fill(p.accent_line),
            )
            .child(el().absolute(LABELS - 7.0, top - 3.5).size(7.0, 7.0).radius(3.5).fill(p.accent_line));
    }
    area
}

fn block_el(block: &TimelineBlock, top: f32, height: f32, outside: Option<f32>, p: &Pal, cx: &Cx<State>) -> El {
    let organizing = block.kind == BlockKind::Organizing;
    let hue = block.color.filter(|_| !organizing).map_or(p.faint, quest_color);
    let range = format!(
        "{}–{}",
        format::hh_mm(block.start_minute),
        if block.running { "now".to_owned() } else { format::hh_mm(block.end_minute) }
    );
    let entry = block.entry;
    let title = if block.rebooked { format!("{} · rebooked", block.title) } else { block.title.clone() };
    let label =
        if outside.is_some() { format!("{title} {range} · outside focus") } else { format!("{title} {range}") };
    let mut b = col()
        .key(format!("tl-block-{entry}"))
        .role("button")
        .label(label.clone())
        .absolute(LABELS + 4.0, top)
        .w(BLOCK_WIDTH)
        .h(height)
        .px(BLOCK_PAD)
        .py(if height < 19.0 { 0.5 } else { 2.0 })
        .radius(6.0)
        .fill(blend(hue, p.side, 0.14))
        .border_sides([0.0, 0.0, 0.0, 2.0], blend(hue, p.side, 0.7))
        .clip_rounded(6.0)
        .cursor(Cursor::Pointer)
        .help(label)
        .on_press(move |s: &mut State| s.open_entry_editor(entry));
    if block.running || block.rebooked {
        b = b.border(1.0, blend(hue, p.side, 0.7)).border_dash(3.0, 2.0);
    }
    if let Some(from) = outside {
        b = b.child(hatching(from, height - from, hue));
    }
    let ink = if organizing { p.muted } else { p.text };
    let label_height = outside.unwrap_or(height);
    if label_height >= 19.0 {
        b = b.child(text(title).font(BLOCK_TITLE).color(ink).single_line().truncate());
    } else if label_height >= 12.0 {
        let key = format!("tl-short-title-{entry}");
        let fits = cx.layout_with(&key, |[_, _, width, _]| width <= BLOCK_WIDTH - 2.0 * BLOCK_PAD);
        // Keep the unconstrained line in layout even when it doesn't fit.
        b = b.child(row().child(
            text(title).key(key).font(SHORT_TITLE).color(ink).a11y_hidden().opacity(if fits == Some(true) {
                1.0
            } else {
                0.0
            }),
        ));
    }
    if label_height > 32.0 {
        b = b.child(text(range).font(BLOCK_RANGE).color(p.muted).single_line());
    }
    b
}

fn hatching(top: f32, height: f32, hue: Color) -> El {
    let mut stripes =
        el().key("timeline-outside-focus").absolute(0.0, top).w(BLOCK_WIDTH).h(height).clip_rounded(0.0).a11y_hidden();
    let length = height * std::f32::consts::SQRT_2 + 2.0;
    for step in 0..=((BLOCK_WIDTH + height) / 7.0).ceil() as usize {
        stripes = stripes.child(
            el().absolute(step as f32 * 7.0 - (height + length) / 2.0, height / 2.0)
                .w(length)
                .h(1.0)
                .rotate(-std::f32::consts::FRAC_PI_4)
                .fill(hue.alpha(0.44)),
        );
    }
    stripes
}

/// `amount` of `color` mixed into `under` in sRGB, as CSS `color-mix(in srgb, …)` (Rune's
/// colours are linear).
fn blend(color: Color, under: Color, amount: f32) -> Color {
    let encode = |c: f32| if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    let decode = |c: f32| if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    let mix = |a: f32, b: f32| decode(encode(b) + (encode(a) - encode(b)) * amount);
    Color { r: mix(color.r, under.r), g: mix(color.g, under.g), b: mix(color.b, under.b), a: 1.0 }
}

fn this_week(week: &WeekBalance, p: &Pal) -> El {
    let total = format!("{} / {}", format::hours(week.quest_ms), format::hours(week.target_ms));
    let mut c = col()
        .key("this-week")
        .w_full()
        .gap(12.0)
        .child(
            row()
                .w_full()
                .items_end()
                .child(head("This week", p))
                .child(spacer())
                .child(text(total).key("week-total").font(ty::META).color(p.muted)),
        )
        .child(parts::progress_bar(
            "week-bar",
            INNER,
            4.0,
            tracked::fraction(week.quest_ms, week.target_ms),
            p.accent_fill,
            p.bar_track,
        ));
    for quest in &week.quests {
        let color = quest_color(quest.color);
        c = c.child(
            col()
                .key(format!("week-quest-{}", quest.quest))
                .w_full()
                .gap(5.0)
                .child(line(parts::dot(color, 8.0), &quest.title, format::hours(quest.ms), p.text, p))
                .child(parts::progress_bar(
                    &format!("week-quest-bar-{}", quest.quest),
                    INNER,
                    4.0,
                    tracked::fraction(quest.ms, week.target_ms),
                    color,
                    p.bar_track,
                )),
        );
    }
    let mut rest = col().w_full().gap(8.0).pt(12.0).border_sides([1.0, 0.0, 0.0, 0.0], p.border).child(line(
        parts::dot(p.faint, 8.0),
        "Organizing",
        format::hours(week.organizing_ms),
        p.muted,
        p,
    ));
    if week.no_quest_ms >= 60_000 {
        rest =
            rest.child(line(parts::dot(p.border_strong, 8.0), "No quest", format::hours(week.no_quest_ms), p.muted, p));
    }
    rest = rest.child(line(parts::dot(p.faint, 8.0), "Postponed", week.postponed.to_string(), p.muted, p));
    c.child(rest)
}

/// A dot, a name and a figure on the right.
fn line(dot: El, name: &str, figure: String, ink: Color, p: &Pal) -> El {
    row()
        .w_full()
        .gap(7.0)
        .center_y()
        .child(dot)
        .child(text(name.to_owned()).font(WEEK_LINE).color(ink).single_line().truncate().grow(1.0).min_w(0.0))
        .child(text(figure).font(WEEK_LINE).color(p.muted))
}
