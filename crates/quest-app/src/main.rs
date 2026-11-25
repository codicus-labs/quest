//! Quest for macOS, built with Rune. `cargo run -p quest-app -- --demo --data-dir /tmp/q` opens
//! the window; add Rune's `--snap out.png --scene plan` to render a view headless.

#![forbid(unsafe_code)]

mod browser;
mod cli;
mod clock;
mod demo;
mod dnd;
mod icons;
mod keys;
mod loopback;
mod settings;
mod state;
mod theme;
mod ui;

// The pure presentation modules live in quest-view; mounted here so `crate::format` etc. resolve.
use quest_view::{entry_editor, format, log_work, menu, palette, plan_strip, timescale, undo};

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
    ("home", |s| s.go(View::Home)),
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
    ("calendar", |s| s.go(View::Calendar)),
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
    ("palette", |s| s.overlay = Overlay::Palette),
    ("account", |s| s.overlay = Overlay::Account),
    ("log-work", |s| {
        let (start, end) = (chrono::NaiveTime::from_hms_opt(14, 50, 0), chrono::NaiveTime::from_hms_opt(15, 5, 0));
        if let (Some(start), Some(end)) = (start, end) {
            s.go(View::Calendar);
            s.open_log_work(s.today(), start, end);
        }
    }),
    ("shortcuts", |s| s.overlay = Overlay::Shortcuts),
    ("entry-editor", |s| {
        s.go(View::Calendar);
        if let Some(entry) = finished_entry_on(s, s.today()) {
            s.open_entry_editor(entry);
        }
    }),
    ("entry-midnight", entry_midnight),
    ("home-tracked", home_tracked),
    ("quest-detail", |s| {
        let quest = s.snap.active_quests().next().map(|q| q.id);
        if let Some(quest) = quest {
            s.go(View::Quest(quest));
        }
    }),
    ("quest-detail-no-milestones", |s| {
        let quest = s.snap.active_quests().find(|q| s.snap.milestones_of(q.id).next().is_none()).map(|q| q.id);
        if let Some(quest) = quest {
            s.go(View::Quest(quest));
        }
    }),
    ("quest-rename", quest_rename),
    ("archive", archive),
    ("undo-toast", |s| {
        if let Some(task) = first_open_task(s) {
            s.delete(undo::Deletable::Task(task));
        }
    }),
    ("card-menu", |s| s.menu = first_open_task(s).map(|task| state::TaskMenu { task, prefix: "home" })),
    ("task-modal", |s| {
        if let Some(task) = first_open_task(s) {
            s.open_task_modal(task);
        }
    }),
    ("task-modal-plan", |s| {
        s.go(View::Plan);
        let done = s.snap.tasks_on(s.today()).find(|t| t.is_done()).map(|t| t.id);
        if let Some(task) = done {
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
            s.go(View::Calendar);
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
    ("entry-editor-quests", |s| {
        more_quests(s);
        s.go(View::Calendar);
        if let Some(entry) = finished_entry_on(s, s.today()) {
            s.open_entry_editor(entry);
        }
        s.toggle_quest_picker(state::quest_picker::PickFor::Entry);
    }),
    ("home-quests", |s| {
        more_quests(s);
        s.toggle_quest_picker(state::quest_picker::PickFor::HomeAdd);
    }),
    ("quest-picker-empty", |s| task_modal_quests(s, "zzz")),
    ("light", |s| s.dark = false),
    ("light-plan", |s| {
        s.dark = false;
        s.go(View::Plan);
    }),
];

/// Today's first open task's dialog, with the quest picker open on `search`.
fn task_modal_quests(s: &mut State, search: &str) {
    more_quests(s);
    if let Some(task) = first_open_task(s) {
        s.open_task_modal(task);
    }
    s.toggle_quest_picker(state::quest_picker::PickFor::TaskModal);
    s.quest_picker_search().insert(search);
    s.quest_search_changed();
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
    let milestone = quests.first().and_then(|q| s.snap.milestones_of(*q).next()).map(|m| m.id);
    let entry = s.snap.time_entries.iter().find(|e| !e.is_running()).map(|e| e.id);
    let items =
        [first_open_task(s).map(Deletable::Task), milestone.map(Deletable::Milestone), entry.map(Deletable::Entry)];
    for item in items.into_iter().flatten() {
        s.delete(item);
    }
    if let Some(quest) = quests.last() {
        s.dispatch(quest_domain::Command::SetQuestStatus { id: *quest, status: quest_domain::QuestStatus::Archived });
    }
    s.step_undo(undo::UndoEvent::Dismiss);
    s.go(View::Archive);
}

/// The last finished session on `day`.
fn finished_entry_on(s: &State, day: chrono::NaiveDate) -> Option<quest_domain::EntityId> {
    let on_day = |e: &&quest_domain::TimeEntry| !e.is_running() && clock::local_day(e.started_at) == day;
    s.snap.time_entries.iter().filter(on_day).max_by_key(|e| e.started_at).map(|e| e.id)
}

/// The calendar after the editor moved yesterday's last session to run over midnight (22:15 to
/// 07:30 today): a segment on each day, and each day's total counts its part.
fn entry_midnight(s: &mut State) {
    s.go(View::Calendar);
    let Some(entry) = finished_entry_on(s, s.today() - chrono::Days::new(1)) else { return };
    s.open_entry_editor(entry);
    let form = &mut s.entry_form;
    form.start = rune::TextEdit::new("22:15");
    form.end_date = rune::TextEdit::new(log_work::date_label(clock::local_day(s.now)));
    form.end = rune::TextEdit::new("07:30");
    s.save_entry_editor();
}

/// Home with a manual session that overlaps a tracked one (two lanes).
fn home_tracked(s: &mut State) {
    let today = s.today();
    let at = |h, m| clock::at_local(today, chrono::NaiveTime::from_hms_opt(h, m, 0).unwrap_or_default());
    let entry_id = quest_domain::EntityId::new();
    s.dispatch(quest_domain::Command::LogTime {
        entry_id,
        task_id: None,
        started_at: at(11, 30),
        ended_at: at(12, 15),
    });
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
        min_size: Some((900.0, 600.0)),
        poll: Some(|s: &mut State, clock, tx| s.refresh(clock, tx)),
        on_key: Some(keys::on_key),
        ..AppSpec::new("Quest", (1100.0, 750.0), || unreachable!("state comes from App::init"), ui::view)
    };
    // Quest draws its own top bar; the traffic lights sit in it, and the bar keeps clear of them.
    let app = App::new(spec)
        .titlebar(Titlebar::new(ui::topbar::HEIGHT))
        .on_close(|s: &mut State, _: &mut rune::Tx| s.cancel_capture());
    let handle = app.handle();
    // Opening the database happens once at launch, before the event loop draws anything;
    // afterwards all disk and network work is on the store's worker thread and the account
    // flows' threads.
    let config = ProfilesConfig { data_dir: options.data_dir.clone(), vault: token_vault(&options) };
    let (profiles, store) = match Profiles::open(config, move || handle.wake()) {
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
    let wake = app.handle();
    if options.demo
        && let Err(error) = seed_demo(&store, clock)
    {
        eprintln!("quest: seeding the demo failed: {error}");
    }
    app.init(move || {
        let mut state = State::new(store, profiles, clock);
        state.repeat_zone = repeat_zone;
        state.language = language;
        state.capture.source = capture_source;
        state.auth.browser = browser;
        let auth_wake = wake.clone();
        state.auth.set_waker(Arc::new(move || auth_wake.wake()));
        let connections_wake = wake.clone();
        state.capture.set_waker(std::sync::Arc::new(move || wake.wake()));
        state.ai_connections.set_waker(std::sync::Arc::new(move || connections_wake.wake()));
        state
    })
    .run();
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
