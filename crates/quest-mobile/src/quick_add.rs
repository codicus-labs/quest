//! Quick add (share target, app shortcut, quick settings tile): a new task with a quest and a
//! day in one step. Pure: the core passes the snapshot and the time in.

use quest_domain::{Command, EntityId, Position, Snapshot};
use quest_view::home::{new_task_quest, one_line};

use crate::error::MobileError;
use crate::home::{chip, quest_options};
use crate::ids::{parse_id, parse_iso_day};
use crate::task_detail::day_value;
use crate::task_types::QuickAddForm;
use crate::time::Now;

/// The sheet, prefilled from `shared_text`: its first non-empty line is the title, the rest
/// (e.g. a link under a page title) the description. The quest is the one new quest work gets
/// on Home without a filter or pick; the day is today.
pub(crate) fn form(snapshot: &Snapshot, shared_text: Option<&str>, now: Now) -> QuickAddForm {
    let (title, description) = split_shared(shared_text.unwrap_or_default());
    let quest = new_task_quest(snapshot, None, None).and_then(|id| snapshot.quest(id)).map(chip);
    QuickAddForm {
        title,
        description,
        quest,
        quest_options: quest_options(snapshot, now.at),
        today: day_value(now.today()),
    }
}

/// The commands that create the task `id`: at the bottom of `day` (None: unplanned), with
/// `quest_id` (None: no quest) and, if not blank, `description`.
pub(crate) fn commands(
    snapshot: &Snapshot,
    id: EntityId,
    title: &str,
    description: &str,
    quest_id: Option<&str>,
    day: Option<&str>,
) -> Result<Vec<Command>, MobileError> {
    let title = one_line(title).trim().to_owned();
    if title.is_empty() {
        return Err(MobileError::EmptyTitle);
    }
    let quest_id = quest_id.map(|raw| quest(snapshot, raw)).transpose()?;
    let planned_on = day.map(parse_iso_day).transpose()?;
    let create = Command::CreateTask { id, title, quest_id, planned_on, due_on: None, position: Position::Bottom };
    let mut commands = vec![create];
    let description = description.trim();
    if !description.is_empty() {
        commands.push(Command::SetTaskDescription { id, description: description.to_owned() });
    }
    Ok(commands)
}

/// A quest that still exists (deleted elsewhere while the sheet was open: `UnknownQuest`).
fn quest(snapshot: &Snapshot, raw: &str) -> Result<EntityId, MobileError> {
    let id = parse_id(raw)?;
    snapshot.quest(id).map(|q| q.id).ok_or_else(|| MobileError::UnknownQuest { id: raw.to_owned() })
}

/// How much shared text the sheet takes (in chars); a whole page pasted into the share sheet is
/// cut here, so the form stays small enough for Android's saved state.
const SHARED_TEXT_LIMIT: usize = 10_000;

fn split_shared(text: &str) -> (String, String) {
    let text = match text.char_indices().nth(SHARED_TEXT_LIMIT) {
        Some((end, _)) => &text[..end],
        None => text,
    };
    let text = text.trim();
    let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
    (one_line(first).trim().to_owned(), rest.trim().to_owned())
}

#[cfg(test)]
#[path = "quick_add_tests.rs"]
mod tests;
