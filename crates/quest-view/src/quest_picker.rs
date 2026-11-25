//! The quest picker's order and filter, shared by every place that chooses a quest.

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use quest_domain::{EntityId, Quest, Snapshot};

/// The active quests, most recently used first, then the never used ones in the Quests page's
/// order. A quest is used by its newest session (a running one at `now`) and by the newest
/// creation or change of one of its tasks; a repeating task's generated instances count only by
/// their sessions (the app writes them, not the user). Equal times keep the Quests page's order.
pub fn recent_quests(snapshot: &Snapshot, now: DateTime<Utc>) -> Vec<&Quest> {
    let mut last_used: BTreeMap<EntityId, DateTime<Utc>> = BTreeMap::new();
    let mut use_at = |quest: EntityId, at: DateTime<Utc>| {
        let last = last_used.entry(quest).or_insert(at);
        *last = (*last).max(at);
    };
    for entry in &snapshot.time_entries {
        if let Some(quest) = snapshot.entry_quest(entry) {
            use_at(quest, entry.ended_at.unwrap_or(now).max(entry.started_at));
        }
    }
    for task in snapshot.tasks.iter().filter(|t| t.series.is_none()) {
        if let (Some(quest), Some(at)) = (task.quest_id, snapshot.task_written.get(&task.id)) {
            use_at(quest, *at);
        }
    }
    let mut quests: Vec<&Quest> = snapshot.active_quests().collect();
    // Stable: never used quests and equal times keep the page's order.
    quests.sort_by_key(|q| std::cmp::Reverse(last_used.get(&q.id).copied()));
    quests
}

/// Whether `title` contains `query`, ignoring case and the query's outer spaces. An empty
/// query matches every title.
pub fn matches(title: &str, query: &str) -> bool {
    let query = query.trim();
    query.is_empty() || title.to_lowercase().contains(&query.to_lowercase())
}

/// The quests among `quests` whose title matches `query`, in their order.
pub fn filter<'a>(quests: &[&'a Quest], query: &str) -> Vec<&'a Quest> {
    quests.iter().copied().filter(|q| matches(&q.title, query)).collect()
}
