//! Quest for macOS, built with Rune. `cargo run -p quest-app -- --demo --data-dir /tmp/q` opens
//! the window; add Rune's `--snap out.png --scene plan` to render a view headless.

#![forbid(unsafe_code)]

mod browser;
mod cli;
mod clock;
mod demo;
mod desktop;
mod dnd;
mod icons;
mod keys;
mod loopback;
mod menu_bar;
mod review_demo;
mod settings;
mod state;
mod theme;
mod ui;

// The pure presentation modules live in quest-view; mounted here so `crate::format` etc. resolve.
use quest_view::{format, log_work, menu, palette, undo};

use std::sync::Arc;

use quest_store::{Profiles, ProfilesConfig, Store, TokenVault};
use rune::{App, AppSpec, Titlebar};

use crate::clock::Clock;
use crate::state::{Overlay, State, View};

/// A second workspace "Work" and a deleted one; returns Work's id.
fn workspaces_scene(s: &mut State) -> quest_domain::EntityId {
    let work = quest_domain::EntityId::new();
    let old = quest_domain::EntityId::new();
    s.dispatch(quest_domain::Command::CreateWorkspace { id: work, name: "Work".into() });
    s.dispatch(quest_domain::Command::CreateWorkspace { id: old, name: "Side project".into() });
    s.dispatch(quest_domain::Command::DeleteWorkspace { id: old, at: s.clock.now() });
    work
}

/// A named starting state for `--scene`.
type Scene = (&'static str, fn(&mut State));

/// Named starting states for headless snapshots (`--scene plan`).
const SCENES: &[Scene] = &[
    ("home", |s| s.go(View::Today)),
    ("plan", |s| s.go(View::Plan)),
    ("workspace-menu", |s| {
        workspaces_scene(s);
        s.workspace_menu = true;
    }),
    ("workspaces", |s| {
        workspaces_scene(s);
        s.open_workspaces();
    }),
    ("workspace-delete", |s| {
        workspaces_scene(s);
        s.open_workspaces();
        s.ask_delete_workspace(quest_domain::EntityId::DEFAULT_WORKSPACE);
    }),
    ("timer-elsewhere", |s| {
        let work = workspaces_scene(s);
        s.switch_workspace(work);
    }),
    ("focus", |s| s.go(View::Focus)),
    ("quests", |s| s.go(View::Quests)),
    ("quests-completed", |s| quests_completed(s, 1, state::QuestsTab::Completed)),
    ("quests-empty-completed", |s| quests_completed(s, 0, state::QuestsTab::Completed)),
    ("quests-empty-active", |s| quests_completed(s, usize::MAX, state::QuestsTab::Active)),
    ("quest-editor", |s| {
        s.go(View::Quests);
        let quest = s.snap.active_quests().next().map(|q| q.id);
        if let Some(quest) = quest {
            s.open_quest_editor(quest);
        }
    }),
    ("quest-new", |s| {
        s.go(View::Quests);
        s.open_new_quest();
    }),
    ("review", |s| s.go(View::Review)),
    ("review-demo-seed", review_demo::seed),
    ("review-demo-seed-light", |s| light(s, review_demo::seed)),
    ("palette", |s| s.overlay = Overlay::Palette),
    ("account", |s| s.overlay = Overlay::Account),
    ("log-work", |s| {
        let (start, end) = (chrono::NaiveTime::from_hms_opt(14, 50, 0), chrono::NaiveTime::from_hms_opt(15, 5, 0));
        if let (Some(start), Some(end)) = (start, end) {
            s.open_log_work(s.today(), start, end);
        }
    }),
    ("shortcuts", |s| s.overlay = Overlay::Shortcuts),
    ("quest-detail", |s| {
        let quest = s.snap.active_quests().next().map(|q| q.id);
        if let Some(quest) = quest {
            s.go(View::Quest(quest));
        }
    }),
    ("quest-colors", quest_colors),
    ("quest-colors-light", |s| light(s, quest_colors)),
    ("quest-rename", quest_rename),
    ("archive", archive),
    ("undo-toast", |s| {
        if let Some(task) = first_open_task(s) {
            s.delete(undo::Deletable::Task(task));
        }
    }),
    ("card-menu", |s| {
        s.go(View::Plan);
        s.menu = first_open_task(s).map(|task| state::TaskMenu { task });
    }),
    ("r4-detail", r4_detail),
    ("r4-detail-light", |s| light(s, r4_detail)),
    ("r4-edit", r4_edit),
    ("r4-edit-light", |s| light(s, r4_edit)),
    ("r4-focus", r4_focus),
    ("r4-focus-light", |s| light(s, r4_focus)),
    ("r4-list", |s| {
        r4_notes(s);
    }),
    ("r4-list-light", |s| {
        light(s, |s| {
            r4_notes(s);
        })
    }),
    ("task-detail", task_detail),
    ("task-detail-light", |s| light(s, task_detail)),
    ("task-quest-picker", task_quest_picker),
    ("task-quest-picker-light", |s| light(s, task_quest_picker)),
    ("task-plan-calendar", task_plan_calendar),
    ("task-plan-calendar-light", |s| light(s, task_plan_calendar)),
    ("task-deadline-calendar", task_deadline_calendar),
    ("task-deadline-calendar-light", |s| light(s, task_deadline_calendar)),
    ("quick-add-calendar", quick_add_calendar),
    ("quick-add-calendar-light", |s| light(s, quick_add_calendar)),
    ("task-done-toast", task_done_toast),
    ("task-done-toast-light", |s| light(s, task_done_toast)),
    ("task-modal", |s| {
        if let Some(task) = first_open_task(s) {
            s.open_task_modal(task);
        }
    }),
    ("task-modal-plan", |s| {
        s.go(View::Plan);
        if let Some(task) = first_open_task(s) {
            s.open_task_modal(task);
        }
    }),
    ("repeat-editor", |s| {
        s.go(View::Plan);
        if let Some(task) = first_open_task(s) {
            s.open_repeat_editor(task);
        }
    }),
    ("ai-notice", |s| s.overlay = Overlay::AiNotice),
    ("guest-import", |s| s.overlay = Overlay::GuestImport),
    ("capture", capture_compose),
    ("capture-review", capture_review),
    ("capture-toast", capture_toast),
    ("ai-notice-light", |s| {
        s.dark = false;
        s.overlay = Overlay::AiNotice;
    }),
    ("capture-light", |s| {
        s.dark = false;
        capture_compose(s);
    }),
    ("capture-review-light", |s| {
        s.dark = false;
        capture_review(s);
    }),
    ("task-modal-quests", |s| task_modal_quests(s, "")),
    ("task-modal-quests-filtered", |s| task_modal_quests(s, "ar")),
    ("log-work-quests", |s| {
        more_quests(s);
        let (start, end) = (chrono::NaiveTime::from_hms_opt(14, 50, 0), chrono::NaiveTime::from_hms_opt(15, 5, 0));
        if let (Some(start), Some(end)) = (start, end) {
            s.open_log_work(s.today(), start, end);
        }
        s.log_work.title = rune::TextEdit::new("Sketch the cover");
        s.toggle_quest_picker(state::quest_picker::PickFor::LogWork);
    }),
    ("capture-review-quests", |s| {
        more_quests(s);
        capture_review(s);
        let task = s.capture.review().and_then(|r| r.draft.proposal.tasks.first()).map(|t| t.key.clone());
        if let Some(task) = task {
            s.toggle_quest_picker(state::quest_picker::PickFor::CaptureTask(task));
        }
    }),
    ("home-quests", |s| {
        more_quests(s);
        s.toggle_quest_picker(state::quest_picker::PickFor::QuickAdd);
    }),
    ("quest-picker-empty", |s| task_modal_quests(s, "zzz")),
    ("today", today_selected),
    ("today-light", |s| light(s, today_selected)),
    ("inbox", |s| s.go(View::Inbox)),
    ("inbox-light", |s| light(s, |s| s.go(View::Inbox))),
    ("logbook", |s| s.go(View::Logbook)),
    ("logbook-light", |s| light(s, |s| s.go(View::Logbook))),
    ("quest-detail-light", |s| {
        light(s, |s| {
            let quest = s.snap.active_quests().next().map(|q| q.id);
            if let Some(quest) = quest {
                s.go(View::Quest(quest));
            }
        })
    }),
    ("tracked-closed", |s| s.right_column = false),
    ("tracked-closed-light", |s| light(s, |s| s.right_column = false)),
    ("entry-editor", entry_editor),
    ("entry-editor-light", |s| light(s, entry_editor)),
    ("shortcuts-light", |s| light(s, |s| s.overlay = Overlay::Shortcuts)),
    ("r5-hint", focus_hint),
    ("r5-hint-light", |s| light(s, focus_hint)),
    ("r5-booked", |s| {
        focus_hint(s);
        s.book_organizing();
    }),
    ("r5-booked-light", |s| {
        light(s, |s| {
            focus_hint(s);
            s.book_organizing();
        })
    }),
    ("organizing", organizing),
    ("organizing-light", |s| light(s, organizing)),
    ("light", |s| s.dark = false),
    ("light-plan", |s| {
        s.dark = false;
        s.go(View::Plan);
    }),
];

/// `scene` in the light appearance.
fn light(s: &mut State, scene: fn(&mut State)) {
    s.dark = false;
    scene(s);
}

/// Today with its second row selected, as J leaves it.
fn today_selected(s: &mut State) {
    s.go(View::Today);
    s.select_next_row(true);
    s.select_next_row(true);
}

/// The entry editor on the timeline's first session today, as a click on its block opens it.
fn entry_editor(s: &mut State) {
    let day = quest_view::tracked::day_timeline(&s.snap, s.today(), &chrono::Local, s.now);
    if let Some(block) = day.blocks.first() {
        s.open_entry_editor(block.entry);
    }
}

/// The running session booked as Organizing for its last twelve minutes: the player offers to go
/// back to the task.
fn organizing(s: &mut State) {
    let now = s.clock.now();
    s.dispatch(quest_domain::Command::RebookAsOrganizing {
        expected_task_entry: None,
        entry_id: quest_domain::EntityId::new(),
        since: now - chrono::Duration::minutes(12),
        now,
    });
}

/// Today's first open task's dialog, with the quest picker open on `search`.
fn focus_hint(s: &mut State) {
    let original = s.clock;
    s.go(View::Focus);
    s.clock = Clock::Fixed(original.now() - chrono::TimeDelta::minutes(22));
    s.go(View::Today);
    s.clock = original;
}

fn task_modal_quests(s: &mut State, search: &str) {
    more_quests(s);
    if let Some(task) = first_open_task(s) {
        s.open_task_modal(task);
    }
    s.toggle_quest_picker(state::quest_picker::PickFor::TaskModal);
    s.quest_picker_search().insert(search);
    s.quest_search_changed();
}

fn task_detail(s: &mut State) {
    use quest_domain::Command;
    let Some(id) = first_open_task(s) else { return };
    let today = s.today();
    s.dispatch(Command::SetTaskDescription { id, description: "Start with the opening spread.".into() });
    s.dispatch(Command::SetTaskDue { id, due_on: Some(today), today, at: s.now });
    s.dispatch(Command::SetTaskDue { id, due_on: today.checked_add_days(chrono::Days::new(3)), today, at: s.now });
    s.open_task_modal(id);
}

fn r4_notes(s: &mut State) -> Option<quest_domain::EntityId> {
    use quest_domain::{Command, EntityId, Position};

    let id = first_open_task(s)?;
    let description = "Three screens: **onboarding**, *first list*, first tracked hour.\n\n### Open\n- Empty state: copy or a quiet illustration?\n- Does `Today` start empty?\n\n> Calm first, clever later.\n\nSketches: [shared board](https://example.org/board)";
    s.dispatch(Command::SetTaskDescription { id, description: description.into() });
    for (index, text) in ["Onboarding sketch", "First list", "First tracked hour", "Review with Anna", "Export frames"]
        .into_iter()
        .enumerate()
    {
        let item = EntityId::new();
        s.dispatch(Command::AddChecklistItem { id: item, task_id: id, text: text.into(), position: Position::Bottom });
        if index < 2 {
            s.dispatch(Command::SetChecklistItemDone { id: item, done: true });
        }
    }
    Some(id)
}

fn r4_detail(s: &mut State) {
    if let Some(id) = r4_notes(s) {
        s.open_task_modal(id);
    }
}

fn r4_edit(s: &mut State) {
    if let Some(id) = r4_notes(s) {
        s.open_task_modal(id);
        s.open_note(state::notes::NoteHost::Detail, id, &mut rune::Tx::default());
    }
}

fn r4_focus(s: &mut State) {
    if let Some(id) = r4_notes(s) {
        let clock = s.clock;
        s.clock = Clock::Fixed(clock.now() - chrono::TimeDelta::minutes(18));
        s.start_focus(id);
        s.clock = clock;
    }
}

fn task_quest_picker(s: &mut State) {
    task_detail(s);
    s.toggle_quest_picker(state::quest_picker::PickFor::TaskModal);
}

fn task_date_calendar(s: &mut State, field: quest_domain::DateField) {
    task_detail(s);
    if let Some(id) = s.task_modal.as_ref().map(|form| form.task) {
        s.open_calendar(Some(id), field, &mut rune::Tx::default());
    }
}

fn task_plan_calendar(s: &mut State) {
    task_date_calendar(s, quest_domain::DateField::Plan);
}

fn task_deadline_calendar(s: &mut State) {
    task_date_calendar(s, quest_domain::DateField::Due);
}

fn quick_add_calendar(s: &mut State) {
    s.quick_add = rune::TextEdit::new("Sketch the cover");
    s.open_calendar(None, quest_domain::DateField::Plan, &mut rune::Tx::default());
}

fn task_done_toast(s: &mut State) {
    s.go(View::Inbox);
    if let Some(id) = quest_view::lists::inbox(&s.snap).first().map(|task| task.id) {
        s.toggle_done(id);
    }
}

/// Eleven more quests, fifteen in all: for the quest picker.
fn more_quests(s: &mut State) {
    let titles = [
        "Learn Spanish",
        "Renovate the kitchen",
        "Read 20 books",
        "Grow tomatoes",
        "Ship the mobile app",
        "Plan the wedding",
        "Practise guitar",
        "Get fit for skiing",
        "Clear the attic",
        "Start a podcast",
        "Family history",
    ];
    for (i, title) in titles.into_iter().enumerate() {
        let color = quest_domain::QuestColor::ALL[i % quest_domain::QuestColor::ALL.len()];
        s.dispatch(quest_domain::Command::CreateQuest {
            id: quest_domain::EntityId::new(),
            title: title.into(),
            color,
        });
    }
}

/// The capture panel with the brief's example typed in.
fn capture_compose(s: &mut State) {
    s.open_capture();
    s.capture.text = rune::TextEdit::new(
        "Tomorrow write the offer for Müller, takes about two hours, belongs to Launch my idea. New quest tax \
         return with collect receipts on Friday. Today from nine to eleven I worked on the design.",
    );
}

/// The proposal for that text, as the review shows it.
fn capture_review(s: &mut State) {
    capture_compose(s);
    let proposal = demo::capture_proposal(&s.snap, s.today());
    s.capture.step = state::CaptureStep::Review(state::capture_draft::Review::new(proposal));
}

/// Home after creating that proposal: the toast offers to undo it.
fn capture_toast(s: &mut State) {
    capture_review(s);
    s.create_from_capture();
}

/// Quests with its first `count` active quests completed, showing `tab`.
fn quests_completed(s: &mut State, count: usize, tab: state::QuestsTab) {
    s.go(View::Quests);
    let quests: Vec<_> = s.snap.active_quests().map(|q| q.id).take(count).collect();
    for quest in quests {
        s.dispatch(quest_domain::Command::SetQuestStatus { id: quest, status: quest_domain::QuestStatus::Completed });
    }
    s.quests_tab = tab;
}

/// Today's first open task.
fn first_open_task(s: &State) -> Option<quest_domain::EntityId> {
    s.snap.tasks_on(s.today()).find(|t| !t.is_done()).map(|t| t.id)
}

/// The first quest's page with its name open in the rename field.
fn quest_colors(s: &mut State) {
    let Some(id) = s.snap.active_quests().next().map(|q| q.id) else { return };
    s.go(View::Quest(id));
    s.quest_edit = state::QuestEdit::Color(id);
}

fn quest_rename(s: &mut State) {
    let Some(quest) = s.snap.active_quests().next().map(|q| q.id) else { return };
    s.go(View::Quest(quest));
    let title = s.snap.quest(quest).map(|q| q.title.clone()).unwrap_or_default();
    s.quest_text = rune::TextEdit::new(title);
    // Asks Rune to focus the field once it shows.
    s.quest_text.focused = true;
    s.quest_edit = state::QuestEdit::Rename(quest);
}

/// The archive with one thing of each kind deleted and a quest archived.
fn archive(s: &mut State) {
    use undo::Deletable;
    let quests: Vec<_> = s.snap.active_quests().map(|q| q.id).collect();
    let entry = s.snap.time_entries.iter().find(|e| !e.is_running()).map(|e| e.id);
    let items = [first_open_task(s).map(Deletable::Task), entry.map(Deletable::Entry)];
    for item in items.into_iter().flatten() {
        s.delete(item);
    }
    if let Some(quest) = quests.last() {
        s.dispatch(quest_domain::Command::SetQuestStatus { id: *quest, status: quest_domain::QuestStatus::Archived });
    }
    s.step_undo(undo::UndoEvent::Dismiss);
    s.go(View::Archive);
}

fn main() {
    rune::prewarm_gpu();
    let options = match cli::parse(
        rune::extra_args(),
        std::env::var_os("QUEST_DATA_DIR").map(Into::into),
        std::env::var_os("HOME").map(Into::into),
    ) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("quest: {error}");
            std::process::exit(2);
        }
    };
    let clock = Clock::from_local(options.now);
    let repeat_zone = iana_time_zone::get_timezone().ok().and_then(|zone| zone.parse().ok()).unwrap_or(chrono_tz::UTC);
    let spec = AppSpec {
        scenes: SCENES,
        min_size: Some((1000.0, 640.0)),
        poll: Some(State::refresh),
        on_key: Some(keys::on_key),
        ..AppSpec::new("Quest", (1440.0, 900.0), || unreachable!("state comes from App::init"), ui::view)
    };
    // Quest draws its own title band; the traffic lights sit in the sidebar's, above the
    // workspace switcher.
    let app = menu_bar::install(
        App::new(spec)
            .titlebar(Titlebar::new(ui::sidebar::TITLEBAR))
            .keep_running()
            .on_focus(|s: &mut State, focused, tx| {
                if let Some(windows) = &s.desktop {
                    windows.set_main_focused(focused);
                }
                if !focused {
                    s.close_note(tx);
                }
            })
            .on_visibility(|s: &mut State, visible, _| {
                if !visible && let Some(windows) = &s.desktop {
                    windows.set_main_focused(false);
                }
            })
            .on_close(finish_window)
            .on_quit(finish_window),
    );
    let waker = Arc::new(std::sync::OnceLock::<Box<dyn Fn() + Send + Sync>>::new());
    let store_waker = Arc::downgrade(&waker);
    let config = ProfilesConfig { data_dir: options.data_dir.clone(), vault: token_vault(&options) };
    let (profiles, store) = match Profiles::open(config, move || {
        if let Some(wake) = store_waker.upgrade().as_deref().and_then(|slot| slot.get()) {
            wake();
        }
    }) {
        Ok((profiles, store)) => (profiles, Arc::new(store)),
        Err(error) => {
            eprintln!("quest: {error}");
            std::process::exit(1);
        }
    };
    let browser = options.browser.as_ref().map_or_else(browser::Browser::default, browser::Browser::with_program);
    let language = settings::system_language();
    let capture_source = match &options.capture_fixture {
        None => state::capture::CaptureSource::Server,
        Some(path) => match read_fixture(path) {
            Ok(proposal) => state::capture::CaptureSource::Fixture(proposal),
            Err(error) => {
                eprintln!("quest: --capture-fixture {}: {error}", path.display());
                std::process::exit(2);
            }
        },
    };
    if options.demo
        && let Err(error) = seed_demo(&store, clock)
    {
        eprintln!("quest: seeding the demo failed: {error}");
    }
    let (preferences, preference_worker) = desktop::PreferenceWriter::open(&options.data_dir);
    let register_waker = waker.clone();
    rune::run_windows(move |displays| {
        let wake = app.handle();
        let (app, windows, wake_all) = desktop::MainWindows::install(app, store.clone(), clock, displays, preferences);
        let _ = register_waker.set(Box::new(wake_all));
        let app = app.init(move || {
            let mut state = State::new(store, profiles, clock);
            state.desktop = Some(windows);
            state.repeat_zone = repeat_zone;
            state.language = language;
            state.capture.source = capture_source;
            state.auth.browser = browser;
            let auth_wake = wake.clone();
            state.auth.set_waker(Arc::new(move || auth_wake.wake()));
            let connections_wake = wake.clone();
            state.capture.set_waker(Arc::new(move || wake.wake()));
            state.ai_connections.set_waker(Arc::new(move || connections_wake.wake()));
            state
        });
        [rune::WindowSpec::from_app("main", app)]
    });
    drop(waker);
    let _ = preference_worker.join();
}

fn finish_window(s: &mut State, _: &mut rune::Tx) {
    s.finish_note();
    s.cancel_capture();
}

/// Where sign-in tokens are kept: the macOS keychain, or for development and E2E runs the file
/// given with `--token-file`.
fn token_vault(options: &cli::Options) -> Arc<dyn TokenVault> {
    match &options.token_file {
        Some(path) => Arc::new(quest_store::FileVault::new(path)),
        None => Arc::new(quest_store::KeychainVault::new(KEYCHAIN_SERVICE)),
    }
}

/// Names the keychain items of the sign-in tokens.
const KEYCHAIN_SERVICE: &str = "app.quest.desktop";

/// A `CaptureProposal` as JSON, read once at launch (before the event loop).
fn read_fixture(path: &std::path::Path) -> Result<quest_protocol::CaptureProposal, String> {
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&json).map_err(|e| format!("not a capture proposal: {e}"))
}

/// Seeds the demo data into an empty store; a store that has anything in it is left alone.
fn seed_demo(store: &Store, clock: Clock) -> Result<(), quest_domain::CommandError> {
    let snapshot = store.snapshot();
    if !snapshot.quests.is_empty() || !snapshot.tasks.is_empty() {
        return Ok(());
    }
    let now = clock.now();
    demo::commands(now, clock::local_day(now), clock::to_utc)
        .into_iter()
        .try_for_each(|command| store.dispatch(command))
}
