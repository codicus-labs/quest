use chrono::{Datelike, Local, NaiveDate};
use quest_domain::{EntityId, TaskList};
use quest_view::plan::{self, Day, Plan};
use rune::{Cursor, El, Tx, col, icon, row, spacer, text};

use super::{
    drag::{self, Zone},
    lists, parts,
    task_row::{self, Shows},
};
use crate::dnd::ListId;
use crate::icons;
use crate::state::State;
use crate::theme::{Pal, ty};

const SHOWS: Shows = Shows { quest: true, plan_ahead: false };

pub fn model(s: &State) -> Plan<'_> {
    plan::build(&s.snap, s.today(), s.now, &Local)
}

pub fn shown_tasks(s: &State) -> Vec<EntityId> {
    let plan = model(s);
    lists::today_groups(s)
        .into_iter()
        .flat_map(|group| group.tasks.into_iter().map(|task| task.id))
        .chain(plan.all_days().skip(1).flat_map(|day| day.tasks.iter().map(|task| task.id)))
        .chain(
            plan.not_planned
                .iter()
                .filter(|task| s.show_not_planned && !quest_domain::is_today(task, s.today()))
                .map(|task| task.id),
        )
        .collect()
}

pub fn jump_today(s: &mut State, tx: &mut Tx) {
    tx.blur();
    s.selected_task = None;
    s.close_task_modal();
    tx.scroll_to_y("plan-page-scroll", 0.0);
}

pub fn view(s: &State, p: &Pal) -> El {
    let plan = model(s);
    let header = lists::header(&icons::PLAN, p.icon_plan, "Plan", None, p);
    let body = col()
        .w_full()
        .mt(8.0)
        .children(plan.days.iter().map(|day| section(s, day, p)))
        .children(plan.later.iter().map(|month| {
            let label = if month.start.year() == s.today().year() {
                month.start.format("%B").to_string()
            } else {
                month.start.format("%B %Y").to_string()
            };
            col()
                .key(format!("plan-month-{}", month.start))
                .w_full()
                .mt(14.0)
                .child(
                    text(if month.partial { format!("Rest of {label}") } else { label })
                        .font(ty::H1)
                        .color(p.text)
                        .mb(6.0),
                )
                .children(month.days.iter().map(|day| section(s, day, p)))
        }))
        .child(not_planned(s, &plan, p));
    lists::page(s, "plan-page", header, None, body)
}

fn section(s: &State, day: &Day<'_>, p: &Pal) -> El {
    let date = day.date;
    let today = date == s.today();
    let list = if today { TaskList::Today } else { TaskList::Day(date) };
    let label = if today {
        "Today".into()
    } else if s.today().succ_opt() == Some(date) {
        "Tomorrow".into()
    } else {
        date.format("%A").to_string()
    };
    let ink = if today { p.accent } else { p.text };
    let header = row()
        .key(format!("plan-head-{date}"))
        .w_full()
        .min_h(38.0)
        .center_y()
        .gap(10.0)
        .hover_group()
        .child(text(date.day().to_string()).font(ty::H1).color(ink))
        .child(text(label).font(ty::GROUP).color(ink))
        .child(spacer())
        .child(
            parts::icon_button(
                format!("plan-add-{date}"),
                "Add a task",
                &icons::ADD,
                ink,
                p,
                move |s: &mut State, tx: &mut Tx| {
                    open_add(s, date, tx);
                },
            )
            .group_opacity(0.0, 1.0),
        );
    let mut groups = if today {
        lists::today_groups(s).into_iter()
    } else {
        vec![lists::Group { head: None, tasks: day.tasks.iter().map(|task| task.as_ref()).collect() }].into_iter()
    };
    let first = groups.next().expect("a day always has an open group");
    let rows = drag::rows(s, p, list, &first.tasks, SHOWS);
    drag::zone(
        col()
            .key(format!("plan-day-{date}"))
            .w_full()
            .py(3.0)
            .border_sides([0.0, 0.0, 1.0, 0.0], p.border)
            .child(header)
            .children(if s.plan_quick_day == Some(date) { super::quick_add::inline(s, p) } else { None })
            .child(rows)
            .children(groups.map(|group| lists::group_rows(s, group, SHOWS, p)))
            .children(day.due.iter().map(|task| {
                row()
                    .key(format!("plan-due-{}", task.id))
                    .gap(7.0)
                    .center_y()
                    .px(10.0)
                    .py(5.0)
                    .child(text("⚑").font(ty::META).color(p.faint))
                    .child(text(format!("Due: {}", task.title)).font(ty::META).color(p.faint).single_line().truncate())
            })),
        Zone::List(ListId::Rows(list)),
    )
}

pub fn open_add(s: &mut State, date: NaiveDate, tx: &mut Tx) {
    if s.plan_quick_day != Some(date) {
        s.reset_quick_add();
        s.plan_quick_day = Some(date);
    }
    tx.focus_when_shown("quick-add", true);
}

fn not_planned(s: &State, plan: &Plan<'_>, p: &Pal) -> El {
    col()
        .w_full()
        .mt(14.0)
        .child(
            row()
                .key("plan-not-planned")
                .role("button")
                .label("Not planned")
                .checked(s.show_not_planned)
                .focusable(true)
                .w_full()
                .h(38.0)
                .px(8.0)
                .gap(8.0)
                .center_y()
                .radius(7.0)
                .hover(p.hover)
                .cursor(Cursor::Pointer)
                .child(
                    icon(if s.show_not_planned { &icons::CHEVRON_DOWN } else { &icons::CHEVRON_RIGHT }, 16.0)
                        .color(p.faint),
                )
                .child(text("Not planned").font(ty::GROUP).color(p.text))
                .child(text(plan.not_planned.len().to_string()).font(ty::META).color(p.faint))
                .on_press(|s: &mut State| s.show_not_planned = !s.show_not_planned),
        )
        .children(
            plan.not_planned.iter().filter(|_| s.show_not_planned).map(|task| task_row::tray_row(s, task, SHOWS, p)),
        )
}
