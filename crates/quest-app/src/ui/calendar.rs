//! Calendar: the week as a time grid with tracked sessions and planned blocks, each day's tracked
//! total under its name and the week's Quest Hours against the target in the header. Drag a task
//! from the list on the left onto a day to plan it; drag blocks and sessions to move or resize
//! them, and over empty space to log time (Log work, as from the header's button). A picked
//! task's block can be cleared from the list.

use chrono::{Datelike, Days};
use quest_domain::{Command, EntityId};
use rune::{Color, Cursor, El, ScrollSpec, Tx, col, el, row, spacer, text};

use crate::clock;
use crate::dnd::{GridId, Payload};
use crate::format;
use crate::icons;
use crate::state::State;
use crate::theme::{Pal, ty};
use crate::ui::drag::{self, Zone};
use crate::ui::parts;
use crate::ui::timegrid::{self, GridSpec};

pub fn view(s: &State, p: &Pal) -> El {
    let monday = s.calendar_week;
    let week = s.snap.work_between(clock::midnight(monday), clock::midnight(monday + Days::new(7)), s.now);
    let target_ms = i64::from(s.snap.settings.weekly_target_minutes) * 60_000;
    let quest_hours = row()
        .gap(14.0)
        .center_y()
        .child(
            row()
                .gap(6.0)
                .items_end()
                .child(text(format::hours(week.quest_ms)).key("calendar-quest-hours").font(ty::STAT).color(p.text))
                .child(text(format!("/ {} quest", format::hours(target_ms))).font(ty::BODY.lh(26.0)).color(p.muted)),
        )
        .child(parts::progress_bar(
            "calendar-progress",
            160.0,
            6.0,
            week.quest_ms as f32 / target_ms.max(1) as f32,
            p.accent,
            p.bar_track,
        ));
    let header = row()
        .w_full()
        .center_y()
        .gap(12.0)
        .pb(24.0)
        .child(text("Calendar").font(ty::HEADING).color(p.text).mr(12.0))
        .child(parts::icon_button(
            "calendar-prev",
            "Previous week",
            &icons::CHEVRON_LEFT,
            p.muted,
            p,
            |s: &mut State, _: &mut Tx| s.calendar_week = s.calendar_week - Days::new(7),
        ))
        .child(parts::period_label("calendar-range", format::week_range(monday), ty::BODY_MEDIUM, p.text))
        .child(parts::icon_button(
            "calendar-next",
            "Next week",
            &icons::CHEVRON_RIGHT,
            p.muted,
            p,
            |s: &mut State, _: &mut Tx| s.calendar_week = s.calendar_week + Days::new(7),
        ))
        .child(parts::button("calendar-today", "This week", Some(&icons::CALENDAR), p, |s: &mut State, _: &mut Tx| {
            s.calendar_week = s.this_week()
        }))
        .child(parts::button("calendar-log-work", "Log work", None, p, |s: &mut State, tx: &mut Tx| {
            s.open_log_work_now();
            tx.focus_when_shown("log-work-task", false);
        }))
        .child(spacer())
        .child(quest_hours);

    let days: Vec<_> = State::week_days(monday).collect();
    let today = s.today();
    let mut day_heads = row().w_full().pl(64.0).h(56.0);
    for (i, day) in days.iter().enumerate() {
        let on = *day == today;
        // All tracked work: the day's total matches the sessions drawn under it.
        let total = s.snap.work_between(clock::midnight(*day), clock::midnight(*day + Days::new(1)), s.now).total_ms();
        let name = row()
            .gap(6.0)
            .child(text(day.format("%a").to_string()).font(ty::BODY_MEDIUM).color(if on { p.text } else { p.muted }))
            .child(text(day.day().to_string()).font(ty::BODY).color(if on { p.accent } else { p.faint }));
        day_heads =
            day_heads.child(col().grow(1.0).center().gap(2.0).child(name).child(
                text(format::hours(total)).key(format!("calendar-day-total-{i}")).font(ty::SMALL).color(p.faint),
            ));
    }
    let spec = GridSpec { key: "calendar-grid", grid: GridId::Calendar, days, scale: timegrid::CALENDAR_SCALE };
    let scroller = col().w_full().child(timegrid::grid(s, &spec, p)).scroll(ScrollSpec { stick_bottom: false });
    let grid = parts::card(p)
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .clip_rounded(12.0)
        .fill(p.grid)
        .child(day_heads.fill(p.card).border_sides([0.0, 0.0, 1.0, 0.0], p.border))
        .child(drag::zone(scroller.grow(1.0).min_h(0.0), Zone::GridView(GridId::Calendar)));
    let body = row().w_full().grow(1.0).min_h(0.0).gap(20.0).child(picker(s, p)).child(grid);
    col().key("calendar").w_full().h_full().px(30.0).pt(40.0).pb(16.0).child(header).child(body)
}

/// This week's open tasks, to drag onto the grid.
fn picker(s: &State, p: &Pal) -> El {
    let monday = s.calendar_week;
    let mut list = col().key("calendar-tasks").w_full().gap(6.0);
    let week = State::week_days(monday).collect::<Vec<_>>();
    let open = s.snap.tasks.iter().filter(|t| !t.is_done() && t.day.is_none_or(|d| week.contains(&d)));
    for task in open {
        list = list.child(pick_row(s, task, p));
    }
    let mut panel = col()
        .w(250.0)
        .h_full()
        .min_h(0.0)
        .gap(12.0)
        .child(parts::caps("Schedule", p))
        .child(
            text("Drag a task onto the week. Drag on empty space to log time.").font(ty::SMALL).color(p.faint).wrap(),
        )
        .child(list.scroll(ScrollSpec { stick_bottom: false }).grow(1.0).min_h(0.0));
    if let Some(id) = s.calendar_pick.filter(|id| s.snap.task(*id).is_some_and(|t| t.block.is_some())) {
        panel = panel.child(parts::button(
            "calendar-clear",
            "Clear time block",
            Some(&icons::CLOSE),
            p,
            move |s: &mut State, _: &mut Tx| {
                s.dispatch(Command::SetTaskBlock { id, block: None });
            },
        ));
    }
    panel
}

fn pick_row(s: &State, task: &quest_domain::Task, p: &Pal) -> El {
    let id: EntityId = task.id;
    let picked = s.calendar_pick == Some(id);
    let color = parts::accent_of(parts::quest_of(s, task), p);
    let when = match (task.day, task.block) {
        (Some(day), Some(block)) => format!("{} · {}", day.format("%a"), block.start.format("%H:%M")),
        (Some(day), None) => day.format("%a").to_string(),
        (None, _) => "Backlog".to_owned(),
    };
    let block_minutes = task.block.map(|b| b.minutes as i32);
    let row = row()
        .key(format!("calendar-pick-{id}"))
        .role("radio")
        .label(task.title.clone())
        .checked(picked)
        .focusable(true)
        .w_full()
        .py(8.0)
        .px(10.0)
        .gap(10.0)
        .center_y()
        .radius(8.0)
        .cursor(Cursor::Pointer)
        .fill(if picked { p.selected } else { Color::TRANSPARENT })
        .border(1.0, if picked { p.accent.alpha(0.7) } else { p.border })
        .hover(p.hover)
        .child(el().w(3.0).h(28.0).radius(2.0).fill(color))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(2.0)
                .child(text(task.title.clone()).font(ty::SMALL_MEDIUM).color(p.text).single_line().truncate())
                .child(text(when).font(ty::SMALL).color(p.faint)),
        )
        .on_press(move |s: &mut State| s.calendar_pick = if s.calendar_pick == Some(id) { None } else { Some(id) });
    drag::source(row.cursor(Cursor::Grab), None, move |_, _| Payload::Task { id, block_minutes })
}
