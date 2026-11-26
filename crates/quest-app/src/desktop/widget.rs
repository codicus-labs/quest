use std::{cell::RefCell, rc::Rc, sync::Arc};

use chrono::{DateTime, Utc};
use quest_domain::{Command, EntityId, EntryKind, RunningFocus, Snapshot};
use quest_view::{
    focus_exit,
    widget::{self, Mode, Pause, Screen},
};
use rune::{
    App, AppHandle, AppSpec, Cx, Display, Drag, El, Phase, PopupItem, PopupMenu, Tx, WindowHandle, WindowSpec, col,
    icon, row, text,
};

use super::{Desktop, PreferenceWriter, passive_panel};
use crate::{
    clock::{self, Clock},
    format, icons,
    state::{State, View},
    theme::{pal, quest_color, ty},
    ui::parts,
};

const CARD: (f32, f32) = (246.0, 88.0);
const CARD_BACK: (f32, f32) = (246.0, 120.0);
const PILL: (f32, f32) = (108.0, 38.0);
const PILL_ACTION: (f32, f32) = (140.0, 38.0);

pub(super) fn window(
    shared: Desktop,
    clock: Clock,
    main: AppHandle<State>,
    displays: &[Display],
    preferences: PreferenceWriter,
) -> WindowSpec<Widget> {
    let displays = displays.to_vec();
    let native = Rc::new(RefCell::new(None));
    let observe = native.clone();
    let app = App::new(AppSpec {
        poll: Some(Widget::poll),
        scenes: &[
            ("light", |s| s.appearance_override = Some(false)),
            ("dark", |_| {}),
            ("pill-light", |s| {
                s.appearance_override = Some(false);
                s.mode = Mode::Pill;
            }),
            ("pill-dark", |s| s.mode = Mode::Pill),
        ],
        ..AppSpec::new("Quest — Timer", CARD, || unreachable!(), view)
    })
    .windowless()
    .on_window(move |window| *observe.borrow_mut() = Some(window.clone()));
    let handle = app.handle();
    let app = app.init(move || {
        let data = shared.lock().unwrap();
        let root = data.store.snapshot();
        let workspace = timer_workspace(&root, None);
        let snap = data.store.snapshot_in(workspace).unwrap_or_else(|| root.clone());
        let (dark, profile) = (data.dark, data.profile);
        drop(data);
        let screens = displays
            .iter()
            .map(|d| Screen { origin: d.position, size: (d.size.0 as i32, d.size.1 as i32), scale: d.scale })
            .collect();
        let primary = displays.iter().find(|d| d.primary).or(displays.first());
        let mode = preferences.preferences.mode;
        Widget {
            shared,
            clock,
            main,
            handle,
            native,
            preferences,
            screens,
            primary: primary.map(|d| Screen {
                origin: d.position,
                size: (d.size.0 as i32, d.size.1 as i32),
                scale: d.scale,
            }),
            root,
            snap,
            now: clock.now(),
            dark,
            profile,
            mode,
            paused: None,
            showing: false,
            size: CARD,
            drag_start: None,
            dragged: false,
            appearance_override: None,
        }
    });
    passive_panel("timer-widget", app, false)
}

pub struct Widget {
    shared: Desktop,
    clock: Clock,
    main: AppHandle<State>,
    handle: AppHandle<Self>,
    native: Rc<RefCell<Option<WindowHandle>>>,
    preferences: PreferenceWriter,
    screens: Vec<Screen>,
    primary: Option<Screen>,
    root: Arc<Snapshot>,
    snap: Arc<Snapshot>,
    now: DateTime<Utc>,
    dark: bool,
    profile: u64,
    mode: Mode,
    paused: Option<Pause>,
    showing: bool,
    size: (f32, f32),
    drag_start: Option<DragStart>,
    dragged: bool,
    appearance_override: Option<bool>,
}

#[derive(Clone, Copy)]
struct DragStart {
    pointer: (i32, i32),
    origin: (i32, i32),
    scale: f64,
}

fn timer_workspace(root: &Snapshot, paused: Option<Pause>) -> EntityId {
    root.running_focus()
        .map(|running| running.workspace)
        .or(paused.map(|pause| pause.workspace))
        .unwrap_or(root.workspace)
}

impl Widget {
    fn poll(&mut self, _: f64, tx: &mut Tx) -> bool {
        let data = self.shared.lock().unwrap();
        let root = data.store.snapshot();
        let profile_changed = self.profile != data.profile;
        let mut changed = profile_changed || self.dark != data.dark;
        (self.profile, self.dark) = (data.profile, data.dark);
        let active = data.main_focused;
        if profile_changed {
            self.paused = None;
        }
        let workspace = timer_workspace(&root, self.paused);
        if !Arc::ptr_eq(&root, &self.root) || workspace != self.snap.workspace || profile_changed {
            self.snap = data.store.snapshot_in(workspace).unwrap_or_else(|| root.clone());
            self.root = root;
            changed = true;
        }
        drop(data);
        if self.paused.is_some_and(|pause| !pause.valid(&self.snap)) {
            self.paused = None;
            changed = true;
        }
        let running = self.root.running_focus().is_some();
        let showing = widget::visible(self.mode, active, running || self.paused.is_some());
        let size = self.content_size();
        let resized = self.size != size;
        if resized {
            tx.resize_window_instant(size);
            self.size = size;
            changed = true;
        }
        if showing && (!self.showing || resized && self.drag_start.is_none()) {
            self.handle.set_position(self.preferred_position());
        }
        if self.showing != showing {
            self.showing = showing;
            if showing {
                self.handle.show();
            } else {
                self.handle.hide();
            }
            changed = true;
        }
        if showing && running {
            let now = self.clock.now();
            changed |= self.now.timestamp() != now.timestamp();
            self.now = now;
            tx.wake_after(clock::wake_for(now, clock::next_whole(now, 1)));
        }
        changed
    }

    fn content_size(&self) -> (f32, f32) {
        let has_back = self.back_task().is_some();
        match self.mode {
            Mode::Pill if has_back || self.paused.is_some() => PILL_ACTION,
            Mode::Pill => PILL,
            _ if has_back => CARD_BACK,
            _ => CARD,
        }
    }

    fn preferred_position(&self) -> (i32, i32) {
        let default = self.primary.map_or((20, 48), |screen| {
            let dx = (f64::from(self.size.0 + 24.0) * screen.scale).ceil() as i32;
            (screen.origin.0 + screen.size.0 - dx, screen.origin.1 + (48.0 * screen.scale) as i32)
        });
        widget::clamp_position(self.preferences.preferences.position.unwrap_or(default), self.size, &self.screens)
    }

    pub(super) fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.preferences.change(|settings| settings.mode = mode);
    }

    fn drag(&mut self, drag: &mut Drag, _: &mut Tx) {
        let native = self.native.borrow();
        let screen = drag.screen_point();
        if drag.phase == Phase::Down {
            self.dragged = false;
            self.drag_start = screen
                .zip(native.as_ref().and_then(WindowHandle::placement))
                .map(|(pointer, placement)| DragStart { pointer, origin: placement.position, scale: placement.scale });
            return;
        }
        let end = drag.phase == Phase::Up;
        let Some(DragStart { pointer, origin, scale }) = self.drag_start else {
            self.dragged |= drag.dx.hypot(drag.dy) >= 4.0;
            return;
        };
        if end {
            self.drag_start = None;
        }
        let Some(window) = native.as_ref() else {
            return;
        };
        if drag.cancelled() {
            window.set_position(origin);
            return;
        }
        let Some((x, y)) = screen else {
            return;
        };
        let (dx, dy) = (x - pointer.0, y - pointer.1);
        self.dragged |= f64::from(dx).hypot(f64::from(dy)) >= 4.0 * scale;
        if !self.dragged {
            return;
        }
        let position = widget::clamp_position((origin.0 + dx, origin.1 + dy), self.size, &self.screens);
        window.set_position(position);
        if end {
            self.preferences.change(|settings| settings.position = Some(position));
        }
    }

    fn running(&self) -> Option<&RunningFocus> {
        self.root.running_focus()
    }

    fn back_task(&self) -> Option<&quest_domain::Task> {
        let entry = &self.running()?.entry;
        if entry.kind != EntryKind::Organizing {
            return None;
        }
        self.snap.task(entry.origin_task_id?).filter(|task| !task.is_done())
    }

    fn open_main(&mut self, _: &mut Tx) {
        if std::mem::take(&mut self.dragged) {
            return;
        }
        let workspace = self.snap.workspace;
        let task = self
            .running()
            .and_then(|r| r.entry.task_id)
            .or_else(|| self.back_task().map(|t| t.id))
            .or(self.paused.map(|p| p.task));
        if let Some(task) = task {
            self.main.update(move |s, tx| {
                s.switch_workspace(workspace);
                if s.snap.task(task).is_some() {
                    s.focus_task = Some(task);
                    s.go_with(View::Focus, tx);
                }
            });
        }
        self.main.open();
    }

    fn action(&mut self, action: Action) {
        let data = self.shared.lock().unwrap();
        if data.locks_edits || data.profile != self.profile {
            return;
        }
        let store = data.store.clone();
        drop(data);
        let fresh = store.snapshot();
        let running = fresh.running_focus();
        if running.map(|r| r.entry.id) != self.running().map(|r| r.entry.id) {
            return;
        }
        let at = self.clock.now();
        let workspace = timer_workspace(&fresh, self.paused);
        let Some(snapshot) = store.snapshot_in(workspace) else {
            return;
        };
        let (command, paused) = match action {
            Action::Pause => {
                let Some((paused, command)) = running.and_then(|r| Pause::stop(r, at)) else {
                    return;
                };
                (Some(command), Some(paused))
            }
            Action::Resume => {
                let Some(command) = self.paused.and_then(|p| p.resume(&snapshot, EntityId::new(), at)) else {
                    return;
                };
                (Some(command), None)
            }
            Action::Stop => (running.map(|_| Command::StopFocus { at }), None),
            Action::Back => {
                let Some(command) = focus_exit::back_to_task(&snapshot, EntityId::new(), at) else {
                    return;
                };
                (Some(command), None)
            }
        };
        if let Some(command) = command
            && let Err(error) = store.dispatch_in(workspace, command)
        {
            self.main.update(move |s, _| s.notice = Some(format!("Couldn’t change timer: {error}.")));
            return;
        }
        self.paused = paused;
        self.handle.wake();
    }
}

#[derive(Clone, Copy)]
enum Action {
    Pause,
    Resume,
    Stop,
    Back,
}

fn context_menu(s: &mut Widget, point: rune::PointerPress, tx: &mut Tx) {
    let mut menu = PopupMenu::new()
        .item(PopupItem::new("card", "Card").checked(s.mode == Mode::Card))
        .item(PopupItem::new("pill", "Pill").checked(s.mode == Mode::Pill))
        .item(PopupItem::new("hide", "Hide widget"));
    if s.mode == Mode::Pill {
        menu = menu.separator();
        if s.running().is_some_and(|r| r.entry.kind == EntryKind::Task && r.entry.task_id.is_some()) {
            menu = menu.item(PopupItem::new("pause", "Pause"));
        }
        menu = menu.item(PopupItem::new("stop", "Stop"));
    }
    tx.popup_menu(menu, point, |s: &mut Widget, picked, _| match picked {
        Some("card") => s.set_mode(Mode::Card),
        Some("pill") => s.set_mode(Mode::Pill),
        Some("hide") => s.set_mode(Mode::Hidden),
        Some("pause") => s.action(Action::Pause),
        Some("stop") => s.action(Action::Stop),
        _ => {}
    });
}

fn view(s: &Widget, _: &mut Cx<Widget>) -> El {
    let p = pal(s.appearance_override.unwrap_or(s.dark));
    let running = s.running();
    if s.mode == Mode::Hidden || running.is_none() && s.paused.is_none() {
        return col();
    }
    let paused = s.paused.filter(|_| running.is_none());
    let organizing = running.is_some_and(|r| r.entry.kind == EntryKind::Organizing);
    let task = running.and_then(|r| r.entry.task_id).or(paused.map(|pause| pause.task)).and_then(|id| s.snap.task(id));
    let title = if organizing { "Organizing" } else { task.map_or("No task", |t| &t.title) };
    let quest = task.and_then(|task| task.quest_id).and_then(|id| s.snap.quest(id));
    let color = if organizing { p.faint } else { quest.map_or(p.accent, |q| quest_color(q.color)) };
    let ms = running.map_or_else(
        || paused.map_or(0, |pause| pause.elapsed_ms),
        |r| (s.now - r.entry.started_at).num_milliseconds(),
    );
    let time = || {
        text(format::compact_timer(ms))
            .grow(1.0)
            .min_w(0.0)
            .single_line()
            .truncate()
            .key("widget-time")
            .font(ty::BODY_MEDIUM.features("'tnum' 1"))
            .color(if paused.is_some() { p.muted } else { p.text })
    };
    let pill = s.mode == Mode::Pill;
    let main = row()
        .key("widget-open")
        .role("button")
        .label(format!("{title} · Open in Focus"))
        .focusable(true)
        .gap(8.0)
        .h(34.0)
        .center_y()
        .grow(1.0)
        .min_w(0.0)
        .cursor(rune::Cursor::Pointer)
        .child(parts::dot(color, 7.0))
        .child(if pill {
            time()
        } else {
            text(title.to_owned()).font(ty::BODY_MEDIUM).color(p.text).single_line().truncate()
        })
        .on_press_tx(Widget::open_main)
        .on_drag(Widget::drag);
    let button = |key: &str, label: &str, glyph: &'static rune::Icon, ink, action| {
        parts::icon_control(key, label, glyph, ink, p).on_press_tx(move |s: &mut Widget, _| s.action(action))
    };
    let play = || {
        if paused.is_some() {
            Some(button("widget-resume", "Resume", &icons::PLAY, p.accent, Action::Resume))
        } else if task.is_some() && !pill {
            Some(button("widget-pause", "Pause", &icons::PAUSE, p.muted, Action::Pause))
        } else {
            None
        }
    };
    let back = s.back_task().map(|task| {
        let label = format!("Back to “{}”", task.title);
        if pill {
            button("widget-back", &label, &icons::BACK_TO, p.accent, Action::Back)
        } else {
            row()
                .key("widget-back")
                .role("button")
                .label(label.clone())
                .focusable(true)
                .w_full()
                .h(29.0)
                .px(6.0)
                .gap(7.0)
                .center_y()
                .hover(p.hover)
                .radius(6.0)
                .cursor(rune::Cursor::Pointer)
                .child(icon(&icons::BACK_TO, 14.0).color(p.accent))
                .child(text(label).font(ty::SMALL).color(p.text).single_line().truncate())
                .on_press_tx(|s: &mut Widget, _| s.action(Action::Back))
        }
    });
    let body = if pill {
        row().w_full().h_full().px(10.0).gap(4.0).center_y().child(main).children(play()).children(back)
    } else {
        col()
            .w_full()
            .h_full()
            .px(13.0)
            .pt(6.0)
            .child(main)
            .child(row().w_full().h(40.0).gap(4.0).center_y().child(time()).children(play()).child(button(
                "widget-stop",
                "Stop",
                &icons::STOP,
                p.muted,
                Action::Stop,
            )))
            .children(back)
    };
    body.key("timer-widget")
        .role("group")
        .label("Quest timer")
        .radius(if pill { 19.0 } else { 14.0 })
        .fill(p.card)
        .border(1.0, p.border_strong)
        .on_secondary_press_local(|s: &mut Widget, point, tx| context_menu(s, point.window(), tx))
}
