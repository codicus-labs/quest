//! The list pages: Today, Inbox and Logbook, and the frame a quest's page shares with them
//! (centred column, header, quick-add, rows in groups). What each lists comes from
//! `quest_view::lists`.

use chrono::Local;
use quest_domain::{EntityId, Task, TaskList};
use quest_view::{lists, row_meta, tracked};
use rune::{Color, El, Icon, ScrollSpec, col, icon, row, text};

use crate::dnd::ListId;
use crate::format;
use crate::icons;
use crate::state::{State, View};
use crate::theme::{Pal, ty};
use crate::ui::{
    drag::{self, Zone},
    task_row::{self, Shows},
};

/// Rows under an optional head ("3 done", a Logbook day).
pub struct Group<'a> {
    pub head: Option<String>,
    pub tasks: Vec<&'a Task>,
}

/// What the current view lists, top to bottom; empty for views without rows.
pub fn groups(s: &State) -> Vec<Group<'_>> {
    let snap = &s.snap;
    let today = s.today();
    match s.view {
        View::Today => today_groups(s),
        View::Inbox => vec![Group { head: None, tasks: lists::inbox_with_linger(snap, |task| s.lingering(task)) }],
        View::Logbook => lists::logbook_days(snap, &Local)
            .into_iter()
            .map(|(day, tasks)| Group { head: Some(row_meta::day_name(day, today)), tasks })
            .collect(),
        View::Quest(quest) => open_then_done(
            snap.tasks
                .iter()
                .filter(|task| {
                    let before = s.lingering(task).unwrap_or(task);
                    before.quest_id == Some(quest) && !before.is_done()
                })
                .collect(),
            lists::quest_done(snap, quest).into_iter().filter(|t| s.lingering(t).is_none()).collect(),
        ),
        _ => Vec::new(),
    }
}

pub fn today_groups(s: &State) -> Vec<Group<'_>> {
    let snap = &s.snap;
    let today = s.today();
    open_then_done(
        lists::today_with_linger(snap, today, |task| s.lingering(task)),
        lists::done_today(snap, today, &Local).into_iter().filter(|t| s.lingering(t).is_none()).collect(),
    )
}

pub fn sort_list(view: View) -> Option<TaskList> {
    match view {
        View::Today => Some(TaskList::Today),
        View::Inbox => Some(TaskList::Inbox),
        View::Quest(id) => Some(TaskList::Quest(id)),
        _ => None,
    }
}

/// Open tasks, then "n done" with the done ones when there are any.
fn open_then_done<'a>(open: Vec<&'a Task>, done: Vec<&'a Task>) -> Vec<Group<'a>> {
    let mut groups = vec![Group { head: None, tasks: open }];
    if !done.is_empty() {
        groups.push(Group { head: Some(format!("{} done", done.len())), tasks: done });
    }
    groups
}

/// The rows J/K step through, in order.
pub fn shown_tasks(s: &State) -> Vec<EntityId> {
    if s.view == View::Plan {
        return super::plan::shown_tasks(s);
    }
    groups(s).iter().flat_map(|g| g.tasks.iter().map(|t| t.id)).collect()
}

pub fn view(s: &State, p: &Pal) -> El {
    let (header, add, empty) = match s.view {
        View::Today => (today_header(s, p), Some(super::task_fields::quick_add(s, p, "Add to Today")), "All clear"),
        View::Inbox => (
            header(&icons::INBOX, p.icon_inbox, "Inbox", None, p),
            Some(super::task_fields::quick_add(s, p, "Add to Inbox")),
            "Inbox zero",
        ),
        _ => (header(&icons::LOGBOOK, p.icon_logbook, "Logbook", None, p), None, "Nothing done yet"),
    };
    let shows = Shows { quest: true, plan_ahead: s.view != View::Today };
    page("list-page", header, add, rows(s, shows, empty, p))
}

/// The centred column: header, quick-add, then `body`, scrolling together.
pub fn page(key: &str, header: El, add: Option<El>, body: El) -> El {
    let column = col().w_full().max_w(720.0).px(40.0).pt(4.0).pb(80.0).child(header).children(add).child(body);
    col().key(key.to_owned()).w_full().h_full().child(drag::zone(
        col()
            .key(format!("{key}-scroll"))
            .w_full()
            .items_center()
            .child(column)
            .scroll(ScrollSpec { stick_bottom: false })
            .grow(1.0)
            .min_h(0.0)
            .scroll_edge_fade(12.0),
        Zone::Viewport,
    ))
}

/// The view's groups as rows; `empty` when there are none at all.
pub fn rows(s: &State, shows: Shows, empty: &str, p: &Pal) -> El {
    let groups = groups(s);
    let mut list = col().key("rows").w_full().mt(10.0);
    if groups.iter().all(|g| g.tasks.is_empty()) {
        return list.child(text(empty.to_owned()).key("rows-empty").font(ty::BODY).color(p.faint).py(18.0));
    }
    for group in groups {
        if group.head.is_none()
            && let Some(id) = sort_list(s.view)
        {
            list = list.child(drag::zone(drag::rows(s, p, id, &group.tasks, shows), Zone::List(ListId::Rows(id))));
        } else {
            list = list.child(group_rows(s, group, shows, p));
        }
    }
    list
}

pub fn group_rows(s: &State, group: Group<'_>, shows: Shows, p: &Pal) -> El {
    col()
        .w_full()
        .children(group.head.map(|head| {
            row()
                .w_full()
                .mt(16.0)
                .mb(4.0)
                .pb(6.0)
                .border_sides([0.0, 0.0, 1.0, 0.0], p.border)
                .child(text(head).font(ty::GROUP).color(p.muted))
        }))
        .children(group.tasks.into_iter().map(|task| task_row::task_row(s, task, shows, p)))
}

/// Icon and title, with an optional muted line under it.
pub fn header(glyph: &'static Icon, ink: Color, title: &str, sub: Option<String>, p: &Pal) -> El {
    let title = row()
        .gap(12.0)
        .center_y()
        .child(icon(glyph, 26.0).color(ink))
        .child(text(title.to_owned()).key("page-title").font(ty::H1).color(p.text));
    let mut head = col().w_full().gap(4.0).child(title);
    if let Some(sub) = sub {
        head = head.child(text(sub).key("page-sub").font(ty::BODY).color(p.muted).ml(38.0));
    }
    head
}

fn today_header(s: &State, p: &Pal) -> El {
    let tracked = tracked::day_timeline(&s.snap, s.today(), &Local, s.now).tracked_ms;
    let sub = format!("{} · {} tracked", format::long_day(s.today()), format::hours(tracked));
    header(&icons::TODAY, p.accent, "Today", Some(sub), p)
}
