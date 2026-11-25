//! The Log work sheet and the entry editor, pure: their first drafts, their live checks and
//! the commands saving sends. Validation and messages are the desktop's
//! (`quest_view::log_work::decide`, `quest_view::entry_editor::times`). Mobile differs in one
//! rule: the entry editor has no quest field, the session counts towards its task's quest.

use chrono::{Duration, NaiveDate, Timelike};
use quest_domain::{Command, EntityId, EntryKind, Snapshot, Task, TimeEntry, WorkTask};
use quest_view::entry_editor::{self, EndFields};
use quest_view::log_work::{self, FieldError};
use quest_view::{format, undo::Deletable};

use crate::error::MobileError;
use crate::home::{chip, quest_options};
use crate::ids::{iso_day, parse_id, parse_iso_day};
use crate::task_types::DayValue;
use crate::time::Now;
use crate::work_types::{
    EntryCheck, EntryDraft, FormError, FormField, LogWorkCheck, LogWorkDraft, SessionKind, TaskChoices, TaskOption,
};

/// Minutes in a day: the form's times are clock times below it.
const DAY_MINUTES: u32 = 24 * 60;

/// Shown for a duration while a field is wrong.
const NO_DURATION: &str = "–";

// ---------- Task choices ----------

/// The desktop's task combobox: without a query every task, open ones first, then done ones
/// (latest done first); with one the suggestions, and "Create “…”" when none matched.
pub(crate) fn task_choices(snapshot: &Snapshot, query: &str) -> TaskChoices {
    let query = query.trim();
    let tasks: Vec<&Task> = if query.is_empty() {
        let mut all: Vec<&Task> = snapshot.tasks.iter().collect();
        all.sort_by_key(|t| (t.is_done(), std::cmp::Reverse(t.completed_at)));
        all
    } else {
        log_work::suggestions(snapshot, query)
    };
    let create_label = (!query.is_empty() && tasks.is_empty()).then(|| format!("Create “{query}”"));
    TaskChoices { options: tasks.into_iter().map(|t| task_option(snapshot, t)).collect(), create_label }
}

fn task_option(snapshot: &Snapshot, task: &Task) -> TaskOption {
    TaskOption {
        id: task.id.to_string(),
        title: task.title.clone(),
        done: task.is_done(),
        status: if task.is_done() { "Done" } else { "Open" }.to_owned(),
        quest: task.quest_id.and_then(|id| snapshot.quest(id)).map(chip),
    }
}

// ---------- Log work ----------

/// Log work from the header or the button: the quarter hour that just ended, today (desktop
/// `open_log_work_now`; shortly after midnight it starts at 00:00).
pub(crate) fn log_work_now(now: Now) -> LogWorkDraft {
    let local = now.local(now.at);
    let end = local.hour() * 60 + local.minute();
    LogWorkDraft {
        task_id: None,
        new_title: String::new(),
        quest_id: None,
        day: iso_day(local.date()),
        start_minute: end.saturating_sub(15),
        end_minute: end,
    }
}

pub(crate) fn check_log_work(snapshot: &Snapshot, draft: &LogWorkDraft, now: Now) -> Result<LogWorkCheck, MobileError> {
    let picked = picked_task(snapshot, draft.task_id.as_deref())?;
    let chosen = chosen_quest(snapshot, draft.quest_id.as_deref())?;
    let quest = log_work::quest(snapshot, picked.map(|t| t.id), chosen).and_then(|q| snapshot.quest(q));
    let error = decide_log_work(snapshot, draft, EntityId::new(), now).err();
    let new_title = draft.new_title.trim();
    let is_new_task = picked.is_none() && !new_title.is_empty();
    Ok(LogWorkCheck {
        date: parse_iso_day(&draft.day).ok().map(day_value),
        start_label: clock_label(draft.start_minute),
        end_label: clock_label(draft.end_minute),
        duration: span_duration(draft.start_minute, draft.end_minute),
        task_title: match picked {
            Some(task) => Some(task.title.clone()),
            None => (!new_title.is_empty()).then(|| new_title.to_owned()),
        },
        is_new_task,
        // Without a task nothing counts yet; a picked task's quest is its own.
        quest: quest.filter(|_| picked.is_some() || is_new_task).map(chip),
        // A session's quest follows its task: only a new task's quest is chosen here.
        can_choose_quest: is_new_task,
        quest_options: quest_options(snapshot, now.at),
        error,
    })
}

/// The `LogWork` command the draft saves, or the field to correct.
pub(crate) fn decide_log_work(
    snapshot: &Snapshot,
    draft: &LogWorkDraft,
    entry_id: EntityId,
    now: Now,
) -> Result<Command, FormError> {
    let picked = picked_task(snapshot, draft.task_id.as_deref()).map_err(|_| form_error(FieldError::Task))?;
    if draft.task_id.is_some() && picked.is_none() {
        return Err(form_error(FieldError::Task));
    }
    let chosen = chosen_quest(snapshot, draft.quest_id.as_deref()).map_err(|_| form_error(FieldError::Task))?;
    let (start, end) = (clock_text(draft.start_minute), clock_text(draft.end_minute));
    // The desktop's date parser reads ISO days too.
    let desktop = log_work::Draft {
        task: picked.map(|t| t.id),
        title: &draft.new_title,
        date: &draft.day,
        start: &start,
        end: &end,
        quest: chosen,
    };
    log_work::decide(&desktop, now.today(), |local| now.instant(local), entry_id, EntityId::new()).map_err(form_error)
}

// ---------- Entry editor ----------

/// The editor as it opens on `entry`: its task and local times; over midnight it ends on
/// another day.
pub(crate) fn entry_draft(snapshot: &Snapshot, entry_id: &str, now: Now) -> Result<EntryDraft, MobileError> {
    let entry = entry(snapshot, entry_id)?;
    let start = now.local(entry.started_at);
    let end = now.local(entry.ended_at.unwrap_or(entry.started_at));
    Ok(EntryDraft {
        entry_id: entry.id.to_string(),
        task_id: entry.task_id.map(|id| id.to_string()),
        start_day: iso_day(start.date()),
        start_minute: start.hour() * 60 + start.minute(),
        end_day: iso_day(end.date()),
        end_minute: end.hour() * 60 + end.minute(),
        ends_other_day: start.date() != end.date(),
    })
}

pub(crate) fn check_entry(snapshot: &Snapshot, draft: &EntryDraft, now: Now) -> Result<EntryCheck, MobileError> {
    let entry = entry(snapshot, &draft.entry_id)?;
    let task = picked_task(snapshot, draft.task_id.as_deref())?;
    let times = entry_times(entry, draft, now);
    let quest = task.and_then(|t| t.quest_id).and_then(|q| snapshot.quest(q));
    Ok(EntryCheck {
        kind: session_kind(entry.kind),
        running: entry.is_running(),
        task_title: task.map(|t| t.title.clone()),
        quest: quest.map(chip),
        can_open_task: task.is_some(),
        start_date: parse_iso_day(&draft.start_day).ok().map(day_value),
        start_label: clock_label(draft.start_minute),
        end_date: parse_iso_day(end_day(draft)).ok().map(day_value),
        end_label: clock_label(draft.end_minute),
        duration: match &times {
            Ok(t) => entry_editor::duration_label((t.ended_at.unwrap_or(now.at) - t.started_at).num_milliseconds()),
            Err(_) => NO_DURATION.to_owned(),
        },
        error: times.err().map(entry_form_error),
    })
}

/// The `EditEntry` the draft saves: its times, its task and that task's quest (none without a
/// task; an organizing session given a task becomes task work); or the field to correct.
pub(crate) fn decide_entry(snapshot: &Snapshot, draft: &EntryDraft, now: Now) -> Result<Command, MobileError> {
    let entry = entry(snapshot, &draft.entry_id)?;
    let task = picked_task(snapshot, draft.task_id.as_deref())?.map(|t| t.id);
    if draft.task_id.is_some() && task.is_none() {
        return Err(MobileError::UnknownTask { id: draft.task_id.clone().unwrap_or_default() });
    }
    let times =
        entry_times(entry, draft, now).map_err(|e| MobileError::InvalidForm { reason: entry_form_error(e).message })?;
    Ok(Command::EditEntry {
        id: entry.id,
        task: task.map(WorkTask::Existing),
        quest_id: log_work::quest(snapshot, task, None),
        started_at: times.started_at,
        ended_at: times.ended_at,
    })
}

/// "Deleted “…”" names the session's task, else "Organizing" or "Session" (desktop `title_of`).
pub(crate) fn delete_entry(snapshot: &Snapshot, entry_id: &str) -> Result<(Deletable, String), MobileError> {
    let entry = entry(snapshot, entry_id)?;
    let untitled = if entry.kind == EntryKind::Organizing { "Organizing" } else { "Session" };
    let title = entry.task_id.and_then(|t| snapshot.task(t)).map_or_else(|| untitled.to_owned(), |t| t.title.clone());
    Ok((Deletable::Entry(entry.id), title))
}

fn entry_times(
    entry: &TimeEntry,
    draft: &EntryDraft,
    now: Now,
) -> Result<entry_editor::Times, entry_editor::FieldError> {
    let (start, end) = (clock_text(draft.start_minute), clock_text(draft.end_minute));
    let desktop = entry_editor::Draft {
        start_date: &draft.start_day,
        start: &start,
        end: (!entry.is_running()).then_some(EndFields { date: end_day(draft), time: &end }),
    };
    entry_editor::times(&desktop, entry, now.today(), |local| now.instant(local))
}

fn end_day(draft: &EntryDraft) -> &str {
    if draft.ends_other_day { &draft.end_day } else { &draft.start_day }
}

fn entry<'a>(snapshot: &'a Snapshot, entry_id: &str) -> Result<&'a TimeEntry, MobileError> {
    let id = parse_id(entry_id)?;
    snapshot
        .time_entries
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| MobileError::UnknownEntry { id: entry_id.to_owned() })
}

// ---------- Shared ----------

/// The picked task, if it still exists; None when nothing is picked or it is gone.
fn picked_task<'a>(snapshot: &'a Snapshot, task_id: Option<&str>) -> Result<Option<&'a Task>, MobileError> {
    Ok(task_id.map(parse_id).transpose()?.and_then(|id| snapshot.task(id)))
}

/// The chosen quest, if it exists.
fn chosen_quest(snapshot: &Snapshot, quest_id: Option<&str>) -> Result<Option<EntityId>, MobileError> {
    Ok(quest_id.map(parse_id).transpose()?.filter(|id| snapshot.quest(*id).is_some()))
}

/// "14:50"; empty (so it fails as the desktop's field would) past the day's last minute.
fn clock_text(minute: u32) -> String {
    if minute < DAY_MINUTES { format::hh_mm(minute) } else { String::new() }
}

fn clock_label(minute: u32) -> String {
    format::hh_mm(minute.min(DAY_MINUTES - 1))
}

pub(crate) fn session_kind(kind: EntryKind) -> SessionKind {
    match kind {
        EntryKind::Task => SessionKind::Task,
        EntryKind::Organizing => SessionKind::Organizing,
    }
}

fn span_duration(start: u32, end: u32) -> String {
    if end > start {
        entry_editor::duration_label(Duration::minutes(i64::from(end - start)).num_milliseconds())
    } else {
        NO_DURATION.to_owned()
    }
}

fn day_value(day: NaiveDate) -> DayValue {
    DayValue { iso: iso_day(day), label: log_work::date_label(day) }
}

fn form_error(error: FieldError) -> FormError {
    let field = match error {
        FieldError::Task => FormField::Task,
        FieldError::Date => FormField::Date,
        FieldError::Start => FormField::Start,
        FieldError::End | FieldError::EndNotAfterStart => FormField::End,
    };
    FormError { field, message: error.message().to_owned() }
}

fn entry_form_error(error: entry_editor::FieldError) -> FormError {
    use entry_editor::FieldError as E;
    let field = match error {
        E::StartDate => FormField::Date,
        E::EndDate => FormField::EndDate,
        E::Start => FormField::Start,
        E::End | E::EndNotAfterStart => FormField::End,
    };
    FormError { field, message: error.message().to_owned() }
}

#[cfg(test)]
#[path = "work_forms_tests.rs"]
mod tests;
