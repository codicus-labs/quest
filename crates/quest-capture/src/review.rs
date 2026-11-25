//! Reviewing a proposal before it is applied, pure and shared by the clients: which items are
//! selected, what applying keeps, and how items read on screen.

use std::collections::BTreeSet;

use chrono::{NaiveDate, NaiveTime, Timelike};
use quest_domain::{EntityId, QuestColor, Snapshot};
use quest_protocol::{CaptureProposal, DropReason, Dropped, ProposedTask, ProposedWork, QuestRef, TempKey, WorkTarget};
use quest_view::format::hh_mm;
use quest_view::quests::day_label;

/// A proposal under review: everything starts selected.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub proposal: CaptureProposal,
    /// Keys of the quests, tasks and sessions the user deselected (keys are unique across all lists).
    pub deselected: BTreeSet<TempKey>,
}

impl Draft {
    pub fn new(proposal: CaptureProposal) -> Draft {
        Draft { proposal, deselected: BTreeSet::new() }
    }

    pub fn is_selected(&self, key: &TempKey) -> bool {
        !self.deselected.contains(key)
    }

    pub fn set_selected(&mut self, key: &TempKey, selected: bool) {
        if selected {
            self.deselected.remove(key);
        } else {
            self.deselected.insert(key.clone());
        }
    }

    /// How many items applying creates or logs.
    pub fn selected_count(&self) -> usize {
        let p = &self.proposal;
        let keys =
            p.quests.iter().map(|q| &q.key).chain(p.tasks.iter().map(|t| &t.key)).chain(p.work.iter().map(|w| &w.key));
        keys.filter(|key| self.is_selected(key)).count()
    }

    /// The quest an item will get: a deselected new quest is none.
    pub fn effective_quest<'a>(&self, quest: &'a Option<QuestRef>) -> Option<&'a QuestRef> {
        quest.as_ref().filter(|quest| match quest {
            QuestRef::Existing { .. } => true,
            QuestRef::New { key } => self.is_selected(key) && self.proposal.quests.iter().any(|q| q.key == *key),
        })
    }

    /// The proposal to apply: the selected items only, without references to deselected quests.
    pub fn confirmed(&self) -> CaptureProposal {
        let p = &self.proposal;
        let quest = |quest: &Option<QuestRef>| self.effective_quest(quest).cloned();
        CaptureProposal {
            quests: p.quests.iter().filter(|q| self.is_selected(&q.key)).cloned().collect(),
            tasks: p
                .tasks
                .iter()
                .filter(|t| self.is_selected(&t.key))
                .map(|t| ProposedTask { quest: quest(&t.quest), ..t.clone() })
                .collect(),
            work: p
                .work
                .iter()
                .filter(|w| self.is_selected(&w.key))
                .map(|w| match &w.task {
                    WorkTarget::New { title, quest: q } => {
                        ProposedWork { task: WorkTarget::New { title: title.clone(), quest: quest(q) }, ..w.clone() }
                    }
                    WorkTarget::Existing { .. } => w.clone(),
                })
                .collect(),
            dropped: Vec::new(),
        }
    }

    /// What is wrong with a selected task, in the words the dialogs use; `None` if it can be created.
    pub fn task_error(&self, task: &ProposedTask, snapshot: &Snapshot) -> Option<&'static str> {
        if task.title.trim().is_empty() {
            return Some("Type a title.");
        }
        if let Some(QuestRef::Existing { id }) = &task.quest
            && snapshot.quest(*id).is_none()
        {
            return Some("This quest no longer exists. Pick another one.");
        }
        None
    }

    /// What is wrong with a selected session; `None` if it can be logged.
    pub fn work_error(&self, work: &ProposedWork, snapshot: &Snapshot) -> Option<&'static str> {
        match &work.task {
            WorkTarget::Existing { id } if snapshot.task(*id).is_none() => {
                return Some("This task no longer exists. Pick another one.");
            }
            WorkTarget::New { title, .. } if title.trim().is_empty() => return Some("Type a title."),
            WorkTarget::New { quest: Some(QuestRef::Existing { id }), .. } if snapshot.quest(*id).is_none() => {
                return Some("This quest no longer exists. Pick another one.");
            }
            _ => {}
        }
        if !crate::apply::is_time_range(work.start, work.end) {
            return Some("The end must be after the start.");
        }
        None
    }

    /// The colour each selected new quest will get (see [`crate::new_quest_colors`]).
    pub fn new_quest_colors(&self, snapshot: &Snapshot) -> Vec<(TempKey, QuestColor)> {
        let selected: Vec<&TempKey> =
            self.proposal.quests.iter().map(|q| &q.key).filter(|key| self.is_selected(key)).collect();
        let colors = crate::new_quest_colors(snapshot, selected.len());
        selected.into_iter().cloned().zip(colors).collect()
    }

    /// True if something is selected and every selected item can be applied.
    pub fn can_apply(&self, snapshot: &Snapshot) -> bool {
        let p = &self.proposal;
        let quests_ok = p.quests.iter().filter(|q| self.is_selected(&q.key)).all(|q| !q.title.trim().is_empty());
        let tasks_ok =
            p.tasks.iter().filter(|t| self.is_selected(&t.key)).all(|t| self.task_error(t, snapshot).is_none());
        let work_ok =
            p.work.iter().filter(|w| self.is_selected(&w.key)).all(|w| self.work_error(w, snapshot).is_none());
        self.selected_count() > 0 && quests_ok && tasks_ok && work_ok
    }
}

/// The title of the task a session is for.
pub fn work_task_title<'a>(target: &'a WorkTarget, snapshot: &'a Snapshot) -> &'a str {
    match target {
        WorkTarget::Existing { id } => snapshot.task(*id).map_or("", |t| t.title.as_str()),
        WorkTarget::New { title, .. } => title,
    }
}

/// The quest of an existing task a session is for.
pub fn work_task_quest(target: &WorkTarget, snapshot: &Snapshot) -> Option<EntityId> {
    match target {
        WorkTarget::Existing { id } => snapshot.task(*id).and_then(|t| t.quest_id),
        WorkTarget::New { .. } => None,
    }
}

/// "Tomorrow", "Tomorrow · due Fri, Nov 27", "Due Fri, Nov 27", "Someday".
pub fn task_when(planned_on: Option<NaiveDate>, due_on: Option<NaiveDate>, today: NaiveDate) -> String {
    match (planned_on, due_on) {
        (Some(plan), Some(due)) => format!("{} · due {}", day_label(plan, today), day_label(due, today)),
        (Some(plan), None) => day_label(plan, today),
        (None, Some(due)) => format!("Due {}", day_label(due, today)),
        (None, None) => "Someday".to_owned(),
    }
}

/// "Today · 09:00–11:00"; an end at midnight reads "24:00".
pub fn work_when(work: &ProposedWork, today: NaiveDate) -> String {
    format!("{} · {}–{}", day_label(work.day, today), hh_mm(minutes_of_day(work.start)), hh_mm(end_minutes(work.end)))
}

/// "2:00"; "0:00" if the end is not after the start.
pub fn work_duration(work: &ProposedWork) -> String {
    let minutes = end_minutes(work.end).saturating_sub(minutes_of_day(work.start));
    format!("{}:{:02}", minutes / 60, minutes % 60)
}

/// "Create (3)".
pub fn apply_label(count: usize) -> String {
    format!("Create ({count})")
}

/// "Created 1 quest, 2 tasks and 1 session".
pub fn created_message(quests: usize, tasks: usize, sessions: usize) -> String {
    let parts: Vec<String> = [(quests, "quest"), (tasks, "task"), (sessions, "session")]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, noun)| format!("{n} {noun}{}", if n == 1 { "" } else { "s" }))
        .collect();
    match parts.as_slice() {
        [] => "Nothing created".to_owned(),
        [one] => format!("Created {one}"),
        [rest @ .., last] => format!("Created {} and {last}", rest.join(", ")),
    }
}

/// "“Pay taxes” was left out: the date does not exist or is too far away."
pub fn dropped_message(dropped: &Dropped) -> String {
    let why = match dropped.reason {
        DropReason::Incomplete => "something was missing",
        DropReason::InvalidDate => "the date or time does not exist or is too far away",
        DropReason::InvalidTimeRange => "the end is not after the start, or it lies in the future",
        DropReason::TooLong => "it was too long, or there were too many items",
        DropReason::UnknownTask => "the task was not found",
        DropReason::UnknownQuest => "the quest was not found",
    };
    format!("“{}” was left out: {why}.", dropped.title)
}

/// Minutes since midnight.
pub fn minutes_of_day(time: NaiveTime) -> u32 {
    time.num_seconds_from_midnight() / 60
}

/// Like [`minutes_of_day`], with midnight at the end of the day as 1440.
pub fn end_minutes(end: NaiveTime) -> u32 {
    if end == NaiveTime::MIN { 24 * 60 } else { minutes_of_day(end) }
}

#[cfg(test)]
#[path = "review_tests.rs"]
mod tests;
