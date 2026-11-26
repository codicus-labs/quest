use chrono::{DateTime, NaiveDate, Utc};
use quest_domain::{Command, EntityId, Position, RepeatRule, Snapshot};
use quest_view::{calendar, quick_add};
use rune::{Cursor, El, Key, KeyEvent, TextEdit, Tx, col, icon, row, text, text_input};
use std::ops::Range;

use super::{
    quest_picker::{self, QuestHost},
    recurrence::{self, RepeatHost},
    task_fields::{self, CalendarHost, Picker},
};
use crate::{
    icons,
    state::{
        Overlay, State, View,
        quest_picker::{Pick, PickFor, PickRow, QuestPicker},
        recurrence::RepeatEditor,
        task_edits::Calendar,
    },
    theme::{Pal, ty},
};

const NOTE_HEIGHT: f32 = 76.0;
const PANEL_PICKER_HEIGHT: f32 = 444.0;
const PANEL_REPEAT_HEIGHT: f32 = 630.0;

#[derive(Default)]
pub struct QuickAdd {
    pub title: TextEdit,
    pub note: Option<TextEdit>,
    fields: Option<quick_add::Fields>,
    choice: usize,
    pub calendar: Option<Calendar>,
    pub quest: Option<QuestPicker>,
    pub closed_search: TextEdit,
    pub repeat: Option<RepeatEditor>,
    pub closed_repeat: RepeatEditor,
    rule: Option<RepeatRule>,
}

impl QuickAdd {
    pub fn picker_open(&self) -> bool {
        self.calendar.is_some() || self.quest.is_some() || self.repeat.is_some()
    }

    pub fn has_fields(&self) -> bool {
        self.fields.is_some()
    }

    pub fn close_picker(&mut self) {
        self.calendar = None;
        self.quest = None;
        self.repeat = None;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Inline,
    Panel,
}

pub trait Host: CalendarHost + QuestHost + RepeatHost {
    fn quick(&self) -> &QuickAdd;
    fn quick_mut(&mut self) -> &mut QuickAdd;
    fn snapshot(&self) -> &Snapshot;
    fn now(&self) -> DateTime<Utc>;
    fn zone(&self) -> chrono_tz::Tz;
    fn defaults(&self) -> quick_add::Fields;
    fn layout(&self) -> Layout;
    fn dismiss(&mut self, tx: &mut Tx);
    fn create(&mut self, id: EntityId, commands: &[Command], tx: &mut Tx);
    fn picker_changed(&mut self, _tx: &mut Tx) {}
    fn begin_edit(&mut self) {}

    fn input(&self) -> &TextEdit {
        &self.quick().title
    }
    fn input_mut(&mut self) -> &mut TextEdit {
        &mut self.quick_mut().title
    }
    fn note(&self) -> &Option<TextEdit> {
        &self.quick().note
    }
    fn note_mut(&mut self) -> &mut Option<TextEdit> {
        &mut self.quick_mut().note
    }
    fn has_fields(&self) -> bool {
        self.quick().has_fields()
    }
    fn quick_base(&self) -> quick_add::Fields {
        self.quick().fields.unwrap_or_else(|| self.defaults())
    }
    fn set_fields(&mut self, fields: quick_add::Fields) {
        self.quick_mut().fields = Some(fields);
    }
    fn set_choice(&mut self, choice: usize) {
        self.quick_mut().choice = choice;
    }
    fn choice(&self) -> usize {
        self.quick().choice
    }

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
            *self.input_mut() = TextEdit::new(draft.title);
        }
        draft.fields
    }

    fn open_note(&mut self, tx: &mut Tx) {
        self.begin_edit();
        clear_picker(self);
        self.note_mut().get_or_insert_with(TextEdit::default);
        tx.focus_when_shown("quick-add-note", true);
        self.picker_changed(tx);
    }

    fn open_field(&mut self, picker: Picker, tx: &mut Tx) {
        self.begin_edit();
        let base = self.quick_base();
        let fields = self.draft().fields;
        let close = match picker {
            Picker::Quest => self.quick().quest.is_some(),
            Picker::Date(field) => self.quick().calendar.is_some_and(|open| open.field == field),
            Picker::Repeat => self.quick().repeat.is_some(),
        };
        clear_picker(self);
        self.set_fields(base);
        if close {
            tx.focus_when_shown("quick-add", true);
        } else {
            let focus = match picker {
                Picker::Quest => {
                    self.quick_mut().quest = Some(QuestPicker {
                        target: PickFor::QuickAdd,
                        highlight: fields.quest.map(Pick::Quest),
                        search: TextEdit::default(),
                    });
                    "quest-picker-search"
                }
                Picker::Date(field) => {
                    let cursor = calendar::initial(fields.dates, field, self.today());
                    self.quick_mut().calendar = Some(Calendar { task: None, field, cursor, review: false });
                    "task-calendar"
                }
                Picker::Repeat => {
                    let rule = self.quick().rule.unwrap_or_else(|| {
                        RepeatEditor::initial_rule(fields.dates.planned_on, self.today(), self.zone())
                    });
                    self.quick_mut().repeat = Some(RepeatEditor::from_rule(rule));
                    "repeat-editor"
                }
            };
            tx.focus_when_shown(focus, picker != Picker::Quest);
        }
        self.picker_changed(tx);
    }
}

fn clear_picker<S: Host>(s: &mut S) {
    if s.quick().repeat.is_some() {
        *s.date_picker_mut() = None;
    }
    s.quick_mut().close_picker();
}

pub fn escape<S: Host>(s: &mut S, tx: &mut Tx) {
    if s.quick().picker_open() {
        if s.quick().repeat.is_some() && s.date_picker_mut().take().is_some() {
            return;
        }
        close_picker(s, tx);
    } else if tx.focused() == Some("quick-add-note") {
        tx.focus_when_shown("quick-add", true);
    } else {
        s.dismiss(tx);
    }
}

fn close_picker<S: Host>(s: &mut S, tx: &mut Tx) {
    clear_picker(s);
    tx.focus_when_shown("quick-add", true);
    s.picker_changed(tx);
}

fn submit<S: Host>(s: &mut S, tx: &mut Tx) {
    let id = EntityId::new();
    let description = s.note().as_ref().map_or("", |note| note.text.as_str());
    let draft = s.draft();
    let commands = draft.commands_with_repeat(id, Position::Bottom, description, s.quick().rule, s.now());
    if !commands.is_empty() {
        s.create(id, &commands, tx);
    }
}

pub fn choose_date<S: Host>(s: &mut S, day: Option<NaiveDate>, tx: &mut Tx) {
    let Some(open) = s.quick().calendar else { return };
    if day.is_some_and(|day| day < s.today()) {
        return;
    }
    close_picker(s, tx);
    let mut fields = s.commit_quick_tokens();
    fields.dates = calendar::set_date(fields.dates, open.field, day);
    s.set_fields(fields);
}

pub fn choose_quest<S: Host>(s: &mut S, pick: Pick) {
    s.quick_mut().quest = None;
    let mut fields = s.commit_quick_tokens();
    fields.quest = if let Pick::Quest(id) = pick { Some(id) } else { None };
    s.set_fields(fields);
}

pub fn quest_rows<S: Host>(s: &S) -> Vec<PickRow> {
    let Some(picker) = &s.quick().quest else { return Vec::new() };
    crate::state::quest_picker::existing_rows(s.snapshot(), s.now())
        .chain(std::iter::once(crate::state::quest_picker::no_quest_row()))
        .filter(|row| quest_view::quest_picker::matches(&row.title, &picker.search.text))
        .collect()
}

pub fn picked<S: Host>(s: &S) -> Pick {
    s.draft().fields.quest.map_or(Pick::NoQuest, Pick::Quest)
}

pub fn rule<S: Host>(s: &S) -> Option<RepeatRule> {
    s.quick().rule
}

struct InlineConfig {
    placeholder: String,
    defaults: quick_add::Fields,
}

fn inline_config(s: &State) -> Option<InlineConfig> {
    let mut defaults = quick_add::Fields::for_view(s.view, s.today());
    let placeholder = match s.view {
        View::Today => "Add to Today".into(),
        View::Inbox => "Add to Inbox".into(),
        View::Quest(id) => format!("Add to {}", s.snap.quest(id)?.title),
        View::Plan => {
            defaults.dates.planned_on = Some(s.plan_quick_day?);
            "New to-do…".into()
        }
        _ => return None,
    };
    Some(InlineConfig { placeholder, defaults })
}

pub fn inline(s: &State, p: &Pal) -> Option<El> {
    inline_config(s).map(|config| form(s, p, &config.placeholder))
}

pub fn defaults(s: &State) -> quick_add::Fields {
    inline_config(s).map_or_else(quick_add::Fields::default, |config| config.defaults)
}

pub fn inline_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    s.overlay == Overlay::None && inline_config(s).is_some() && (inline_chord(s, ev, tx) || non_chord_key(s, ev, tx))
}

fn inline_chord(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    (ev.key != Key::Command('r') && ev.key != Key::Command('R')
        || s.quick.picker_open()
        || tx.focused().is_some_and(|key| key.starts_with("quick-add")))
        && quick_chord(s, ev, tx)
}

pub fn key<S: Host>(s: &mut S, ev: &KeyEvent, tx: &mut Tx) -> bool {
    quick_chord(s, ev, tx) || non_chord_key(s, ev, tx)
}

fn non_chord_key<S: Host>(s: &mut S, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if s.quick().calendar.is_some() && task_fields::calendar_keys(s, ev, tx) {
        return true;
    }
    if command_enter(ev) && !s.quick().picker_open() && tx.focused().is_some_and(|key| key.starts_with("quick-add-")) {
        submit(s, tx);
        return true;
    }
    if ev.key == Key::Escape
        && (s.layout() == Layout::Panel
            || s.quick().picker_open()
            || tx.focused().is_some_and(|key| key.starts_with("quick-add")))
    {
        escape(s, tx);
        return true;
    }
    false
}

fn command_enter(ev: &KeyEvent) -> bool {
    ev.key == Key::Enter && ev.mods.cmd && !ev.mods.alt && !ev.mods.ctrl && !ev.mods.shift
}

pub fn save_repeat<S: Host>(s: &mut S, tx: &mut Tx) {
    let today = s.today();
    let Some(editor) = s.quick_mut().repeat.as_mut() else { return };
    match editor.rule(today) {
        Ok(rule) => {
            s.quick_mut().rule = Some(rule);
            close_picker(s, tx);
        }
        Err(error) => editor.error = Some(error),
    }
}

pub fn stop_repeat<S: Host>(s: &mut S, tx: &mut Tx) {
    s.quick_mut().rule = None;
    close_picker(s, tx);
}

pub fn inline_layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    if s.overlay == Overlay::None && s.quick.repeat.is_some() {
        recurrence::quick_layer(s, p, layer);
    }
}

impl Host for State {
    fn quick(&self) -> &QuickAdd {
        &self.quick
    }
    fn quick_mut(&mut self) -> &mut QuickAdd {
        &mut self.quick
    }
    fn snapshot(&self) -> &Snapshot {
        &self.snap
    }
    fn now(&self) -> DateTime<Utc> {
        self.now
    }
    fn zone(&self) -> chrono_tz::Tz {
        self.repeat_zone
    }
    fn defaults(&self) -> quick_add::Fields {
        defaults(self)
    }
    fn layout(&self) -> Layout {
        Layout::Inline
    }
    fn begin_edit(&mut self) {
        self.task_calendar = None;
        self.quest_picker = None;
    }
    fn dismiss(&mut self, tx: &mut Tx) {
        if self.quick.title.text.trim().is_empty() {
            self.reset_quick_add();
        }
        tx.blur();
    }
    fn create(&mut self, _: EntityId, commands: &[Command], tx: &mut Tx) {
        self.save_quick_add(commands);
        tx.focus_when_shown("quick-add", true);
    }
}

pub fn panel_height<S: Host>(s: &S, form_height: f32, has_error: bool) -> f32 {
    let picker = if s.quick().repeat.is_some() {
        PANEL_REPEAT_HEIGHT
    } else if s.quick().picker_open() {
        PANEL_PICKER_HEIGHT
    } else {
        form_height + 25.0 * shown_suggestions(s) as f32 + if has_error { 24.0 } else { 0.0 }
    };
    picker + if s.note().is_some() { NOTE_HEIGHT } else { 0.0 }
}

pub fn panel<S: Host>(s: &S, p: &Pal, error: Option<&str>) -> El {
    let picker = task_fields::calendar_surface(s, p).or_else(|| quest_picker::panel_surface(s, p));
    let (repeat, popups) = if s.quick().repeat.is_some() {
        let (body, popups) = recurrence::quick_editor(s, p);
        let surface = col()
            .key("quick-repeat-picker")
            .role("dialog")
            .label("⟳ Repeat")
            .focus_return("quick-add")
            .modal_tx(true, escape::<S>)
            .w_full()
            .fill(p.card)
            .radius(10.0)
            .border(1.0, p.border)
            .child(task_fields::picker_heading("⟳ Repeat", p).mx(12.0))
            .child(body);
        (Some(surface), popups)
    } else {
        (None, Vec::new())
    };
    let base = col()
        .key("app")
        .w_full()
        .h_full()
        .child(form(s, p, "New to-do…"))
        .children(picker.map(|picker| picker.ml(12.0).mt(4.0).transition(rune::Transition::none())))
        .children(error.map(|error| text(error.to_owned()).font(ty::SMALL).color(p.error).fill(p.card)))
        .children(repeat);
    rune_kit::root(base, popups).on_background_press_tx(escape::<S>)
}

fn form<S: Host>(s: &S, p: &Pal, placeholder: &str) -> El {
    let global = s.layout() == Layout::Panel;
    let active = s.input().focused;
    let shown = active || !s.input().text.is_empty() || s.has_fields() || s.note().is_some();
    let input = text_input(s.input(), ty::BODY)
        .key("quick-add")
        .label(placeholder)
        .help("Add Markdown note (⌘⇧N)")
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
            submit(s, tx);
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
        && !s.quick().picker_open()
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
    if let Some(note) = s.note() {
        box_ = box_.child(
            text_input(note, ty::BODY)
                .key("quick-add-note")
                .label("Markdown note")
                .placeholder("Note (Markdown)", p.faint)
                .w_full()
                .h(NOTE_HEIGHT - 4.0)
                .mb(4.0)
                .padding(8.0)
                .radius(6.0)
                .border(1.0, p.border)
                .color(p.text)
                .on_edit(|s: &mut S| s.note_mut().get_or_insert_with(rune::TextEdit::default))
                .on_input_key(note_key::<S>)
                .on_submit(|s: &mut S, _| {
                    if let Some(note) = s.note_mut() {
                        note.insert("\n");
                    }
                }),
        );
    }
    if shown || global {
        let fields = s.draft().fields;
        box_ = box_.child(task_fields::quick_fields(s, fields, p)).pb(8.0);
    }
    box_
}

fn shown_suggestions<S: Host>(s: &S) -> usize {
    if s.input().focused && !s.quick().picker_open() {
        suggestions(s).map_or(0, |(_, rows)| rows.len().min(6))
    } else {
        0
    }
}

fn suggestions<S: Host>(s: &S) -> Option<(Range<usize>, Vec<&quest_domain::Quest>)> {
    let (range, query) = quick_add::quest_token(&s.input().text, s.input().focus)?;
    Some((range, quick_add::quest_suggestions(s.snapshot(), s.now(), query)))
}

fn accept_suggestion<S: Host>(s: &mut S, range: Range<usize>, id: EntityId, tx: &mut Tx) {
    s.input_mut().set_selection(range.start, range.end);
    s.input_mut().insert("");
    s.set_choice(0);
    let mut fields = s.commit_quick_tokens();
    fields.quest = Some(id);
    s.set_fields(fields);
    tx.focus_when_shown("quick-add", true);
}

fn note_key<S: Host>(s: &mut S, ev: &KeyEvent, tx: &mut Tx) -> bool {
    match ev.key {
        Key::Escape => escape(s, tx),
        Key::Enter if command_enter(ev) => submit(s, tx),
        _ => return quick_chord(s, ev, tx),
    }
    true
}

fn quick_chord<S: Host>(s: &mut S, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if ev.mods.cmd
        && ev.mods.shift
        && !ev.mods.alt
        && !ev.mods.ctrl
        && let Key::Command(key) = ev.key
    {
        if key.eq_ignore_ascii_case(&'n') {
            s.open_note(tx);
            return true;
        }
        if let Some(picker) = Picker::quick_letter(key) {
            s.open_field(picker, tx);
            return true;
        }
    }
    false
}

fn quick_key<S: Host>(s: &mut S, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if quick_chord(s, ev, tx) {
        return true;
    }
    if ev.key == Key::Escape {
        escape(s, tx);
        return true;
    }
    if command_enter(ev) {
        submit(s, tx);
        return true;
    }
    if ev.key == Key::Tab && !ev.mods.shift {
        s.set_fields(s.quick_base());
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
