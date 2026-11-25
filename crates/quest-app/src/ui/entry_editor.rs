//! The entry editor a timeline block opens: what the session is booked on (a task or
//! Organizing), its start and end, and delete.

use quest_domain::EntityId;
use rune::{Cursor, El, Key, KeyEvent, Tx, col, el, icon, row, spacer, text};
use rune_kit::{Button, ButtonVariant, Combobox, ComboboxOption, Dialog};

use super::log_work::{boxed, plain_field};
use crate::state::State;
use crate::state::entry_editor::Booking;
use crate::theme::{Pal, ty};
use crate::ui::parts;

const ORGANIZING: &str = "organizing";
const NO_TASK: &str = "no-task";

pub fn layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let Some(form) = &s.entry_form else { return };
    let times = s.entry_form_times();
    let mut popups = Vec::new();
    let end: El = if form.running {
        boxed(row().grow(1.0).child(text("running").key("entry-editor-running").font(ty::TASK).color(p.muted)), p)
    } else {
        boxed(time_field("entry-editor-end", &form.end, "End time", p, |s| &mut s.entry_form_mut().end), p)
    };
    let start =
        boxed(time_field("entry-editor-start", &form.start, "Start time", p, |s| &mut s.entry_form_mut().start), p);
    let times_row = row()
        .w_full()
        .gap(12.0)
        .child(col().grow(1.0).w(0.0).gap(8.0).child(parts::caps("Start", p)).child(start))
        .child(col().grow(1.0).w(0.0).gap(8.0).child(parts::caps("End", p)).child(end));
    let error = match &times {
        Err(e) => text(e.message()).key("entry-editor-error").font(ty::SMALL).color(p.error).wrap(),
        Ok(_) => el(),
    };
    let body = col()
        .w_full()
        .gap(8.0)
        .px(20.0)
        .pb(20.0)
        .child(parts::caps("Booked on", p))
        .child(booking(s, p, &mut popups))
        .child(times_row.mt(8.0))
        .child(error)
        .child(buttons(times.is_ok(), p));
    Dialog::new("entry-editor", "Edit session")
        .open(true)
        .fit()
        .width(480.0)
        .on_close(|s: &mut State| {
            if s.entry_form_mut().picker.is_open() {
                s.entry_form_mut().picker.close();
                return;
            }
            s.close_entry_editor();
        })
        .child(body)
        .build(&p.kit, layer);
    layer.extend(popups);
}

fn buttons(can_save: bool, p: &Pal) -> El {
    let k = &p.kit;
    row()
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
                .disabled(!can_save)
                .on_press(|s: &mut State| s.save_entry_editor())
                .build(k)
                .key("entry-editor-save"),
        )
}

/// Organizing, or one of the tasks (open ones first), searchable.
fn booking(s: &State, p: &Pal, popups: &mut Vec<El>) -> El {
    let Some(form) = &s.entry_form else { return el() };
    let state = &form.picker;
    let query = state.search().text.trim();
    let hits = (!query.is_empty()).then(|| crate::log_work::suggestions(&s.snap, query));
    let mut tasks: Vec<_> = s
        .snap
        .tasks
        .iter()
        .filter(|t| {
            form.booking == Booking::Task(Some(t.id))
                || hits.as_ref().is_none_or(|hits| hits.iter().any(|hit| hit.id == t.id))
        })
        .collect();
    tasks.sort_by_key(|t| (t.is_done(), std::cmp::Reverse(t.completed_at)));
    let mut options = vec![ComboboxOption::new(ORGANIZING, "Organizing")];
    if form.booking == Booking::Task(None) {
        options.push(ComboboxOption::new(NO_TASK, "No task"));
    }
    options.extend(tasks.into_iter().map(|t| {
        let mut option = ComboboxOption::new(t.id.to_string(), &t.title);
        option.detail = if t.is_done() { "Done" } else { "Open" }.into();
        option
    }));
    let (current, shown) = match form.booking {
        Booking::Organizing => (ORGANIZING.to_owned(), "Organizing".to_owned()),
        Booking::Task(None) => (NO_TASK.to_owned(), "No task".to_owned()),
        Booking::Task(Some(id)) => {
            (id.to_string(), s.snap.task(id).map_or_else(|| "Deleted task".to_owned(), |t| t.title.clone()))
        }
    };
    let trigger = row()
        .key("entry-editor-booking-value")
        .role("combobox")
        .label(format!("Booked on: {shown}"))
        .focusable(true)
        .w_full()
        .h(44.0)
        .px(14.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(p.bg)
        .border(1.0, p.border_strong)
        .hover(p.hover)
        .cursor(Cursor::Pointer)
        .child(text(shown).font(ty::BODY).color(p.text).single_line().truncate().grow(1.0).min_w(0.0).a11y_hidden())
        .child(icon(p.kit.glyph.chevron_down, 14.0).color(p.muted).a11y_hidden());
    Combobox::new("entry-editor-booking", state, &options, |s: &mut State| &mut s.entry_form_mut().picker)
        .label("Booked on")
        .trigger(trigger)
        .current(Some(current.as_str()))
        .search_placeholder("Search tasks…")
        .width(360.0)
        .on_choose(|s, id, _: &mut Tx| choose(s, id))
        .build(&p.kit, popups)
        .w_full()
}

fn choose(s: &mut State, id: &str) {
    let booking = match id {
        ORGANIZING => Booking::Organizing,
        NO_TASK => Booking::Task(None),
        id => match id.parse::<EntityId>() {
            Ok(task) => Booking::Task(Some(task)),
            Err(_) => return,
        },
    };
    s.book_entry(booking);
}

/// A time field; ⌘↵ saves.
fn time_field(
    key: &str,
    edit: &rune::TextEdit,
    label: &str,
    p: &Pal,
    bind: fn(&mut State) -> &mut rune::TextEdit,
) -> El {
    plain_field(key, edit, "14:50", p, bind).label(label).on_input_key(save_on_command_return)
}

fn save_on_command_return(s: &mut State, ev: &KeyEvent, _: &mut Tx) -> bool {
    if ev.key != Key::Enter || !ev.mods.cmd {
        return false;
    }
    s.save_entry_editor();
    true
}
