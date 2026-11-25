//! The Plan, pure: which days to build around what shows (`window`), each day's cards as the
//! desktop's day columns list them (`plan`), and how an edit reaches a repeat the plan only
//! previews (`plan_edit`).

use chrono::{DateTime, Datelike, Days, NaiveDate, Utc};
use quest_domain::{Command, EntityId, Position, Snapshot, Task, week_start};
use quest_view::plan_strip::REACH_DAYS;
use quest_view::reorder::nudge;
use quest_view::{format, palette};

use crate::error::MobileError;
use crate::home::{Choice, chip, home_task, quest_options, section_of};
use crate::ids::{iso_day, parse_iso_day};
use crate::plan_types::{MoveTarget, PlanData, PlanDay, PlanWindow};
use crate::time::Now;
use crate::types::{TaskSection, UndoToast};

/// Days built beyond each visible edge.
const OVERSCAN_DAYS: u64 = 14;
/// A built window holds while at least this many built days stay beyond each visible edge.
const HOLD_DAYS: u64 = 3;

/// The days built: `count` days from `first`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DayWindow {
    pub first: NaiveDate,
    pub count: u32,
}

impl DayWindow {
    pub fn parse(window: &PlanWindow) -> Result<DayWindow, MobileError> {
        Ok(DayWindow { first: parse_iso_day(&window.first_day)?, count: window.count })
    }

    pub fn record(self) -> PlanWindow {
        PlanWindow { first_day: iso_day(self.first), count: self.count }
    }

    /// The day after the last one.
    fn end(self) -> NaiveDate {
        self.first + Days::new(u64::from(self.count))
    }

    pub fn days(self) -> impl Iterator<Item = NaiveDate> {
        (0..u64::from(self.count)).map(move |i| self.first + Days::new(i))
    }
}

/// What the plan last showed: the days built, the instant it was read at and its filter. Edits on a
/// previewed repeat find it there, as the desktop finds it in its built days.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PlanMemory {
    pub window: DayWindow,
    pub at: DateTime<Utc>,
    /// The quest filter it showed, so "Move up" / "Move down" step past the cards on screen.
    pub choice: Choice,
}

/// The days to build when `visible` days from `first_visible` show and `built` are built:
/// `built` while it still holds `HOLD_DAYS` either side of what shows, else whole weeks
/// reaching `OVERSCAN_DAYS` beyond each edge. Never more than `REACH_DAYS` from today.
pub(crate) fn window(first_visible: NaiveDate, visible: u32, built: Option<DayWindow>, today: NaiveDate) -> DayWindow {
    let earliest = today - Days::new(REACH_DAYS);
    let end = today + Days::new(REACH_DAYS + 1);
    let first = first_visible.clamp(earliest, end - Days::new(1));
    let visible = u64::from(visible).clamp(1, 2 * REACH_DAYS + 1);
    let shown_end = (first + Days::new(visible)).min(end);
    let wanted_first = (first - Days::new(HOLD_DAYS)).max(earliest);
    let wanted_end = (shown_end + Days::new(HOLD_DAYS)).min(end);
    if let Some(built) = built.filter(|b| b.first <= wanted_first && wanted_end <= b.end()) {
        return built;
    }
    let from = week_start(first - Days::new(OVERSCAN_DAYS)).max(earliest);
    let to = week_start(shown_end + Days::new(OVERSCAN_DAYS + 6)).min(end);
    let count = u32::try_from((to - from).num_days()).unwrap_or(0);
    DayWindow { first: from, count }
}

pub(crate) fn plan(
    snapshot: &Snapshot,
    window: DayWindow,
    now: Now,
    choice: Choice,
    toast: Option<UndoToast>,
) -> PlanData {
    let choice = choice.known_in(snapshot);
    let today = now.today();
    PlanData {
        window: window.record(),
        days: window.days().map(|day| plan_day(snapshot, day, today, now.at, choice)).collect(),
        today_iso: iso_day(today),
        quest_filter: choice.filter.and_then(|id| snapshot.quest(id)).map(chip),
        quests: quest_options(snapshot, now.at),
        toast,
    }
}

fn plan_day(snapshot: &Snapshot, day: NaiveDate, today: NaiveDate, at: DateTime<Utc>, choice: Choice) -> PlanDay {
    let cards = |section| {
        day_tasks(snapshot, day, at, choice, section).iter().map(|t| home_task(snapshot, t, at, today)).collect()
    };
    PlanDay {
        day_iso: iso_day(day),
        weekday: day.format("%a").to_string(),
        day_number: day.day().to_string(),
        label: format::long_day(day),
        month_label: day.format("%B %Y").to_string(),
        is_today: day == today,
        is_past: day < today,
        quest_work: cards(TaskSection::QuestWork),
        other: cards(TaskSection::Other),
    }
}

/// One section of a day as the desktop's day column lists it: its tasks and the repeats
/// planned for it, in list order, open ones first. QUEST WORK shows only the filtered quest.
pub(crate) fn day_tasks(
    snapshot: &Snapshot,
    day: NaiveDate,
    at: DateTime<Utc>,
    choice: Choice,
    section: TaskSection,
) -> Vec<Task> {
    let shown = |task: &Task| {
        section_of(snapshot, task) == section
            && (section == TaskSection::Other || choice.filter.is_none_or(|q| task.quest_id == Some(q)))
    };
    let mut tasks: Vec<Task> =
        snapshot.tasks_on(day).cloned().chain(snapshot.planned_occurrences(day, at)).filter(|t| shown(t)).collect();
    tasks.sort_by(|a, b| (&a.day_order, a.id).cmp(&(&b.day_order, b.id)));
    let (open, done): (Vec<Task>, Vec<Task>) = tasks.into_iter().partition(|t| !t.is_done());
    open.into_iter().chain(done).collect()
}

/// A repeat the plan shows on one of its days but that is not saved yet, by its future id.
pub(crate) fn preview(snapshot: &Snapshot, memory: PlanMemory, id: EntityId) -> Option<Task> {
    if snapshot.task(id).is_some() {
        return None;
    }
    memory.window.days().flat_map(|day| snapshot.planned_occurrences(day, memory.at)).find(|t| t.id == id)
}

/// `command` as the plan carries it out (desktop `State::plan_edit`): on a previewed repeat it
/// saves the repeat first, and a move to a day may sort among that day's previews.
pub(crate) fn plan_edit(snapshot: &Snapshot, memory: PlanMemory, command: Command) -> Command {
    let Some(id) = edited_task(&command) else { return command };
    let occurrence = preview(snapshot, memory, id).and_then(|task| task.series.zip(task.planned_on));
    if occurrence.is_some()
        || matches!(command, Command::MoveTask { planned_on: Some(_), .. } | Command::PlaceTask { .. })
    {
        return Command::EditPlan { occurrence, now: memory.at, edit: Box::new(command) };
    }
    command
}

/// The task an edit the plan may wrap is about (the desktop's list).
fn edited_task(command: &Command) -> Option<EntityId> {
    match command {
        Command::RenameTask { id, .. }
        | Command::SetTaskDescription { id, .. }
        | Command::AssignTask { id, .. }
        | Command::MoveTask { id, .. }
        | Command::PlaceTask { id, .. }
        | Command::SetTaskDue { id, .. }
        | Command::AddChecklistItem { task_id: id, .. }
        | Command::CompleteTaskFromPlan { id, .. }
        | Command::DeleteTask { id, .. }
        | Command::StartFocus { task_id: id, .. } => Some(*id),
        _ => None,
    }
}

/// A drop: `task` to `day` (None: unplanned), right before the card `before` or at the end.
/// Nothing when dropped before itself (back where it was). Lists show open cards before done
/// ones whatever their order, so an open card dropped before a done one goes to the end, which
/// is right after the last open card, where it showed.
pub(crate) fn drop_move(
    task: &Task,
    day: Option<NaiveDate>,
    before: Option<&Task>,
    at: DateTime<Utc>,
) -> Option<Command> {
    if before.is_some_and(|b| b.id == task.id) {
        return None;
    }
    let anchor = before.filter(|b| task.is_done() || !b.is_done());
    let position = anchor.map_or(Position::Bottom, |b| Position::Before(b.id));
    Some(Command::MoveTask { id: task.id, planned_on: day, position, at })
}

/// "Move up" / "Move down": one place within the task's day and section, as the plan lists it
/// at `at` with `choice`'s filter.
/// None at either end, or for an unplanned task.
pub(crate) fn nudge_move(
    snapshot: &Snapshot,
    at: DateTime<Utc>,
    choice: Choice,
    task: &Task,
    up: bool,
    now: DateTime<Utc>,
) -> Option<Command> {
    let day = task.planned_on?;
    let section = section_of(snapshot, task);
    let order: Vec<EntityId> =
        day_tasks(snapshot, day, at, choice.known_in(snapshot), section).iter().map(|t| t.id).collect();
    let position = nudge(&order, task.id, up)?;
    Some(Command::MoveTask { id: task.id, planned_on: Some(day), position, at: now })
}

/// "Move to previous / next day": the bottom of the day before or after the task's day
/// (today's for an unplanned task).
pub(crate) fn shift_move(task: &Task, later: bool, now: Now) -> Command {
    let from = task.planned_on.unwrap_or(now.today());
    let day = if later { from + Days::new(1) } else { from - Days::new(1) };
    Command::MoveTask { id: task.id, planned_on: Some(day), position: Position::Bottom, at: now.at }
}

pub(crate) fn move_targets(today: NaiveDate) -> Vec<MoveTarget> {
    palette::move_days(today).into_iter().map(|(label, day)| MoveTarget { label, day_iso: day.map(iso_day) }).collect()
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
