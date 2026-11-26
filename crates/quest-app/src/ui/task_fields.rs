use super::quick_add::{self, Host};
use chrono::Datelike;
use quest_domain::{DateField, EntityId, Task};
use quest_view::{
    calendar::{self, QuickDate, Step},
    lists,
};
use rune::{Cursor, Cx, El, Key, KeyEvent, Tx, col, icon, row, spacer, text};
use rune_kit::{Placement, popover};

use super::parts;
use crate::icons;
use crate::state::quest_picker::PickFor;
use crate::state::task_edits::field_key;
use crate::state::{Overlay, State};
use crate::theme::{Pal, quest_color, ty};

pub trait CalendarHost: 'static {
    fn today(&self) -> chrono::NaiveDate;
    fn calendar(&self) -> Option<crate::state::task_edits::Calendar>;
    fn calendar_mut(&mut self) -> &mut Option<crate::state::task_edits::Calendar>;
    fn field_dates(&self, task: Option<EntityId>) -> Option<quest_domain::TaskDates>;
    fn close_calendar(&mut self, tx: &mut Tx);
    fn choose_calendar(&mut self, day: Option<chrono::NaiveDate>, tx: &mut Tx);
}

impl CalendarHost for State {
    fn today(&self) -> chrono::NaiveDate {
        State::today(self)
    }
    fn calendar(&self) -> Option<crate::state::task_edits::Calendar> {
        self.quick.calendar.or(self.task_calendar).filter(|_| self.overlay == Overlay::None)
    }
    fn calendar_mut(&mut self) -> &mut Option<crate::state::task_edits::Calendar> {
        if self.quick.calendar.is_some() { &mut self.quick.calendar } else { &mut self.task_calendar }
    }
    fn field_dates(&self, task: Option<EntityId>) -> Option<quest_domain::TaskDates> {
        task.map_or_else(|| Some(self.draft().fields.dates), |id| State::field_dates(self, id))
    }
    fn close_calendar(&mut self, tx: &mut Tx) {
        if self.quick.calendar.is_some() {
            quick_add::escape(self, tx);
        } else {
            State::close_calendar(self, tx);
        }
    }
    fn choose_calendar(&mut self, day: Option<chrono::NaiveDate>, tx: &mut Tx) {
        if self.quick.calendar.is_some() {
            quick_add::choose_date(self, day, tx);
        } else {
            State::choose_calendar(self, day, tx);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picker {
    Quest,
    Date(DateField),
    Repeat,
}

impl Picker {
    pub fn quick_letter(letter: char) -> Option<Self> {
        if letter.eq_ignore_ascii_case(&'r') { Some(Self::Repeat) } else { Self::from_letter(letter) }
    }
    pub fn from_letter(letter: char) -> Option<Self> {
        match letter.to_ascii_lowercase() {
            'l' => Some(Self::Quest),
            'p' => Some(Self::Date(DateField::Plan)),
            'd' => Some(Self::Date(DateField::Due)),
            _ => None,
        }
    }
}

pub fn open(s: &mut State, id: EntityId, picker: Picker, tx: &mut Tx) {
    if s.task_modal.as_ref().is_none_or(|form| form.task != id) {
        s.open_task_modal(id);
        tx.scroll_into_view("task-modal");
    }
    s.quick.close_picker();
    match picker {
        Picker::Quest => {
            s.toggle_quest_picker(PickFor::TaskModal);
            if s.quest_picker.is_some() {
                tx.focus_when_shown("quest-picker-search", false);
            }
        }
        Picker::Date(field) => s.open_calendar(id, field, tx),
        Picker::Repeat => s.open_repeat_editor(id),
    }
}

pub fn detail_fields(s: &State, task: &Task, p: &Pal) -> El {
    field_line(s, Some(task.id), task.quest_id, task.dates(), lists::postponed(&s.snap, task.id), p, open_detail)
        .w_full()
        .child(super::recurrence::task_chip(s, task, p))
        .child(spacer().min_w(0.0))
        .child(
            parts::icon_button("task-modal-delete", "Delete task", &icons::TRASH, p.muted, p, |s, tx| {
                tx.blur();
                s.delete_modal_task();
            })
            .help("Delete (⌘⌫)"),
        )
}

fn field_line<S: Host>(
    s: &S,
    id: Option<EntityId>,
    quest: Option<EntityId>,
    dates: quest_domain::TaskDates,
    postponed: lists::Postponed,
    p: &Pal,
    open: fn(&mut S, Option<EntityId>, Picker, &mut Tx),
) -> El {
    let quest = quest.and_then(|id| s.snapshot().quest(id));
    let target = if id.is_some() { PickFor::TaskModal } else { PickFor::QuickAdd };
    let quest_open = s.picker().is_some_and(|picker| picker.target == target);
    let button = active_chip(p, quest_open)
        .max_w(160.0)
        .min_w(0.0)
        .child(parts::dot(quest.map_or(p.border_strong, |q| quest_color(q.color)), 7.0))
        .child(
            text(quest.map_or("Quest", |q| &q.title))
                .key(format!("{}-title", target.trigger_key()))
                .font(ty::SMALL)
                .color(if quest.is_some() { p.text } else { p.faint })
                .min_w(0.0)
                .single_line()
                .truncate(),
        )
        .child(icon(&icons::CHEVRON_DOWN, 12.0).color(p.faint));
    let quest = button
        .key(target.trigger_key())
        .role("combobox")
        .label(quest.map_or("Quest", |q| &q.title))
        .expanded(quest_open)
        .focusable(true)
        .cursor(Cursor::Pointer)
        .on_press_tx(move |s: &mut S, tx| open(s, id, Picker::Quest, tx));
    let mut line = row().center_y();
    line = line
        .key(if id.is_none() { "quick-add-fields" } else { "task-fields" })
        .gap(7.0)
        .on_key(move |s: &mut S, ev, tx| {
            let Key::Text(letter) = &ev.key else { return false };
            if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt {
                return false;
            }
            let Some(picker) = letter.chars().next().and_then(Picker::quick_letter) else { return false };
            open(s, id, picker, tx);
            true
        })
        .child(quest);
    for (field, label, glyph, count) in
        [(DateField::Plan, "Plan", "▦", postponed.plan), (DateField::Due, "Deadline", "⚑", postponed.due)]
    {
        let selected = dates.get(field);
        let value = selected.map_or_else(|| label.to_owned(), |day| day.format("%a %-d %b").to_string());
        let key = field_key(id, field);
        let overdue = field == DateField::Due && selected.is_some_and(|day| day < s.today());
        let (ink, glyph_ink) = if overdue {
            (p.error, p.error)
        } else if selected.is_some() {
            (p.text, p.muted)
        } else {
            (p.faint, p.faint)
        };
        let active = s.calendar().is_some_and(|open| open.anchor() == key);
        let button = active_chip(p, active)
            .key(key)
            .expanded(active)
            .role("button")
            .label(format!("{label}: {value}"))
            .focusable(true)
            .cursor(Cursor::Pointer)
            .child(text(glyph).font(ty::SMALL).color(glyph_ink))
            .child(text(value).font(ty::SMALL).color(ink).single_line())
            .on_press_tx(move |s: &mut S, tx: &mut Tx| open(s, id, Picker::Date(field), tx));
        line = line.child(
            row()
                .gap(4.0)
                .center_y()
                .child(button)
                .children((count > 0).then(|| text(format!("↷ {count}")).font(ty::SMALL).color(p.muted))),
        );
    }
    line
}

fn open_detail(s: &mut State, task: Option<EntityId>, picker: Picker, tx: &mut Tx) {
    if let Some(task) = task {
        open(s, task, picker, tx);
    }
}

pub(super) fn quick_fields<S: Host>(s: &S, fields: quest_view::quick_add::Fields, p: &Pal) -> El {
    field_line(s, None, fields.quest, fields.dates, lists::Postponed::default(), p, |s, _, picker, tx| {
        s.open_field(picker, tx);
    })
    .child(super::recurrence::quick_chip(s, p))
}

pub(super) fn active_chip(p: &Pal, active: bool) -> El {
    let chip = field_chip(p);
    if active { chip.fill(p.selected).border(2.0, p.accent_line) } else { chip }
}

pub(super) fn field_chip(p: &Pal) -> El {
    row()
        .h(28.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .radius(8.0)
        .border(1.0, p.border)
        .hover(p.hover)
        .focus_ring(2.0, 2.0, p.text)
}

pub fn layer<S: CalendarHost>(s: &S, p: &Pal, cx: &Cx<S>, layer: &mut Vec<El>) {
    let Some(open) = s.calendar() else { return };
    let Some(panel) = calendar_surface(s, p) else { return };
    let anchor = open.anchor();
    let placement = match (cx.window_layout(&anchor), cx.window_layout("app")) {
        (Some([x, y, w, h]), Some([_, _, window_w, window_h])) => {
            let above = window_h - (y + h) < 315.0 && y > window_h - (y + h);
            if !open.review && window_w - x >= 380.0 {
                if above { Placement::TopStart } else { Placement::BottomStart }
            } else if x + w >= 380.0 {
                if above { Placement::TopEnd } else { Placement::BottomEnd }
            } else {
                Placement::BottomStart
            }
        }
        _ => Placement::BottomStart,
    };
    layer.push(popover::anchored(&p.kit, panel, &anchor, placement, 4.0));
}

pub(crate) fn calendar_surface<S: CalendarHost>(s: &S, p: &Pal) -> Option<El> {
    let open = s.calendar()?;
    let dates = s.field_dates(open.task)?;
    let today = s.today();
    let anchor = open.anchor();
    let heading = if open.field == DateField::Plan { "▦ Plan" } else { "⚑ Deadline" };
    let button = |key: &str, label: &str, date| {
        parts::button_control(key, label, None, p).on_press_tx(move |s: &mut S, tx| s.choose_calendar(date, tx)).h(29.0)
    };
    let mut quick = row().w_full().gap(3.0);
    for choice in QuickDate::ALL {
        quick = quick.child(button(&format!("calendar-{choice:?}"), choice.label(), choice.date(today)));
    }
    let nav = |label, glyph, forward| {
        parts::icon_control(label, label, glyph, p.muted, p)
            .on_press_tx(move |s: &mut S, tx| {
                move_cursor(s, Step::Month, forward);
                tx.focus("task-calendar", true);
            })
            .disabled(calendar::shift(open.cursor, Step::Month, forward, today).month() == open.cursor.month())
    };
    let mut panel = col()
        .key("task-calendar")
        .role("dialog")
        .label(heading)
        .focusable(true)
        .w(378.0)
        .padding(10.0)
        .gap(4.0)
        .focus_return(if open.task.is_none() { "quick-add".into() } else { anchor.clone() })
        .modal_tx(true, |s: &mut S, tx| s.close_calendar(tx))
        .on_key(calendar_keys::<S>)
        .child(picker_heading(heading, p))
        .child(quick)
        .child(
            row()
                .w_full()
                .center_y()
                .child(nav("Previous month", &icons::CHEVRON_LEFT, false))
                .child(spacer())
                .child(text(open.cursor.format("%B %Y").to_string()).font(ty::BODY_MEDIUM).color(p.text))
                .child(spacer())
                .child(nav("Next month", &icons::CHEVRON_RIGHT, true)),
        );
    let mut weekdays = row().w_full();
    for day in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] {
        weekdays = weekdays.child(row().grow(1.0).center().child(text(day).font(ty::SMALL).color(p.muted)));
    }
    panel = panel.child(weekdays);
    for week in calendar::month(open.cursor, today, dates, open.field).chunks(7) {
        let mut line = row().w_full();
        for day in week {
            let date = day.date;
            let marker = match day.marker {
                Some(DateField::Plan) => " ●",
                Some(DateField::Due) => " ◆",
                None => "",
            };
            let mut cell = row()
                .key(format!("calendar-day-{date}"))
                .role("button")
                .label(format!("{}{marker}", date.format("%A, %B %-d, %Y")))
                .checked(day.selected)
                .grow(1.0)
                .w(0.0)
                .h(32.0)
                .radius(6.0)
                .center()
                .child(text(format!("{}{marker}", date.day())).font(ty::SMALL).color(if day.in_month && day.enabled {
                    p.text
                } else {
                    p.muted
                }))
                .disabled(!day.enabled);
            if day.selected {
                cell = cell.fill(p.selected);
            } else if date == open.cursor {
                cell = cell.fill(p.hover);
            }
            if date == open.cursor || day.today {
                cell = cell.border(1.0, p.accent_line);
            }
            if day.enabled {
                cell = cell
                    .focusable(true)
                    .hover(p.hover)
                    .cursor(Cursor::Pointer)
                    .on_press_tx(move |s: &mut S, tx| s.choose_calendar(Some(date), tx));
            }
            line = line.child(cell);
        }
        panel = panel.child(line);
    }
    Some(popover::raised(&p.kit, panel, 10.0))
}

pub(crate) fn picker_heading(heading: &str, p: &Pal) -> El {
    text(heading.to_owned()).font(ty::BODY_MEDIUM).color(p.text).px(4.0).py(3.0)
}

fn move_cursor<S: CalendarHost>(s: &mut S, step: Step, forward: bool) {
    let today = s.today();
    if let Some(open) = s.calendar_mut().as_mut() {
        open.cursor = calendar::shift(open.cursor, step, forward, today);
    }
}

pub(crate) fn calendar_keys<S: CalendarHost>(s: &mut S, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt {
        return false;
    }
    let Some(open) = s.calendar() else { return false };
    let step = match &ev.key {
        Key::Left => (Step::Day, false),
        Key::Right => (Step::Day, true),
        Key::Up => (Step::Week, false),
        Key::Down => (Step::Week, true),
        Key::PageUp => (Step::Month, false),
        Key::PageDown => (Step::Month, true),
        Key::Escape => {
            s.close_calendar(tx);
            return true;
        }
        Key::Enter if tx.focused() == Some("task-calendar") => {
            s.choose_calendar(Some(open.cursor), tx);
            return true;
        }
        Key::Backspace | Key::Delete => {
            s.choose_calendar(None, tx);
            return true;
        }
        Key::Text(letter) => {
            match calendar::letter_selection(letter, open.cursor, s.today()) {
                Some(calendar::Selection::Move(cursor)) => {
                    if let Some(open) = s.calendar_mut().as_mut() {
                        open.cursor = cursor;
                    }
                    tx.focus("task-calendar", true);
                }
                Some(calendar::Selection::Apply(day)) => s.choose_calendar(day, tx),
                None => return false,
            }
            return true;
        }
        _ => return false,
    };
    move_cursor(s, step.0, step.1);
    tx.focus("task-calendar", true);
    true
}
