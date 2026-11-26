mod panel;
mod widget;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};

use quest_store::Store;
use quest_view::widget::Mode;
use rune::{App, AppHandle, Display, Tx, WindowOptions, WindowSpec};

use crate::{clock::Clock, state::State};

pub const SHORTCUT: &str = "ctrl+alt+space";

#[derive(Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct Preferences {
    mode: Mode,
    position: Option<(i32, i32)>,
}

pub struct PreferenceWriter {
    preferences: Preferences,
    save: mpsc::Sender<Preferences>,
}

impl PreferenceWriter {
    pub fn open(data: &Path) -> (Self, std::thread::JoinHandle<()>) {
        let path = data.join("desktop-widget.json");
        let preferences =
            std::fs::read(&path).ok().and_then(|data| serde_json::from_slice(&data).ok()).unwrap_or_default();
        let (save, updates) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            while let Ok(mut preferences) = updates.recv() {
                while let Ok(newer) = updates.try_recv() {
                    preferences = newer;
                }
                if let Err(error) = save_preferences(&path, preferences) {
                    eprintln!("quest: couldn’t save timer widget settings: {error}");
                }
            }
        });
        (Self { preferences, save }, worker)
    }

    fn change(&mut self, update: impl FnOnce(&mut Preferences)) {
        let before = self.preferences;
        update(&mut self.preferences);
        if self.preferences != before {
            let _ = self.save.send(self.preferences);
        }
    }
}

fn save_preferences(path: &Path, preferences: Preferences) -> std::io::Result<()> {
    let temp: PathBuf = path.with_extension("tmp");
    std::fs::write(&temp, serde_json::to_vec(&preferences)?)?;
    std::fs::rename(temp, path)
}

struct DesktopData {
    store: Arc<Store>,
    dark: bool,
    main_focused: bool,
    panel_open: bool,
    locks_edits: bool,
    profile: u64,
}

type Desktop = Arc<Mutex<DesktopData>>;

pub struct MainWindows {
    shared: Desktop,
    panel: AppHandle<panel::Panel>,
    widget: AppHandle<widget::Widget>,
}

impl MainWindows {
    pub fn install(
        mut app: App<State>,
        store: Arc<Store>,
        clock: Clock,
        displays: &[Display],
        writer: PreferenceWriter,
    ) -> (App<State>, Self, impl Fn() + Send + Sync + 'static) {
        let shared = Arc::new(Mutex::new(DesktopData {
            store,
            dark: true,
            main_focused: false,
            panel_open: false,
            locks_edits: false,
            profile: 0,
        }));
        let main = app.handle();
        let (panel_spec, feedback_spec) = panel::windows(shared.clone(), clock, main.clone());
        app.window(feedback_spec);
        let (panel, _) = app.window(panel_spec);
        let (widget, _) = app.window(widget::window(shared.clone(), clock, main.clone(), displays, writer));
        let (wake_panel, wake_widget) = (panel.clone(), widget.clone());
        let wake = move || {
            main.wake();
            wake_panel.wake();
            wake_widget.wake();
        };
        (app, Self { shared, panel, widget }, wake)
    }

    pub fn set_main_focused(&self, focused: bool) {
        self.shared.lock().unwrap().main_focused = focused;
        self.widget.wake();
    }

    pub fn publish(&self, store: &Arc<Store>, dark: bool, locks_edits: bool) {
        let mut shared = self.shared.lock().unwrap();
        let profile_changed = !Arc::ptr_eq(store, &shared.store);
        if profile_changed {
            shared.store = store.clone();
            shared.profile += 1;
        }
        if profile_changed || shared.dark != dark || shared.locks_edits != locks_edits {
            shared.dark = dark;
            shared.locks_edits = locks_edits;
            drop(shared);
            self.panel.wake();
            self.widget.wake();
        }
    }
}

pub fn quick_add(s: &mut State, _: &mut Tx) {
    if let Some(windows) = &s.desktop {
        windows.panel.update(panel::Panel::toggle);
    }
}

// Rune's native keys-first menu dispatch excludes NSPanels.
pub fn forward_panel_key(s: &State, event: rune::KeyEvent) -> bool {
    let Some(windows) = &s.desktop else { return false };
    if !windows.shared.lock().unwrap().panel_open {
        return false;
    }
    windows.panel.update(move |panel, tx| {
        panel.key(&event, tx);
    });
    true
}

pub fn show_widget(s: &mut State, _: &mut Tx) {
    if let Some(windows) = &s.desktop {
        windows.widget.update(|widget, _| widget.set_mode(Mode::Card));
    }
}

fn passive_panel<S: 'static>(key: &str, app: App<S>, click_through: bool) -> WindowSpec<S> {
    WindowSpec::from_app(key, app).configured_options(
        WindowOptions {
            transparent: true,
            borderless: true,
            visible: false,
            resizable: false,
            always_on_top: true,
            click_through,
            min_size: Some((0.0, 0.0)),
            ..Default::default()
        }
        .macos(rune::MacWindowOptions::default().nonactivating_panel(true)),
    )
}
