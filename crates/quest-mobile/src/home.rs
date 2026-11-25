//! Home: today's QUEST WORK and OTHER lists and Quest Hours, mapped from the snapshot as the
//! desktop's Home shows them. Pure.

use chrono::{DateTime, Days, NaiveDate, Utc};
use quest_domain::{EntityId, Quest, QuestColor, Snapshot, Task, overdue, week_start};
use quest_view::format;
use quest_view::home::new_task_quest;

use crate::error::MobileError;
use crate::ids::{iso_day, parse_id};
use crate::task_detail::{day_value, postponed};
use crate::time::Now;
use crate::types::{HomeData, HomeQuery, HomeTask, QuestChip, TaskSection, UndoToast};

/// `HomeQuery` with its ids parsed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Choice {
    pub filter: Option<EntityId>,
    pub pick: Option<EntityId>,
}

impl Choice {
    pub fn parse(query: &HomeQuery) -> Result<Choice, MobileError> {
        let parse = |id: &Option<String>| id.as_deref().map(parse_id).transpose();
        Ok(Choice { filter: parse(&query.quest_filter)?, pick: parse(&query.quest_pick)? })
    }

    /// Without a filter on a quest that no longer exists (deleted, or never synced here): QUEST
    /// WORK would stay empty with no chip to clear it. An archived quest still filters, as on the
    /// desktop.
    pub fn known_in(self, snapshot: &Snapshot) -> Choice {
        Choice { filter: self.filter.filter(|id| snapshot.quest(*id).is_some()), ..self }
    }

    /// The quest a new task in `section` gets (the desktop's `State::home_quest` for quest work).
    pub fn quest_for(self, snapshot: &Snapshot, section: TaskSection) -> Option<EntityId> {
        match section {
            TaskSection::QuestWork => new_task_quest(snapshot, self.filter, self.pick),
            TaskSection::Other => None,
        }
    }
}

pub(crate) fn home(snapshot: &Snapshot, now: Now, choice: Choice, toast: Option<UndoToast>) -> HomeData {
    let choice = choice.known_in(snapshot);
    let today = now.today();
    let tomorrow = today + Days::new(1);
    let monday = week_start(today);
    let today_ms = snapshot.work_between(now.midnight(today), now.midnight(tomorrow), now.at).quest_ms;
    let week_ms = snapshot.work_between(now.midnight(monday), now.midnight(monday + Days::new(7)), now.at).quest_ms;
    let target_ms = i64::from(snapshot.settings.weekly_target_minutes) * 60_000;
    let list =
        |section| list(snapshot, now, choice, section).into_iter().map(|t| home_task(snapshot, t, now.at, today));
    HomeData {
        day_label: format::long_day(today),
        day_iso: iso_day(today),
        today_hours: format::hours(today_ms),
        week_hours: format::hours(week_ms),
        weekly_target: format::hours(target_ms),
        week_progress: (week_ms as f32 / target_ms.max(1) as f32).clamp(0.0, 1.0),
        quest_work: list(TaskSection::QuestWork).collect(),
        other: list(TaskSection::Other).collect(),
        quest_filter: choice.filter.and_then(|id| snapshot.quest(id)).map(chip),
        add_quest: match choice.filter {
            Some(_) => None,
            None => choice.quest_for(snapshot, TaskSection::QuestWork).and_then(|id| snapshot.quest(id)).map(chip),
        },
        quests: quest_options(snapshot, now.at),
        toast,
    }
}

/// Today's tasks of one section, as the desktop lists them: quest work only for the filtered
/// quest; open ones first, done ones after, each in list order.
fn list(snapshot: &Snapshot, now: Now, choice: Choice, section: TaskSection) -> Vec<&Task> {
    let shown = |task: &&Task| {
        section_of(snapshot, task) == section
            && (section == TaskSection::Other || choice.filter.is_none_or(|q| task.quest_id == Some(q)))
    };
    let (open, done): (Vec<&Task>, Vec<&Task>) =
        snapshot.tasks_on(now.today()).filter(shown).partition(|t| !t.is_done());
    open.into_iter().chain(done).collect()
}

/// QUEST WORK when the task has a quest that exists, else OTHER (desktop `State::section_of`).
pub(crate) fn section_of(snapshot: &Snapshot, task: &Task) -> TaskSection {
    if snapshot.is_quest_work(task) { TaskSection::QuestWork } else { TaskSection::Other }
}

pub(crate) fn home_task(snapshot: &Snapshot, task: &Task, now: DateTime<Utc>, today: NaiveDate) -> HomeTask {
    let running = snapshot.running_entry().is_some_and(|e| e.task_id == Some(task.id));
    let tracked_ms = snapshot.task_tracked_ms(task.id, now);
    HomeTask {
        id: task.id.to_string(),
        title: task.title.clone(),
        done: task.is_done(),
        quest: task.quest_id.and_then(|id| snapshot.quest(id)).map(chip),
        tracked: (running || tracked_ms > 0).then(|| format::timer(tracked_ms)),
        timer_running: running,
        can_start_timer: !task.is_done(),
        planned_on: task.planned_on.map(day_value),
        due_on: task.due_on.map(day_value),
        overdue: overdue(task, today),
        postponed: postponed(snapshot, task.id),
    }
}

/// The active quests to choose from, most recently used first (`quest_view::quest_picker`).
pub(crate) fn quest_options(snapshot: &Snapshot, at: DateTime<Utc>) -> Vec<QuestChip> {
    quest_view::quest_picker::recent_quests(snapshot, at).into_iter().map(chip).collect()
}

pub(crate) fn chip(quest: &Quest) -> QuestChip {
    QuestChip { id: quest.id.to_string(), name: quest.title.clone(), color_hex: color_hex(quest.color).to_owned() }
}

/// Same palette as the desktop client.
pub(crate) fn color_hex(color: QuestColor) -> &'static str {
    match color {
        QuestColor::Red => "#d9625c",
        QuestColor::Orange => "#d98a4e",
        QuestColor::Yellow => "#d9b84a",
        QuestColor::Lime => "#b3c94c",
        QuestColor::Green => "#62b87a",
        QuestColor::Teal => "#4cb5a8",
        QuestColor::Blue => "#5c8fd9",
        QuestColor::Violet => "#906bd9",
        QuestColor::Pink => "#d96ba8",
    }
}

#[cfg(test)]
#[path = "home_tests.rs"]
mod tests;
