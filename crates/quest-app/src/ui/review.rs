use chrono::{Days, Local, NaiveDate};
use quest_domain::{Command, DateField, Placement, Task, TaskList};
use quest_view::review::{Bucket, Review, Since, offset_week};
use rune::{Color, Cursor, El, ScrollSpec, Tx, col, el, icon, row, spacer, text};

use super::{parts, task_modal};
use crate::format;
use crate::icons;
use crate::state::task_edits::field_key;
use crate::state::{State, View};
use crate::theme::{Pal, quest_color, ty};
use crate::undo::Deletable;

const RANK: &str = "rank";

pub fn browse(s: &mut State, offset: i32, tx: &mut Tx) {
    if s.view == View::Review
        && let Some(week) = offset_week(s.review_week, offset)
    {
        s.close_task_modal();
        s.selected_task = None;
        s.review_week = week;
        tx.scroll_to_y("review-scroll", 0.0);
    }
}

pub fn view(s: &State, p: &Pal) -> El {
    let review = Review::build(&s.snap, s.review_week, &Local, s.now);
    let week = &review.tasks;
    let prev = &review.previous_tasks;
    let time = &review.time;
    let nav = row()
        .center_y()
        .child(parts::icon_button("review-prev", "Previous week", &icons::CHEVRON_LEFT, p.muted, p, |s, tx| {
            browse(s, -1, tx)
        }))
        .child(parts::period_label("review-range", format::week_range(week.week_start), ty::BODY_MEDIUM, p.text))
        .child(parts::icon_button("review-next", "Next week", &icons::CHEVRON_RIGHT, p.muted, p, |s, tx| {
            browse(s, 1, tx)
        }));
    let header = row()
        .w_full()
        .center_y()
        .gap(10.0)
        .child(icon(&icons::REVIEW, 24.0).color(p.accent))
        .child(text("Review").font(ty::HEADING).color(p.text))
        .child(spacer())
        .child(nav);
    let metrics = row()
        .w_full()
        .border_sides([1.0, 0.0, 1.0, 0.0], p.border)
        .child(metric(
            "review-done-added",
            "Done / added",
            format!("{} / {}", week.completed, week.created),
            format!("{} / {}", prev.completed, prev.created),
            p,
        ))
        .child(metric(
            "review-quest-hours",
            "Quest hours / goal",
            format!("{} / {}", format::hours(time.quest_ms), format::hours(time.target_ms)),
            format::hours(review.previous_quest_ms),
            p,
        ))
        .child(metric(
            "review-postponements",
            "Postponements",
            week.postponements.to_string(),
            prev.postponements.to_string(),
            p,
        ));
    let mut ranked = section("Most postponed", p);
    if review.most_postponed.is_empty() {
        ranked = ranked.child(empty(p));
    }
    for item in &review.most_postponed {
        let Some(task) = s.snap.task(item.task_id) else { continue };
        let mut line = row().w_full().center_y().gap(4.0).child(title(s, task, RANK, p)).child(
            text(format!("↷ {}", item.count))
                .key(format!("review-count-{}", task.id))
                .font(ty::SMALL_MEDIUM)
                .color(p.accent),
        );
        if !task.is_done() {
            line = line.child(actions(task, p));
        }
        let body = task_card(line, p)
            .children(history("Plan", &icons::DATE, &item.plan, week.week_start, p))
            .children(history("Deadline", &icons::DEADLINE, &item.deadline, week.week_start, p));
        ranked = ranked.child(detail(s, task, RANK, body, p));
    }
    let statuses = row()
        .w_full()
        .gap(12.0)
        .child(status(s, "Carried over", "plan", &review.carried_over, p.muted, p))
        .child(status(s, "Overdue", "due", &review.overdue, p.error, p));
    col().key("review").w_full().h_full().child(
        col()
            .key("review-scroll")
            .grow(1.0)
            .min_h(0.0)
            .w_full()
            .items_center()
            .scroll(ScrollSpec { stick_bottom: false })
            .scroll_edge_fade(12.0)
            .child(
                col()
                    .w_full()
                    .max_w(688.0)
                    .px(24.0)
                    .pt(8.0)
                    .pb(32.0)
                    .gap(16.0)
                    .child(header)
                    .child(metrics)
                    .child(ranked)
                    .child(statuses)
                    .child(time_breakdown(s, &review, p)),
            ),
    )
}

fn metric(key: &str, label: &str, value: String, previous: String, p: &Pal) -> El {
    col()
        .grow(1.0)
        .w(0.0)
        .min_w(0.0)
        .padding(12.0)
        .gap(4.0)
        .child(text(label.to_owned()).font(ty::SMALL).color(p.muted))
        .child(text(value).key(key.to_owned()).font(ty::HEADING).color(p.text))
        .child(text(format!("Prev. {previous}")).font(ty::SMALL).color(p.muted))
}

fn section(label: &str, p: &Pal) -> El {
    col().gap(6.0).child(text(label.to_owned()).font(ty::BODY_MEDIUM).color(p.text))
}

fn empty(p: &Pal) -> El {
    text("None this week").font(ty::SMALL).color(p.muted).py(6.0)
}

fn task_card(line: El, p: &Pal) -> El {
    col().w_full().py(5.0).gap(3.0).border_sides([0.0, 0.0, 1.0, 0.0], p.border).child(line)
}

fn title(s: &State, task: &Task, group: &'static str, p: &Pal) -> El {
    if let Some(form) = open_detail(s, task, group) {
        return row().grow(1.0).min_w(0.0).child(task_modal::title(form, p));
    }
    let id = task.id;
    text(task.title.clone())
        .key(format!("review-{group}-{id}"))
        .font(ty::BODY_MEDIUM)
        .color(p.text)
        .single_line()
        .truncate()
        .min_w(0.0)
        .role("button")
        .expanded(false)
        .grow(1.0)
        .focusable(true)
        .cursor(Cursor::Pointer)
        .on_press_tx(move |s: &mut State, tx| {
            s.open_task_modal(id);
            if let Some(form) = &mut s.task_modal {
                form.review_group = Some(group);
            }
            tx.scroll_into_view("task-modal");
        })
}

fn open_detail<'a>(s: &'a State, task: &Task, group: &str) -> Option<&'a crate::state::TaskModal> {
    s.task_modal.as_ref().filter(|form| form.task == task.id && form.review_group == Some(group))
}

fn detail(s: &State, task: &Task, group: &'static str, body: El, p: &Pal) -> El {
    body.children(open_detail(s, task, group).map(|_| task_modal::inline(s, task, p)))
}

fn actions(task: &Task, p: &Pal) -> El {
    let id = task.id;
    let date = |field, label, glyph: &'static rune::Icon| {
        parts::icon_button(format!("review-{}", field_key(Some(id), field)), label, glyph, p.muted, p, move |s, tx| {
            s.open_calendar(Some(id), field, tx);
            if let Some(calendar) = &mut s.task_calendar {
                calendar.review = true;
            }
        })
        .help(label)
    };
    row()
        .center_y()
        .child(date(DateField::Plan, "Schedule", &icons::DATE))
        .child(date(DateField::Due, "Set deadline", &icons::DEADLINE))
        .child(parts::icon_button(
            format!("review-inbox-{id}"),
            "Move to Inbox",
            &icons::INBOX,
            p.muted,
            p,
            move |s, _| {
                s.dispatch(Command::PlaceTask {
                    id,
                    placement: Placement::FileIn(TaskList::Inbox),
                    today: s.today(),
                    at: s.clock.now(),
                });
            },
        ))
        .child(parts::icon_button(format!("review-delete-{id}"), "Delete", &icons::TRASH, p.muted, p, move |s, _| {
            if s.task_modal.as_ref().is_some_and(|form| form.task == id) {
                s.close_task_modal();
            }
            s.delete(Deletable::Task(id));
        }))
}

fn history(
    label: &str,
    glyph: &'static rune::Icon,
    chains: &[Vec<NaiveDate>],
    start: NaiveDate,
    p: &Pal,
) -> Option<El> {
    if chains.is_empty() {
        return None;
    }
    let path = chains
        .iter()
        .map(|chain| {
            chain
                .iter()
                .map(|day| {
                    day.format(if (start..start + Days::new(7)).contains(day) { "%a" } else { "%b %-d" }).to_string()
                })
                .collect::<Vec<_>>()
                .join(" → ")
        })
        .collect::<Vec<_>>()
        .join(" · ");
    Some(
        row()
            .gap(6.0)
            .center_y()
            .child(icon(glyph, 13.0).color(p.muted))
            .child(text(format!("{label}  {path}")).font(ty::SMALL).color(p.muted).min_w(0.0)),
    )
}

fn status(s: &State, label: &str, group: &'static str, entries: &[Since], ink: Color, p: &Pal) -> El {
    let mut list = section(label, p).grow(1.0).w(0.0).min_w(0.0);
    if entries.is_empty() {
        return list.child(empty(p));
    }
    for entry in entries {
        let Some(task) = s.snap.task(entry.task_id) else { continue };
        let card = task_card(row().w_full().child(title(s, task, group, p)), p)
            .child(text(format!("since {}", entry.day.format("%b %-d"))).font(ty::SMALL).color(ink));
        list = list.child(detail(s, task, group, card, p));
    }
    list
}

fn time_breakdown(s: &State, review: &Review, p: &Pal) -> El {
    let daily = review.time.total_ms_per_day;
    let max = daily.iter().copied().max().unwrap_or_default().max(1) as f32;
    let mut bars = row().w_full().items_end().gap(4.0);
    for (day, ms) in State::week_days(review.time.week_start).zip(daily) {
        bars = bars.child(
            col()
                .grow(1.0)
                .w(0.0)
                .items_center()
                .gap(3.0)
                .child(
                    col()
                        .w_full()
                        .h(32.0)
                        .justify_end()
                        .radius(4.0)
                        .fill(p.bar_track)
                        .child(el().w_full().h(ms as f32 / max * 32.0).radius(3.0).fill(p.accent)),
                )
                .child(text(day.format("%a").to_string()).font(ty::SMALL).color(p.muted))
                .child(text(format::hours(ms)).font(ty::SMALL).color(p.text)),
        );
    }
    let mut out = section("Where the time went", p).child(bars.mb(2.0));
    for share in &review.shares {
        let (name, color) = match share.bucket {
            Bucket::Quest(id) => {
                let Some(quest) = s.snap.quest(id) else { continue };
                (quest.title.as_str(), quest_color(quest.color))
            }
            Bucket::Organizing => ("Organizing", p.muted),
            Bucket::NoQuest => ("No quest", p.bar_idle),
        };
        out = out.child(
            col()
                .w_full()
                .gap(2.0)
                .child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .center_y()
                        .child(text(name.to_owned()).grow(1.0).font(ty::SMALL).color(p.text).single_line().truncate())
                        .child(
                            text(format!("{} · {:.0}%", format::hours(share.ms), share.fraction * 100.0))
                                .font(ty::SMALL)
                                .color(p.muted),
                        ),
                )
                .child(
                    row()
                        .w_full()
                        .h(3.0)
                        .radius(2.0)
                        .fill(p.bar_track)
                        .child(el().w_frac(share.fraction as f32).h_full().radius(2.0).fill(color)),
                ),
        );
    }
    out
}
