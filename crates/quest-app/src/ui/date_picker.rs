use chrono::{Datelike, Days, Months, NaiveDate};
use rune::{El, Tx, col, row, spacer, text};
use rune_kit::{Button, ButtonVariant, Placement, Popover};

use crate::icons;
use crate::state::State;
use crate::theme::{Pal, ty};
use crate::ui::parts;

#[derive(Clone, Copy)]
pub(crate) struct Picker {
    pub key: &'static str,
    month: NaiveDate,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn field(
    input: El,
    key: &'static str,
    label: &'static str,
    selected: Option<NaiveDate>,
    latest: Option<NaiveDate>,
    s: &State,
    p: &Pal,
    pick: fn(&mut State, NaiveDate),
    popups: &mut Vec<El>,
) -> El {
    let picker = s.date_picker.filter(|picker| picker.key == key);
    let month = picker.map_or_else(|| first(selected.unwrap_or_else(|| s.today())), |picker| picker.month);
    let trigger = parts::icon_button(
        format!("{key}-button"),
        label,
        &icons::DATE,
        p.muted,
        p,
        move |s: &mut State, tx: &mut Tx| {
            s.date_picker =
                if s.date_picker.is_some_and(|picker| picker.key == key) { None } else { Some(Picker { key, month }) };
            tx.blur();
        },
    );
    let mut popup = Popover::new(key)
        .trigger(trigger)
        .label(label)
        .placement(Placement::BottomEnd)
        .open(picker.is_some())
        .on_close(|s: &mut State| s.date_picker = None);
    if picker.is_some() {
        popup = popup.child(grid(key, month, selected, latest, s.today(), pick, p));
    }
    let trigger = popup.build(&p.kit, popups);
    row()
        .w_full()
        .h(42.0)
        .pl(12.0)
        .pr(4.0)
        .gap(4.0)
        .center_y()
        .radius(8.0)
        .fill(p.bg)
        .border(1.0, p.border_strong)
        .child(input)
        .child(trigger)
}

fn first(day: NaiveDate) -> NaiveDate {
    day.with_day(1).unwrap_or(day)
}

fn grid(
    key: &'static str,
    month: NaiveDate,
    selected: Option<NaiveDate>,
    latest: Option<NaiveDate>,
    today: NaiveDate,
    pick: fn(&mut State, NaiveDate),
    p: &Pal,
) -> El {
    let shift = |label, glyph, to: Option<NaiveDate>| {
        let id = format!("{key}-{label}");
        parts::icon_button(&id, label, glyph, p.muted, p, move |s: &mut State, _: &mut Tx| {
            if let Some(month) = to {
                s.date_picker = Some(Picker { key, month });
            }
        })
        .disabled(to.is_none())
    };
    let prev = month.checked_sub_months(Months::new(1));
    let next = month.checked_add_months(Months::new(1)).filter(|day| latest.is_none_or(|max| *day <= max));
    let mut grid = col().w(252.0).gap(4.0).child(
        row()
            .w_full()
            .center_y()
            .child(shift("Previous month", &icons::CHEVRON_LEFT, prev))
            .child(spacer())
            .child(text(month.format("%B %Y").to_string()).font(ty::SMALL_MEDIUM).color(p.text))
            .child(spacer())
            .child(shift("Next month", &icons::CHEVRON_RIGHT, next)),
    );
    let mut weekdays = row();
    for label in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] {
        weekdays = weekdays.child(row().w(36.0).h(24.0).center().child(text(label).font(ty::SMALL).color(p.faint)));
    }
    grid = grid.child(weekdays);
    let offset = i64::from(month.weekday().num_days_from_monday());
    for week in 0..6 {
        let mut line = row();
        for weekday in 0..7 {
            let day_offset = week * 7 + weekday - offset;
            let day = if day_offset < 0 {
                month.checked_sub_days(Days::new(day_offset.unsigned_abs()))
            } else {
                month.checked_add_days(Days::new(day_offset.unsigned_abs()))
            };
            let Some(day) = day else {
                line = line.child(row().w(36.0));
                continue;
            };
            let is_selected = selected == Some(day);
            let disabled = latest.is_some_and(|max| day > max);
            let color = if disabled || day.month() != month.month() { p.faint } else { p.text };
            let mut cell = row()
                .key(format!("{key}-day-{day}"))
                .role("button")
                .label(day.format("%A, %B %-d, %Y").to_string())
                .checked(is_selected)
                .w(36.0)
                .h(32.0)
                .center()
                .radius(6.0)
                .disabled(disabled);
            if is_selected {
                cell = cell.fill(p.chip);
            } else if day == today {
                cell = cell.border(1.0, p.border_strong);
            }
            cell = cell.child(text(day.day().to_string()).font(ty::SMALL).color(color));
            if !disabled {
                cell =
                    cell.focusable(true).cursor(rune::Cursor::Pointer).hover(p.hover).on_press(move |s: &mut State| {
                        s.date_picker = None;
                        pick(s, day);
                    });
            }
            line = line.child(cell);
        }
        grid = grid.child(line);
    }
    grid.child(
        Button::new("Today")
            .variant(ButtonVariant::Ghost)
            .on_press(move |s: &mut State| {
                let today = s.today();
                s.date_picker = None;
                pick(s, today);
            })
            .disabled(latest.is_some_and(|day| today > day))
            .build(&p.kit),
    )
}
