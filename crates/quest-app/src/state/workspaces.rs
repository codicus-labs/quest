//! Workspaces in the app: switching, "Manage workspaces" (create, rename, delete with a
//! confirmation, restore) and what a switch resets.
//!
//! A switch (by the user, or because another device deleted the shown workspace) keeps the view
//! (a quest's page goes back to Quests) and the account-level overlays (palette, account,
//! shortcuts, Manage workspaces). It closes everything that holds the old workspace's data:
//! the task, quest, entry, repeat, Log work and capture dialogs, menus and pickers. It drops
//! the undo stack, the capture's undo, the quest filter and picks, the selection and the Focus
//! task. Text typed into "Add …" fields stays: it is not data yet, and Return adds it to the
//! workspace then shown. The focus timer is global and keeps running (see the top bar).

use std::collections::HashMap;

use quest_domain::{Command, EntityId};
use quest_view::workspaces::check_name;
use rune::TextEdit;

use super::{CaptureStep, LogWorkForm, Overlay, State, View};
use crate::dnd::DragState;

#[derive(Default)]
pub struct WorkspacesForm {
    /// "New workspace" in Manage workspaces.
    pub new_name: TextEdit,
    /// The name fields of the live workspaces, filled when the dialog opens.
    pub names: HashMap<EntityId, TextEdit>,
    /// The workspace whose delete waits for "Delete workspace".
    pub confirm_delete: Option<EntityId>,
    /// Why the last create, rename or delete did not happen.
    pub error: Option<String>,
    /// Takes edits for a name field whose workspace is gone.
    scratch: TextEdit,
}

impl WorkspacesForm {
    pub fn name_field(&mut self, id: EntityId) -> &mut TextEdit {
        self.names.get_mut(&id).unwrap_or(&mut self.scratch)
    }
}

impl State {
    pub fn toggle_workspace_menu(&mut self) {
        self.workspace_menu = !self.workspace_menu;
    }

    /// Shows `workspace`; nothing happens if it is the shown one.
    pub fn switch_workspace(&mut self, workspace: EntityId) {
        self.workspace_menu = false;
        if workspace == self.snap.workspace {
            return;
        }
        if !self.snap.live_workspaces().any(|w| w.id == workspace) {
            return;
        }
        // Open dialogs save into the workspace they belong to before it changes.
        self.close_workspace_views();
        if let Err(error) = self.store.switch_workspace(workspace) {
            self.notice = Some(format!("That didn’t work: {error}."));
            return;
        }
        self.snap = self.store.snapshot();
        self.left_workspace();
    }

    /// Takes the store's latest snapshot. If it shows another workspace (another device deleted
    /// the shown one), resets what belonged to the old one.
    pub(crate) fn adopt_snapshot(&mut self) {
        let workspace = self.snap.workspace;
        self.snap = self.store.snapshot();
        if self.snap.workspace != workspace {
            self.left_workspace();
        }
    }

    /// Resets what belongs to the workspace shown before (see the module docs).
    pub(super) fn left_workspace(&mut self) {
        self.close_workspace_views();
        self.undo = crate::undo::UndoState::default();
        self.capture_undo = None;
        self.capture.step = CaptureStep::Compose { error: None };
        self.focus_task = None;
        self.focus_exit.open();
        self.focus_return = View::Today;
        self.reset_quick_add();
        self.lingering_tasks.clear();
        self.date_picker = None;
        self.quest_picker = None;
        self.drag = DragState::Idle;
        self.landing = None;
        self.log_work = LogWorkForm::default();
        self.notice = None;
        self.sync_focus_draft();
        self.fill_workspace_names();
    }

    /// Closes the dialogs and pages that show one workspace's data.
    fn close_workspace_views(&mut self) {
        let view = match self.view {
            View::Quest(_) => View::Quests,
            view => view,
        };
        let overlay = match self.overlay {
            keep @ (Overlay::Palette
            | Overlay::Account
            | Overlay::Shortcuts
            | Overlay::GuestImport
            | Overlay::AiNotice
            | Overlay::Workspaces) => keep,
            _ => Overlay::None,
        };
        self.go(view);
        self.overlay = overlay;
    }

    pub fn open_workspaces(&mut self) {
        self.workspace_menu = false;
        self.workspaces.new_name = TextEdit::new("");
        self.workspaces.confirm_delete = None;
        self.workspaces.error = None;
        self.fill_workspace_names();
        self.overlay = Overlay::Workspaces;
    }

    pub fn close_workspaces(&mut self) {
        self.workspaces.confirm_delete = None;
        self.overlay = Overlay::None;
    }

    /// One name field per live workspace, keeping what is being typed into the others.
    fn fill_workspace_names(&mut self) {
        let live: Vec<(EntityId, String)> = self.snap.live_workspaces().map(|w| (w.id, w.name.clone())).collect();
        let names = &mut self.workspaces.names;
        names.retain(|id, _| live.iter().any(|(live, _)| live == id));
        for (id, name) in live {
            names.entry(id).or_insert_with(|| TextEdit::new(&name));
        }
    }

    /// Creates the workspace named in "New workspace" and shows it.
    pub fn create_workspace(&mut self) {
        let name = match check_name(&self.snap, &self.workspaces.new_name.text, None) {
            Ok(name) => name,
            Err(error) => return self.workspaces.error = Some(error.message().to_owned()),
        };
        let id = EntityId::new();
        self.dispatch(Command::CreateWorkspace { id, name });
        if self.notice.is_some() {
            return;
        }
        self.close_workspaces();
        self.switch_workspace(id);
    }

    /// Saves the name typed into `id`'s field.
    pub fn rename_workspace(&mut self, id: EntityId) {
        let typed = self.workspaces.name_field(id).text.clone();
        let Some(current) = self.snap.live_workspace(id).map(|w| w.name.clone()) else { return };
        let name = match check_name(&self.snap, &typed, Some(id)) {
            Ok(name) => name,
            Err(error) => return self.workspaces.error = Some(error.message().to_owned()),
        };
        self.workspaces.error = None;
        *self.workspaces.name_field(id) = TextEdit::new(&name);
        if name != current {
            self.dispatch(Command::RenameWorkspace { id, name });
        }
    }

    /// Asks to confirm deleting `id`, or says why it can't be deleted.
    pub fn ask_delete_workspace(&mut self, id: EntityId) {
        match self.snap.check_deletable(id) {
            Ok(()) => {
                self.workspaces.error = None;
                self.workspaces.confirm_delete = Some(id);
            }
            Err(error) => self.workspaces.error = Some(format!("That didn’t work: {error}.")),
        }
    }

    /// Deletes the workspace waiting for confirmation. If it is the shown one, the store shows a
    /// remaining one (the default workspace while it is live).
    pub fn delete_workspace(&mut self) {
        let Some(id) = self.workspaces.confirm_delete.take() else { return };
        self.dispatch(Command::DeleteWorkspace { id, at: self.clock.now() });
        self.fill_workspace_names();
    }

    /// Restores `id` unless a live workspace took its name meanwhile (names stay unique).
    pub fn restore_workspace(&mut self, id: EntityId) {
        let Some(name) = self.snap.workspace(id).map(|w| w.name.clone()) else { return };
        if let Err(error) = check_name(&self.snap, &name, None) {
            self.workspaces.error = Some(format!("Rename the other “{name}” first. {}", error.message()));
            return;
        }
        self.workspaces.error = None;
        self.dispatch(Command::RestoreWorkspace { id });
        self.fill_workspace_names();
    }

    /// Shows the workspace the running timer is in, on Focus.
    pub fn go_to_running_timer(&mut self) {
        let Some(running) = self.snap.running_focus() else { return };
        self.switch_workspace(running.workspace);
        self.go(View::Focus);
    }
}
