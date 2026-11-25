//! The write tools' decisions, pure: the snapshot, the input and the clock in; the domain commands
//! to run (all or nothing) out. Planned repeats are saved on their first edit, as in the apps'
//! plan (`Command::EditPlan`).

use chrono::{NaiveDate, Weekday};
use quest_domain::{
    Command, EntityId, EntityKind, Frequency, MonthDay, Position, QuestStatus, RepeatPin, RepeatRule, Snapshot, Task,
    Weekdays, WorkTask,
};
use quest_view::repeat::series_of;

use super::error::{ToolCode, ToolError};
use super::types::{
    AddChecklistItemInput, CheckChecklistItemInput, CreateQuestInput, CreateTaskInput, FrequencyInput, LogWorkInput,
    MoveTaskInput, PositionInput, QuestStatusInput, RenameChecklistItemInput, RepeatInput, RestoreInput,
    StartTimerInput, UpdateQuestInput, UpdateTaskInput,
};
use super::view::{
    Clock, color, entity_kind, parse_clock, parse_day, parse_day_or_none, parse_id, require_quest, require_task,
};

/// Ids the commands create, chosen by the caller.
pub trait NewIds {
    fn next(&mut self) -> EntityId;
}

impl<F: FnMut() -> EntityId> NewIds for F {
    fn next(&mut self) -> EntityId {
        self()
    }
}

/// A task edit as the plan carries it out: on a planned repeat it saves the repeat first, and a
/// move to a day sorts among that day's planned repeats (desktop `State::plan_edit`).
fn plan_edit(task: &Task, planned: bool, command: Command, clock: &Clock) -> Command {
    let occurrence = if planned { task.series.zip(task.planned_on) } else { None };
    if occurrence.is_some() || matches!(command, Command::MoveTask { planned_on: Some(_), .. }) {
        return Command::EditPlan { occurrence, now: clock.now, edit: Box::new(command) };
    }
    command
}

fn optional_quest(snapshot: &Snapshot, quest_id: Option<&str>) -> Result<Option<EntityId>, ToolError> {
    let Some(id) = quest_id else { return Ok(None) };
    let id = parse_id(id, "quest_id")?;
    require_quest(snapshot, id)?;
    Ok(Some(id))
}

/// Optional plan date and deadline as given; both given, the plan date may not be after the
/// deadline. One alone follows the domain rule (`set_planned`, `set_due`).
fn dates(planned_on: Option<&str>, due_on: Option<&str>) -> Result<DateInput, ToolError> {
    let planned_on = planned_on.map(|day| parse_day_or_none(day, "planned_on")).transpose()?;
    let due_on = due_on.map(|day| parse_day_or_none(day, "due_on")).transpose()?;
    if let (Some(Some(plan)), Some(Some(due))) = (planned_on, due_on)
        && plan > due
    {
        return Err(ToolError::invalid("planned_on must be on or before due_on"));
    }
    Ok(DateInput { planned_on, due_on })
}

/// Dates the caller gave: `None` keeps the date, `Some(None)` clears it.
struct DateInput {
    planned_on: Option<Option<NaiveDate>>,
    due_on: Option<Option<NaiveDate>>,
}

fn title(text: &str) -> Result<String, ToolError> {
    let title = text.trim();
    if title.is_empty() {
        return Err(ToolError::new(ToolCode::EmptyTitle, "The title cannot be empty"));
    }
    Ok(title.to_owned())
}

pub fn create_task(
    snapshot: &Snapshot,
    input: &CreateTaskInput,
    clock: &Clock,
    ids: &mut impl NewIds,
) -> Result<(Vec<Command>, EntityId), ToolError> {
    let title = title(&input.title)?;
    let dates = dates(input.planned_on.as_deref(), input.due_on.as_deref())?;
    let (planned_on, due_on) = (dates.planned_on.flatten(), dates.due_on.flatten());
    let quest_id = optional_quest(snapshot, input.quest_id.as_deref())?;
    let id = ids.next();
    let mut commands =
        vec![Command::CreateTask { id, title, quest_id, planned_on, due_on, position: Position::Bottom }];
    if let Some(description) = input.description.as_deref().map(str::trim).filter(|text| !text.is_empty()) {
        commands.push(Command::SetTaskDescription { id, description: description.to_owned() });
    }
    if let Some(repeat) = &input.repeat {
        let rule = repeat_rule(repeat, planned_on, None, clock)?;
        commands.push(Command::SetTaskRepeat { id, rule: Some(rule), now: clock.now });
    }
    Ok((commands, id))
}

pub fn update_task(
    snapshot: &Snapshot,
    input: &UpdateTaskInput,
    clock: &Clock,
) -> Result<(Vec<Command>, EntityId), ToolError> {
    let (task, planned) = require_task(snapshot, parse_id(&input.task_id, "task_id")?, clock)?;
    let dates = dates(input.planned_on.as_deref(), input.due_on.as_deref())?;
    if input.repeat.is_some() && input.clear_repeat {
        return Err(ToolError::invalid("Give repeat or clear_repeat, not both"));
    }
    let id = task.id;
    let mut edits = Vec::new();
    if let Some(text) = &input.title {
        edits.push(Command::RenameTask { id, title: title(text)? });
    }
    if let Some(description) = &input.description {
        edits.push(Command::SetTaskDescription { id, description: description.trim().to_owned() });
    }
    if let Some(due_on) = dates.due_on.filter(|due_on| *due_on != task.due_on) {
        edits.push(Command::SetTaskDue { id, due_on, at: clock.now });
    }
    if let Some(planned_on) = dates.planned_on.filter(|planned_on| *planned_on != task.planned_on) {
        edits.push(Command::MoveTask { id, planned_on, position: Position::Bottom, at: clock.now });
    }
    if let Some(quest) = &input.quest_id {
        let quest_id =
            if quest.trim().eq_ignore_ascii_case("none") { None } else { optional_quest(snapshot, Some(quest))? };
        edits.push(Command::AssignTask { id, quest_id });
    }
    let mut commands: Vec<Command> = edits.into_iter().map(|edit| plan_edit(&task, planned, edit, clock)).collect();
    // A repeat belongs to the series (an occurrence edits its series), which is always saved.
    let series = series_of(snapshot, &task);
    if let Some(repeat) = &input.repeat {
        let previous = series.repeat.map(|schedule| schedule.rule);
        let planned_on = dates.planned_on.unwrap_or(task.planned_on);
        let rule = repeat_rule(repeat, series.planned_on.or(planned_on), previous, clock)?;
        commands.push(Command::SetTaskRepeat { id: series.id, rule: Some(rule), now: clock.now });
    }
    if input.clear_repeat {
        commands.push(Command::SetTaskRepeat { id: series.id, rule: None, now: clock.now });
    }
    if commands.is_empty() {
        return Err(ToolError::invalid("Nothing to change; give at least one field besides task_id"));
    }
    Ok((commands, id))
}

/// Completing an open task; a done one stays as it is.
pub fn complete_task(snapshot: &Snapshot, task_id: &str, clock: &Clock) -> Result<(Vec<Command>, EntityId), ToolError> {
    let (task, planned) = require_task(snapshot, parse_id(task_id, "task_id")?, clock)?;
    if task.is_done() {
        return Ok((Vec::new(), task.id));
    }
    let command = Command::CompleteTaskFromPlan { id: task.id, at: clock.now, today: clock.today() };
    Ok((vec![plan_edit(&task, planned, command, clock)], task.id))
}

/// Reopening a done task; an open one stays as it is.
pub fn reopen_task(snapshot: &Snapshot, task_id: &str, clock: &Clock) -> Result<(Vec<Command>, EntityId), ToolError> {
    let (task, _) = require_task(snapshot, parse_id(task_id, "task_id")?, clock)?;
    if !task.is_done() {
        return Ok((Vec::new(), task.id));
    }
    Ok((vec![Command::ReopenTask { id: task.id }], task.id))
}

pub fn move_task(
    snapshot: &Snapshot,
    input: &MoveTaskInput,
    clock: &Clock,
) -> Result<(Vec<Command>, EntityId), ToolError> {
    let (task, planned) = require_task(snapshot, parse_id(&input.task_id, "task_id")?, clock)?;
    let planned_on = parse_day_or_none(&input.planned_on, "planned_on")?;
    let position = match input.position {
        PositionInput::Top => Position::Top,
        PositionInput::Bottom => Position::Bottom,
    };
    let command = Command::MoveTask { id: task.id, planned_on, position, at: clock.now };
    Ok((vec![plan_edit(&task, planned, command, clock)], task.id))
}

pub fn archive_task(snapshot: &Snapshot, task_id: &str, clock: &Clock) -> Result<(Vec<Command>, EntityId), ToolError> {
    let (task, planned) = require_task(snapshot, parse_id(task_id, "task_id")?, clock)?;
    let command = Command::DeleteTask { id: task.id, at: clock.now };
    Ok((vec![plan_edit(&task, planned, command, clock)], task.id))
}

pub fn restore(input: &RestoreInput) -> Result<(Vec<Command>, (EntityKind, EntityId)), ToolError> {
    let kind = entity_kind(input.kind);
    let id = parse_id(&input.id, "id")?;
    Ok((vec![Command::Restore { kind, id }], (kind, id)))
}

pub fn create_quest(
    snapshot: &Snapshot,
    input: &CreateQuestInput,
    ids: &mut impl NewIds,
) -> Result<(Vec<Command>, EntityId), ToolError> {
    let title = title(&input.title)?;
    let color = input.color.map_or_else(|| quest_view::quests::new_quest_color(snapshot), color);
    let id = ids.next();
    Ok((vec![Command::CreateQuest { id, title, color }], id))
}

pub fn update_quest(snapshot: &Snapshot, input: &UpdateQuestInput) -> Result<(Vec<Command>, EntityId), ToolError> {
    let id = require_quest(snapshot, parse_id(&input.quest_id, "quest_id")?)?.id;
    let mut commands = Vec::new();
    if let Some(text) = &input.title {
        commands.push(Command::RenameQuest { id, title: title(text)? });
    }
    if let Some(choice) = input.color {
        commands.push(Command::SetQuestColor { id, color: color(choice) });
    }
    if let Some(status) = input.status {
        commands.push(Command::SetQuestStatus { id, status: quest_status(status) });
    }
    if commands.is_empty() {
        return Err(ToolError::invalid("Nothing to change; give at least one field besides quest_id"));
    }
    Ok((commands, id))
}

fn quest_status(status: QuestStatusInput) -> QuestStatus {
    match status {
        QuestStatusInput::Active => QuestStatus::Active,
        QuestStatusInput::Completed => QuestStatus::Completed,
    }
}

pub fn complete_quest(snapshot: &Snapshot, quest_id: &str) -> Result<(Vec<Command>, EntityId), ToolError> {
    let quest = require_quest(snapshot, parse_id(quest_id, "quest_id")?)?;
    if quest.status == QuestStatus::Completed {
        return Ok((Vec::new(), quest.id));
    }
    Ok((vec![Command::SetQuestStatus { id: quest.id, status: QuestStatus::Completed }], quest.id))
}

/// Adds an item at the end of a task's checklist. Returns the task's id.
pub fn add_checklist_item(
    snapshot: &Snapshot,
    input: &AddChecklistItemInput,
    clock: &Clock,
    ids: &mut impl NewIds,
) -> Result<(Vec<Command>, EntityId), ToolError> {
    let (task, planned) = require_task(snapshot, parse_id(&input.task_id, "task_id")?, clock)?;
    let command = Command::AddChecklistItem {
        id: ids.next(),
        task_id: task.id,
        text: title(&input.text)?,
        position: Position::Bottom,
    };
    Ok((vec![plan_edit(&task, planned, command, clock)], task.id))
}

/// The commands below return the item's task id.
pub fn check_checklist_item(
    snapshot: &Snapshot,
    input: &CheckChecklistItemInput,
) -> Result<(Vec<Command>, EntityId), ToolError> {
    let (id, task) = checklist_item(snapshot, &input.item_id)?;
    Ok((vec![Command::SetChecklistItemDone { id, done: input.done }], task))
}

pub fn rename_checklist_item(
    snapshot: &Snapshot,
    input: &RenameChecklistItemInput,
) -> Result<(Vec<Command>, EntityId), ToolError> {
    let (id, task) = checklist_item(snapshot, &input.item_id)?;
    Ok((vec![Command::RenameChecklistItem { id, text: title(&input.text)? }], task))
}

pub fn delete_checklist_item(
    snapshot: &Snapshot,
    item_id: &str,
    clock: &Clock,
) -> Result<(Vec<Command>, EntityId), ToolError> {
    let (id, task) = checklist_item(snapshot, item_id)?;
    Ok((vec![Command::DeleteChecklistItem { id, at: clock.now }], task))
}

/// The item's id and its task's.
fn checklist_item(snapshot: &Snapshot, text: &str) -> Result<(EntityId, EntityId), ToolError> {
    let id = parse_id(text, "item_id")?;
    let item = snapshot.checklist_item(id).ok_or(quest_domain::CommandError::UnknownChecklistItem(id))?;
    Ok((item.id, item.task_id))
}

pub fn start_timer(
    snapshot: &Snapshot,
    input: &StartTimerInput,
    clock: &Clock,
    ids: &mut impl NewIds,
) -> Result<(Vec<Command>, EntityId), ToolError> {
    let task = input.task_id.as_deref().map(|id| parse_id(id, "task_id")).transpose()?;
    let task = task.map(|id| require_task(snapshot, id, clock)).transpose()?;
    let entry_id = ids.next();
    if input.organizing {
        let Some((task, planned)) = task else {
            return Ok((vec![Command::StartOrganizing { entry_id, origin_task_id: None, at: clock.now }], entry_id));
        };
        let command = Command::StartOrganizing { entry_id, origin_task_id: Some(task.id), at: clock.now };
        return Ok((vec![plan_edit(&task, planned, command, clock)], entry_id));
    }
    let Some((task, planned)) = task else {
        return Err(ToolError::invalid("Give task_id, or organizing true to track organizing"));
    };
    let command = Command::StartFocus { entry_id, task_id: task.id, at: clock.now };
    Ok((vec![plan_edit(&task, planned, command, clock)], entry_id))
}

/// Stops the running timer, in any workspace; nothing to do when none runs. Returns the stopped
/// session's id.
pub fn stop_timer(snapshot: &Snapshot, clock: &Clock) -> (Vec<Command>, Option<EntityId>) {
    match snapshot.running_focus() {
        Some(running) => (vec![Command::StopFocus { at: clock.now }], Some(running.entry.id)),
        None => (Vec::new(), None),
    }
}

pub fn log_work(
    snapshot: &Snapshot,
    input: &LogWorkInput,
    clock: &Clock,
    ids: &mut impl NewIds,
) -> Result<(Vec<Command>, EntityId), ToolError> {
    let day = input.day.as_deref().map(|day| parse_day(day, "day")).transpose()?.unwrap_or(clock.today());
    let start = parse_clock(&input.start, "start")?;
    let end = parse_clock(&input.end, "end")?;
    if end <= start {
        return Err(ToolError::new(ToolCode::InvalidTimeRange, "end must be after start on the same day"));
    }
    let started_at = clock.utc(day, start);
    let ended_at = clock.utc(day, end);
    if ended_at > clock.now {
        return Err(ToolError::new(
            ToolCode::InvalidTimeRange,
            "The work cannot end in the future; use start_timer for work that is going on",
        ));
    }
    let task = match (&input.task_id, &input.new_task_title) {
        (Some(_), Some(_)) => return Err(ToolError::invalid("Give task_id or new_task_title, not both")),
        (None, None) => return Err(ToolError::invalid("Give task_id (existing task) or new_task_title (new task)")),
        (Some(id), None) => {
            if input.quest_id.is_some() {
                return Err(ToolError::invalid("An existing task brings its own quest; leave out quest_id"));
            }
            let (task, planned) = require_task(snapshot, parse_id(id, "task_id")?, clock)?;
            let entry_id = ids.next();
            let command = Command::LogWork { entry_id, task: WorkTask::Existing(task.id), started_at, ended_at };
            return Ok((vec![plan_edit(&task, planned, command, clock)], entry_id));
        }
        (None, Some(text)) => WorkTask::New {
            id: ids.next(),
            title: title(text)?,
            quest_id: optional_quest(snapshot, input.quest_id.as_deref())?,
            planned_on: day,
        },
    };
    let entry_id = ids.next();
    Ok((vec![Command::LogWork { entry_id, task, started_at, ended_at }], entry_id))
}

/// The rule a repeat input describes. It starts on the task's plan date (not before today), keeps a
/// previous rule's pin, and runs in the call's time zone. The domain checks the rest.
pub fn repeat_rule(
    input: &RepeatInput,
    task_day: Option<NaiveDate>,
    previous: Option<RepeatRule>,
    clock: &Clock,
) -> Result<RepeatRule, ToolError> {
    let today = clock.today();
    let starts = match input.starts.as_deref() {
        Some(day) => parse_day(day, "repeat.starts")?,
        None => task_day.unwrap_or(today).max(today),
    };
    let ends = input.ends.as_deref().map(|day| parse_day(day, "repeat.ends")).transpose()?;
    let frequency = match input.frequency {
        FrequencyInput::Day => Frequency::Day,
        FrequencyInput::Week => Frequency::Week,
        FrequencyInput::Month => Frequency::Month,
        FrequencyInput::Year => Frequency::Year,
    };
    if !input.weekdays.is_empty() && frequency != Frequency::Week {
        return Err(ToolError::new(ToolCode::InvalidRepeatRule, "weekdays only work with frequency week"));
    }
    if input.month_day.is_some() && frequency != Frequency::Month {
        return Err(ToolError::new(ToolCode::InvalidRepeatRule, "month_day only works with frequency month"));
    }
    let mut weekdays = Weekdays::default();
    for name in &input.weekdays {
        let day = weekday(name)?;
        if !weekdays.contains(day) {
            weekdays = weekdays.toggled(day);
        }
    }
    let month_day = input.month_day.as_deref().map(month_day).transpose()?;
    Ok(RepeatRule {
        every: input.every.unwrap_or(1),
        frequency,
        starts,
        ends,
        time_zone: clock.zone,
        pin: previous.map_or(RepeatPin::None, |rule| rule.pin),
        weekdays,
        month_day,
    })
}

fn weekday(name: &str) -> Result<Weekday, ToolError> {
    name.trim().parse::<Weekday>().map_err(|_| {
        ToolError::new(
            ToolCode::InvalidRepeatRule,
            format!("Unknown weekday \"{name}\"; use mon, tue, wed, thu, fri, sat or sun"),
        )
    })
}

fn month_day(text: &str) -> Result<MonthDay, ToolError> {
    if text.trim().eq_ignore_ascii_case("last") {
        return Ok(MonthDay::Last);
    }
    text.trim()
        .parse::<u32>()
        .ok()
        .filter(|day| (1..=31).contains(day))
        .map(MonthDay::Day)
        .ok_or_else(|| ToolError::new(ToolCode::InvalidRepeatRule, "month_day must be 1 to 31 or \"last\""))
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
