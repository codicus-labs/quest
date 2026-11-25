//! The compact context sent with a capture: just enough to assign the text to existing quests and
//! tasks, and no more (it goes to an external AI provider).

use chrono::{DateTime, Days, NaiveDate, Utc};
use chrono_tz::Tz;
use quest_domain::{Snapshot, Task};
use quest_protocol::{
    CAPTURE_VERSION, CaptureContext, CaptureRequest, ContextQuest, ContextTask, MAX_CONTEXT_DESCRIPTION_CHARS,
    MAX_CONTEXT_QUESTS, MAX_CONTEXT_TASKS, MAX_CONTEXT_TITLE_CHARS,
};

use crate::text::{shorten, single_line};

/// Planned tasks this many days before today count as "around today" (for logging work on them).
const DAYS_BEFORE: u64 = 7;
/// And this many days after.
const DAYS_AFTER: u64 = 14;

/// The request a client sends for `text`, with the context from `snapshot`.
pub fn request(text: &str, now: DateTime<Utc>, time_zone: Tz, language: &str, snapshot: &Snapshot) -> CaptureRequest {
    let today = now.with_timezone(&time_zone).date_naive();
    CaptureRequest {
        version: CAPTURE_VERSION,
        text: text.trim().to_owned(),
        now,
        time_zone: time_zone.name().to_owned(),
        language: language.to_owned(),
        context: context(snapshot, today),
    }
}

/// Active quests (titles, shortened descriptions) and the tasks around `today`: planned ones from a
/// week before to two weeks after, nearest first, then open backlog tasks, within the protocol's limits.
pub fn context(snapshot: &Snapshot, today: NaiveDate) -> CaptureContext {
    let quests: Vec<ContextQuest> = snapshot
        .active_quests()
        .take(MAX_CONTEXT_QUESTS)
        .map(|quest| ContextQuest {
            id: quest.id,
            title: shorten(&single_line(&quest.title), MAX_CONTEXT_TITLE_CHARS),
            description: shorten(&single_line(&quest.description), MAX_CONTEXT_DESCRIPTION_CHARS),
        })
        .collect();
    let from = today.checked_sub_days(Days::new(DAYS_BEFORE)).unwrap_or(today);
    let to = today.checked_add_days(Days::new(DAYS_AFTER)).unwrap_or(today);
    let mut planned: Vec<&Task> =
        snapshot.tasks.iter().filter(|t| t.day.is_some_and(|day| (from..=to).contains(&day))).collect();
    planned.sort_by_key(|t| t.day.map(|day| (day - today).num_days().abs()));
    let backlog = snapshot.backlog().filter(|t| !t.is_done());
    let tasks = planned
        .into_iter()
        .chain(backlog)
        .take(MAX_CONTEXT_TASKS)
        .map(|task| ContextTask {
            id: task.id,
            title: shorten(&single_line(&task.title), MAX_CONTEXT_TITLE_CHARS),
            quest_id: task.quest_id.filter(|id| quests.iter().any(|q| q.id == *id)),
            day: task.day,
            done: task.is_done(),
        })
        .collect();
    CaptureContext { quests, tasks }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
