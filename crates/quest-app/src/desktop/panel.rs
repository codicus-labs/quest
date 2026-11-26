use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{DateTime, NaiveDate, Utc};
use quest_domain::{EntityId, Position, Snapshot, TaskDates};
use quest_view::{calendar, quick_add};
use rune::{
    Align, App, AppHandle, AppSpec, DisplayTarget, El, Key, KeyEvent, OverlayPlacement, ScreenEdge, TextEdit, Tx,
    WindowOptions, WindowSpec, col, icon, row, text,
};

use super::{Desktop, SHORTCUT, passive_panel};
use crate::{
    clock::{self, Clock},
    icons,
    state::{
        State,
        quest_picker::{Pick, PickFor, PickRow, QuestPicker},
        task_edits::Calendar,
    },
    theme::{pal, ty},
    ui::{
        quest_picker::{self, QuestHost},
        task_fields::{self, CalendarHost, Picker, QuickHost},
    },
};

const POSITION: OverlayPlacement =
    OverlayPlacement::Edge { edge: ScreenEdge::Top, margin: 128.0, align: Align::Center, inset: 0.0 };
const WIDTH: f32 = 480.0;
const FORM_HEIGHT: f32 = 78.0;
const PICKER_HEIGHT: f32 = 414.0;
const FEEDBACK_SIZE: (f32, f32) = (218.0, 38.0);
const FEEDBACK_TIME: Duration = Duration::from_millis(1800);

pub(super) fn windows(
    shared: Desktop,
    clock: Clock,
    main: AppHandle<State>,
) -> (WindowSpec<Panel>, WindowSpec<Feedback>) {
    let feedback = App::new(AppSpec {
        poll: Some(Feedback::poll),
        scenes: &[
            ("light", |s| {
                s.dark = false;
                s.label = "Today".into();
            }),
            ("dark", |s| s.label = "Today".into()),
        ],
        ..AppSpec::new("Quest — Saved", FEEDBACK_SIZE, Feedback::default, |s, _| feedback_view(&s.label, s.dark))
    })
    .windowless();
    let feedback_handle = feedback.handle();
    let app = App::new(AppSpec {
        poll: Some(Panel::poll),
        on_key: Some(Panel::key),
        scenes: &[("light", |s| s.appearance_override = Some(false)), ("dark", |_| {})],
        ..AppSpec::new("Quest — Quick Add", (WIDTH, FORM_HEIGHT), || unreachable!(), panel_view)
    })
    .windowless()
    .global_shortcut(SHORTCUT, Panel::toggle)
    .on_focus(|s, focused, tx| {
        if !focused && s.open {
            s.close(tx);
        }
    });
    let handle = app.handle();
    let app = app.init(move || {
        let data = shared.lock().unwrap();
        let snap = data.store.snapshot();
        let (dark, profile) = (data.dark, data.profile);
        drop(data);
        Panel {
            shared,
            clock,
            snap,
            now: clock.now(),
            dark,
            profile,
            handle,
            feedback: feedback_handle,
            main,
            input: TextEdit::default(),
            note: None,
            fields: quick_add::Fields::default(),
            choice: 0,
            calendar: None,
            picker: None,
            closed_search: TextEdit::default(),
            open: false,
            landed: None,
            error: None,
            appearance_override: None,
            height: FORM_HEIGHT,
        }
    });
    (
        WindowSpec::from_app("quick-add", app)
            .configured_options(WindowOptions::launcher_panel().macos(|mac| mac.level(25))),
        passive_panel("quick-add-feedback", feedback, true),
    )
}

pub struct Panel {
    shared: Desktop,
    clock: Clock,
    snap: Arc<Snapshot>,
    now: DateTime<Utc>,
    dark: bool,
    profile: u64,
    handle: AppHandle<Self>,
    feedback: AppHandle<Feedback>,
    main: AppHandle<State>,
    input: TextEdit,
    note: Option<TextEdit>,
    fields: quick_add::Fields,
    choice: usize,
    calendar: Option<Calendar>,
    picker: Option<QuestPicker>,
    closed_search: TextEdit,
    open: bool,
    landed: Option<String>,
    error: Option<String>,
    appearance_override: Option<bool>,
    height: f32,
}

impl Panel {
    pub(super) fn toggle(&mut self, tx: &mut Tx) {
        if self.open {
            self.close(tx);
            return;
        }
        self.feedback.hide();
        self.landed = None;
        self.error = None;
        self.open = true;
        self.shared.lock().unwrap().panel_open = true;
        self.resize(tx);
        self.handle.place(DisplayTarget::Primary, POSITION);
        self.handle.show();
        tx.focus_when_shown("quick-add", false);
    }

    fn close(&mut self, tx: &mut Tx) {
        self.open = false;
        self.shared.lock().unwrap().panel_open = false;
        self.calendar = None;
        self.picker = None;
        self.input = TextEdit::default();
        self.note = None;
        self.fields = quick_add::Fields::default();
        self.choice = 0;
        tx.instant();
        tx.blur();
        self.handle.hide();
    }

    pub(super) fn key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if !self.open {
            return false;
        }
        if task_fields::quick_chord(self, ev, tx) {
            return true;
        }
        if self.calendar.is_some() && task_fields::calendar_keys(self, ev, tx) {
            return true;
        }
        if ev.key != Key::Escape {
            return false;
        }
        self.escape_quick(tx);
        true
    }

    fn resize(&mut self, tx: &mut Tx) -> bool {
        let height = if self.calendar.is_some() || self.picker.is_some() {
            PICKER_HEIGHT
        } else {
            FORM_HEIGHT
                + 25.0 * task_fields::shown_suggestions(self) as f32
                + if self.error.is_some() { 24.0 } else { 0.0 }
        } + if self.note.is_some() { task_fields::NOTE_HEIGHT } else { 0.0 };
        if self.height == height {
            return false;
        }
        tx.resize_window_instant((WIDTH, height));
        self.handle.place(DisplayTarget::Primary, POSITION);
        self.height = height;
        true
    }

    fn poll(&mut self, _: f64, tx: &mut Tx) -> bool {
        if let Some(error) = self.handle.take_shortcut_errors().into_iter().next() {
            let owner = SHORTCUT.parse().ok().and_then(|chord| self.handle.shortcut_owner(&chord));
            let notice = match error {
                rune::ShortcutError::Conflict => format!(
                    "Global Quick Add: ⌃⌥Space is already in use{}. Free the shortcut and restart Quest, or open Global Quick Add from View.",
                    owner.map_or(String::new(), |app| format!(" by {app}"))
                ),
                error => format!(
                    "Global Quick Add couldn’t register ⌃⌥Space ({error}). Open Global Quick Add from View instead."
                ),
            };
            self.main.update(move |s, _| s.hotkey_notice = Some(notice));
        }
        let data = self.shared.lock().unwrap();
        let snap = data.store.snapshot();
        let (dark, profile) = (data.dark, data.profile);
        drop(data);
        if self.profile != profile || self.snap.workspace != snap.workspace {
            self.close(tx);
        }
        let now = self.clock.now();
        let mut changed =
            !Arc::ptr_eq(&self.snap, &snap) || self.dark != dark || clock::local_day(self.now) != clock::local_day(now);
        (self.snap, self.dark, self.profile, self.now) = (snap, dark, profile, now);
        if self.open {
            changed |= self.resize(tx);
            tx.wake_after(clock::wake_for(now, clock::next_whole(now, 60)));
        }
        changed
    }
}

impl QuickHost for Panel {
    fn note(&self) -> &Option<TextEdit> {
        &self.note
    }
    fn note_mut(&mut self) -> &mut Option<TextEdit> {
        &mut self.note
    }

    fn open_note(&mut self, tx: &mut Tx) {
        self.calendar = None;
        self.picker = None;
        self.note.get_or_insert_with(TextEdit::default);
        tx.focus_when_shown("quick-add-note", true);
        self.resize(tx);
    }
    fn snapshot(&self) -> &Snapshot {
        &self.snap
    }
    fn now(&self) -> DateTime<Utc> {
        self.now
    }
    fn input(&self) -> &TextEdit {
        &self.input
    }
    fn input_mut(&mut self) -> &mut TextEdit {
        &mut self.input
    }
    fn has_fields(&self) -> bool {
        true
    }
    fn quick_base(&self) -> quick_add::Fields {
        self.fields
    }
    fn set_fields(&mut self, fields: quick_add::Fields) {
        self.fields = fields;
    }
    fn choice(&self) -> usize {
        self.choice
    }
    fn set_choice(&mut self, choice: usize) {
        self.choice = choice;
    }
    fn open_field(&mut self, _: Option<EntityId>, picker: Picker, tx: &mut Tx) {
        match picker {
            Picker::Quest => {
                self.toggle_picker(PickFor::QuickAdd);
                tx.focus_when_shown(if self.picker.is_some() { "quest-picker-search" } else { "quick-add" }, false);
            }
            Picker::Date(field) => {
                self.picker = None;
                let cursor = calendar::initial(self.draft().fields.dates, field, self.today());
                self.calendar = Some(Calendar { task: None, field, cursor, review: false });
                tx.focus_when_shown("task-calendar", true);
            }
        }
        self.resize(tx);
    }
    fn escape_quick(&mut self, tx: &mut Tx) {
        if self.calendar.is_some() {
            self.close_calendar(tx);
        } else if self.picker.take().is_some() {
            tx.focus_when_shown("quick-add", true);
        } else {
            self.close(tx);
        }
    }
    fn submit(&mut self, tx: &mut Tx) {
        if !self.open {
            return;
        }
        let id = EntityId::new();
        let description = self.note.as_ref().map_or("", |note| note.text.as_str());
        let commands = self.draft().commands(id, Position::Bottom, description);
        if commands.is_empty() {
            return;
        }
        let data = self.shared.lock().unwrap();
        if data.locks_edits || self.profile != data.profile || self.snap.workspace != data.store.snapshot().workspace {
            self.error = Some("The active workspace or account is changing. Reopen Quick Add to try again.".into());
            return;
        }
        let store = data.store.clone();
        drop(data);
        if let Err(error) = store.dispatch_all_in(self.snap.workspace, &commands) {
            self.error = Some(format!("Couldn’t add task: {error}."));
            return;
        }
        let snapshot = store.snapshot();
        let Some(task) = snapshot.task(id) else {
            self.close(tx);
            return;
        };
        let label = quick_add::landing(task, &snapshot, self.today()).label();
        self.close(tx);
        self.landed = Some(label.clone());
        let dark = self.appearance_override.unwrap_or(self.dark);
        self.feedback.update(move |s, tx| {
            s.label = label;
            s.dark = dark;
            s.expires = Some(Instant::now() + FEEDBACK_TIME);
            tx.wake_after(FEEDBACK_TIME);
        });
        self.feedback.place(DisplayTarget::Primary, POSITION);
        self.feedback.show();
    }
}

impl CalendarHost for Panel {
    fn today(&self) -> NaiveDate {
        clock::local_day(self.now)
    }
    fn calendar(&self) -> Option<Calendar> {
        self.calendar
    }
    fn calendar_mut(&mut self) -> &mut Option<Calendar> {
        &mut self.calendar
    }
    fn field_dates(&self, _: Option<EntityId>) -> Option<TaskDates> {
        Some(self.draft().fields.dates)
    }
    fn close_calendar(&mut self, tx: &mut Tx) {
        self.calendar = None;
        tx.focus_when_shown("quick-add", true);
    }
    fn choose_calendar(&mut self, day: Option<NaiveDate>, tx: &mut Tx) {
        let Some(open) = self.calendar else {
            return;
        };
        if day.is_some_and(|day| day < self.today()) {
            return;
        }
        self.close_calendar(tx);
        let mut fields = self.commit_quick_tokens();
        fields.dates = calendar::set_date(fields.dates, open.field, day);
        self.fields = fields;
    }
}

impl QuestHost for Panel {
    fn picker(&self) -> Option<&QuestPicker> {
        self.picker.as_ref()
    }
    fn picker_mut(&mut self) -> &mut Option<QuestPicker> {
        &mut self.picker
    }
    fn picker_search(&mut self) -> &mut TextEdit {
        self.picker.as_mut().map_or(&mut self.closed_search, |picker| &mut picker.search)
    }
    fn picker_hosted(&self, _: &PickFor) -> bool {
        self.open
    }
    fn toggle_picker(&mut self, target: PickFor) {
        if self.picker.take().is_some() {
            return;
        }
        self.calendar = None;
        self.picker = Some(QuestPicker {
            highlight: self.draft().fields.quest.map(Pick::Quest),
            target,
            search: TextEdit::default(),
        });
    }
    fn picker_rows(&self) -> Vec<PickRow> {
        let Some(picker) = &self.picker else {
            return Vec::new();
        };
        crate::state::quest_picker::existing_rows(&self.snap, self.now)
            .chain(std::iter::once(crate::state::quest_picker::no_quest_row()))
            .filter(|row| quest_view::quest_picker::matches(&row.title, &picker.search.text))
            .collect()
    }
    fn picked(&self, _: &PickFor) -> Option<Pick> {
        Some(self.draft().fields.quest.map_or(Pick::NoQuest, Pick::Quest))
    }
    fn choose_quest(&mut self, pick: Pick) {
        self.picker = None;
        let mut fields = self.commit_quick_tokens();
        fields.quest = if let Pick::Quest(id) = pick { Some(id) } else { None };
        self.fields = fields;
    }
}

fn panel_view(s: &Panel, _: &mut rune::Cx<Panel>) -> El {
    let dark = s.appearance_override.unwrap_or(s.dark);
    let p = pal(dark);
    if !s.open {
        return s.landed.as_ref().map_or_else(col, |label| feedback_view(label, dark));
    }
    let picker = task_fields::calendar_surface(s, p).or_else(|| quest_picker::panel_surface(s, p));
    col()
        .key("app")
        .w_full()
        .h_full()
        .child(task_fields::quick_add_form(s, p, "New to-do…", true))
        .children(picker.map(|picker| picker.ml(12.0).mt(4.0).transition(rune::Transition::none())))
        .children(s.error.as_ref().map(|error| text(error.clone()).font(ty::SMALL).color(p.error).fill(p.card)))
        .on_background_press_tx(|s: &mut Panel, tx| s.escape_quick(tx))
}

pub(super) struct Feedback {
    label: String,
    dark: bool,
    expires: Option<Instant>,
}

impl Default for Feedback {
    fn default() -> Self {
        Self { label: String::new(), dark: true, expires: None }
    }
}

impl Feedback {
    fn poll(&mut self, _: f64, tx: &mut Tx) -> bool {
        let Some(expires) = self.expires else {
            return false;
        };
        let wait = expires.saturating_duration_since(Instant::now());
        if !wait.is_zero() {
            tx.wake_after(wait);
            return false;
        }
        self.expires = None;
        tx.close_window();
        false
    }
}

fn feedback_view(label: &str, dark: bool) -> El {
    let p = pal(dark);
    row()
        .key("quick-add-feedback")
        .role("status")
        .label(format!("Saved to {label}"))
        .size(FEEDBACK_SIZE.0, FEEDBACK_SIZE.1)
        .px(13.0)
        .gap(8.0)
        .center()
        .radius(19.0)
        .fill(p.card)
        .border(1.0, p.border_strong)
        .child(icon(&icons::CHECK, 16.0).color(p.accent))
        .child(text(label.to_owned()).font(ty::BODY_MEDIUM).color(p.text).single_line().truncate())
}
