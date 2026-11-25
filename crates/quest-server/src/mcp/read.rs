//! The read tools' answers, pure: a snapshot, the input and the clock in, JSON out.

use chrono::Days;
use quest_domain::{QuestStatus, Snapshot, WeeklyReview};

use super::error::ToolError;
use super::types::{
    ChecklistOut, DayMinutesOut, DoneFilter, EntriesOut, ListEntriesInput, ListQuestsInput, ListTasksInput,
    QuestDetailOut, QuestFilter, QuestMinutesOut, QuestsOut, RefOut, SearchInput, SearchOut, TaskOut, TaskResult,
    TasksOut, TimerOut, WeekInput, WeekReviewOut,
};
use super::view::{
    Clock, all_archived, checklist_out, day_bounds, day_range, day_tasks, entry_out, overlap_ms, parse_day, parse_id,
    quest_out, require_quest, require_task, task_out, workspace_ref,
};

/// Most hits per kind a search returns.
const MAX_HITS: usize = 50;
/// Done tasks `get_quest` shows besides the open ones.
const RECENT_DONE: usize = 20;

pub fn list_tasks(snapshot: &Snapshot, input: &ListTasksInput, clock: &Clock) -> Result<TasksOut, ToolError> {
    let quest = input.quest_id.as_deref().map(|id| parse_id(id, "quest_id")).transpose()?;
    if let Some(quest) = quest {
        require_quest(snapshot, quest)?;
    }
    let candidates: Vec<(quest_domain::Task, bool)> = if input.unplanned {
        if input.day.is_some() || input.from.is_some() || input.to.is_some() {
            return Err(ToolError::invalid("Use either unplanned or day/from/to, not both"));
        }
        snapshot.tasks.iter().filter(|task| task.planned_on.is_none()).map(|task| (task.clone(), false)).collect()
    } else {
        let days = match (&input.day, &input.from, &input.to) {
            (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
                return Err(ToolError::invalid("Use either day or from/to, not both"));
            }
            (Some(day), None, None) => vec![parse_day(day, "day")?],
            (None, from, to) => {
                let from = from.as_deref().map(|day| parse_day(day, "from")).transpose()?.unwrap_or(clock.today());
                let to = to.as_deref().map(|day| parse_day(day, "to")).transpose()?.unwrap_or(from);
                day_range(from, to)?
            }
        };
        days.into_iter().flat_map(|day| day_tasks(snapshot, day, clock.now)).collect()
    };
    let tasks = candidates
        .iter()
        .filter(|(task, _)| quest.is_none_or(|quest| task.quest_id == Some(quest)))
        .filter(|(task, _)| match input.status {
            DoneFilter::Open => !task.is_done(),
            DoneFilter::Done => task.is_done(),
            DoneFilter::All => true,
        })
        .map(|(task, planned)| task_out(snapshot, task, *planned, clock))
        .collect();
    Ok(TasksOut { time_zone: clock.zone_name(), tasks })
}

pub fn get_task(snapshot: &Snapshot, task_id: &str, clock: &Clock) -> Result<TaskResult, ToolError> {
    let (task, planned) = require_task(snapshot, parse_id(task_id, "task_id")?, clock)?;
    Ok(TaskResult { time_zone: clock.zone_name(), task: task_out(snapshot, &task, planned, clock) })
}

pub fn get_checklist(snapshot: &Snapshot, task_id: &str, clock: &Clock) -> Result<ChecklistOut, ToolError> {
    let (task, _) = require_task(snapshot, parse_id(task_id, "task_id")?, clock)?;
    Ok(checklist_out(snapshot, &task))
}

pub fn list_quests(snapshot: &Snapshot, input: &ListQuestsInput, clock: &Clock) -> QuestsOut {
    let quests = snapshot
        .quests
        .iter()
        .filter(|quest| match input.status {
            QuestFilter::Active => quest.status == QuestStatus::Active,
            QuestFilter::Completed => quest.status == QuestStatus::Completed,
            QuestFilter::All => true,
        })
        .map(|quest| quest_out(snapshot, quest, clock.now))
        .collect();
    QuestsOut { quests }
}

pub fn get_quest(snapshot: &Snapshot, quest_id: &str, clock: &Clock) -> Result<QuestDetailOut, ToolError> {
    let quest = require_quest(snapshot, parse_id(quest_id, "quest_id")?)?;
    let mine = || snapshot.tasks.iter().filter(|task| task.quest_id == Some(quest.id));
    let mut open: Vec<_> = mine().filter(|task| !task.is_done()).collect();
    // By plan date, unplanned last; the snapshot's order (sort keys) within a day.
    open.sort_by_key(|task| (task.planned_on.is_none(), task.planned_on));
    let mut done: Vec<_> = mine().filter(|task| task.is_done()).collect();
    done.sort_by_key(|task| std::cmp::Reverse(task.completed_at));
    done.truncate(RECENT_DONE);
    Ok(QuestDetailOut {
        time_zone: clock.zone_name(),
        quest: quest_out(snapshot, quest, clock.now),
        tasks: open.into_iter().chain(done).map(|task| task_out(snapshot, task, false, clock)).collect(),
    })
}

pub fn search(snapshot: &Snapshot, input: &SearchInput, clock: &Clock) -> Result<SearchOut, ToolError> {
    let words: Vec<String> = input.query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return Err(ToolError::invalid("query is empty; give one or more words"));
    }
    let hit = |texts: &[&str]| {
        let text = texts.join("\n").to_lowercase();
        words.iter().all(|word| text.contains(word.as_str()))
    };
    let quests = snapshot
        .quests
        .iter()
        .filter(|quest| hit(&[&quest.title]))
        .take(MAX_HITS)
        .map(|quest| quest_out(snapshot, quest, clock.now))
        .collect();
    let mut tasks: Vec<_> = snapshot.tasks.iter().filter(|task| hit(&[&task.title, &task.description])).collect();
    tasks.sort_by_key(|task| (task.is_done(), std::cmp::Reverse(task.completed_at)));
    let tasks: Vec<TaskOut> =
        tasks.into_iter().take(MAX_HITS).map(|task| task_out(snapshot, task, false, clock)).collect();
    let archived = if input.include_archived {
        all_archived(snapshot, clock)
            .filter(|item| hit(&[item.title.as_deref().unwrap_or_default()]))
            .take(MAX_HITS)
            .collect()
    } else {
        Vec::new()
    };
    Ok(SearchOut { time_zone: clock.zone_name(), quests, tasks, archived })
}

pub fn week_review(snapshot: &Snapshot, input: &WeekInput, clock: &Clock) -> Result<WeekReviewOut, ToolError> {
    let any_day = input.week_of.as_deref().map(|day| parse_day(day, "week_of")).transpose()?.unwrap_or(clock.today());
    let review = WeeklyReview::build(snapshot, any_day, &clock.zone, clock.now);
    let minutes = |ms: i64| ms / 60_000;
    let days = review
        .quest_ms_per_day
        .iter()
        .enumerate()
        .map(|(offset, ms)| DayMinutesOut {
            day: (review.week_start + Days::new(offset as u64)).to_string(),
            quest_minutes: minutes(*ms),
        })
        .collect();
    let quests = review
        .by_quest
        .iter()
        .filter_map(|hours| {
            let quest = snapshot.quest(hours.quest_id)?;
            Some(QuestMinutesOut {
                quest: RefOut { id: quest.id.to_string(), title: quest.title.clone() },
                minutes: minutes(hours.ms),
            })
        })
        .collect();
    Ok(WeekReviewOut {
        time_zone: clock.zone_name(),
        week_start: review.week_start.to_string(),
        quest_minutes: minutes(review.quest_ms),
        organizing_minutes: minutes(review.organizing_ms),
        other_minutes: minutes(review.other_ms),
        target_minutes: minutes(review.target_ms),
        quest_share_percent: review.quest_share_percent(),
        days,
        quests,
    })
}

/// The running session; `snapshot` should be of the workspace it runs in (`Scope::Timer`).
pub fn running_timer(snapshot: &Snapshot, clock: &Clock) -> TimerOut {
    let running = snapshot.running_focus();
    TimerOut {
        time_zone: clock.zone_name(),
        running: running.map(|running| entry_out(snapshot, &running.entry, clock)),
        workspace: running.and_then(|running| snapshot.workspace(running.workspace)).map(workspace_ref),
    }
}

pub fn list_entries(snapshot: &Snapshot, input: &ListEntriesInput, clock: &Clock) -> Result<EntriesOut, ToolError> {
    let from = input.from.as_deref().map(|day| parse_day(day, "from")).transpose()?.unwrap_or(clock.today());
    let to = input.to.as_deref().map(|day| parse_day(day, "to")).transpose()?.unwrap_or(from);
    day_range(from, to)?;
    let task = input.task_id.as_deref().map(|id| parse_id(id, "task_id")).transpose()?;
    let quest = input.quest_id.as_deref().map(|id| parse_id(id, "quest_id")).transpose()?;
    if let Some(quest) = quest {
        require_quest(snapshot, quest)?;
    }
    let (start, end) = day_bounds(clock, from, to);
    let entries: Vec<_> = snapshot
        .time_entries
        .iter()
        .filter(|entry| overlap_ms(entry, start, end, clock.now) > 0)
        .filter(|entry| task.is_none_or(|task| entry.task_id == Some(task)))
        .filter(|entry| quest.is_none_or(|quest| snapshot.entry_quest(entry) == Some(quest)))
        .collect();
    let total_ms: i64 = entries.iter().map(|entry| overlap_ms(entry, start, end, clock.now)).sum();
    Ok(EntriesOut {
        time_zone: clock.zone_name(),
        entries: entries.into_iter().map(|entry| entry_out(snapshot, entry, clock)).collect(),
        total_minutes: total_ms / 60_000,
    })
}
