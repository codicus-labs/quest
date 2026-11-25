use quest_domain::{EntityId, Task, TaskList};
use quest_view::row_meta::{self, Tone};
use rune::{Color, Cursor, El, Transition, Tx, col, el, icon, row, text};

use crate::icons;
use crate::state::State;
use crate::theme::{self, Pal, quest_color, ty};
use crate::ui::{drag, parts};

/// What a row shows besides its dates.
#[derive(Clone, Copy)]
pub struct Shows {
    /// The quest's dot and name (not on that quest's own page).
    pub quest: bool,
    /// A plan date ahead (not where every task is planned for today or earlier).
    pub plan_ahead: bool,
}

pub fn open(s: &mut State, id: EntityId, tray_alias: bool, tx: &mut Tx) {
    tx.blur();
    let already_open = s.task_modal.as_ref().is_some_and(|form| form.task == id && form.tray_alias == tray_alias);
    s.close_task_modal();
    if !already_open {
        s.open_task_modal(id);
        if let Some(form) = &mut s.task_modal {
            form.tray_alias = tray_alias;
        }
    }
    s.selected_task = Some(id);
    tx.animated();
}

pub fn task_row(s: &State, task: &Task, shows: Shows, p: &Pal) -> El {
    build(s, task, shows, p, None, false)
}

pub fn in_list(s: &State, task: &Task, shows: Shows, p: &Pal, list: TaskList) -> El {
    build(s, task, shows, p, Some(list), false)
}

fn build(s: &State, task: &Task, shows: Shows, p: &Pal, list: Option<TaskList>, tray_alias: bool) -> El {
    let id = task.id;
    let prefix = if tray_alias { "not-planned" } else { "row" };
    let expanded = s.task_modal.as_ref().filter(|form| form.task == id && form.tray_alias == tray_alias);
    let line = row()
        .key(format!("{prefix}-{id}"))
        .role("listitem")
        .label(task.title.clone())
        .w_full()
        .child(drag::face(s, id, tray_alias, head(s, task, shows, p, prefix, expanded, tray_alias)))
        .children((!tray_alias).then(|| super::menu::anchor(s, id)).into_iter().flatten())
        .on_secondary_press(move |s: &mut State, _, _: &mut Tx| super::menu::toggle(s, id))
        .on_hover_change(move |s: &mut State, inside: bool, tx: &mut Tx| {
            if inside {
                s.hovered_task = Some(id);
            } else if s.hovered_task == Some(id) {
                s.hovered_task = None;
            }
            tx.live_only();
        });
    let line = match list {
        Some(list) if !task.is_done() && expanded.is_none() => {
            drag::source(line.cursor(Cursor::Grab), list, id, tray_alias)
        }
        _ => line.on_press_tx(move |s: &mut State, tx: &mut Tx| open(s, id, tray_alias, tx)),
    };
    let mut card = col().key(format!("task-item-{id}")).w_full().radius(8.0);
    if expanded.is_some() {
        card = card.fill(p.card).child(
            el().key("task-outline").absolute(0.0, 0.0).w_full().h_full().radius(8.0).border(1.0, p.border).inert(),
        );
    }
    card.child(line).children(expanded.map(|_| super::task_modal::inline(s, task, p)))
}

pub fn tray_row(s: &State, task: &Task, shows: Shows, p: &Pal) -> El {
    let id = task.id;
    let Some(quest) = task.quest_id else { return task_row(s, task, shows, p) };
    let list = TaskList::Quest(quest);
    let is_alias = quest_domain::is_today(task, s.today());
    let row = build(s, task, shows, p, Some(list), is_alias);
    if s.drag.dragging().is_some_and(|payload| payload.task == id) && is_alias == s.drag_tray_alias {
        row.absolute(0.0, 0.0).opacity(0.0)
    } else {
        row
    }
}

pub fn face(s: &State, task: &Task, shows: Shows, p: &Pal, prefix: &str) -> El {
    head(s, task, shows, p, prefix, None, false)
}

fn head(
    s: &State,
    task: &Task,
    shows: Shows,
    p: &Pal,
    prefix: &str,
    form: Option<&crate::state::TaskModal>,
    tray_alias: bool,
) -> El {
    let id = task.id;
    let selected = s.selected_task == Some(id) && form.is_none();
    let mut label = text(task.title.clone())
        .key(format!("{prefix}-title-{id}"))
        .font(ty::BODY)
        .color(if task.is_done() { p.muted } else { p.text })
        .single_line()
        .truncate()
        .transition(Transition::none());
    if task.is_done() {
        label = label.strike(0, task.title.len(), Some(p.muted));
    }
    let title = form.map_or(label, |form| super::task_modal::title(form, p));
    let mut line = row()
        .w_full()
        .min_h(38.0)
        .px(10.0)
        .gap(12.0)
        .center_y()
        .radius(7.0)
        .hover(p.hover)
        .cursor(Cursor::Default)
        .child(check(format!("{prefix}-check-{id}"), id, task.is_done(), tray_alias, p))
        .child(title.grow(1.0).w(0.0).min_w(0.0))
        .children(form.is_none().then(|| marks(s, task, shows, p)))
        .children((!task.is_done()).then(|| {
            let running = s.snap.running_entry().is_some_and(|e| e.task_id == Some(id));
            parts::timer_button(format!("{prefix}-play-{id}"), id, running, tray_alias, p)
        }));
    if selected {
        line = line.fill(p.selected).child(
            el().key("selection-rail")
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .radius(7.0)
                .border_sides([0.0, 0.0, 0.0, 3.0], p.accent_line)
                .inert(),
        );
    }
    line
}

/// The round check: an empty ring, or the accent fill with a dark tick.
fn check(key: String, id: EntityId, done: bool, tray_alias: bool, p: &Pal) -> El {
    let (fill, border, ink) = if done {
        (p.accent_fill, p.accent_fill, theme::on_accent_fill())
    } else {
        (Color::TRANSPARENT, p.border_strong, Color::TRANSPARENT)
    };
    el().key(key)
        .role("checkbox")
        .label("Complete")
        .checked(done)
        .focusable(true)
        .size(20.0, 20.0)
        .radius(10.0)
        .fill(fill)
        .border(1.6, border)
        .center()
        .cursor(Cursor::Pointer)
        .child(icon(&icons::CHECK, 14.0).color(ink))
        .on_press_tx(move |s: &mut State, tx| {
            super::task_modal::close_other(s, id, tray_alias, tx);
            s.toggle_done(id);
        })
}

/// Dates and the quest.
fn marks(s: &State, task: &Task, shows: Shows, p: &Pal) -> El {
    let mut marks = row().gap(10.0).center_y().shrink().min_w(0.0);
    for mark in row_meta::date_marks(task, s.today(), shows.plan_ahead) {
        let color = match mark.tone {
            Tone::Muted => p.faint,
            Tone::Accent => p.accent,
            Tone::Overdue => p.error,
        };
        marks = marks.child(text(mark.text).font(ty::META).color(color).single_line());
    }
    if let Some(quest) = parts::quest_of(s, task).filter(|_| shows.quest) {
        marks =
            marks.child(
                row().gap(6.0).center_y().min_w(0.0).shrink().child(parts::dot(quest_color(quest.color), 7.0)).child(
                    text(quest.title.clone()).font(ty::META).color(p.faint).single_line().max_w(160.0).truncate(),
                ),
            );
    }
    marks
}
