//! The Log work dialog: TASK (search, done tasks too, or a new title for a
//! completed task), DATE, START, END, QUEST, Cancel / Log work. Nothing is saved before Log work.

use quest_domain::Quest;
use rune::{El, Key, KeyEvent, TextEdit, Tx, col, row, spacer, text, text_input};
use rune_kit::{Button, ButtonVariant, Dialog};

use super::{date_picker, quest_picker, task_picker};
use crate::log_work::{self, FieldError};
use crate::state::State;
use crate::state::quest_picker::PickFor;
use crate::theme::{Pal, quest_color, ty};
use crate::ui::parts;

pub fn layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let form = &s.log_work;
    let mut body = col().w_full().gap(8.0).px(20.0).pb(20.0);
    let mut popups = Vec::new();
    let task = task_picker::combo(task_picker::Form::LogWork, s, p, &mut popups);
    body = body.child(parts::caps("Task", p)).child(task);
    if let Some(error) = form.error.filter(|e| *e == FieldError::Task) {
        body = body.child(text(error.message()).key("log-work-error").font(ty::SMALL).color(p.accent).wrap());
    }
    body = body.child(times(s, p, &mut popups).mt(12.0));
    body = body.child(parts::caps("Quest", p).mt(12.0)).child(quest_field(s, p));
    let k = &p.kit;
    let buttons = row()
        .w_full()
        .gap(10.0)
        .mt(16.0)
        .child(spacer())
        .child(
            Button::new("Cancel")
                .variant(ButtonVariant::Ghost)
                .on_press(|s: &mut State| s.close_log_work())
                .build(k)
                .key("log-work-cancel"),
        )
        .child(
            Button::new("Log work")
                .variant(ButtonVariant::Contrast)
                .on_press(|s: &mut State| s.save_log_work())
                .build(k)
                .key("log-work-save"),
        );
    Dialog::new("log-work", "Log work")
        .open(true)
        .fit()
        .width(560.0)
        .on_close(|s: &mut State| {
            if s.date_picker.take().is_some() {
                return;
            }
            if s.log_work.task_picker.is_open() {
                s.log_work.task_picker.close();
                return;
            }
            s.close_log_work();
        })
        .child(body.child(buttons))
        .build(k, layer);
    layer.extend(popups);
}

/// A single-line field that clears the last error as it changes. Return doesn't save: only
/// Log work (or ⌘↵) does.
fn field(key: &str, edit: &TextEdit, placeholder: &str, p: &Pal, bind: fn(&mut State) -> &mut TextEdit) -> El {
    plain_field(key, edit, placeholder, p, bind)
        .on_input_change(|s: &mut State, _: &mut Tx| s.log_work.error = None)
        .on_input_key(save_on_command_return)
}

/// A dialog's single-line text field, without handlers.
pub(super) fn plain_field(
    key: &str,
    edit: &TextEdit,
    placeholder: &str,
    p: &Pal,
    bind: fn(&mut State) -> &mut TextEdit,
) -> El {
    text_input(edit, ty::TASK)
        .key(key.to_owned())
        .label(placeholder)
        .single_line()
        .grow(1.0)
        .min_w(0.0)
        .color(p.text)
        .placeholder(placeholder, p.faint)
        .on_edit(bind)
}

/// ⌘↵ in any field saves, as the Log work button does.
fn save_on_command_return(s: &mut State, ev: &KeyEvent, _: &mut Tx) -> bool {
    if ev.key != Key::Enter || !ev.mods.cmd {
        return false;
    }
    s.save_log_work();
    true
}

pub(super) fn boxed(field: El, p: &Pal) -> El {
    row().w_full().h(44.0).px(14.0).center_y().radius(8.0).fill(p.bg).border(1.0, p.border_strong).child(field)
}

fn input(key: &str, edit: &TextEdit, placeholder: &str, p: &Pal, bind: fn(&mut State) -> &mut TextEdit) -> El {
    boxed(field(key, edit, placeholder, p, bind), p)
}

/// DATE, START and END side by side (flush with TASK), each with its error under it.
fn times(s: &State, p: &Pal, popups: &mut Vec<El>) -> El {
    let form = &s.log_work;
    let date = field("log-work-date", &form.date, "YYYY-MM-DD", p, |s| &mut s.log_work.date).label("Date");
    let selected = log_work::parse_day(&form.date.text, s.today());
    let date = date_picker::field(
        date,
        "log-work-date-picker",
        "Choose date",
        selected,
        None,
        s,
        p,
        |s, day| {
            s.log_work.date = TextEdit::new(log_work::date_label(day));
            s.log_work.error = None;
        },
        popups,
    );
    let error = form.error;
    let column = |label: &str, field: El, own: &[FieldError], share: f32| {
        let mut c = col().gap(8.0).w(0.0).grow(share).min_w(0.0).child(parts::caps(label, p)).child(field);
        if let Some(e) = error.filter(|e| own.contains(e)) {
            c = c.child(text(e.message()).key("log-work-error").font(ty::SMALL).color(p.accent).wrap());
        }
        c
    };
    row()
        .w_full()
        .gap(12.0)
        .child(column("Date", date, &[FieldError::Date], 1.4))
        .child(column(
            "Start",
            input("log-work-start", &form.start, "14:50", p, |s: &mut State| &mut s.log_work.start),
            &[FieldError::Start],
            1.0,
        ))
        .child(column(
            "End",
            input("log-work-end", &form.end, "15:05", p, |s: &mut State| &mut s.log_work.end),
            &[FieldError::End, FieldError::EndNotAfterStart],
            1.0,
        ))
}

/// A picked task shows its own quest; a new task's quest is chosen here (or none).
fn quest_field(s: &State, p: &Pal) -> El {
    let form = &s.log_work;
    let quest = log_work::quest(&s.snap, form.task, form.quest).and_then(|q| s.snap.quest(q));
    let chip = quest_chip("log-work-quest", quest, "Add quest", p);
    if form.task.is_some() {
        return row().child(chip);
    }
    let label = quest.map_or_else(|| "Choose quest".to_owned(), |q| format!("Quest: {}", q.title));
    row().child(quest_picker::trigger(chip, PickFor::LogWork, label, s, p))
}

/// A quest's chip: its dot and title, or `empty` in a plain outline.
pub(super) fn quest_chip(key: &str, quest: Option<&Quest>, empty: &str, p: &Pal) -> El {
    row()
        .key(key.to_owned())
        .h(30.0)
        .px(12.0)
        .gap(8.0)
        .center_y()
        .radius(15.0)
        .border(1.0, quest.map_or(p.border_strong, |q| quest_color(q.color).alpha(0.5)))
        .child(parts::dot(quest.map_or(p.faint, |q| quest_color(q.color)), 7.0))
        .child(
            text(quest.map_or(empty.to_owned(), |q| q.title.clone()))
                .key(format!("{key}-title"))
                .font(ty::SMALL)
                .color(p.muted),
        )
}
