//! The search, pure: the desktop palette's entries (`quest_view::palette`, same matching and
//! order) as records, and the commands an entry saves. Keyboard hints become words on touch.

use chrono::{NaiveDate, NaiveTime};
use quest_domain::{Command, EntityId, Position, Snapshot, TimeBlock};
use quest_view::nav::View;
use quest_view::palette::{self, Action, Entry, Mode};
use quest_view::timescale::DEFAULT_BLOCK_MINUTES;
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
        SearchMode::ScheduleAt { .. } => Mode::ScheduleAt(EntityId::SETTINGS),
        SearchMode::SwitchWorkspace => Mode::SwitchWorkspace,
    };
    palette::placeholder(mode).to_owned()
}

fn palette_mode(mode: &SearchMode) -> Result<Mode, MobileError> {
    Ok(match mode {
        SearchMode::Commands => Mode::Commands,
        SearchMode::MoveTo { task_id } => Mode::MoveTo(parse_id(task_id)?),
        SearchMode::ScheduleAt { task_id } => Mode::ScheduleAt(parse_id(task_id)?),
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
        Action::Choose(Mode::ScheduleAt(task)) => {
            (SearchKind::Action, SearchToken::Choose { mode: SearchMode::ScheduleAt { task_id: task.to_string() } })
        }
        Action::MoveTask { task, day } => {
            (SearchKind::Choice, SearchToken::MoveTask { task_id: task.to_string(), day: day.map(iso_day) })
        }
        Action::Schedule { task, day, start } => (
            SearchKind::Choice,
            SearchToken::Schedule {
                task_id: task.to_string(),
                day: iso_day(day),
                start_minute: u32::try_from(start).ok()?,
            },
        ),
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
        View::Home => AppView::Home,
        View::Plan => AppView::Plan,
        View::Calendar => AppView::Calendar,
        View::Focus => AppView::Focus,
        View::Quests => AppView::Quests,
        View::Review => AppView::Review,
        View::Archive => AppView::Archive,
        View::Quest(_) => return None,
    })
}

/// Desktop palette "Create task": today, no quest, at the top.
pub(crate) fn create_task(id: EntityId, title: &str, today: NaiveDate) -> Result<Command, MobileError> {
    let title = crate::quests::new_title(title)?;
    Ok(Command::CreateTask { id, title, quest_id: None, day: Some(today), position: Position::Top })
}

/// "Move to…": the day (None: the backlog), at the bottom.
pub(crate) fn move_task(id: EntityId, day: Option<&str>) -> Result<Command, MobileError> {
    let day = day.map(parse_iso_day).transpose()?;
    Ok(Command::MoveTask { id, day, position: Position::Bottom })
}

/// "Schedule at…": a block on `day` at `start_minute`, keeping the length of one it has.
pub(crate) fn schedule(
    snapshot: &Snapshot,
    id: EntityId,
    day: &str,
    start_minute: u32,
) -> Result<Command, MobileError> {
    let day = parse_iso_day(day)?;
    if start_minute >= 24 * 60 {
        return Err(MobileError::InvalidForm { reason: "Pick a time within the day.".into() });
    }
    let start = NaiveTime::MIN + chrono::Duration::minutes(i64::from(start_minute));
    let minutes = snapshot.task(id).and_then(|t| t.block).map_or(DEFAULT_BLOCK_MINUTES as u32, |b| b.minutes);
    Ok(Command::ScheduleTask { id, day, block: TimeBlock { start, minutes } })
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
