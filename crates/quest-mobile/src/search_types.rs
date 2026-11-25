//! Records of the search (the desktop's ⌘K palette): entries, what picking one does, and
//! what the UI does next.

/// A screen the search can jump to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AppView {
    Home,
    Plan,
    Calendar,
    Focus,
    Quests,
    Review,
    Archive,
}

/// Which list the search shows.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SearchMode {
    Commands,
    /// A day (or the backlog) for a task.
    MoveTo {
        task_id: String,
    },
    /// A time today for a task's block.
    ScheduleAt {
        task_id: String,
    },
    /// The other workspaces, then "Manage workspaces…".
    SwitchWorkspace,
}

/// What an entry is, for its icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SearchKind {
    Jump,
    Action,
    Task,
    Quest,
    Create,
    Choice,
}

/// What picking an entry does (the desktop's `palette::Action`).
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SearchToken {
    Go { view: AppView },
    CreateTask { title: String },
    CreateQuest { title: String },
    Focus { task_id: String },
    ShowQuest { quest_id: String },
    ToggleTheme,
    Account,
    Choose { mode: SearchMode },
    MoveTask { task_id: String, day: Option<String> },
    Schedule { task_id: String, day: String, start_minute: u32 },
    DeleteTask { task_id: String },
    LogWork,
    SwitchWorkspace { workspace_id: String },
    ManageWorkspaces,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct SearchResult {
    pub title: String,
    /// A short tag on the right: "Jump", "Task", "Quest", "Today", …
    pub hint: String,
    pub kind: SearchKind,
    /// Pass it to `run_search` as it is.
    pub token: SearchToken,
}

/// What the UI does after `run_search` (the core has already saved what there was to save).
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SearchOutcome {
    /// Show this screen (Focus: open it). A new task goes to Home.
    Show {
        view: AppView,
    },
    /// Open the quest's page (also a quest just created).
    ShowQuest {
        quest_id: String,
    },
    /// Open Focus on this task; its timer is running.
    Focus {
        task_id: String,
    },
    /// Clear the field and list this mode's entries.
    Choose {
        mode: SearchMode,
        placeholder: String,
    },
    /// Switch between light and dark.
    ToggleTheme,
    Account,
    /// Open the calendar's Log work (with `log_work_now`).
    LogWork,
    /// Open "Manage workspaces".
    ManageWorkspaces,
    /// Close the search: a move, schedule, delete or workspace switch is done (a delete shows its undo toast).
    Done,
}
