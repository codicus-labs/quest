use chrono::{Datelike, Days, Weekday};
use quest_domain::{Frequency, RepeatPin, RepeatRule, Task};
use quest_view::repeat::{self, unit};
use rune::{Cursor, El, Key, ScrollSpec, Shadow, TextEdit, Tx, col, icon, row, spacer, text};
use rune_kit::{Button, ButtonVariant, Dialog, Kit, Select, SelectAppearance};

use crate::icons;
use crate::log_work::{date_label, parse_day};
use crate::state::recurrence::{RepeatChoice, RepeatEditor};
use crate::state::{Overlay, State};
use crate::theme::{Pal, ty};

use super::log_work::{boxed, plain_field};
use super::{
    date_picker::{self, DateHost},
    quick_add::{self, Host},
};

const FREQUENCIES: [Frequency; 4] = [Frequency::Day, Frequency::Week, Frequency::Month, Frequency::Year];

fn cadence(rule: RepeatRule) -> String {
    repeat::cadence(rule.every, rule.frequency)
}

pub(super) fn task_chip(s: &State, task: &Task, p: &Pal) -> El {
    let id = task.id;
    let cadence = repeat::series_of(&s.snap, task).repeat.map(|schedule| self::cadence(schedule.rule));
    super::task_fields::active_chip(
        p,
        s.overlay == Overlay::RepeatEditor && s.repeat_editor.series == Some(s.series_of(task).id),
    )
    .key("task-modal-repeat")
    .role("button")
    .label(format!("Repeat: {}", cadence.as_deref().unwrap_or("Does not repeat")))
    .focusable(true)
    .min_w(0.0)
    .max_w(140.0)
    .cursor(Cursor::Pointer)
    .help("Repeat (R / ⌘⇧R)")
    .child(icon(&icons::REPEAT, 15.0).color(p.muted))
    .children(cadence.map(|label| text(label).font(ty::SMALL).color(p.text).min_w(0.0).single_line().truncate()))
    .on_press_tx(move |s: &mut State, tx: &mut Tx| {
        tx.blur();
        s.open_repeat_editor(id);
    })
}

pub fn editor_layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let form = &s.repeat_editor;
    let Some(task) = form.series.and_then(|id| s.snap.task(id)) else {
        Dialog::new("repeat-editor", "Repeat task")
            .open(true)
            .fit()
            .on_close(|s: &mut State| s.close_repeat_editor())
            .child(text("This task is no longer available.").font(ty::BODY).color(p.muted).padding(20.0))
            .build(&p.kit, layer);
        return;
    };
    let (body, popups) = editor(
        s,
        p,
        task.title.clone(),
        task.repeat.is_some(),
        |s, _| s.save_repeat_editor(),
        |s, _| s.stop_repeating(),
        |s, _| s.close_repeat_editor(),
    );
    dialog(
        "repeat-editor",
        "⟳ Repeat",
        body,
        |s| {
            if s.date_picker.take().is_none() {
                s.close_repeat_editor();
            }
        },
        p,
        layer,
    );
    layer.extend(popups);
}

pub(crate) trait RepeatHost: DateHost {
    fn repeat_form(&self) -> &RepeatEditor;
    fn repeat_form_mut(&mut self) -> &mut RepeatEditor;
}

impl RepeatHost for State {
    fn repeat_form(&self) -> &RepeatEditor {
        self.quick.repeat.as_ref().unwrap_or(&self.repeat_editor)
    }
    fn repeat_form_mut(&mut self) -> &mut RepeatEditor {
        self.quick.repeat.as_mut().unwrap_or(&mut self.repeat_editor)
    }
}

pub(super) fn quick_chip<S: Host>(s: &S, p: &Pal) -> El {
    let label = quick_add::rule(s).map(cadence);
    let active = s.quick().repeat.is_some();
    super::task_fields::active_chip(p, active)
        .key("quick-add-repeat")
        .role("button")
        .label(format!("Repeat: {}", label.as_deref().unwrap_or("Does not repeat")))
        .expanded(active)
        .focusable(true)
        .min_w(0.0)
        .max_w(140.0)
        .cursor(Cursor::Pointer)
        .help("Repeat (⌘⇧R)")
        .child(icon(&icons::REPEAT, 15.0).color(p.muted))
        .children(label.map(|label| text(label).font(ty::SMALL).color(p.text).min_w(0.0).single_line().truncate()))
        .on_press_tx(|s: &mut S, tx| s.open_field(super::task_fields::Picker::Repeat, tx))
}

pub(super) fn quick_layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let (body, popups) = quick_editor(s, p);
    Dialog::new("quick-repeat-dialog", "⟳ Repeat")
        .open(true)
        .fit()
        .width(510.0)
        .on_close(|s: &mut State| s.quick.close_picker())
        .child(body)
        .build(&p.kit, layer);
    layer.extend(popups);
}

pub fn quick_editor<S: Host>(s: &S, p: &Pal) -> (El, Vec<El>) {
    let title = s.draft().title;
    let title = if title.is_empty() { "New to-do".into() } else { title };
    let (body, popups) = editor(
        s,
        p,
        title,
        quick_add::rule(s).is_some(),
        quick_add::save_repeat::<S>,
        quick_add::stop_repeat::<S>,
        quick_add::escape::<S>,
    );
    let body = body.key("repeat-editor").focusable(true).on_key(|s: &mut S, ev, tx| {
        if ev.key == Key::Enter && (tx.focused() == Some("repeat-editor") || ev.mods.cmd) {
            quick_add::save_repeat(s, tx);
            true
        } else {
            false
        }
    });
    (body, popups)
}

fn editor<S: RepeatHost>(
    s: &S,
    p: &Pal,
    title: String,
    can_stop: bool,
    save: fn(&mut S, &mut Tx),
    stop: fn(&mut S, &mut Tx),
    close: fn(&mut S, &mut Tx),
) -> (El, Vec<El>) {
    let form = s.repeat_form();
    let mut popups = Vec::new();
    let frequency = select(
        "repeat-frequency",
        "Frequency",
        RepeatChoice::Frequency,
        &FREQUENCIES.map(unit),
        unit(form.frequency),
        s,
        160.0,
    )
    .on_choose(|s: &mut S, value, tx| {
        if let Some(frequency) = FREQUENCIES.into_iter().find(|frequency| unit(*frequency) == value) {
            s.repeat_form_mut().frequency = frequency;
        }
        if s.repeat_form().series.is_none() {
            tx.focus_when_shown("repeat-editor", true);
        }
        s.repeat_form_mut().choice = None;
    })
    .build(&field_kit(p), &mut popups);
    let interval = field::<S>("repeat-every", &form.every, "Interval", p, |s| &mut s.repeat_form_mut().every);
    let interval =
        row().w_full().gap(10.0).center_y().child(boxed(interval, p).w(78.0).h(FIELD_H)).child(fill(frequency));
    let pin = select(
        "repeat-pin",
        "Pin",
        RepeatChoice::Pin,
        &["None", "Top", "Bottom"],
        match form.pin {
            RepeatPin::None => "None",
            RepeatPin::Top => "Top",
            RepeatPin::Bottom => "Bottom",
        },
        s,
        250.0,
    )
    .on_choose(|s: &mut S, value, _| {
        s.repeat_form_mut().pin = match value {
            "Top" => RepeatPin::Top,
            "Bottom" => RepeatPin::Bottom,
            _ => RepeatPin::None,
        };
        s.repeat_form_mut().choice = None;
    })
    .build(&field_kit(p), &mut popups);
    let start = date_picker::field(
        field::<S>("repeat-start", &form.starts, "Start date", p, |s| &mut s.repeat_form_mut().starts),
        "repeat-start-picker",
        "Choose start date",
        parse_day(&form.starts.text, s.date_today()),
        None,
        s,
        p,
        |s, day| s.repeat_form_mut().starts = TextEdit::new(date_label(day)),
        &mut popups,
    );
    let end = select(
        "repeat-ends",
        "Ends",
        RepeatChoice::Ends,
        &["Never", "On date"],
        if form.ends_on_date { "On date" } else { "Never" },
        s,
        250.0,
    )
    .on_choose(|s: &mut S, value, _| {
        s.repeat_form_mut().ends_on_date = value == "On date";
        s.repeat_form_mut().choice = None;
    })
    .build(&field_kit(p), &mut popups);
    let mut body = col()
        .w_full()
        .gap(12.0)
        .px(20.0)
        .pb(20.0)
        .child(text(title).font(ty::BODY_MEDIUM).color(p.muted).wrap())
        .child(setting("Every", interval, p));
    match form.frequency {
        Frequency::Week => body = body.child(weekdays(s, p)),
        Frequency::Month => body = body.child(month_day(s, p, &mut popups)),
        _ => {}
    }
    body = body.child(setting("Pin", fill(pin), p)).child(setting("Starts", start, p)).child(setting(
        "Ends",
        fill(end),
        p,
    ));
    if form.ends_on_date {
        let date = date_picker::field(
            field::<S>("repeat-end", &form.ends, "End date", p, |s| &mut s.repeat_form_mut().ends),
            "repeat-end-picker",
            "Choose end date",
            parse_day(&form.ends.text, s.date_today()),
            None,
            s,
            p,
            |s, day| s.repeat_form_mut().ends = TextEdit::new(date_label(day)),
            &mut popups,
        );
        body = body.child(setting("End date", date, p));
    }
    let timezone =
        field::<S>("repeat-timezone", &form.time_zone, "Timezone", p, |s| &mut s.repeat_form_mut().time_zone);
    body = body.child(setting("Timezone", boxed(timezone, p).h(FIELD_H), p));
    if let Some(message) = form.error {
        body = body.child(text(message).key("repeat-error").color(p.accent).font(ty::SMALL).wrap());
    }
    let mut actions = row().w_full().center_y().gap(8.0).mt(8.0);
    if can_stop {
        actions = actions.child(
            Button::new("Stop repeating")
                .variant(ButtonVariant::GhostMuted)
                .on_press_tx(stop)
                .build(&p.kit)
                .key("repeat-stop"),
        );
    }
    actions = actions
        .child(spacer())
        .child(
            Button::new("Cancel").variant(ButtonVariant::Ghost).on_press_tx(close).build(&p.kit).key("repeat-cancel"),
        )
        .child(Button::new("Save").variant(ButtonVariant::Contrast).on_press_tx(save).build(&p.kit).key("repeat-save"));
    (body.child(actions), popups)
}

fn weekdays<S: RepeatHost>(s: &S, p: &Pal) -> El {
    let selected = s.repeat_form().shown_weekdays(s.date_today());
    let mut days = row().gap(5.0);
    for (day, short, full) in [
        (Weekday::Mon, "Mo", "Monday"),
        (Weekday::Tue, "Tu", "Tuesday"),
        (Weekday::Wed, "We", "Wednesday"),
        (Weekday::Thu, "Th", "Thursday"),
        (Weekday::Fri, "Fr", "Friday"),
        (Weekday::Sat, "Sa", "Saturday"),
        (Weekday::Sun, "Su", "Sunday"),
    ] {
        let checked = selected.contains(day);
        days = days.child(
            row()
                .key(format!("repeat-weekday-{short}"))
                .role("checkbox")
                .label(full)
                .checked(checked)
                .focusable(true)
                .size(35.0, 36.0)
                .center()
                .radius(7.0)
                .fill(if checked { p.selected } else { p.bg })
                .border(1.0, p.border)
                .hover(p.hover)
                .cursor(Cursor::Pointer)
                .child(text(short).font(ty::SMALL_MEDIUM).color(if checked { p.text } else { p.muted }))
                .on_press(move |s: &mut S| {
                    let shown = s.repeat_form().shown_weekdays(s.date_today());
                    let form = s.repeat_form_mut();
                    form.weekdays = repeat::toggle_weekday(form.weekdays, shown, day);
                }),
        );
    }
    setting("On", days, p)
}

fn month_day<S: RepeatHost>(s: &S, p: &Pal, popups: &mut Vec<El>) -> El {
    let form = s.repeat_form();
    let month_day = select(
        "repeat-month-day",
        "Monthly date",
        RepeatChoice::MonthDay,
        &["Day of the month", "Last day"],
        if form.last_day { "Last day" } else { "Day of the month" },
        s,
        184.0,
    )
    .on_choose(|s: &mut S, value, _| {
        s.repeat_form_mut().last_day = value == "Last day";
        s.repeat_form_mut().choice = None;
    })
    .build(&field_kit(p), popups);
    let mut controls = row().w_full().gap(10.0).center_y().child(fill(month_day));
    if !form.last_day {
        let default = parse_day(&form.starts.text, s.date_today()).unwrap_or(s.date_today()).day().to_string();
        let input =
            field::<S>("repeat-month-day-number", &form.month_day, &default, p, |s| &mut s.repeat_form_mut().month_day)
                .label("Day of the month");
        controls = controls.child(boxed(input, p).w(72.0).h(FIELD_H));
    }
    setting("On", controls, p)
}

pub fn open_overview(s: &mut State, tx: &mut Tx) {
    tx.blur();
    s.overlay = Overlay::RecurringTasks;
}

pub fn overview_layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let mut scheduled: Vec<_> = s
        .snap
        .tasks
        .iter()
        .filter_map(|task| {
            let schedule = task.repeat?;
            let tomorrow = schedule.rule.date_at(s.now).checked_add_days(Days::new(1));
            let next = tomorrow.and_then(|day| schedule.next_on_or_after(day));
            Some((next, task, schedule.rule))
        })
        .collect();
    scheduled.sort_by_key(|(next, task, _)| (next.is_none(), *next, task.title.as_str(), task.id));
    let mut list = col().w_full().gap(6.0);
    if scheduled.is_empty() {
        list = list
            .child(text("Set a task to repeat from its details or card menu.").font(ty::BODY).color(p.muted).wrap());
    }
    for (next, task, rule) in scheduled {
        let id = task.id;
        let next = next.map_or("Ended".to_owned(), |day| format!("Next: {}", date_label(day)));
        list = list.child(
            row()
                .key(format!("recurring-{id}"))
                .role("button")
                .label(format!("Edit repeat for {}", task.title))
                .focusable(true)
                .w_full()
                .gap(12.0)
                .px(12.0)
                .py(9.0)
                .radius(8.0)
                .center_y()
                .cursor(Cursor::Pointer)
                .hover(p.hover)
                .child(icon(&icons::REPEAT, 16.0).color(p.accent))
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .child(text(task.title.clone()).font(ty::BODY_MEDIUM).color(p.text).single_line().truncate())
                        .child(text(cadence(rule)).font(ty::SMALL).color(p.muted)),
                )
                .child(text(next).font(ty::SMALL).color(p.muted))
                .child(icon(&icons::CHEVRON_RIGHT, 14.0).color(p.faint))
                .on_press(move |s: &mut State| s.open_repeat_editor(id)),
        );
    }
    let body = col().w_full().px(20.0).pb(20.0).child(list.max_h(360.0).scroll(ScrollSpec { stick_bottom: false }));
    dialog("recurring-tasks", "Recurring tasks", body, |s| s.overlay = Overlay::None, p, layer);
}

fn dialog(key: &'static str, title: &'static str, body: El, close: fn(&mut State), p: &Pal, layer: &mut Vec<El>) {
    Dialog::new(key, title).open(true).fit().width(510.0).on_close(close).child(body).build(&p.kit, layer);
}

fn field<S: 'static>(key: &str, edit: &TextEdit, label: &str, p: &Pal, bind: fn(&mut S) -> &mut TextEdit) -> El {
    plain_field(key, edit, label, p, bind).on_submit(|_: &mut S, tx: &mut Tx| tx.blur())
}

/// Height of every control in the dialog, matching the date field.
const FIELD_H: f32 = 42.0;

/// A label and a control that takes the rest of the row.
fn setting(label: &str, field: El, p: &Pal) -> El {
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(text(label.to_owned()).w(78.0).font(ty::BODY).color(p.muted))
        .child(row().grow(1.0).min_w(0.0).child(field))
}

/// Stretches a select to the rest of its row at the dialog's control height, its value as far
/// in as a text field's (`log_work::boxed`: 14 points).
fn fill(select: El) -> El {
    row().grow(1.0).min_w(0.0).child(select.w_full().h(FIELD_H).pl(6.0))
}

/// The kit with its field select drawn like the dialog's text fields (`log_work::boxed`): same
/// fill, 1 point edge, corner radius and type.
fn field_kit(p: &Pal) -> Kit {
    let mut kit = p.kit.clone();
    kit.c.bg_base = p.bg;
    kit.ty.ui_compact = ty::TASK;
    kit.radius.md = 8.0;
    kit.ring = None;
    let edge = Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.0, color: p.border_strong, inset: true };
    kit.elevation = kit.elevation.with_input([edge]);
    kit
}

fn select<S: RepeatHost>(
    key: &str,
    label: &str,
    choice: RepeatChoice,
    options: &[&str],
    current: &str,
    s: &S,
    width: f32,
) -> Select {
    Select::new(key, options)
        .appearance(SelectAppearance::Field)
        .width(width)
        .label(label)
        .current(Some(current))
        .open(s.repeat_form().choice == Some(choice))
        .on_toggle(move |s: &mut S| {
            *s.date_picker_mut() = None;
            s.repeat_form_mut().choice = (s.repeat_form().choice != Some(choice)).then_some(choice);
        })
        .on_close(|s: &mut S| s.repeat_form_mut().choice = None)
}
