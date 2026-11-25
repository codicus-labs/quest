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
    AddMilestoneInput, CreateQuestInput, CreateTaskInput, FrequencyInput, LogWorkInput, MoveTaskInput, PositionInput,
    QuestStatusInput, RepeatInput, RestoreInput, UpdateMilestoneInput, UpdateQuestInput, UpdateTaskInput,
};
use super::view::{
    Clock, color, entity_kind, parse_block, parse_clock, parse_day, parse_day_or_someday, parse_id, require_quest,
    require_task,
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
    let occurrence = if planned { task.series.zip(task.day) } else { None };
    if occurrence.is_some() || matches!(command, Command::MoveTask { day: Some(_), .. }) {
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
    let day = match input.day.as_deref() {
        Some(day) => parse_day_or_someday(day, "day")?,
        None => Some(clock.today()),
    };
    let quest_id = optional_quest(snapshot, input.quest_id.as_deref())?;
    let block = input.block.as_ref().map(parse_block).transpose()?;
    if block.is_some() && day.is_none() {
        return Err(ToolError::invalid("A time block needs a day; give day or leave out block"));
    }
    let id = ids.next();
    let mut commands = vec![Command::CreateTask { id, title, quest_id, day, position: Position::Bottom }];
    if let Some(description) = input.description.as_deref().map(str::trim).filter(|text| !text.is_empty()) {
        commands.push(Command::SetTaskDescription { id, description: description.to_owned() });
    }
    if block.is_some() {
        commands.push(Command::SetTaskBlock { id, block });
    }
    if let Some(repeat) = &input.repeat {
        let rule = repeat_rule(repeat, day, None, clock)?;
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
    if input.block.is_some() && input.clear_block {
        return Err(ToolError::invalid("Give block or clear_block, not both"));
    }
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
    let mut day = task.day;
    if let Some(text) = &input.day {
        day = parse_day_or_someday(text, "day")?;
        if day != task.day {
            edits.push(Command::MoveTask { id, day, position: Position::Bottom });
        }
    }
    if let Some(quest) = &input.quest_id {
        let quest_id =
            if quest.trim().eq_ignore_ascii_case("none") { None } else { optional_quest(snapshot, Some(quest))? };
        edits.push(Command::AssignTask { id, quest_id });
    }
    if let Some(block) = &input.block {
        let block = parse_block(block)?;
        if day.is_none() {
            return Err(ToolError::invalid("A time block needs a day; the task is in the backlog, so give day too"));
        }
        edits.push(Command::SetTaskBlock { id, block: Some(block) });
    }
    if input.clear_block {
        edits.push(Command::SetTaskBlock { id, block: None });
    }
    let mut commands: Vec<Command> = edits.into_iter().map(|edit| plan_edit(&task, planned, edit, clock)).collect();
    // A repeat belongs to the series (an occurrence edits its series), which is always saved.
    let series = series_of(snapshot, &task);
    if let Some(repeat) = &input.repeat {
        let previous = series.repeat.map(|schedule| schedule.rule);
        let rule = repeat_rule(repeat, series.day.or(day), previous, clock)?;
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
    let day = parse_day_or_someday(&input.day, "day")?;
    let position = match input.position {
        PositionInput::Top => Position::Top,
        PositionInput::Bottom => Position::Bottom,
    };
    let command = Command::MoveTask { id: task.id, day, position };
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
    let mut commands = vec![Command::CreateQuest { id, title, color }];
    if let Some(description) = input.description.as_deref().map(str::trim).filter(|text| !text.is_empty()) {
        commands.push(Command::SetQuestDescription { id, description: description.to_owned() });
    }
    Ok((commands, id))
}

pub fn update_quest(snapshot: &Snapshot, input: &UpdateQuestInput) -> Result<(Vec<Command>, EntityId), ToolError> {
    let id = require_quest(snapshot, parse_id(&input.quest_id, "quest_id")?)?.id;
    let mut commands = Vec::new();
    if let Some(text) = &input.title {
        commands.push(Command::RenameQuest { id, title: title(text)? });
    }
    if let Some(description) = &input.description {
        commands.push(Command::SetQuestDescription { id, description: description.trim().to_owned() });
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

pub fn add_milestone(
    snapshot: &Snapshot,
    input: &AddMilestoneInput,
    clock: &Clock,
    ids: &mut impl NewIds,
) -> Result<(Vec<Command>, EntityId), ToolError> {
    let quest_id = require_quest(snapshot, parse_id(&input.quest_id, "quest_id")?)?.id;
    let title = title(&input.title)?;
    let achieved_on =
        input.achieved_on.as_deref().map(|day| parse_day(day, "achieved_on")).transpose()?.unwrap_or(clock.today());
    let id = ids.next();
    Ok((vec![Command::AddMilestone { id, quest_id, title, achieved_on }], id))
}

pub fn update_milestone(
    snapshot: &Snapshot,
    input: &UpdateMilestoneInput,
) -> Result<(Vec<Command>, EntityId), ToolError> {
    let id = milestone_id(snapshot, &input.milestone_id)?;
    let mut commands = Vec::new();
    if let Some(text) = &input.title {
        commands.push(Command::RenameMilestone { id, title: title(text)? });
    }
    if let Some(day) = &input.achieved_on {
        commands.push(Command::SetMilestoneDate { id, achieved_on: parse_day(day, "achieved_on")? });
    }
    if commands.is_empty() {
        return Err(ToolError::invalid("Nothing to change; give title or achieved_on"));
    }
    Ok((commands, id))
}

pub fn delete_milestone(
    snapshot: &Snapshot,
    milestone: &str,
    clock: &Clock,
) -> Result<(Vec<Command>, EntityId), ToolError> {
    let id = milestone_id(snapshot, milestone)?;
    Ok((vec![Command::DeleteMilestone { id, at: clock.now }], id))
}

fn milestone_id(snapshot: &Snapshot, text: &str) -> Result<EntityId, ToolError> {
    let id = parse_id(text, "milestone_id")?;
    if !snapshot.milestones.iter().any(|milestone| milestone.id == id) {
        return Err(ToolError::from(quest_domain::CommandError::UnknownMilestone(id)));
    }
    Ok(id)
}

pub fn start_timer(
    snapshot: &Snapshot,
    task_id: &str,
    clock: &Clock,
    ids: &mut impl NewIds,
) -> Result<(Vec<Command>, EntityId), ToolError> {
    let (task, planned) = require_task(snapshot, parse_id(task_id, "task_id")?, clock)?;
    let entry_id = ids.next();
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
            day,
        },
    };
    let entry_id = ids.next();
    Ok((vec![Command::LogWork { entry_id, task, started_at, ended_at }], entry_id))
}

/// The rule a repeat input describes. It starts on the task's day (not before today), keeps a
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
