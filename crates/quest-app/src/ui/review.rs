//! Review: the weekly review – quest hours vs target, daily average, quest work vs other, hours
//! per day, hours by quest – with week navigation.

use chrono::{Days, Local};
use quest_domain::WeeklyReview;
use rune::{El, Tx, col, el, row, spacer, text};

use crate::format;
use crate::icons;
use crate::state::State;
use crate::theme::{Pal, quest_color, ty};
use crate::ui::parts;

const CHART_H: f32 = 240.0;

pub fn view(s: &State, p: &Pal) -> El {
    let review = WeeklyReview::build(&s.snap, s.review_week, &Local, s.now);
    let nav = row()
        .gap(10.0)
        .center_y()
        .child(parts::icon_button(
            "review-prev",
            "Previous week",
            &icons::CHEVRON_LEFT,
            p.muted,
            p,
            |s: &mut State, _: &mut Tx| s.review_week = s.review_week - Days::new(7),
        ))
        .child(parts::period_label(
            "review-range",
            format::week_range(review.week_start),
            ty::BODY_MEDIUM.lh(22.0),
            p.text,
        ))
        .child(parts::icon_button(
            "review-next",
            "Next week",
            &icons::CHEVRON_RIGHT,
            p.muted,
            p,
            |s: &mut State, _: &mut Tx| s.review_week = s.review_week + Days::new(7),
        ))
        .child(
            parts::button("review-this-week", "This week", Some(&icons::DATE), p, |s: &mut State, _: &mut Tx| {
                s.review_week = s.this_week()
            })
            .radius(18.0)
            .ml(16.0),
        );
    let header = row()
        .w_full()
        .items_end()
        .pb(28.0)
        .child(
            col()
                .gap(10.0)
                .child(text("Weekly review").font(ty::HEADING).color(p.text))
                .child(text("How much of the week moved your quests forward.").font(ty::BODY).color(p.muted)),
        )
        .child(spacer())
        .child(nav);
    let share = review.quest_share_percent();
    let target_pct = if review.target_ms > 0 { review.quest_ms * 100 / review.target_ms } else { 0 };
    let cards = row()
        .w_full()
        .gap(16.0)
        .pb(16.0)
        .child(stat_card(
            p,
            "Quest hours",
            row()
                .gap(6.0)
                .items_end()
                .child(
                    text(format::hours(review.quest_ms))
                        .key("review-quest-hours")
                        .font(ty::HEADING.lh(40.0))
                        .color(p.accent),
                )
                .child(text(format!("/ {}", format::hours(review.target_ms))).font(ty::BODY.lh(30.0)).color(p.muted)),
            Some(format!("{target_pct}% of target")),
        ))
        .child(stat_card(
            p,
            "Daily average",
            text(format::hours(review.daily_average_ms(s.today()))).font(ty::HEADING.lh(40.0)).color(p.text),
            None,
        ))
        .child(stat_card(
            p,
            "Quest work vs other",
            row()
                .gap(6.0)
                .items_end()
                .child(text(share.to_string()).font(ty::HEADING.lh(40.0)).color(p.text))
                .child(text("%").font(ty::BODY.lh(30.0)).color(p.muted)),
            None,
        ));
    let lower =
        row().w_full().grow(1.0).min_h(0.0).gap(16.0).child(per_day(s, &review, p).grow(1.4).min_w(0.0)).child(
            col().grow(1.0).min_w(0.0).gap(16.0).child(by_quest(s, &review, p)).child(work_vs_other(&review, p)),
        );
    col().key("review").w_full().h_full().px(30.0).pt(40.0).pb(16.0).child(header).child(cards).child(lower)
}

fn stat_card(p: &Pal, label: &str, value: El, note: Option<String>) -> El {
    let mut card = parts::card(p)
        .grow(1.0)
        .min_w(0.0)
        .h(130.0)
        .px(22.0)
        .py(22.0)
        .gap(10.0)
        .child(parts::caps(label, p))
        .child(value);
    if let Some(note) = note {
        card = card.child(text(note).font(ty::BODY).color(p.faint));
    }
    card
}

fn per_day(s: &State, review: &WeeklyReview, p: &Pal) -> El {
    let max = review.quest_ms_per_day.iter().copied().max().unwrap_or(0).max(3_600_000) as f32;
    let today = s.today();
    let mut bars = row().w_full().grow(1.0).items_end().gap(14.0);
    for (i, day) in State::week_days(review.week_start).enumerate() {
        let ms = review.quest_ms_per_day[i];
        let is_today = day == today;
        let fill = if is_today { p.accent } else { p.bar_idle };
        let label_color = if is_today {
            p.accent
        } else if ms > 0 {
            p.muted
        } else {
            p.faint
        };
        let height = (ms as f32 / max * CHART_H).max(4.0);
        bars = bars.child(
            col()
                .key(format!("review-day-{i}"))
                .grow(1.0)
                .min_w(0.0)
                .items_center()
                .gap(10.0)
                .child(text(format::hours(ms)).font(ty::SMALL_MEDIUM).color(if ms > 0 { label_color } else { p.faint }))
                .child(el().w(30.0).h(height).radius(4.0).fill(if ms > 0 { fill } else { p.bar_track }))
                .child(text(day.format("%a").to_string()).font(ty::SMALL).color(label_color)),
        );
    }
    parts::card(p).h_full().padding(22.0).gap(20.0).child(parts::caps("Quest hours · per day", p)).child(bars)
}

fn by_quest(s: &State, review: &WeeklyReview, p: &Pal) -> El {
    let max = review.by_quest.first().map_or(1, |q| q.ms.max(1)) as f32;
    let mut rows = col().w_full().gap(14.0);
    for (i, hours) in review.by_quest.iter().enumerate() {
        let Some(quest) = s.snap.quest(hours.quest_id) else { continue };
        let color = quest_color(quest.color);
        rows = rows.child(
            row()
                .key(format!("review-quest-{i}"))
                .w_full()
                .gap(12.0)
                .center_y()
                .child(parts::dot(color, 8.0))
                .child(text(quest.title.clone()).font(ty::BODY).color(p.text).w(150.0).single_line().truncate())
                .child(
                    row()
                        .grow(1.0)
                        .h(8.0)
                        .radius(4.0)
                        .fill(p.bar_track)
                        .child(el().w_frac(hours.ms as f32 / max).h(8.0).radius(4.0).fill(color)),
                )
                .child(row().w(50.0).justify_end().child(text(format::hours(hours.ms)).font(ty::BODY).color(p.muted))),
        );
    }
    if review.by_quest.is_empty() {
        rows = rows.child(text("No quest time this week").font(ty::BODY).color(p.faint));
    }
    parts::card(p).w_full().padding(22.0).gap(18.0).child(parts::caps("Hours by quest", p)).child(rows)
}

fn work_vs_other(review: &WeeklyReview, p: &Pal) -> El {
    let total = (review.quest_ms + review.other_ms).max(1) as f32;
    let bar = row()
        .key("review-split")
        .w_full()
        .h(32.0)
        .radius(6.0)
        .fill(p.bar_track)
        .clip_rounded(6.0)
        .child(el().w_frac(review.quest_ms as f32 / total).h_full().fill(p.accent));
    let legend =
        row()
            .gap(26.0)
            .center_y()
            .child(
                row().gap(8.0).center_y().child(parts::dot(p.accent, 9.0)).child(
                    text(format!("Quest work · {}", format::hours(review.quest_ms))).font(ty::BODY).color(p.text),
                ),
            )
            .child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(parts::dot(p.bar_idle, 9.0))
                    .child(text(format!("Other · {}", format::hours(review.other_ms))).font(ty::BODY).color(p.muted)),
            );
    parts::card(p)
        .w_full()
        .grow(1.0)
        .padding(22.0)
        .gap(18.0)
        .child(parts::caps("Quest work vs other", p))
        .child(bar)
        .child(legend)
}
