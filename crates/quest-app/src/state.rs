//! App state: the store, the latest snapshot and sync status it reported, and what the UI shows.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Days, NaiveDate, Utc};
use quest_domain::{Command, EntityId, Snapshot, Task, week_start};
use quest_store::{Account, Profiles, Store, SyncStatus};
use rune::{TextEdit, Tx};

use crate::clock::{self, Clock};
use crate::dnd::{DragState, Rect};
use crate::palette::Mode;
use crate::ui::drag::{self, AutoScroll, Landing, Zone};
use crate::undo::{Deletable, UndoEffect, UndoEvent, UndoState};

pub mod ai_connections;
pub mod auth;
pub mod capture;
pub mod capture_draft;
#[cfg(test)]
mod capture_tests;
mod checklist;
pub mod entry_editor;
mod focus;
mod log_work;
pub mod notes;
pub mod quest_editor;
pub mod quest_picker;
pub mod recurrence;
pub(crate) mod task_edits;
pub mod task_modal;
mod workspaces;

use ai_connections::AiConnectionsState;
pub use auth::{AuthState, AuthStep};
pub use capture::{CaptureState, CaptureStep, CaptureUndo};
pub use focus::{FocusDraft, FocusTarget};
pub use log_work::LogWorkForm;
pub use quest_editor::QuestEditor;
pub use quest_view::nav::View;
pub use quest_view::quests::QuestsTab;
pub use task_modal::TaskModal;
pub use workspaces::WorkspacesForm;

/// The context menu open on a task row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskMenu {
    pub task: EntityId,
}

/// What floats above the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    None,
    Palette,
    Account,
    /// The Log work dialog; its fields are `State::log_work`.
    LogWork,
    /// The keyboard shortcut sheet (?).
    Shortcuts,
    RepeatEditor,
    RecurringTasks,
    /// AI capture; its steps are `State::capture`.
    Capture,
    /// After the first sign-in of an account: import the guest data into it?
    GuestImport,
    /// The privacy notice before turning AI features on.
    AiNotice,
    /// A quest's dialog over the Quests page; its fields are `State::quest_editor`.
    Quest,
    /// Manage workspaces; its fields are `State::workspaces`.
    Workspaces,
    /// A session's editor, opened from the timeline; its fields are `State::entry_form`.
    EntryEditor,
}

/// An inline editor on a quest page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestEdit {
    None,
    Rename(EntityId),
    Color(EntityId),
}

pub struct PaletteState {
    pub mode: Mode,
    pub query: TextEdit,
    pub picked: usize,
}

pub struct AccountForm {
    pub server: TextEdit,
}

pub struct State {
    pub store: Arc<Store>,
    pub snap: Arc<Snapshot>,
    pub sync: SyncStatus,
    /// The store's account; `None` for the guest.
    pub account: Option<Account>,
    /// Signing in and out, guest import, the AI setting.
    pub auth: AuthState,
    /// Local changes the server refused for good; they never reach other devices.
    pub rejected: usize,
    pub clock: Clock,
    pub now: DateTime<Utc>,
    pub dark: bool,
    pub view: View,
    pub overlay: Overlay,
    pub palette: PaletteState,
    pub account_form: AccountForm,
    /// The quick-add field above Today, Inbox and a quest's list.
    pub quick_add: TextEdit,
    /// The quest picked on Today's quick-add chip; none files the task without a quest.
    pub quick_fields: Option<quest_view::quick_add::Fields>,
    pub quick_choice: usize,
    pub task_calendar: Option<task_edits::Calendar>,
    pub lingering_tasks: HashMap<EntityId, (DateTime<Utc>, Task)>,
    pub show_task_time: bool,
    /// Whether the tracked-time column shows (T, ⌥⌘I); it stays across views.
    pub right_column: bool,
    /// The list row the keyboard moved to (J/K, ↑/↓); the task keys act on it.
    pub selected_task: Option<EntityId>,
    pub plan_quick_day: Option<NaiveDate>,
    pub show_not_planned: bool,
    /// The task card under the pointer: the task shortcuts (Space, X, T, D, W, ⌘⌫, ⌫, ↵) act on
    /// it.
    pub hovered_task: Option<EntityId>,
    pub menu: Option<TaskMenu>,
    pub undo: UndoState,
    pub drag: DragState,
    pub drag_scroll: AutoScroll,
    /// Where drop zones were last laid out, as their elements reported it.
    pub zones: HashMap<Zone, Rect>,
    pub drag_tray_alias: bool,
    pub landing: Option<Landing>,
    pub review_week: NaiveDate,
    /// The task Focus is about when nothing runs: the one started or selected last.
    pub focus_task: Option<EntityId>,
    pub focus_exit: quest_view::focus_exit::FocusExit,
    focus_return: View,
    pub focus_draft: Option<FocusDraft>,
    pub note: Option<notes::NoteDraft>,
    closed_note: TextEdit,
    pub date_picker: Option<crate::ui::date_picker::Picker>,
    pub quest_picker: Option<quest_picker::QuestPicker>,
    /// Takes edits for a quest picker that already closed (`State::quest_picker_search`).
    closed_quest_search: TextEdit,
    pub log_work: LogWorkForm,
    pub task_modal: Option<TaskModal>,
    pub repeat_editor: recurrence::RepeatEditor,
    pub repeat_zone: chrono_tz::Tz,
    recurrence_minute: Option<i64>,
    /// Takes edits for a dialog that already closed (`State::task_modal_fields`).
    closed_task_modal: TaskModal,
    /// The tab the Quests page shows; it stays while the quest dialog opens and closes.
    pub quests_tab: QuestsTab,
    pub quest_editor: Option<QuestEditor>,
    /// Takes edits for a quest dialog that already closed (`State::quest_editor_fields`).
    closed_quest_editor: QuestEditor,
    pub quest_edit: QuestEdit,
    pub quest_text: TextEdit,
    /// The last command or sign-in failure, shown until the next action.
    pub notice: Option<String>,
    pub capture: CaptureState,
    /// "Connected AI clients" in the account dialog.
    pub ai_connections: AiConnectionsState,
    /// What the last AI capture created, until ⌘Z, the next delete or another capture.
    pub capture_undo: Option<CaptureUndo>,
    /// The system language (BCP 47), sent with AI captures.
    pub language: String,
    /// Whether the top bar's workspace switcher is open.
    pub workspace_menu: bool,
    pub workspaces: WorkspacesForm,
    pub entry_form: Option<entry_editor::EntryForm>,
    /// Takes edits for an entry editor that already closed (`State::entry_form_mut`).
    closed_entry_form: entry_editor::EntryForm,
}

impl State {
    pub fn new(store: Arc<Store>, profiles: Profiles, clock: Clock) -> State {
        let now = clock.now();
        let monday = week_start(clock::local_day(now));
        State {
            snap: store.snapshot(),
            sync: store.status(),
            account: store.account(),
            auth: AuthState::new(profiles),
            rejected: store.rejected_count(),
            store,
            clock,
            now,
            dark: true,
            view: View::Today,
            overlay: Overlay::None,
            palette: PaletteState { mode: Mode::Commands, query: TextEdit::new(""), picked: 0 },
            account_form: AccountForm { server: TextEdit::new("") },
            quick_add: TextEdit::new(""),
            quick_fields: None,
            quick_choice: 0,
            task_calendar: None,
            lingering_tasks: HashMap::new(),
            show_task_time: false,
            right_column: true,
            selected_task: None,
            plan_quick_day: None,
            show_not_planned: false,
            hovered_task: None,
            menu: None,
            undo: UndoState::default(),
            drag: DragState::Idle,
            drag_scroll: AutoScroll::default(),
            zones: HashMap::new(),
            drag_tray_alias: false,
            landing: None,
            review_week: monday,
            focus_task: None,
            focus_exit: Default::default(),
            focus_return: View::Today,
            focus_draft: None,
            note: None,
            closed_note: TextEdit::default(),
            date_picker: None,
            quest_picker: None,
            closed_quest_search: TextEdit::new(""),
            log_work: LogWorkForm::default(),
            task_modal: None,
            repeat_editor: recurrence::RepeatEditor::default(),
            repeat_zone: chrono_tz::UTC,
            recurrence_minute: None,
            closed_task_modal: TaskModal::scratch(),
            quests_tab: QuestsTab::Active,
            quest_editor: None,
            closed_quest_editor: QuestEditor::scratch(),
            quest_edit: QuestEdit::None,
            quest_text: TextEdit::new(""),
            notice: None,
            capture: CaptureState::default(),
            ai_connections: AiConnectionsState::default(),
            capture_undo: None,
            language: "en".to_owned(),
            workspace_menu: false,
            workspaces: WorkspacesForm::default(),
            entry_form: None,
            closed_entry_form: entry_editor::EntryForm::scratch(),
        }
    }

    pub fn today(&self) -> NaiveDate {
        clock::local_day(self.now)
    }

    pub fn this_week(&self) -> NaiveDate {
        week_start(self.today())
    }

    pub fn week_days(monday: NaiveDate) -> impl Iterator<Item = NaiveDate> {
        (0..7).map(move |i| monday + Days::new(i))
    }

    /// Runs a command through the store; a refusal shows as a notice.
    pub fn dispatch(&mut self, command: Command) {
        if self.auth.locks_edits() {
            self.notice = Some("Signing out; this change wasn’t made.".to_owned());
            return;
        }
        self.dispatch_task_change(command, true);
    }

    fn dispatch_task_change(&mut self, command: Command, remember: bool) {
        self.now = self.clock.now();
        let edit = remember.then(|| task_edits::TrackedEdit::from_command(&command)).flatten();
        let before = edit.and_then(|edit| self.plan_task(edit.id()).map(|task| task.into_owned()));
        let old = edit.map(|_| Arc::clone(&self.snap));
        let result = self.store.dispatch(self.plan_edit(command));
        self.adopt_snapshot();
        match result {
            Ok(()) => {
                self.notice = None;
                if let (Some(edit), Some(before), Some(old)) = (edit, before, old) {
                    self.remember_task_edit(edit, before, &old);
                }
            }
            Err(error) => self.notice = Some(format!("That didn’t work: {error}.")),
        }
        self.sync_focus_draft();
    }

    /// Shows `view`. The desktop has no Home: it shows Today.
    pub fn go(&mut self, view: View) {
        let view = if view == View::Home { View::Today } else { view };
        if view != self.view {
            self.selected_task = None;
            self.reset_quick_add();
            self.plan_quick_day = None;
            self.drag = DragState::Idle;
            self.landing = None;
        }
        self.finish_note();
        if self.view != View::Focus && view == View::Focus {
            self.focus_return = self.view;
            self.focus_exit.open();
        } else if self.view == View::Focus && view != View::Focus {
            self.focus_exit.leave(&self.snap, self.clock.now());
        }
        self.view = view;
        self.close_repeat_editor();
        self.close_task_modal();
        self.close_quest_editor();
        self.close_entry_editor();
        self.overlay = Overlay::None;
        self.menu = None;
        self.workspace_menu = false;
        self.hovered_task = None;
        self.quest_edit = QuestEdit::None;
        self.sync_focus_draft();
    }

    pub fn go_with(&mut self, view: View, tx: &mut Tx) {
        if self.view != view {
            tx.instant();
        }
        self.go(view);
    }

    /// The task the task shortcuts act on: the selected row, else the card under the pointer.
    pub fn shortcut_task(&self) -> Option<EntityId> {
        self.selected_task.or(self.hovered_task).filter(|id| self.plan_task(*id).is_some())
    }

    /// J/K: selects the next (or previous) row of the shown list; false without rows.
    pub fn select_next_row(&mut self, next: bool) -> bool {
        let rows = crate::ui::lists::shown_tasks(self);
        let Some(last) = rows.len().checked_sub(1) else { return false };
        let index = match self.selected_task.and_then(|id| rows.iter().position(|row| *row == id)) {
            None if next => 0,
            None => last,
            Some(i) if next => (i + 1).min(last),
            Some(i) => i.saturating_sub(1),
        };
        self.selected_task = Some(rows[index]);
        true
    }

    /// Soft-deletes `item` and offers to undo it.
    pub fn delete(&mut self, item: Deletable) {
        let title = self.title_of(item);
        self.dispatch(item.delete(self.clock.now()));
        if self.notice.is_some() {
            return;
        }
        if self.view == View::Quest(item.id()) {
            self.go(View::Quests);
        }
        self.step_undo(UndoEvent::Deleted { item, title });
    }

    /// Brings back something from the archive.
    pub fn restore(&mut self, item: Deletable) {
        self.dispatch(item.restore());
        self.step_undo(UndoEvent::Restored(item));
    }

    pub fn step_undo(&mut self, event: UndoEvent) {
        match event {
            // A newer delete is what ⌘Z takes back now; any other toast replaces the capture's.
            UndoEvent::Deleted { .. }
            | UndoEvent::QuestStatusChanged { .. }
            | UndoEvent::TaskEdited(_)
            | UndoEvent::ChecklistDone { .. }
            | UndoEvent::Rebooked(_) => {
                self.capture_undo = None;
            }
            _ => {}
        }
        let (next, effect) = self.undo.step(event, self.clock.now());
        self.undo = next;
        match effect {
            UndoEffect::None => {}
            UndoEffect::UndoRebook(entry_id) => {
                self.dispatch(Command::UndoOrganizingRebook { entry_id, at: self.clock.now() });
            }
            UndoEffect::Task(edit) => self.undo_task_edit(edit),
            UndoEffect::Restore(item) => self.dispatch(item.restore()),
            UndoEffect::SetChecklistDone { id, done } => self.dispatch(Command::SetChecklistItemDone { id, done }),
            UndoEffect::SetQuestStatus { quest, status } => {
                self.dispatch(Command::SetQuestStatus { id: quest, status })
            }
        }
    }

    /// ⌘Z: takes back the last AI capture if it is the newest undoable thing, else the last delete
    /// or quest completion.
    pub fn undo_latest(&mut self) {
        if self.capture_undo.is_some() {
            self.undo_capture();
        } else {
            self.step_undo(UndoEvent::Undo);
        }
    }

    /// What a deletable thing is called, for the toast.
    fn title_of(&self, item: Deletable) -> String {
        let snap = &self.snap;
        let title = match item {
            Deletable::Task(id) => self.plan_task(id).map(|t| t.title.clone()),
            Deletable::Quest(id) => snap.quest(id).map(|q| q.title.clone()),
            Deletable::ChecklistItem(id) => snap.checklist_item(id).map(|i| i.text.clone()),
            Deletable::Entry(id) => snap
                .time_entries
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.task_id.and_then(|t| snap.task(t)).map_or("Session".to_owned(), |t| t.title.clone())),
        };
        title.unwrap_or_default()
    }

    pub fn toggle_done(&mut self, task: EntityId) {
        let Some(done) = self.plan_task(task).map(|task| task.is_done()) else { return };
        let at = self.clock.now();
        self.dispatch(if done {
            Command::ReopenTask { id: task }
        } else if self.view == View::Plan {
            Command::CompleteTaskFromPlan { id: task, at, today: clock::local_day(at) }
        } else {
            Command::CompleteTask { id: task, at }
        });
    }

    /// Takes in what the store reported since the last frame; true when anything visible changed.
    pub fn refresh(&mut self, frame_time: f64, tx: &mut Tx) -> bool {
        let mut changed = drag::auto_scroll(self, frame_time, tx);
        let snapshot_changed = !Arc::ptr_eq(&self.store.snapshot(), &self.snap);
        if snapshot_changed {
            self.adopt_snapshot();
            self.sync_focus_draft();
            changed = true;
        }
        let sync = self.store.status();
        if sync != self.sync {
            self.sync = sync;
            changed = true;
        }
        let account = self.store.account();
        if account != self.account {
            self.account = account;
            changed = true;
        }
        let rejected = self.store.rejected_count();
        if rejected != self.rejected {
            self.rejected = rejected;
            changed = true;
        }
        let previous = self.now;
        self.now = self.clock.now();
        if snapshot_changed || self.recurrence_minute != Some(self.now.timestamp().div_euclid(60)) {
            changed |= self.generate_due_tasks();
        }
        changed |= self.refresh_task_fields(previous, tx);
        changed |= self.refresh_note(tx);
        changed |= self.take_auth_events();
        changed |= self.take_capture_answer();
        changed |= self.refresh_ai_connections();
        if let Some(until) = self.capture_undo.as_ref().and_then(|u| u.until) {
            if self.now >= until {
                if let Some(undo) = &mut self.capture_undo {
                    undo.until = None;
                }
                changed = true;
            } else {
                tx.wake_after(clock::wake_for(self.now, until));
            }
        }
        if let Some(until) = self.undo.expires() {
            if self.now >= until {
                self.step_undo(UndoEvent::Tick);
                changed = true;
            } else {
                tx.wake_after(clock::wake_for(self.now, until));
            }
        }
        // Running timers tick each second; times and dates shown move each minute. Wake when
        // the shown second or minute turns, not a fixed time after this poll.
        let ticking = self.snap.running_focus().is_some();
        let step = if ticking { 1 } else { 60 };
        if self.now.timestamp().div_euclid(step) != previous.timestamp().div_euclid(step) {
            changed = true;
        }
        tx.wake_after(clock::wake_for(self.now, clock::next_whole(self.now, step)));
        changed
    }
}
