//! Task details and their editing controls.

use quest_domain::{EntityId, Task, TimeEntry};
use rune::{
    ClipboardContent, Color, Cursor, El, Key, KeyEvent, ScrollSpec, TextStyle, Tx, col, el, icon, row, spacer, text,
    text_input,
};

use crate::clock;
use crate::format;
use crate::icons;
use crate::state::quest_picker::PickFor;
use crate::state::task_modal::is_saved;
use crate::state::{State, TaskModal};
use crate::theme::{Pal, ty};
use crate::ui::{parts, quest_picker};

const TITLE: TextStyle = TextStyle::new(24.0, 500.0).lh(32.0);

pub fn layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let Some(form) = &s.task_modal else { return };
    let Some(task) = s.plan_task(form.task) else { return };
    let status = if is_saved(&task, form) { "Saved" } else { "Saving…" };
    let header = row()
        .w_full()
        .h(56.0)
        .pl(24.0)
        .pr(14.0)
        .gap(12.0)
        .center_y()
        .child(parts::caps("Task", p))
        .child(spacer())
        .child(text(status).key("task-modal-status").font(ty::SMALL).color(p.faint))
        .child(parts::icon_button("task-modal-close", "Close", &icons::CLOSE, p.muted, p, |s: &mut State, tx| {
            tx.blur();
            s.close_task_modal();
        }));
    let mut popups = Vec::new();
    let body =
        row().w_full().grow(1.0).min_h(0.0).child(main(s, &task, form, p)).child(side(s, &task, form, p, &mut popups));
    let panel = col()
        .key("task-modal")
        .role("dialog")
        .label("Task")
        .w_full()
        .max_w(900.0)
        .h_full()
        .max_h(620.0)
        .radius(14.0)
        .fill(p.card_raised)
        .border(1.0, p.border_strong)
        // Inside the border: the right column's fill would cover it.
        .padding(1.0)
        .clip_rounded(14.0)
        .shadow(20.0, 60.0, Color::BLACK.alpha(0.45))
        .child(header)
        .child(el().w_full().h(1.0).fill(p.border))
        .child(body)
        .modal_tx(true, |s: &mut State, tx: &mut Tx| {
            if s.date_picker.take().is_none() {
                tx.blur();
                s.close_task_modal();
            }
        });
    layer.push(el().key("task-modal-scrim").absolute(0.0, 0.0).w_full().h_full().fill(p.scrim).a11y_hidden());
    layer.push(row().key("task-modal-layer").absolute(0.0, 0.0).w_full().h_full().padding(40.0).center().child(panel));
    layer.extend(popups);
}

/// The done check, the title (one line however it wraps) and the description.
fn main(s: &State, task: &Task, form: &TaskModal, p: &Pal) -> El {
    let id = task.id;
    let color = parts::accent_of(parts::quest_of(s, task), p);
    let title = text_input(&form.title, TITLE)
        .key("task-modal-title")
        .label("Task title")
        .grow(1.0)
        .min_w(0.0)
        .color(p.text)
        .placeholder("Task title", p.faint)
        .on_edit(|s: &mut State| &mut s.task_modal_fields().title)
        .on_input_change(|s: &mut State, _: &mut Tx| s.save_task_title())
        .on_submit(|_: &mut State, tx: &mut Tx| tx.blur())
        // Shift/⌥ Return would break the line.
        .on_input_key(|_: &mut State, ev: &KeyEvent, _: &mut Tx| ev.key == Key::Enter && (ev.mods.shift || ev.mods.alt))
        .on_input_paste(|s: &mut State, content: &ClipboardContent, _: &mut Tx| {
            s.task_modal_fields().title.insert(&quest_view::home::one_line(content.text()));
            s.save_task_title();
            true
        });
    let check =
        parts::check("task-modal-done".to_owned(), task.is_done(), color, p, move |s: &mut State| s.toggle_done(id));
    let description = super::notes::field(
        "task-modal-description",
        &form.description,
        p,
        |s| &mut s.task_modal_fields().description,
        State::save_task_description,
    )
    .grow(1.0)
    .min_h(0.0);
    col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .padding(28.0)
        .gap(20.0)
        .child(row().w_full().gap(14.0).items_start().child(check.mt(4.0)).child(title))
        .child(el().w_full().h(1.0).fill(p.border))
        .child(description)
}

fn side(s: &State, task: &Task, form: &TaskModal, p: &Pal, popups: &mut Vec<El>) -> El {
    let mut side =
        col().w(300.0).h_full().padding(22.0).gap(6.0).fill(p.bar).border_sides([0.0, 0.0, 0.0, 1.0], p.border);
    side = side.child(parts::caps("Date", p)).child(date(s, form, task.day, p, popups));
    let repeat = super::recurrence::task_row(s, task, p);
    side = side.child(parts::caps("Repeat", p).mt(8.0)).child(repeat);
    let quest = task.quest_id.and_then(|q| s.snap.quest(q));
    let chip = super::log_work::quest_chip("task-modal-quest", quest, "No quest", p);
    let label = format!("Quest: {}", quest.map_or("No quest", |q| q.title.as_str()));
    side = side.child(parts::caps("Quest", p).mt(8.0)).child(row().child(quest_picker::trigger(
        chip,
        PickFor::TaskModal,
        label,
        s,
        p,
    )));
    side.child(el().w_full().h(1.0).fill(p.border).mt(8.0))
        .child(logged(s, task.id, p))
        .child(el().w_full().h(1.0).fill(p.border))
        .child(delete(p))
        .scroll(ScrollSpec { stick_bottom: false })
}

/// The day as "Nov 24, 2026" (Return or leaving the field takes it; empty is the backlog), and
/// Backlog.
fn date(s: &State, form: &TaskModal, day: Option<chrono::NaiveDate>, p: &Pal, popups: &mut Vec<El>) -> El {
    let field = text_input(&form.date, ty::BODY)
        .key("task-modal-date")
        .label("Date")
        .single_line()
        .grow(1.0)
        .min_w(0.0)
        .color(p.text)
        .placeholder("Backlog", p.faint)
        .on_edit(|s: &mut State| &mut s.task_modal_fields().date)
        .on_submit(|s: &mut State, tx: &mut Tx| {
            tx.blur();
            s.save_task_day();
        });
    let field = super::date_picker::field(
        field,
        "task-date-picker",
        "Choose date",
        day,
        None,
        s,
        p,
        |s, day| {
            s.task_modal_fields().date = rune::TextEdit::new(crate::log_work::date_label(day));
            s.save_task_day();
        },
        popups,
    );
    let backlog = text("Move to backlog")
        .key("task-modal-backlog")
        .role("button")
        .label("Move to backlog")
        .focusable(true)
        .font(ty::SMALL)
        .color(p.muted)
        .cursor(Cursor::Pointer)
        .on_press(|s: &mut State| s.move_task_to_backlog());
    let mut date = col().w_full().gap(6.0).child(field);
    if day.is_some() {
        date = date.child(backlog);
    }
    date
}

/// The total and each session, newest first; editing them is the calendar's.
fn logged(s: &State, task: EntityId, p: &Pal) -> El {
    let total = format!("{} total", format::hours(s.snap.task_tracked_ms(task, s.now)));
    let mut logged = col().key("task-modal-logged").w_full().gap(6.0).child(
        row()
            .w_full()
            .center_y()
            .child(parts::caps("Logged time", p))
            .child(spacer())
            .child(text(total).key("task-modal-logged-total").font(ty::SMALL).color(p.faint)),
    );
    let mut entries: Vec<&TimeEntry> = s.snap.time_entries.iter().filter(|e| e.task_id == Some(task)).collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.started_at));
    for entry in entries.into_iter().take(4) {
        logged = logged.child(entry_row(s, entry, p));
    }
    logged
}

fn entry_row(s: &State, entry: &TimeEntry, p: &Pal) -> El {
    let start = clock::local(entry.started_at);
    let end = entry.ended_at.map_or("now".to_owned(), |end| clock::local(end).format("%H:%M").to_string());
    let when = format!("{} · {}–{end}", start.format("%b %-d"), start.format("%H:%M"));
    row()
        .w_full()
        .center_y()
        .child(text(when).font(ty::SMALL).color(p.muted))
        .child(spacer())
        .child(text(format::timer(entry.tracked_ms(s.now))).font(ty::SMALL).color(p.faint))
}

fn delete(p: &Pal) -> El {
    row()
        .key("task-modal-delete")
        .role("button")
        .label("Delete task")
        .focusable(true)
        .h(36.0)
        .px(10.0)
        .gap(10.0)
        .center_y()
        .radius(8.0)
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .child(icon(&icons::TRASH, 15.0).color(p.muted))
        .child(text("Delete task").font(ty::BODY).color(p.muted))
        .on_press_tx(|s: &mut State, tx: &mut Tx| {
            tx.blur();
            s.delete_modal_task();
        })
}
