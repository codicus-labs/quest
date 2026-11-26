use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{DateTime, NaiveDate, Utc};
use quest_domain::{Command, EntityId, Snapshot, TaskDates};
use quest_view::quick_add as model;
use rune::{
    Align, App, AppHandle, AppSpec, DisplayTarget, El, KeyEvent, OverlayPlacement, ScreenEdge, TextEdit, Tx,
    WindowOptions, WindowSpec, col, icon, row, text,
};

use super::{Desktop, SHORTCUT, passive_panel};
use crate::{
    clock::{self, Clock},
    icons,
    state::{
        State,
        quest_picker::{Pick, PickFor, PickRow, QuestPicker},
        recurrence::RepeatEditor,
        task_edits::Calendar,
    },
    theme::{pal, ty},
    ui::{
        date_picker::{self, DateHost},
        quest_picker::QuestHost,
        quick_add::{self, Host, Layout, QuickAdd},
        recurrence::RepeatHost,
        task_fields::CalendarHost,
    },
};

const POSITION: OverlayPlacement =
    OverlayPlacement::Edge { edge: ScreenEdge::Top, margin: 128.0, align: Align::Center, inset: 0.0 };
const WIDTH: f32 = 480.0;
const FORM_HEIGHT: f32 = 78.0;
const FEEDBACK_SIZE: (f32, f32) = (218.0, 38.0);
const FEEDBACK_TIME: Duration = Duration::from_millis(1800);

pub(super) fn windows(
    shared: Desktop,
    clock: Clock,
    zone: chrono_tz::Tz,
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
    (
        WindowSpec::from_app("quick-add", panel_app(shared, clock, zone, main, feedback_handle))
            .configured_options(WindowOptions::launcher_panel().macos(|mac| mac.level(25))),
        passive_panel("quick-add-feedback", feedback, true),
    )
}

fn panel_app(
    shared: Desktop,
    clock: Clock,
    zone: chrono_tz::Tz,
    main: AppHandle<State>,
    feedback: AppHandle<Feedback>,
) -> App<Panel> {
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
    app.init(move || {
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
            feedback,
            main,
            quick: QuickAdd::default(),
            dates: None,
            zone,
            open: false,
            landed: None,
            error: None,
            appearance_override: None,
            height: FORM_HEIGHT,
        }
    })
}

#[cfg(test)]
pub(crate) fn test_panel(state: &State) -> rune::Runtime<Panel> {
    use super::DesktopData;
    use std::sync::Mutex;

    let shared = Arc::new(Mutex::new(DesktopData {
        store: state.store.clone(),
        dark: true,
        main_focused: false,
        panel_open: false,
        locks_edits: false,
        profile: 0,
    }));
    let main = App::new(AppSpec::new("Test", (1.0, 1.0), || unreachable!(), |_: &State, _| col()));
    let feedback = App::new(AppSpec::new("Test", FEEDBACK_SIZE, Feedback::default, |_, _| col()));
    let mut rt = panel_app(shared, state.clock, state.repeat_zone, main.handle(), feedback.handle())
        .headless_runtime((WIDTH, 720.0), rune::Env::new(true, 1.0));
    rt.update(Panel::toggle);
    rt.frame(0.0);
    rt
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
    quick: QuickAdd,
    dates: Option<date_picker::Picker>,
    zone: chrono_tz::Tz,
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
        self.quick = QuickAdd::default();
        self.dates = None;
        tx.instant();
        tx.blur();
        self.handle.hide();
    }

    pub(super) fn key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        self.open && quick_add::key(self, ev, tx)
    }

    fn resize(&mut self, tx: &mut Tx) -> bool {
        let height = quick_add::panel_height(self, FORM_HEIGHT, self.error.is_some());
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

impl Host for Panel {
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
        self.zone
    }
    fn defaults(&self) -> model::Fields {
        model::Fields::default()
    }
    fn layout(&self) -> Layout {
        Layout::Panel
    }
    fn dismiss(&mut self, tx: &mut Tx) {
        self.close(tx);
    }
    fn picker_changed(&mut self, tx: &mut Tx) {
        self.resize(tx);
    }

    fn create(&mut self, id: EntityId, commands: &[Command], tx: &mut Tx) {
        if !self.open {
            return;
        }
        let data = self.shared.lock().unwrap();
        if data.locks_edits || self.profile != data.profile || self.snap.workspace != data.store.snapshot().workspace {
            self.error = Some("The active workspace or account is changing. Reopen Quick Add to try again.".into());
            return;
        }
        let store = data.store.clone();
        drop(data);
        if let Err(error) = store.dispatch_all_in(self.snap.workspace, commands) {
            self.error = Some(format!("Couldn’t add task: {error}."));
            return;
        }
        let snapshot = store.snapshot();
        let Some(task) = snapshot.task(id) else {
            self.close(tx);
            return;
        };
        let label = model::landing(task, &snapshot, self.today()).label();
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
        self.quick.calendar
    }
    fn calendar_mut(&mut self) -> &mut Option<Calendar> {
        &mut self.quick.calendar
    }
    fn field_dates(&self, _: Option<EntityId>) -> Option<TaskDates> {
        Some(self.draft().fields.dates)
    }
    fn close_calendar(&mut self, tx: &mut Tx) {
        quick_add::escape(self, tx);
    }
    fn choose_calendar(&mut self, day: Option<NaiveDate>, tx: &mut Tx) {
        quick_add::choose_date(self, day, tx);
    }
}

impl QuestHost for Panel {
    fn picker(&self) -> Option<&QuestPicker> {
        self.quick.quest.as_ref()
    }
    fn picker_mut(&mut self) -> &mut Option<QuestPicker> {
        &mut self.quick.quest
    }
    fn picker_search(&mut self) -> &mut TextEdit {
        self.quick.quest.as_mut().map_or(&mut self.quick.closed_search, |picker| &mut picker.search)
    }
    fn picker_rows(&self) -> Vec<PickRow> {
        quick_add::quest_rows(self)
    }
    fn picked(&self, _: &PickFor) -> Option<Pick> {
        Some(quick_add::picked(self))
    }
    fn choose_quest(&mut self, pick: Pick) {
        quick_add::choose_quest(self, pick);
    }
}

impl DateHost for Panel {
    fn date_today(&self) -> NaiveDate {
        self.today()
    }
    fn date_picker(&self) -> Option<date_picker::Picker> {
        self.dates
    }
    fn date_picker_mut(&mut self) -> &mut Option<date_picker::Picker> {
        &mut self.dates
    }
}

impl RepeatHost for Panel {
    fn repeat_form(&self) -> &RepeatEditor {
        self.quick.repeat.as_ref().expect("open repeat editor")
    }
    fn repeat_form_mut(&mut self) -> &mut RepeatEditor {
        self.quick.repeat.as_mut().unwrap_or(&mut self.quick.closed_repeat)
    }
}

fn panel_view(s: &Panel, _: &mut rune::Cx<Panel>) -> El {
    let dark = s.appearance_override.unwrap_or(s.dark);
    let p = pal(dark);
    if !s.open {
        return s.landed.as_ref().map_or_else(col, |label| feedback_view(label, dark));
    }
    quick_add::panel(s, p, s.error.as_deref())
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
