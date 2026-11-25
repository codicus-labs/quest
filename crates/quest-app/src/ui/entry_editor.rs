//! Editor for existing calendar entries.

use rune::{El, Key, KeyEvent, TextEdit, Tx, col, el, row, spacer, text};
use rune_kit::{Button, ButtonVariant, Dialog};

use super::log_work::{boxed, plain_field, quest_chip};
use super::{date_picker, quest_picker, task_picker};
use crate::entry_editor::{self, FieldError};
use crate::log_work;
use crate::state::State;
use crate::state::quest_picker::PickFor;
use crate::theme::{Pal, ty};
use crate::ui::parts;

pub fn layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let times = s.entry_form_times();
    let error = times.err();
    let mut popups = Vec::new();
    let mut body = col().w_full().gap(8.0).px(20.0).pb(20.0);
    body = body.child(parts::caps("Task", p)).child(task_row(s, p, &mut popups));
    body = body.child(parts::caps("Quest", p).mt(8.0)).child(quests(s, p));
    body = body.child(parts::caps("Start", p).mt(8.0)).child(start_row(s, p, &mut popups));
    body = body.child(errors(error, &[FieldError::StartDate, FieldError::Start], p));
    body = body.child(parts::caps("End", p).mt(8.0)).child(end_row(s, p, &mut popups));
    body = body.child(errors(error, &[FieldError::EndDate, FieldError::End, FieldError::EndNotAfterStart], p));
    let duration = match times {
        Ok(t) => entry_editor::duration_label((t.ended_at.unwrap_or(s.now) - t.started_at).num_milliseconds()),
        Err(_) => "–".to_owned(),
    };
    body = body.child(
        row()
            .gap(8.0)
            .mt(4.0)
            .child(text("Duration").font(ty::SMALL).color(p.faint))
            .child(text(duration).key("entry-editor-duration").font(ty::SMALL_MEDIUM).color(p.text)),
    );
    let k = &p.kit;
    let buttons = row()
        .w_full()
        .gap(10.0)
        .mt(16.0)
        .child(
            Button::new("Delete")
                .variant(ButtonVariant::GhostMuted)
                .on_press_tx(|s: &mut State, tx: &mut Tx| {
                    tx.blur();
                    s.delete_edited_entry();
                })
                .build(k)
                .key("entry-editor-delete"),
        )
        .child(spacer())
        .child(
            Button::new("Cancel")
                .variant(ButtonVariant::Ghost)
                .on_press(|s: &mut State| s.close_entry_editor())
                .build(k)
                .key("entry-editor-cancel"),
        )
        .child(
            Button::new("Save")
                .variant(ButtonVariant::Contrast)
                .disabled(times.is_err())
                .on_press(|s: &mut State| s.save_entry_editor())
                .build(k)
                .key("entry-editor-save"),
        );
    Dialog::new("entry-editor", "Edit time entry")
        .open(true)
        .fit()
        .width(560.0)
        .on_close(|s: &mut State| {
            if s.date_picker.take().is_some() {
                return;
            }
            if s.entry_form.task_picker.is_open() {
                s.entry_form.task_picker.close();
                return;
            }
            s.close_entry_editor();
        })
        .child(body.child(buttons))
        .build(k, layer);
    layer.extend(popups);
}

fn task_row(s: &State, p: &Pal, popups: &mut Vec<El>) -> El {
    let combo = task_picker::combo(task_picker::Form::Entry, s, p, popups);
    let mut line = row().w_full().gap(10.0).center_y().child(combo);
    if s.entry_form.task.is_some() || s.entry_form.new_task.is_some() {
        line = line.child(parts::icon_button(
            "entry-editor-clear-task",
            "Clear task",
            &crate::icons::CLOSE,
            p.muted,
            p,
            |s: &mut State, _: &mut Tx| s.pick_entry_task(None),
        ));
    }
    if s.entry_form.task.and_then(|task| s.snap.task(task)).is_some() {
        line = line.child(
            Button::new("Open task")
                .variant(ButtonVariant::Outline)
                .on_press_tx(|s: &mut State, tx: &mut Tx| {
                    tx.blur();
                    s.open_edited_entry_task();
                })
                .build(&p.kit)
                .key("entry-editor-open-task"),
        );
    }
    line
}

fn quests(s: &State, p: &Pal) -> El {
    let quest = s.entry_form.quest.and_then(|id| s.snap.quest(id));
    let chip = quest_chip("entry-editor-quest", quest, "No quest", p);
    let label = format!("Quest: {}", quest.map_or("No quest", |q| q.title.as_str()));
    row().child(quest_picker::trigger(chip, PickFor::Entry, label, s, p))
}

fn start_row(s: &State, p: &Pal, popups: &mut Vec<El>) -> El {
    let form = &s.entry_form;
    let date = field(
        "entry-editor-start-date",
        &form.start_date,
        DATE_HINT,
        p,
        |s| &mut s.entry_form.start_date,
        State::follow_entry_start_date,
    )
    .label("Start date");
    let selected = log_work::parse_day(&form.start_date.text, s.today());
    let date = date_picker::field(
        date,
        "entry-start-picker",
        "Choose start date",
        selected,
        Some(s.today()),
        s,
        p,
        |s, day| {
            s.entry_form.start_date = TextEdit::new(log_work::date_label(day));
            s.follow_entry_start_date();
        },
        popups,
    );
    let time =
        field("entry-editor-start", &form.start, "14:50", p, |s| &mut s.entry_form.start, |_| {}).label("Start time");
    row().w_full().gap(12.0).child(date_box(date)).child(time_box(time, p))
}

fn end_row(s: &State, p: &Pal, popups: &mut Vec<El>) -> El {
    let form = &s.entry_form;
    if form.running {
        let running = text("running").key("entry-editor-end-running").font(ty::TASK).color(p.muted);
        return boxed(row().grow(1.0).child(running), p);
    }
    let time = field("entry-editor-end", &form.end, "15:05", p, |s| &mut s.entry_form.end, |_| {}).label("End time");
    let mut line = row().w_full().gap(12.0);
    if !form.end_follows {
        let date = field("entry-editor-end-date", &form.end_date, DATE_HINT, p, |s| &mut s.entry_form.end_date, |_| {})
            .label("End date");
        let selected = log_work::parse_day(&form.end_date.text, s.today());
        let date = date_picker::field(
            date,
            "entry-end-picker",
            "Choose end date",
            selected,
            Some(s.today()),
            s,
            p,
            |s, day| s.entry_form.end_date = TextEdit::new(log_work::date_label(day)),
            popups,
        );
        line = line.child(date_box(date));
    }
    let toggle = Button::new("Ends on a different day")
        .variant(ButtonVariant::Ghost)
        .on_press(|s: &mut State| {
            s.entry_form.end_follows = !s.entry_form.end_follows;
            s.follow_entry_start_date();
        })
        .build(&p.kit)
        .key("entry-editor-end-on-different-day")
        .role("checkbox")
        .checked(!form.end_follows);
    col().w_full().gap(4.0).child(line.child(time_box(time, p))).child(row().child(toggle))
}

const DATE_HINT: &str = "YYYY-MM-DD";

/// A field that saves on ⌘↵; a change clears a refused save's note and then runs `changed`.
fn field(
    key: &str,
    edit: &TextEdit,
    placeholder: &str,
    p: &Pal,
    bind: fn(&mut State) -> &mut TextEdit,
    changed: fn(&mut State),
) -> El {
    plain_field(key, edit, placeholder, p, bind)
        .on_input_change(move |s: &mut State, _: &mut Tx| {
            changed(s);
        })
        .on_input_key(save_on_command_return)
}

fn date_box(field: El) -> El {
    field.w(0.0).grow(1.4).min_w(0.0)
}

fn time_box(field: El, p: &Pal) -> El {
    boxed(field, p).w(0.0).grow(1.0).min_w(0.0)
}

fn save_on_command_return(s: &mut State, ev: &KeyEvent, _: &mut Tx) -> bool {
    if ev.key != Key::Enter || !ev.mods.cmd {
        return false;
    }
    s.save_entry_editor();
    true
}

/// The error of the row's fields, if it is one of `own`.
fn errors(error: Option<FieldError>, own: &[FieldError], p: &Pal) -> El {
    match error.filter(|e| own.contains(e)) {
        Some(e) => text(e.message()).key("entry-editor-error").font(ty::SMALL).color(p.accent).wrap(),
        None => el(),
    }
}
