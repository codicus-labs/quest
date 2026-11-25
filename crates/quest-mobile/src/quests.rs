//! The Quests page, the quest editor and a quest's page, pure: cards, the detail record and the
//! rules its fields save by. Figures, tabs, hours and groups come from `quest_view::quests`
//! (shared with the desktop).

use chrono::Days;
use quest_domain::{EntityId, Milestone, Quest, QuestColor, QuestStatus, Snapshot, week_start};
use quest_view::format;
use quest_view::home::one_line;
use quest_view::quests::{
    QuestsTab, color_name, count_label, listed, new_quest_color, overview, quest_ms, task_groups, total_ms,
};

use crate::error::MobileError;
use crate::home::{color_hex, home_task};
use crate::quest_types::{HueOption, MilestoneRow, QuestCard, QuestDetail, QuestHue, QuestTaskGroup, QuestsData};
use crate::time::Now;
use crate::types::UndoToast;

/// The Quests page, by the desktop's rules (`quest_view::quests`): figures, tabs and cards.
pub(crate) fn quests(snapshot: &Snapshot, now: Now, toast: Option<UndoToast>) -> QuestsData {
    let cards = |tab: QuestsTab| -> Vec<QuestCard> { listed(snapshot, tab).map(|q| card(snapshot, q, now)).collect() };
    let figures = overview(snapshot, now.at);
    QuestsData {
        all_time_hours: format::hours(figures.all_time_ms),
        in_progress: count_label(figures.in_progress),
        completed_count: count_label(figures.completed),
        active: cards(QuestsTab::Active),
        completed: cards(QuestsTab::Completed),
        active_label: format!("Active · {}", figures.in_progress),
        completed_label: format!("Completed · {}", figures.completed),
        new_quest_hues: hue_options(new_quest_color(snapshot)),
        toast,
    }
}

/// Colour, name, open tasks, hours this week and in total (the desktop's card).
fn card(snapshot: &Snapshot, quest: &Quest, now: Now) -> QuestCard {
    let week = format::hours(week_ms(snapshot, quest.id, now));
    let total = format!("{} total", format::hours(total_ms(snapshot, quest.id, now.at)));
    let open_tasks = open_tasks(snapshot, quest.id);
    QuestCard {
        id: quest.id.to_string(),
        title: quest.title.clone(),
        color_hex: color_hex(quest.color).to_owned(),
        a11y_label: format!("{}, {week} this week, {total}, {open_tasks}", quest.title),
        week_hours: week,
        total_hours: total,
        open_tasks,
    }
}

/// "2 open tasks" / "1 open task".
fn open_tasks(snapshot: &Snapshot, quest: EntityId) -> String {
    let open = snapshot.tasks.iter().filter(|t| t.quest_id == Some(quest) && !t.is_done()).count();
    format!("{open} open {}", if open == 1 { "task" } else { "tasks" })
}

fn week_ms(snapshot: &Snapshot, quest: EntityId, now: Now) -> i64 {
    let monday = week_start(now.today());
    quest_ms(snapshot, quest, now.midnight(monday), now.midnight(monday + Days::new(7)), now.at)
}

fn milestone_row(milestone: &Milestone) -> MilestoneRow {
    let date_label = milestone.achieved_on.format("%b %-d, %Y").to_string();
    MilestoneRow {
        id: milestone.id.to_string(),
        title: milestone.title.clone(),
        a11y_label: format!("{}, {date_label}", milestone.title),
        date_label,
    }
}

/// All nine colours, `selected` selected.
fn hue_options(selected: QuestColor) -> Vec<HueOption> {
    QuestColor::ALL
        .into_iter()
        .map(|color| HueOption {
            hue: hue(color),
            name: color_name(color).to_owned(),
            color_hex: color_hex(color).to_owned(),
            selected: color == selected,
        })
        .collect()
}

/// What the status button does: Active → Completed; Completed or Archived → Active.
pub(crate) fn next_status(status: QuestStatus) -> QuestStatus {
    match status {
        QuestStatus::Active => QuestStatus::Completed,
        QuestStatus::Completed | QuestStatus::Archived => QuestStatus::Active,
    }
}

pub(crate) fn quest_detail(snapshot: &Snapshot, quest: &Quest, now: Now, toast: Option<UndoToast>) -> QuestDetail {
    let id = quest.id;
    let mine = || snapshot.tasks.iter().filter(move |t| t.quest_id == Some(id));
    let groups = task_groups(snapshot, id, now.today());
    let week_hours = format::hours(week_ms(snapshot, id, now));
    let total_hours = format::hours(total_ms(snapshot, id, now.at));
    let (status_action, badge) = match quest.status {
        QuestStatus::Active => ("Complete quest", None),
        QuestStatus::Completed => ("Reopen quest", Some("Completed")),
        QuestStatus::Archived => ("Unarchive", Some("Archived")),
    };
    QuestDetail {
        id: id.to_string(),
        title: quest.title.clone(),
        description: quest.description.clone(),
        color_hex: color_hex(quest.color).to_owned(),
        hues: hue_options(quest.color),
        completed: quest.status == QuestStatus::Completed,
        status_action: status_action.to_owned(),
        badge: badge.map(str::to_owned),
        summary: format!("{week_hours} this week · {total_hours} total · {}", open_tasks(snapshot, id)),
        week_hours,
        total_hours,
        open_count: count(mine().filter(|t| !t.is_done()).count()),
        done_count: count(mine().filter(|t| t.is_done()).count()),
        task_groups: groups
            .into_iter()
            .map(|(label, tasks)| QuestTaskGroup {
                label,
                tasks: tasks.into_iter().map(|t| home_task(snapshot, t, now.at)).collect(),
            })
            .collect(),
        milestones: snapshot.milestones_of(id).collect::<Vec<_>>().into_iter().rev().map(milestone_row).collect(),
        toast,
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// A typed title on one line, trimmed; none when it is empty or what `current` already is.
pub(crate) fn retitle(current: &str, typed: &str) -> Option<String> {
    let title = one_line(typed);
    let title = title.trim();
    (!title.is_empty() && title != current).then(|| title.to_owned())
}

/// A trimmed, non-empty title for something new.
pub(crate) fn new_title(typed: &str) -> Result<String, MobileError> {
    let title = one_line(typed).trim().to_owned();
    if title.is_empty() { Err(MobileError::EmptyTitle) } else { Ok(title) }
}

pub(crate) fn hue(color: QuestColor) -> QuestHue {
    match color {
        QuestColor::Red => QuestHue::Red,
        QuestColor::Orange => QuestHue::Orange,
        QuestColor::Yellow => QuestHue::Yellow,
        QuestColor::Lime => QuestHue::Lime,
        QuestColor::Green => QuestHue::Green,
        QuestColor::Teal => QuestHue::Teal,
        QuestColor::Blue => QuestHue::Blue,
        QuestColor::Violet => QuestHue::Violet,
        QuestColor::Pink => QuestHue::Pink,
    }
}

pub(crate) fn color(hue: QuestHue) -> QuestColor {
    match hue {
        QuestHue::Red => QuestColor::Red,
        QuestHue::Orange => QuestColor::Orange,
        QuestHue::Yellow => QuestColor::Yellow,
        QuestHue::Lime => QuestColor::Lime,
        QuestHue::Green => QuestColor::Green,
        QuestHue::Teal => QuestColor::Teal,
        QuestHue::Blue => QuestColor::Blue,
        QuestHue::Violet => QuestColor::Violet,
        QuestHue::Pink => QuestColor::Pink,
    }
}

#[cfg(test)]
#[path = "quests_tests.rs"]
mod tests;
