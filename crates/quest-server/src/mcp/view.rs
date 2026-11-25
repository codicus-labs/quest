//! Pure translation between the tools' JSON and the domain: parsing inputs (ids, ISO days, clock
//! times, time zones) into domain values with model-readable errors, and describing records.

use chrono::{DateTime, Days, NaiveDate, NaiveTime, SecondsFormat, Utc};
use chrono_tz::Tz;
use quest_domain::{
    EntityId, EntityKind, Milestone, Quest, QuestColor, QuestStatus, Snapshot, Task, TimeBlock, TimeEntry, Workspace,
};
use quest_view::format::hh_mm;
use quest_view::repeat::{cadence, series_of};

use super::error::{ToolCode, ToolError};
use super::types::{
    ArchivedOut, BlockInput, BlockOut, ColorInput, EntryOut, ItemKind, MilestoneOut, QuestOut, RefOut, TaskOut,
    WorkspaceOut,
};

/// Longest day range a list covers.
pub const MAX_RANGE_DAYS: i64 = 62;

/// How far ahead a planned repeat can be addressed by its id.
const PREVIEW_HORIZON_DAYS: u64 = 366;

/// The moment of the call and the user's zone: what "today" and local times mean.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    pub now: DateTime<Utc>,
    pub zone: Tz,
}

impl Clock {
    pub fn today(&self) -> NaiveDate {
        self.now.with_timezone(&self.zone).date_naive()
    }

    pub fn instant(&self, at: DateTime<Utc>) -> String {
        at.with_timezone(&self.zone).to_rfc3339_opts(SecondsFormat::Secs, false)
    }

    /// The UTC instant of a local wall-clock time (the earlier one in a DST overlap, the later
    /// one in a gap).
    pub fn utc(&self, day: NaiveDate, time: NaiveTime) -> DateTime<Utc> {
        quest_capture::instant(self.zone, day.and_time(time))
    }

    pub fn midnight(&self, day: NaiveDate) -> DateTime<Utc> {
        self.utc(day, NaiveTime::MIN)
    }

    pub fn zone_name(&self) -> String {
        self.zone.name().to_owned()
    }
}

pub fn time_zone(input: Option<&str>, default: Tz) -> Result<Tz, ToolError> {
    match input.map(str::trim).filter(|name| !name.is_empty()) {
        None => Ok(default),
        Some(name) => name.parse::<Tz>().map_err(|_| {
            ToolError::invalid(format!("Unknown time_zone \"{name}\"; use an IANA name such as Europe/Berlin or UTC"))
        }),
    }
}

pub fn parse_id(text: &str, field: &str) -> Result<EntityId, ToolError> {
    text.trim()
        .parse::<EntityId>()
        .map_err(|_| ToolError::invalid(format!("{field} must be an id (a UUID) from a list or search")))
}

/// The live workspace `selector` names, by id or by exact name (case-insensitive); none: the
/// default workspace. Deleted, unknown and ambiguous names are refused rather than falling back,
/// so a call never reads or writes a workspace the caller did not mean.
pub fn resolve_workspace(workspaces: &[Workspace], selector: Option<&str>) -> Result<EntityId, ToolError> {
    let live = || workspaces.iter().filter(|workspace| !workspace.is_deleted());
    let Some(selector) = selector.map(str::trim).filter(|selector| !selector.is_empty()) else {
        return live().find(|workspace| workspace.is_default()).map(|workspace| workspace.id).ok_or_else(|| {
            ToolError::new(
                ToolCode::WorkspaceNotFound,
                "The default workspace is deleted; pass `workspace` (an id or name from list_workspaces) \
                    or restore the default workspace in the app",
            )
        });
    };
    if let Ok(id) = selector.parse::<EntityId>() {
        return live()
            .find(|workspace| workspace.id == id)
            .map(|workspace| workspace.id)
            .ok_or_else(|| ToolError::workspace_not_found(format!("with id {id}")));
    }
    let wanted = selector.to_lowercase();
    let mut named = live().filter(|workspace| workspace.name.trim().to_lowercase() == wanted);
    match (named.next(), named.next()) {
        (Some(workspace), None) => Ok(workspace.id),
        (Some(_), Some(_)) => Err(ToolError::new(
            ToolCode::WorkspaceNotFound,
            format!("Several workspaces are named \"{selector}\"; pass the id from list_workspaces instead"),
        )),
        (None, _) => Err(ToolError::workspace_not_found(format!("named \"{selector}\""))),
    }
}

pub fn workspace_ref(workspace: &Workspace) -> RefOut {
    reference(workspace.id, &workspace.name)
}

pub fn workspace_out(workspace: &Workspace) -> WorkspaceOut {
    let tenths_of_hours = (workspace.content.tracked_ms as f64 / 360_000.0).round();
    WorkspaceOut {
        id: workspace.id.to_string(),
        name: workspace.name.clone(),
        is_default: workspace.is_default(),
        quests: workspace.content.quests,
        tasks: workspace.content.tasks,
        tracked_hours: tenths_of_hours / 10.0,
    }
}

pub fn parse_day(text: &str, field: &str) -> Result<NaiveDate, ToolError> {
    NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d")
        .map_err(|_| ToolError::invalid(format!("{field} must be an ISO date like 2026-11-24")))
}

/// A day, or `None` for the backlog ("someday").
pub fn parse_day_or_someday(text: &str, field: &str) -> Result<Option<NaiveDate>, ToolError> {
    if text.trim().eq_ignore_ascii_case("someday") {
        return Ok(None);
    }
    parse_day(text, field).map(Some)
}

pub fn parse_clock(text: &str, field: &str) -> Result<NaiveTime, ToolError> {
    NaiveTime::parse_from_str(text.trim(), "%H:%M")
        .map_err(|_| ToolError::invalid(format!("{field} must be a local time like 09:30 (24-hour HH:MM)")))
}

pub fn parse_block(block: &BlockInput) -> Result<TimeBlock, ToolError> {
    let start = parse_clock(&block.start, "block.start")?;
    let start_minutes = start.signed_duration_since(NaiveTime::MIN).num_minutes();
    if !(5..=1440).contains(&block.minutes) {
        return Err(ToolError::invalid("block.minutes must be from 5 to 1440"));
    }
    if start_minutes + i64::from(block.minutes) > 1440 {
        return Err(ToolError::invalid("The block must end by midnight; start earlier or make it shorter"));
    }
    Ok(TimeBlock { start, minutes: block.minutes })
}

/// `[from, to]` (inclusive), at most [`MAX_RANGE_DAYS`] long.
pub fn day_range(from: NaiveDate, to: NaiveDate) -> Result<Vec<NaiveDate>, ToolError> {
    let length = (to - from).num_days();
    if length < 0 {
        return Err(ToolError::invalid("to must be on or after from"));
    }
    if length >= MAX_RANGE_DAYS {
        return Err(ToolError::invalid(format!("A range covers at most {MAX_RANGE_DAYS} days; ask for less")));
    }
    Ok(from.iter_days().take(usize::try_from(length).unwrap_or(0) + 1).collect())
}

pub fn color(input: ColorInput) -> QuestColor {
    match input {
        ColorInput::Red => QuestColor::Red,
        ColorInput::Orange => QuestColor::Orange,
        ColorInput::Yellow => QuestColor::Yellow,
        ColorInput::Lime => QuestColor::Lime,
        ColorInput::Green => QuestColor::Green,
        ColorInput::Teal => QuestColor::Teal,
        ColorInput::Blue => QuestColor::Blue,
        ColorInput::Violet => QuestColor::Violet,
        ColorInput::Pink => QuestColor::Pink,
    }
}

pub fn entity_kind(kind: ItemKind) -> EntityKind {
    match kind {
        ItemKind::Task => EntityKind::Task,
        ItemKind::Quest => EntityKind::Quest,
        ItemKind::Milestone => EntityKind::Milestone,
        ItemKind::TimeEntry => EntityKind::TimeEntry,
    }
}

fn item_kind(kind: EntityKind) -> Option<ItemKind> {
    match kind {
        EntityKind::Task => Some(ItemKind::Task),
        EntityKind::Quest => Some(ItemKind::Quest),
        EntityKind::Milestone => Some(ItemKind::Milestone),
        EntityKind::TimeEntry => Some(ItemKind::TimeEntry),
        EntityKind::Settings | EntityKind::Workspace => None,
    }
}

/// A task by id: a saved one, or a repeat planned for one of the coming days (`planned`).
pub fn find_task(snapshot: &Snapshot, id: EntityId, clock: &Clock) -> Option<(Task, bool)> {
    if let Some(task) = snapshot.task(id) {
        return Some((task.clone(), false));
    }
    if !snapshot.tasks.iter().any(|task| task.repeat.is_some()) {
        return None;
    }
    let today = clock.today();
    (0..PREVIEW_HORIZON_DAYS)
        .filter_map(|offset| today.checked_add_days(Days::new(offset)))
        .find_map(|day| snapshot.planned_occurrences(day, clock.now).find(|task| task.id == id))
        .map(|task| (task, true))
}

pub fn require_task(snapshot: &Snapshot, id: EntityId, clock: &Clock) -> Result<(Task, bool), ToolError> {
    find_task(snapshot, id, clock).ok_or_else(|| ToolError::task_not_found(id))
}

pub fn require_quest(snapshot: &Snapshot, id: EntityId) -> Result<&Quest, ToolError> {
    snapshot.quest(id).ok_or_else(|| ToolError::quest_not_found(id))
}

/// One day's tasks as the apps list them: saved ones and planned repeats, open ones first.
pub fn day_tasks(snapshot: &Snapshot, day: NaiveDate, now: DateTime<Utc>) -> Vec<(Task, bool)> {
    let mut tasks: Vec<(Task, bool)> = snapshot
        .tasks_on(day)
        .map(|task| (task.clone(), false))
        .chain(snapshot.planned_occurrences(day, now).map(|task| (task, true)))
        .collect();
    tasks.sort_by(|(a, _), (b, _)| {
        a.is_done().cmp(&b.is_done()).then(a.sort_key.cmp(&b.sort_key)).then(a.id.cmp(&b.id))
    });
    tasks
}

fn reference(id: EntityId, title: &str) -> RefOut {
    RefOut { id: id.to_string(), title: title.to_owned() }
}

pub fn task_out(snapshot: &Snapshot, task: &Task, planned: bool, clock: &Clock) -> TaskOut {
    let block = task.block.map(|block| {
        let start = u32::try_from(block.start.signed_duration_since(NaiveTime::MIN).num_minutes()).unwrap_or(0);
        BlockOut { start: hh_mm(start), end: hh_mm(start + block.minutes), minutes: block.minutes }
    });
    TaskOut {
        id: task.id.to_string(),
        title: task.title.clone(),
        description: task.description.clone(),
        quest: task.quest_id.and_then(|id| snapshot.quest(id)).map(|quest| reference(quest.id, &quest.title)),
        day: task.day.map(|day| day.to_string()),
        block,
        done: task.is_done(),
        completed_at: task.completed_at.map(|at| clock.instant(at)),
        repeat: series_of(snapshot, task).repeat.map(|schedule| cadence(schedule.rule.every, schedule.rule.frequency)),
        planned_repeat: planned,
        tracked_minutes: snapshot.task_tracked_ms(task.id, clock.now) / 60_000,
    }
}

pub fn quest_status(status: QuestStatus) -> &'static str {
    match status {
        QuestStatus::Active => "active",
        QuestStatus::Archived => "archived",
        QuestStatus::Completed => "completed",
    }
}

pub fn quest_out(snapshot: &Snapshot, quest: &Quest, now: DateTime<Utc>) -> QuestOut {
    QuestOut {
        id: quest.id.to_string(),
        title: quest.title.clone(),
        description: quest.description.clone(),
        color: quest_view::quests::color_name(quest.color).to_lowercase(),
        status: quest_status(quest.status).to_owned(),
        tracked_minutes: quest_view::quests::total_ms(snapshot, quest.id, now) / 60_000,
        open_tasks: snapshot.tasks.iter().filter(|task| task.quest_id == Some(quest.id) && !task.is_done()).count(),
    }
}

pub fn milestone_out(milestone: &Milestone) -> MilestoneOut {
    MilestoneOut {
        id: milestone.id.to_string(),
        quest_id: milestone.quest_id.to_string(),
        title: milestone.title.clone(),
        achieved_on: milestone.achieved_on.to_string(),
    }
}

pub fn entry_out(snapshot: &Snapshot, entry: &TimeEntry, clock: &Clock) -> EntryOut {
    EntryOut {
        id: entry.id.to_string(),
        task: entry.task_id.and_then(|id| snapshot.task(id)).map(|task| reference(task.id, &task.title)),
        quest: snapshot
            .entry_quest(entry)
            .and_then(|id| snapshot.quest(id))
            .map(|quest| reference(quest.id, &quest.title)),
        started_at: clock.instant(entry.started_at),
        ended_at: entry.ended_at.map(|at| clock.instant(at)),
        minutes: entry.tracked_ms(clock.now) / 60_000,
        running: entry.is_running(),
    }
}

/// The archived (soft-deleted) record `kind`/`id`, if it is in the archive.
pub fn archived_out(snapshot: &Snapshot, kind: EntityKind, id: EntityId, clock: &Clock) -> Option<ArchivedOut> {
    let item = snapshot.trash.iter().find(|item| item.kind == kind && item.id == id)?;
    Some(ArchivedOut {
        kind: item_kind(item.kind)?,
        id: item.id.to_string(),
        title: item.title.clone(),
        archived_at: clock.instant(item.deleted_at),
    })
}

pub fn all_archived(snapshot: &Snapshot, clock: &Clock) -> impl Iterator<Item = ArchivedOut> {
    snapshot.trash.iter().filter_map(|item| archived_out(snapshot, item.kind, item.id, clock))
}

/// Overlap of `entry` with `[from, to)` in milliseconds.
pub fn overlap_ms(entry: &TimeEntry, from: DateTime<Utc>, to: DateTime<Utc>, now: DateTime<Utc>) -> i64 {
    let start = entry.started_at.max(from);
    let end = entry.ended_at.unwrap_or(now).min(to);
    (end - start).num_milliseconds().max(0)
}

/// UTC midnight bounds of `[first, last]` local days.
pub fn day_bounds(clock: &Clock, first: NaiveDate, last: NaiveDate) -> (DateTime<Utc>, DateTime<Utc>) {
    let after = last.checked_add_days(Days::new(1)).unwrap_or(last);
    (clock.midnight(first), clock.midnight(after))
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
