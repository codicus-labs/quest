//! A confirmed proposal as domain commands, applied all or nothing (`Store::dispatch_all`), and how
//! to take them back. The domain keeps its rules: sort keys, "the session counts towards the task's
//! quest" and the repeat checks come from the commands themselves.

use std::collections::BTreeMap;

use chrono::{DateTime, Days, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, TimeZone, Utc};
use chrono_tz::Tz;
use quest_domain::{Command, EntityId, Position, QuestColor, RepeatPin, RepeatRule, Snapshot, Weekdays, WorkTask};
use quest_protocol::{CaptureProposal, ProposedRepeat, ProposedWork, QuestRef, TempKey, WorkTarget};
pub use quest_view::undo::Deletable as Created;

/// The commands for a confirmed proposal, in order, and what they create.
#[derive(Clone, Debug, PartialEq)]
pub struct CapturePlan {
    pub commands: Vec<Command>,
    /// For undo: the quests, tasks and sessions the commands create (milestones go with their quest).
    pub created: Vec<Created>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ApplyError {
    #[error("the quest {0} no longer exists; pick another quest")]
    UnknownQuest(EntityId),
    #[error("the task {0} no longer exists; pick another task")]
    UnknownTask(EntityId),
    #[error("a title is empty; type a title or deselect the item")]
    EmptyTitle,
    #[error("a session must end after it starts; fix its times or deselect it")]
    InvalidTimeRange,
}

/// The commands that create the confirmed `proposal` (only what the user kept selected; see
/// [`crate::Draft::confirmed`]). A task naming a new quest that is not in the proposal gets no
/// quest. `new_id` gives the ids of created entities.
pub fn plan(
    proposal: &CaptureProposal,
    snapshot: &Snapshot,
    now: DateTime<Utc>,
    time_zone: Tz,
    mut new_id: impl FnMut() -> EntityId,
) -> Result<CapturePlan, ApplyError> {
    let mut plan = CapturePlan { commands: Vec::new(), created: Vec::new() };
    let colors = new_quest_colors(snapshot, proposal.quests.len());
    let mut new_quests: BTreeMap<&TempKey, EntityId> = BTreeMap::new();
    for (quest, color) in proposal.quests.iter().zip(colors) {
        let id = new_id();
        plan.commands.push(Command::CreateQuest { id, title: title(&quest.title)?, color });
        if !quest.description.trim().is_empty() {
            plan.commands.push(Command::SetQuestDescription { id, description: quest.description.trim().to_owned() });
        }
        for milestone in &quest.milestones {
            let title = title(&milestone.title)?;
            plan.commands.push(Command::AddMilestone {
                id: new_id(),
                quest_id: id,
                title,
                achieved_on: milestone.achieved_on,
            });
        }
        new_quests.insert(&quest.key, id);
        plan.created.push(Created::Quest(id));
    }
    let quest_of = |quest: &Option<QuestRef>| -> Result<Option<EntityId>, ApplyError> {
        match quest {
            None => Ok(None),
            Some(QuestRef::Existing { id }) => {
                snapshot.quest(*id).map(|q| Some(q.id)).ok_or(ApplyError::UnknownQuest(*id))
            }
            Some(QuestRef::New { key }) => Ok(new_quests.get(key).copied()),
        }
    };
    for task in &proposal.tasks {
        let id = new_id();
        let quest_id = quest_of(&task.quest)?;
        plan.commands.push(Command::CreateTask {
            id,
            title: title(&task.title)?,
            quest_id,
            day: task.day,
            position: Position::Bottom,
        });
        if !task.description.trim().is_empty() {
            plan.commands.push(Command::SetTaskDescription { id, description: task.description.trim().to_owned() });
        }
        if let (Some(_), Some(block)) = (task.day, task.block) {
            plan.commands.push(Command::SetTaskBlock { id, block: Some(block) });
        }
        if let (Some(day), Some(repeat)) = (task.day, &task.repeat) {
            plan.commands.push(Command::SetTaskRepeat { id, rule: Some(rule(repeat, day, time_zone)), now });
        }
        plan.created.push(Created::Task(id));
    }
    for work in &proposal.work {
        let entry_id = new_id();
        let task = match &work.task {
            WorkTarget::Existing { id } => {
                snapshot.task(*id).ok_or(ApplyError::UnknownTask(*id))?;
                WorkTask::Existing(*id)
            }
            WorkTarget::New { title: raw, quest } => {
                let id = new_id();
                plan.created.push(Created::Task(id));
                WorkTask::New { id, title: title(raw)?, quest_id: quest_of(quest)?, day: work.day }
            }
        };
        let (started_at, ended_at) = session_times(work, time_zone)?;
        plan.commands.push(Command::LogWork { entry_id, task, started_at, ended_at });
        plan.created.push(Created::Entry(entry_id));
    }
    Ok(plan)
}

/// The colours `count` new quests get, in order: like `quest_view::quests::new_quest_color`, each
/// counting the ones before it.
pub fn new_quest_colors(snapshot: &Snapshot, count: usize) -> Vec<QuestColor> {
    let mut used: Vec<QuestColor> = snapshot.quests.iter().map(|q| q.color).collect();
    (0..count)
        .map(|_| {
            let color = QuestColor::ALL.into_iter().find(|c| !used.contains(c)).unwrap_or(QuestColor::Blue);
            used.push(color);
            color
        })
        .collect()
}

/// Commands that take back what a plan created and still exists: sessions first, then tasks, then
/// quests (which take their milestones along).
pub fn undo(created: &[Created], snapshot: &Snapshot, at: DateTime<Utc>) -> Vec<Command> {
    let exists = |item: &Created| match item {
        Created::Entry(id) => snapshot.time_entries.iter().any(|e| e.id == *id),
        Created::Task(id) => snapshot.task(*id).is_some(),
        Created::Quest(id) => snapshot.quest(*id).is_some(),
        Created::Milestone(id) => snapshot.milestones.iter().any(|m| m.id == *id),
    };
    let order = |item: &Created| match item {
        Created::Entry(_) => 0,
        Created::Task(_) => 1,
        Created::Milestone(_) => 2,
        Created::Quest(_) => 3,
    };
    let mut items: Vec<Created> = created.iter().copied().filter(exists).collect();
    items.sort_by_key(order);
    items.into_iter().map(|item| item.delete(at)).collect()
}

fn title(raw: &str) -> Result<String, ApplyError> {
    let title = raw.trim();
    if title.is_empty() {
        return Err(ApplyError::EmptyTitle);
    }
    Ok(title.to_owned())
}

fn rule(repeat: &ProposedRepeat, starts: NaiveDate, time_zone: Tz) -> RepeatRule {
    RepeatRule {
        every: repeat.every,
        frequency: repeat.frequency,
        starts,
        ends: repeat.ends,
        time_zone,
        pin: RepeatPin::None,
        weekdays: repeat
            .weekdays
            .iter()
            .fold(Weekdays::default(), |days, day| if days.contains(*day) { days } else { days.toggled(*day) }),
        month_day: repeat.month_day,
    }
}

/// True if a session from `start` to `end` on one day is a time range: an end at `00:00` is
/// midnight at the end of the day, but `00:00`–`00:00` is not a whole day.
pub(crate) fn is_time_range(start: NaiveTime, end: NaiveTime) -> bool {
    if end == NaiveTime::MIN { start != NaiveTime::MIN } else { start < end }
}

/// The session's instants; an end at `00:00` is midnight at the end of its day.
pub(crate) fn session_times(work: &ProposedWork, time_zone: Tz) -> Result<(DateTime<Utc>, DateTime<Utc>), ApplyError> {
    if !is_time_range(work.start, work.end) {
        return Err(ApplyError::InvalidTimeRange);
    }
    let started_at = instant(time_zone, work.day.and_time(work.start));
    let end_day = if work.end == NaiveTime::MIN { work.day.checked_add_days(Days::new(1)) } else { Some(work.day) };
    let end_day = end_day.ok_or(ApplyError::InvalidTimeRange)?;
    let ended_at = instant(time_zone, end_day.and_time(work.end));
    if ended_at <= started_at {
        return Err(ApplyError::InvalidTimeRange);
    }
    Ok((started_at, ended_at))
}

/// The instant a local wall-clock time names: the earlier one in a DST fold; a skipped time reads an
/// hour later (as in `quest-mobile`'s `Now::instant`).
pub fn instant(time_zone: Tz, local: NaiveDateTime) -> DateTime<Utc> {
    let resolve =
        |local: NaiveDateTime| time_zone.from_local_datetime(&local).earliest().map(|t| t.with_timezone(&Utc));
    let hour = TimeDelta::hours(1);
    resolve(local).or_else(|| resolve(local - hour).map(|before| before + hour)).unwrap_or_else(|| local.and_utc())
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
