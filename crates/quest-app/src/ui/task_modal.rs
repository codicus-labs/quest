//! Task details and their editing controls.

use quest_domain::{EntityId, Task, TimeEntry};
use rune::{Cursor, El, Key, KeyEvent, Tx, col, icon, row, spacer, text, text_input};

use crate::state::State;
use crate::theme::{Pal, ty};
use crate::{clock, format, icons};

pub fn title(form: &crate::state::TaskModal, p: &Pal) -> El {
    text_input(&form.title, ty::BODY)
        .key("task-modal-title")
        .label("Task title")
        .w_full()
        .single_line()
        .color(p.text)
        .on_edit(|s: &mut State| &mut s.task_modal_fields().title)
        .on_input_change(|s: &mut State, _: &mut Tx| s.save_task_title())
        .on_input_key(inline_keys)
        .on_submit(close)
        .transition(rune::Transition::none())
}

pub fn inline(s: &State, task: &Task, p: &Pal) -> El {
    let form = s.task_modal.as_ref().expect("an expanded row owns the detail");
    let note = text_input(&form.description, ty::BODY)
        .key("task-modal-description")
        .label("Note")
        .w_full()
        .min_h(38.0)
        .color(p.text)
        .placeholder("Note", p.faint)
        .on_edit(|s: &mut State| &mut s.task_modal_fields().description)
        .on_input_change(|s: &mut State, _: &mut Tx| s.save_task_description())
        .on_input_key(inline_keys)
        .on_submit(|s: &mut State, _: &mut Tx| {
            s.task_modal_fields().description.insert("\n");
            s.save_task_description();
        });
    let mut detail = col()
        .key("task-modal")
        .w_full()
        .gap(8.0)
        .pl(42.0)
        .pr(10.0)
        .pb(10.0)
        .on_background_press(|_: &mut State| {})
        .child(note)
        .child(super::task_fields::fields(s, Some(task), p, false));
    let total = s.snap.task_tracked_ms(task.id, s.now);
    if total > 0 {
        let chevron = if s.show_task_time { "⌄" } else { "›" };
        let label = format!("◷ {} {chevron}", quest_view::entry_editor::duration_label(total));
        detail = detail.child(
            row()
                .key("task-time")
                .role("button")
                .label(label.clone())
                .expanded(s.show_task_time)
                .focusable(true)
                .self_start()
                .h(26.0)
                .px(4.0)
                .center_y()
                .radius(7.0)
                .cursor(Cursor::Pointer)
                .hover(p.hover)
                .child(text(label).font(ty::SMALL).color(p.muted))
                .on_key(|s: &mut State, event, tx| {
                    if event.key != Key::Enter || event.mods != rune::Mods::default() {
                        return false;
                    }
                    if !tx.key_repeat() {
                        s.show_task_time = !s.show_task_time;
                    }
                    true
                })
                .on_press(|s: &mut State| s.show_task_time = !s.show_task_time),
        );
        if s.show_task_time {
            let mut entries: Vec<_> = s.snap.time_entries.iter().filter(|e| e.task_id == Some(task.id)).collect();
            entries.sort_by_key(|e| std::cmp::Reverse(e.started_at));
            for entry in entries {
                let id = entry.id;
                detail = detail.child(
                    entry_row(s, entry, p)
                        .key(format!("task-time-{id}"))
                        .role("button")
                        .label("Edit time entry")
                        .focusable(true)
                        .hover(p.hover)
                        .on_press(move |s: &mut State| s.open_entry_editor(id)),
                );
            }
        }
    }
    detail.child(super::recurrence::task_row(s, task, p)).child(delete(p))
}

fn inline_keys(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    if event.key != Key::Escape {
        return false;
    }
    close(s, tx);
    true
}

pub fn close(s: &mut State, tx: &mut Tx) {
    if s.task_modal.is_some() {
        tx.blur();
        s.close_task_modal();
        tx.animated();
    }
}

pub fn close_other(s: &mut State, id: EntityId, tray_alias: bool, tx: &mut Tx) {
    if s.task_modal.as_ref().is_some_and(|form| form.task != id || form.tray_alias != tray_alias) {
        close(s, tx);
    }
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
        .h(26.0)
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
