//! A list row (Today, Inbox, Logbook, a quest's list): check and title, and on the right the
//! muted marks (dates, the quest, the running timer). A click selects it; a double click or ↵
//! opens the task's dialog.

use quest_domain::{EntityId, Task};
use quest_view::row_meta::{self, Tone};
use rune::{Color, Cursor, El, Tx, el, icon, row, text};

use crate::format;
use crate::icons;
use crate::state::State;
use crate::theme::{self, Pal, quest_color, ty};
use crate::ui::parts;

/// What a row shows besides its dates.
#[derive(Clone, Copy)]
pub struct Shows {
    /// The quest's dot and name (not on that quest's own page).
    pub quest: bool,
    /// A plan date ahead (not where every task is planned for today or earlier).
    pub plan_ahead: bool,
}

pub fn task_row(s: &State, task: &Task, shows: Shows, p: &Pal) -> El {
    let id = task.id;
    let selected = s.selected_task == Some(id);
    let title = text(task.title.clone())
        .key(format!("row-title-{id}"))
        .font(ty::BODY)
        .color(if task.is_done() { p.muted } else { p.text })
        .single_line()
        .truncate()
        .grow(1.0)
        .min_w(0.0);
    let mut line = row()
        .key(format!("row-{id}"))
        .role("listitem")
        .label(task.title.clone())
        .w_full()
        .min_h(38.0)
        .px(10.0)
        .gap(12.0)
        .center_y()
        .radius(7.0)
        .hover(p.hover)
        .cursor(Cursor::Default)
        .on_hover_change(move |s: &mut State, inside: bool, tx: &mut Tx| {
            // The task shortcuts read this; nothing on screen does.
            if inside {
                s.hovered_task = Some(id);
            } else if s.hovered_task == Some(id) {
                s.hovered_task = None;
            }
            tx.live_only();
        })
        .on_press_tx(move |s: &mut State, tx: &mut Tx| {
            tx.blur();
            s.selected_task = Some(id);
        })
        .on_double_press(move |s: &mut State| s.open_task_modal(id))
        .child(check(id, task.is_done(), p))
        .child(title)
        .child(marks(s, task, shows, p));
    if selected {
        line = line.fill(p.selected).border_sides([0.0, 0.0, 0.0, 3.0], p.accent_line);
    }
    line
}

/// The round check: an empty ring, or the accent fill with a dark tick.
fn check(id: EntityId, done: bool, p: &Pal) -> El {
    let (fill, border, ink) = if done {
        (p.accent_fill, p.accent_fill, theme::on_accent_fill())
    } else {
        (Color::TRANSPARENT, p.border_strong, Color::TRANSPARENT)
    };
    el().key(format!("row-check-{id}"))
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

/// Dates, the quest, and the running timer as an accent pill.
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
    let running = s.snap.running_entry().filter(|e| e.task_id == Some(task.id));
    if let Some(entry) = running {
        marks = marks.child(timer_pill(task.id, format::timer(entry.tracked_ms(s.now)), p));
    }
    marks
}

fn timer_pill(id: EntityId, time: String, p: &Pal) -> El {
    row()
        .key(format!("row-timer-{id}"))
        .role("button")
        .label("Stop timer")
        .h(22.0)
        .px(9.0)
        .center_y()
        .radius(11.0)
        .fill(p.accent_fill)
        .cursor(Cursor::Pointer)
        .help("Stop  Space")
        .child(text(time).font(ty::META).color(theme::on_accent_fill()))
        .on_press(|s: &mut State| s.stop_focus())
}
