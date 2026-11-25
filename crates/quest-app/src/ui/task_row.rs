use quest_domain::{EntityId, Task, TaskList};
use quest_view::row_meta::{self, Tone};
use rune::{Color, Cursor, El, Tx, col, el, icon, row, text};

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

pub fn open(s: &mut State, id: EntityId, tx: &mut Tx) {
    tx.blur();
    s.selected_task = Some(id);
    s.toggle_task_detail(id);
    if s.task_modal.is_some() {
        tx.scroll_into_view(format!("row-{id}"));
    }
}

pub fn task_row(s: &State, task: &Task, shows: Shows, p: &Pal) -> El {
    build(s, task, shows, p, None)
}

pub fn in_list(s: &State, task: &Task, shows: Shows, p: &Pal, list: TaskList) -> El {
    build(s, task, shows, p, Some(list))
}

fn build(s: &State, task: &Task, shows: Shows, p: &Pal, list: Option<TaskList>) -> El {
    let id = task.id;
    let line = row()
        .key(format!("row-{id}"))
        .role("listitem")
        .label(task.title.clone())
        .w_full()
        .child(drag::face(s, id, face(s, task, shows, p, "row")))
        .children(super::menu::anchor(s, id))
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
        Some(list) if !task.is_done() => drag::source(line.cursor(Cursor::Grab), list, id),
        _ => line.on_press_tx(move |s: &mut State, tx: &mut Tx| open(s, id, tx)),
    };
    let detail = (!s.drag.dragging().is_some_and(|payload| payload.task == id))
        .then(|| super::task_modal::inline(s, task, p, false))
        .flatten();
    col().key(format!("task-item-{id}")).w_full().child(line).children(detail)
}

pub fn tray_row(s: &State, task: &Task, shows: Shows, p: &Pal) -> El {
    let id = task.id;
    let Some(quest) = task.quest_id else { return task_row(s, task, shows, p) };
    let list = TaskList::Quest(quest);
    let row = if quest_domain::is_today(task, s.today()) {
        let alias = row()
            .key(format!("not-planned-{id}"))
            .role("listitem")
            .label(task.title.clone())
            .w_full()
            .child(face(s, task, shows, p, "not-planned"));
        drag::source(alias.cursor(Cursor::Grab), list, id)
    } else {
        in_list(s, task, shows, p, list)
    };
    if s.drag.dragging().is_some_and(|payload| payload.task == id) { row.absolute(0.0, 0.0).opacity(0.0) } else { row }
}

pub fn face(s: &State, task: &Task, shows: Shows, p: &Pal, prefix: &str) -> El {
    let id = task.id;
    let selected = s.selected_task == Some(id);
    let mut title = text(task.title.clone())
        .key(format!("{prefix}-title-{id}"))
        .font(ty::BODY)
        .color(if task.is_done() { p.muted } else { p.text })
        .single_line()
        .truncate()
        .grow(1.0)
        .min_w(0.0);
    if task.is_done() {
        title = title.strike(0, task.title.len(), Some(p.muted));
    }
    let mut line = row()
        .w_full()
        .min_h(38.0)
        .px(10.0)
        .gap(12.0)
        .center_y()
        .radius(7.0)
        .hover(p.hover)
        .cursor(Cursor::Default)
        .child(check(format!("{prefix}-check-{id}"), id, task.is_done(), p))
        .child(title)
        .child(marks(s, task, shows, p))
        .children((!task.is_done()).then(|| {
            let running = s.snap.running_entry().is_some_and(|e| e.task_id == Some(id));
            parts::timer_button(format!("{prefix}-play-{id}"), id, running, p)
        }));
    if selected {
        line = line.fill(p.selected).border_sides([0.0, 0.0, 0.0, 3.0], p.accent_line);
    }
    line
}

/// The round check: an empty ring, or the accent fill with a dark tick.
fn check(key: String, id: EntityId, done: bool, p: &Pal) -> El {
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
        .on_press(move |s: &mut State| s.toggle_done(id))
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
