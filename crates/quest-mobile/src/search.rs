//! The search, pure: the desktop palette's entries (`quest_view::palette`, same matching and
//! order) as records, and the commands an entry saves. Keyboard hints become words on touch.

use chrono::{DateTime, NaiveDate, Utc};
use quest_domain::{Command, EntityId, Position, Snapshot};
use quest_view::nav::View;
use quest_view::palette::{self, Action, Entry, Mode};
use quest_view::undo::Deletable;

use crate::error::MobileError;
use crate::ids::{iso_day, parse_id, parse_iso_day};
use crate::search_types::{AppView, SearchKind, SearchMode, SearchResult, SearchToken};
use crate::time::Now;

/// As many entries as the desktop palette lists.
const MAX_RESULTS: usize = 12;

pub(crate) fn search(
    snapshot: &Snapshot,
    mode: &SearchMode,
    query: &str,
    now: Now,
) -> Result<Vec<SearchResult>, MobileError> {
    let entries = palette::entries(palette_mode(mode)?, query, snapshot, now.local(now.at), now.at);
    Ok(entries.into_iter().filter_map(result).take(MAX_RESULTS).collect())
}

pub(crate) fn placeholder(mode: &SearchMode) -> String {
    // The text does not depend on the task, so any id will do.
    let mode = match mode {
        SearchMode::Commands => Mode::Commands,
        SearchMode::MoveTo { .. } => Mode::MoveTo(EntityId::SETTINGS),
        SearchMode::SwitchWorkspace => Mode::SwitchWorkspace,
    };
    palette::placeholder(mode).to_owned()
}

fn palette_mode(mode: &SearchMode) -> Result<Mode, MobileError> {
    Ok(match mode {
        SearchMode::Commands => Mode::Commands,
        SearchMode::MoveTo { task_id } => Mode::MoveTo(parse_id(task_id)?),
        SearchMode::SwitchWorkspace => Mode::SwitchWorkspace,
    })
}

/// An entry as a record; entries of modes the search does not offer are left out.
fn result(entry: Entry) -> Option<SearchResult> {
    let (kind, token) = match entry.action {
        Action::Go(view) => (SearchKind::Jump, SearchToken::Go { view: app_view(view)? }),
        Action::CreateTask(title) => (SearchKind::Create, SearchToken::CreateTask { title }),
        Action::CreateQuest(title) => (SearchKind::Create, SearchToken::CreateQuest { title }),
        Action::Focus(task) => (SearchKind::Task, SearchToken::Focus { task_id: task.to_string() }),
        Action::ShowQuest(quest) => (SearchKind::Quest, SearchToken::ShowQuest { quest_id: quest.to_string() }),
        Action::ToggleTheme => (SearchKind::Action, SearchToken::ToggleTheme),
        Action::Account => (SearchKind::Action, SearchToken::Account),
        Action::LogWork => (SearchKind::Action, SearchToken::LogWork),
        Action::Choose(Mode::MoveTo(task)) => {
            (SearchKind::Action, SearchToken::Choose { mode: SearchMode::MoveTo { task_id: task.to_string() } })
        }
        Action::MoveTask { task, planned_on } => {
            (SearchKind::Choice, SearchToken::MoveTask { task_id: task.to_string(), day: planned_on.map(iso_day) })
        }
        Action::Choose(Mode::SwitchWorkspace) => {
            (SearchKind::Action, SearchToken::Choose { mode: SearchMode::SwitchWorkspace })
        }
        Action::SwitchWorkspace(workspace) => {
            (SearchKind::Jump, SearchToken::SwitchWorkspace { workspace_id: workspace.to_string() })
        }
        Action::ManageWorkspaces => (SearchKind::Action, SearchToken::ManageWorkspaces),
        Action::Delete(Deletable::Task(task)) => {
            (SearchKind::Action, SearchToken::DeleteTask { task_id: task.to_string() })
        }
        _ => return None,
    };
    Some(SearchResult { title: entry.title, hint: touch_hint(&entry.hint), kind, token })
}

/// The desktop's keyboard hints in words; other hints stay.
fn touch_hint(hint: &str) -> String {
    match hint {
        "⌘," => "Account",
        "⌥← ⌥→" => "Move",
        "⌫" => "Delete",
        h if h.starts_with('⌘') => "Jump",
        h => h,
    }
    .to_owned()
}

fn app_view(view: View) -> Option<AppView> {
    Some(match view {
        View::Home | View::Today => AppView::Home,
        View::Plan => AppView::Plan,
        View::Focus => AppView::Focus,
        View::Quests => AppView::Quests,
        View::Review => AppView::Review,
        View::Archive => AppView::Archive,
        View::Quest(_) | View::Inbox | View::Logbook => return None,
    })
}

/// Desktop palette "Create task": today, no quest, at the top.
pub(crate) fn create_task(id: EntityId, title: &str, today: NaiveDate) -> Result<Command, MobileError> {
    let title = crate::quests::new_title(title)?;
    let planned_on = Some(today);
    Ok(Command::CreateTask { id, title, quest_id: None, planned_on, due_on: None, position: Position::Top })
}

/// "Move to…": the plan date (None: unplanned), at the bottom.
pub(crate) fn move_task(id: EntityId, day: Option<&str>, at: DateTime<Utc>) -> Result<Command, MobileError> {
    let planned_on = day.map(parse_iso_day).transpose()?;
    Ok(Command::MoveTask { id, planned_on, position: Position::Bottom, at })
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
