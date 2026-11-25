use chrono::Datelike;
use quest_domain::{DateField, EntityId, Task};
use quest_view::{
    calendar::{self, QuickDate, Step},
    lists, quick_add,
};
use rune::{Cursor, Cx, El, Key, KeyEvent, Tx, col, icon, row, spacer, text, text_input};
use rune_kit::{Placement, popover};
use std::ops::Range;

use super::{parts, quest_picker};
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

pub trait QuickHost: CalendarHost + quest_picker::QuestHost {
    fn snapshot(&self) -> &quest_domain::Snapshot;
    fn now(&self) -> chrono::DateTime<chrono::Utc>;
    fn input(&self) -> &rune::TextEdit;
    fn input_mut(&mut self) -> &mut rune::TextEdit;
    fn has_fields(&self) -> bool;
    fn quick_base(&self) -> quick_add::Fields;
    fn set_fields(&mut self, fields: quick_add::Fields);
    fn choice(&self) -> usize;
    fn set_choice(&mut self, choice: usize);
    fn open_field(&mut self, task: Option<EntityId>, picker: Picker, tx: &mut Tx);
    fn escape_quick(&mut self, tx: &mut Tx);
    fn submit(&mut self, tx: &mut Tx);

    fn draft(&self) -> quick_add::Draft {
        quick_add::parse(
            &self.input().text,
            self.quick_base(),
            self.today(),
            &quest_view::quest_picker::recent_quests(self.snapshot(), self.now()),
        )
    }

    fn commit_quick_tokens(&mut self) -> quick_add::Fields {
        let draft = self.draft();
        if draft.title != self.input().text.trim() {
            *self.input_mut() = rune::TextEdit::new(draft.title);
        }
        draft.fields
    }
}

impl CalendarHost for State {
    fn today(&self) -> chrono::NaiveDate {
        State::today(self)
    }
    fn calendar(&self) -> Option<crate::state::task_edits::Calendar> {
        self.task_calendar.filter(|_| self.overlay == Overlay::None)
    }
    fn calendar_mut(&mut self) -> &mut Option<crate::state::task_edits::Calendar> {
        &mut self.task_calendar
    }
    fn field_dates(&self, task: Option<EntityId>) -> Option<quest_domain::TaskDates> {
        State::field_dates(self, task)
    }
    fn close_calendar(&mut self, tx: &mut Tx) {
        State::close_calendar(self, tx);
    }
    fn choose_calendar(&mut self, day: Option<chrono::NaiveDate>, tx: &mut Tx) {
        State::choose_calendar(self, day, tx);
    }
}

impl QuickHost for State {
    fn snapshot(&self) -> &quest_domain::Snapshot {
        &self.snap
    }
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.now
    }
    fn input(&self) -> &rune::TextEdit {
        &self.quick_add
    }
    fn input_mut(&mut self) -> &mut rune::TextEdit {
        &mut self.quick_add
    }
    fn has_fields(&self) -> bool {
        self.quick_fields.is_some()
    }
    fn quick_base(&self) -> quick_add::Fields {
        State::quick_base(self)
    }
    fn set_fields(&mut self, fields: quick_add::Fields) {
        self.quick_fields = Some(fields);
    }
    fn choice(&self) -> usize {
        self.quick_choice
    }
    fn set_choice(&mut self, choice: usize) {
        self.quick_choice = choice;
    }
    fn open_field(&mut self, task: Option<EntityId>, picker: Picker, tx: &mut Tx) {
        open(self, task, picker, tx);
    }
    fn escape_quick(&mut self, tx: &mut Tx) {
        if self.quick_add.text.trim().is_empty() {
            self.reset_quick_add();
        }
        tx.blur();
    }
    fn submit(&mut self, tx: &mut Tx) {
        self.save_quick_add();
        tx.focus_when_shown("quick-add", true);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picker {
    Quest,
    Date(DateField),
}

impl Picker {
    pub fn from_letter(letter: char) -> Option<Self> {
        match letter.to_ascii_lowercase() {
            'l' => Some(Self::Quest),
            'p' => Some(Self::Date(DateField::Plan)),
            'd' => Some(Self::Date(DateField::Due)),
            _ => None,
        }
    }
}

pub fn open(s: &mut State, task: Option<EntityId>, picker: Picker, tx: &mut Tx) {
    if let Some(id) = task
        && s.task_modal.as_ref().is_none_or(|form| form.task != id)
    {
        s.open_task_modal(id);
        tx.scroll_into_view("task-modal");
    }
    match picker {
        Picker::Quest => {
            let target = if task.is_some() { PickFor::TaskModal } else { PickFor::QuickAdd };
            s.toggle_quest_picker(target);
            if s.quest_picker.is_some() {
                tx.focus_when_shown("quest-picker-search", false);
            } else if task.is_none() {
                tx.focus_when_shown("quick-add", true);
            }
        }
        Picker::Date(field) => s.open_calendar(task, field, tx),
    }
}

pub fn fields(s: &State, task: Option<&Task>, p: &Pal, compact: bool) -> El {
    let id = task.map(|task| task.id);
    let (quest, dates) = match task {
        Some(task) => (task.quest_id, task.dates()),
        None => {
            let fields = s.quick_draft().fields;
            (fields.quest, fields.dates)
        }
    };
    let postponed = id.map_or_else(lists::Postponed::default, |id| lists::postponed(&s.snap, id));
    field_line(s, id, quest, dates, postponed, p, compact)
}

fn field_line<S: QuickHost>(
    s: &S,
    id: Option<EntityId>,
    quest: Option<EntityId>,
    dates: quest_domain::TaskDates,
    postponed: lists::Postponed,
    p: &Pal,
    compact: bool,
) -> El {
    let quest = quest.and_then(|id| s.snapshot().quest(id));
    let target = if id.is_some() { PickFor::TaskModal } else { PickFor::QuickAdd };
    let button = field_chip(p)
        .max_w(if compact { 140.0 } else { 220.0 })
        .child(parts::dot(quest.map_or(p.border_strong, |q| quest_color(q.color)), 7.0))
        .child(
            text(quest.map_or("Quest", |q| &q.title))
                .key(format!("{}-title", target.trigger_key()))
                .font(ty::SMALL)
                .color(if quest.is_some() { p.text } else { p.faint })
                .single_line(),
        )
        .child(icon(&icons::CHEVRON_DOWN, 12.0).color(p.faint));
    let quest = quest_picker::trigger(button, target, quest.map_or("Quest", |q| &q.title), s, p);
    let mut line = if compact { col().items_start() } else { row().center_y() };
    line = line
        .key(if id.is_none() { "quick-add-fields" } else { "task-fields" })
        .gap(7.0)
        .on_key(move |s: &mut S, ev, tx| {
            let Key::Text(letter) = &ev.key else { return false };
            if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt {
                return false;
            }
            let Some(picker) = letter.chars().next().and_then(Picker::from_letter) else { return false };
            s.open_field(id, picker, tx);
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
        let button = field_chip(p)
            .key(key)
            .role("button")
            .label(format!("{label}: {value}"))
            .focusable(true)
            .cursor(Cursor::Pointer)
            .child(text(glyph).font(ty::SMALL).color(glyph_ink))
            .child(text(value).font(ty::SMALL).color(ink).single_line())
            .on_press_tx(move |s: &mut S, tx: &mut Tx| s.open_field(id, Picker::Date(field), tx));
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

fn field_chip(p: &Pal) -> El {
    row().h(28.0).px(10.0).gap(6.0).center_y().radius(8.0).border(1.0, p.border).hover(p.hover)
}

pub fn quick_add(s: &State, p: &Pal, placeholder: &str) -> El {
    quick_add_form(s, p, placeholder, false)
}

pub fn quick_add_form<S: QuickHost>(s: &S, p: &Pal, placeholder: &str, global: bool) -> El {
    let active = s.input().focused;
    let shown = active || !s.input().text.is_empty() || s.has_fields();
    let input = text_input(s.input(), ty::BODY)
        .key("quick-add")
        .label(placeholder)
        .single_line()
        .grow(1.0)
        .min_w(0.0)
        .color(p.text)
        .placeholder(placeholder, p.muted)
        .on_edit(|s: &mut S| s.input_mut())
        .on_input_change(|s: &mut S, _: &mut Tx| {
            s.set_fields(s.quick_base());
            s.set_choice(0);
        })
        .on_input_key(quick_key::<S>)
        .on_submit(|s: &mut S, tx: &mut Tx| {
            s.submit(tx);
        });
    let mut box_ = col()
        .key("quick-add-box")
        .w_full()
        .mt(if global { 0.0 } else { 18.0 })
        .px(12.0)
        .radius(10.0)
        .border(1.0, p.border_strong)
        .child(
            row()
                .w_full()
                .h(42.0)
                .gap(10.0)
                .center_y()
                .child(if global {
                    icon(&icons::QUEST_MARK, 22.0).color(p.accent)
                } else {
                    icon(&icons::ADD, 18.0).color(p.accent)
                })
                .child(input),
        );
    if global {
        box_ = box_.fill(p.card);
    } else {
        box_ = box_.border_dash(4.0, 3.0);
    }
    if active
        && s.picker().is_none()
        && s.calendar().is_none()
        && let Some((range, suggestions)) = suggestions(s)
    {
        for (i, q) in suggestions.into_iter().take(6).enumerate() {
            let id = q.id;
            let range = range.clone();
            box_ = box_.child(
                row()
                    .w_full()
                    .h(25.0)
                    .px(8.0)
                    .center_y()
                    .radius(6.0)
                    .fill(if i == s.choice() { p.selected } else { rune::Color::TRANSPARENT })
                    .hover(p.hover)
                    .cursor(Cursor::Pointer)
                    .child(text(q.title.clone()).font(ty::SMALL).color(p.text))
                    .on_press_tx(move |s: &mut S, tx| accept_suggestion(s, range.clone(), id, tx)),
            );
        }
    }
    if shown || global {
        let fields = s.draft().fields;
        box_ =
            box_.child(field_line(s, None, fields.quest, fields.dates, lists::Postponed::default(), p, false)).pb(8.0);
    }
    box_
}

pub(crate) fn suggestions<S: QuickHost>(s: &S) -> Option<(Range<usize>, Vec<&quest_domain::Quest>)> {
    let (range, query) = quick_add::quest_token(&s.input().text, s.input().focus)?;
    Some((range, quick_add::quest_suggestions(s.snapshot(), s.now(), query)))
}

fn accept_suggestion<S: QuickHost>(s: &mut S, range: Range<usize>, id: EntityId, tx: &mut Tx) {
    s.input_mut().set_selection(range.start, range.end);
    s.input_mut().insert("");
    s.set_choice(0);
    let mut fields = s.commit_quick_tokens();
    fields.quest = Some(id);
    s.set_fields(fields);
    tx.focus_when_shown("quick-add", true);
}

fn quick_key<S: QuickHost>(s: &mut S, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if ev.mods.cmd
        && ev.mods.shift
        && !ev.mods.alt
        && !ev.mods.ctrl
        && let Key::Command(key) = ev.key
        && let Some(picker) = Picker::from_letter(key)
    {
        s.open_field(None, picker, tx);
        return true;
    }
    if ev.key == Key::Escape {
        s.escape_quick(tx);
        return true;
    }
    let Some((range, options)) = suggestions(s) else { return false };
    let count = options.len().min(6);
    if count == 0 {
        return false;
    }
    match ev.key {
        Key::Down => s.set_choice((s.choice() + 1) % count),
        Key::Up => s.set_choice((s.choice() + count - 1) % count),
        Key::Tab | Key::Enter => {
            let id = options[s.choice().min(count - 1)].id;
            accept_suggestion(s, range, id, tx);
        }
        _ => return false,
    }
    true
}

pub fn layer<S: CalendarHost>(s: &S, p: &Pal, cx: &Cx<S>, layer: &mut Vec<El>) {
    let Some(open) = s.calendar() else { return };
    let Some(dates) = s.field_dates(open.task) else { return };
    let today = s.today();
    let anchor = open.anchor();
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
        .label("Choose date")
        .focusable(true)
        .w(378.0)
        .padding(10.0)
        .gap(4.0)
        .focus_return(if open.task.is_none() { "quick-add".into() } else { anchor.clone() })
        .modal_tx(true, |s: &mut S, tx| s.close_calendar(tx))
        .on_key(calendar_keys::<S>)
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
    layer.push(popover::anchored(&p.kit, popover::raised(&p.kit, panel, 10.0), &anchor, placement, 4.0));
}

fn move_cursor<S: CalendarHost>(s: &mut S, step: Step, forward: bool) {
    let today = s.today();
    if let Some(open) = s.calendar_mut().as_mut() {
        open.cursor = calendar::shift(open.cursor, step, forward, today);
    }
}

fn calendar_keys<S: CalendarHost>(s: &mut S, ev: &KeyEvent, tx: &mut Tx) -> bool {
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
            let choice = match letter.to_ascii_lowercase().as_str() {
                "t" => QuickDate::Today,
                "m" => QuickDate::Tomorrow,
                "n" => QuickDate::NextMonday,
                _ => return false,
            };
            s.choose_calendar(choice.date(s.today()), tx);
            return true;
        }
        _ => return false,
    };
    move_cursor(s, step.0, step.1);
    tx.focus("task-calendar", true);
    true
}
