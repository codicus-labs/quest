//! The Quests page and a quest's page, pure: the page's figures and tabs, the colour a new quest
//! gets, the time tracked on a quest, and how its tasks are grouped.

#[cfg(test)]
mod tests;

use chrono::{DateTime, Days, NaiveDate, Utc};
use quest_domain::{EntityId, Quest, QuestColor, QuestStatus, Snapshot, Task};

/// Which quests the Quests page lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QuestsTab {
    #[default]
    Active,
    Completed,
}

impl QuestsTab {
    fn status(self) -> QuestStatus {
        match self {
            QuestsTab::Active => QuestStatus::Active,
            QuestsTab::Completed => QuestStatus::Completed,
        }
    }
}

/// The figures at the top of the Quests page. Archived quests (put away, also those an older
/// version "completed") count in neither tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Overview {
    /// Quest Hours ever, of every quest that exists.
    pub all_time_ms: i64,
    pub in_progress: usize,
    pub completed: usize,
}

pub fn overview(snapshot: &Snapshot, now: DateTime<Utc>) -> Overview {
    Overview {
        all_time_ms: snapshot.work_between(DateTime::<Utc>::MIN_UTC, DateTime::<Utc>::MAX_UTC, now).quest_ms,
        in_progress: snapshot.active_quests().count(),
        completed: snapshot.completed_quests().count(),
    }
}

/// The quests `tab` lists, in their order.
pub fn listed(snapshot: &Snapshot, tab: QuestsTab) -> impl Iterator<Item = &Quest> {
    snapshot.quests.iter().filter(move |q| q.status == tab.status())
}

/// A figure as the page shows it, with two digits at least: "01", "12".
pub fn count_label(count: usize) -> String {
    format!("{count:02}")
}

/// The first colour no quest uses yet, else blue.
pub fn new_quest_color(snapshot: &Snapshot) -> QuestColor {
    let used: Vec<QuestColor> = snapshot.quests.iter().map(|q| q.color).collect();
    QuestColor::ALL.into_iter().find(|c| !used.contains(c)).unwrap_or(QuestColor::Blue)
}

/// Time tracked on `quest`'s work within `[from, to)`; a running session counts up to `now`.
pub fn quest_ms(
    snapshot: &Snapshot,
    quest: EntityId,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    now: DateTime<Utc>,
) -> i64 {
    snapshot
        .time_entries
        .iter()
        .filter(|e| e.quest_id == Some(quest))
        .map(|e| {
            let start = e.started_at.max(from);
            let end = e.ended_at.unwrap_or(now).min(to);
            (end - start).num_milliseconds().max(0)
        })
        .sum()
}

/// Time tracked on `quest`'s work ever.
pub fn total_ms(snapshot: &Snapshot, quest: EntityId, now: DateTime<Utc>) -> i64 {
    quest_ms(snapshot, quest, DateTime::<Utc>::MIN_UTC, DateTime::<Utc>::MAX_UTC, now)
}

/// The quest's tasks in display order: open ones by day, then the backlog, then what's done
/// (latest first). Empty groups are left out.
pub fn task_groups(snapshot: &Snapshot, quest: EntityId, today: NaiveDate) -> Vec<(String, Vec<&Task>)> {
    let mine = || snapshot.tasks.iter().filter(move |t| t.quest_id == Some(quest));
    let mut days: Vec<NaiveDate> = mine().filter(|t| !t.is_done()).filter_map(|t| t.day).collect();
    days.sort();
    days.dedup();
    let mut out: Vec<(String, Vec<&Task>)> = days
        .into_iter()
        .map(|day| (day_label(day, today), mine().filter(|t| !t.is_done() && t.day == Some(day)).collect()))
        .collect();
    let backlog: Vec<&Task> = mine().filter(|t| !t.is_done() && t.day.is_none()).collect();
    if !backlog.is_empty() {
        out.push(("Backlog".to_owned(), backlog));
    }
    let mut done: Vec<&Task> = mine().filter(|t| t.is_done()).collect();
    done.sort_by_key(|t| std::cmp::Reverse(t.completed_at));
    if !done.is_empty() {
        out.push(("Done".to_owned(), done));
    }
    out
}

/// "Today", "Tomorrow", "Yesterday", else "Mon, Nov 30".
pub fn day_label(day: NaiveDate, today: NaiveDate) -> String {
    match day {
        d if d == today => "Today".to_owned(),
        d if d == today + Days::new(1) => "Tomorrow".to_owned(),
        d if d + Days::new(1) == today => "Yesterday".to_owned(),
        d => d.format("%a, %b %-d").to_string(),
    }
}

/// A colour's name, for labels and accessibility.
pub fn color_name(color: QuestColor) -> &'static str {
    match color {
        QuestColor::Red => "Red",
        QuestColor::Orange => "Orange",
        QuestColor::Yellow => "Yellow",
        QuestColor::Lime => "Lime",
        QuestColor::Green => "Green",
        QuestColor::Teal => "Teal",
        QuestColor::Blue => "Blue",
        QuestColor::Violet => "Violet",
        QuestColor::Pink => "Pink",
    }
}
