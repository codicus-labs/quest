use quest_domain::{Milestone, QuestColor, QuestStatus, Snapshot};

use super::*;
use crate::test_support::{at, berlin, day, entry, id, quest, task};

/// Wednesday, 25 Nov 2026, 10:00 in Berlin.
fn now() -> Now {
    berlin("2026-11-25T10:00:00+01:00")
}

fn milestone(n: u128, quest: u128, title: &str, on: &str) -> Milestone {
    Milestone { id: id(n), quest_id: id(quest), title: title.into(), achieved_on: day(on) }
}

fn snapshot() -> Snapshot {
    let mut done = task(12, "Print", Some(1), Some("2026-11-20"));
    done.completed_at = Some(at("2026-11-20T12:00:00Z"));
    let completed = Quest { status: QuestStatus::Completed, color: QuestColor::Green, ..quest(2, "Garden") };
    Snapshot {
        quests: vec![quest(1, "Photo series"), completed],
        tasks: vec![
            task(10, "Pick motifs", Some(1), Some("2026-11-25")),
            task(11, "Scout", Some(1), None),
            done,
            task(13, "Mail", None, Some("2026-11-25")),
        ],
        milestones: vec![
            milestone(30, 1, "First roll", "2026-11-01"),
            milestone(31, 1, "Darkroom", "2026-11-10"),
            milestone(32, 1, "Contact sheet", "2026-11-20"),
            milestone(33, 1, "Show", "2026-11-24"),
        ],
        time_entries: vec![
            // This week (Monday 23 Nov): 1 h; three weeks ago: 2 h; the other quest 30 min.
            entry(20, Some(10), Some(1), "2026-11-24T09:00:00+01:00", Some("2026-11-24T10:00:00+01:00")),
            entry(21, Some(11), Some(1), "2026-11-03T09:00:00+01:00", Some("2026-11-03T11:00:00+01:00")),
            entry(22, None, Some(2), "2026-11-24T12:00:00+01:00", Some("2026-11-24T12:30:00+01:00")),
            entry(23, Some(13), None, "2026-11-24T13:00:00+01:00", Some("2026-11-24T14:00:00+01:00")),
        ],
        ..Snapshot::default()
    }
}

#[test]
fn the_page_splits_active_and_completed_quests_with_their_hours() {
    let data = quests(&snapshot(), now(), None);
    assert_eq!(data.all_time_hours, "3.5h", "quest work only, of quests that exist");
    assert_eq!((data.in_progress.as_str(), data.completed_count.as_str()), ("01", "01"));
    assert_eq!((data.active_label.as_str(), data.completed_label.as_str()), ("Active · 1", "Completed · 1"));
    let card = &data.active[0];
    assert_eq!(
        (card.title.as_str(), card.week_hours.as_str(), card.total_hours.as_str(), card.open_tasks.as_str()),
        ("Photo series", "1h", "3h total", "2 open tasks")
    );
    assert_eq!(card.a11y_label, "Photo series, 1h this week, 3h total, 2 open tasks");
    assert_eq!(data.completed[0].title, "Garden");
    assert_eq!(data.completed[0].open_tasks, "0 open tasks");
    let new = data.new_quest_hues.iter().filter(|h| h.selected).map(|h| h.hue).collect::<Vec<_>>();
    assert_eq!(new, [QuestHue::Red], "the first colour no quest uses");
}

#[test]
fn a_quest_archived_by_an_older_version_is_not_counted_as_completed() {
    let mut snap = snapshot();
    snap.quests.push(Quest { status: QuestStatus::Archived, ..quest(3, "Old archive") });
    let data = quests(&snap, now(), None);
    assert_eq!((data.active_label.as_str(), data.completed_label.as_str()), ("Active · 1", "Completed · 1"));
    assert_eq!(data.completed_count, "01");
    assert!(data.completed.iter().chain(&data.active).all(|c| c.title != "Old archive"));
}

#[test]
fn the_detail_has_stats_groups_read_only_milestones_and_colours() {
    let snap = snapshot();
    let detail = quest_detail(&snap, &snap.quests[0], now(), None);
    assert_eq!((detail.week_hours.as_str(), detail.total_hours.as_str()), ("1h", "3h"));
    assert_eq!((detail.open_count, detail.done_count), (2, 1));
    assert_eq!((detail.completed, detail.status_action.as_str(), detail.badge), (false, "Complete quest", None));
    assert_eq!(detail.summary, "1h this week · 3h total · 2 open tasks");
    let groups: Vec<(&str, usize)> = detail.task_groups.iter().map(|g| (g.label.as_str(), g.tasks.len())).collect();
    assert_eq!(groups, [("Today", 1), ("Backlog", 1), ("Done", 1)]);
    assert_eq!(detail.task_groups[0].tasks[0].tracked.as_deref(), Some("1:00:00"));
    let milestones: Vec<&str> = detail.milestones.iter().map(|m| m.title.as_str()).collect();
    assert_eq!(milestones, ["Show", "Contact sheet", "Darkroom", "First roll"], "newest first");
    assert_eq!(detail.milestones[0].date_label, "Nov 24, 2026");
    assert_eq!(detail.milestones[0].a11y_label, "Show, Nov 24, 2026");
    assert_eq!(detail.hues.len(), 9);
    assert_eq!(detail.hues.iter().filter(|h| h.selected).map(|h| h.hue).collect::<Vec<_>>(), [QuestHue::Blue]);

    let completed = quest_detail(&snap, &snap.quests[1], now(), None);
    assert_eq!((completed.completed, completed.status_action.as_str()), (true, "Reopen quest"));
    assert_eq!(completed.badge.as_deref(), Some("Completed"));
    assert!(completed.milestones.is_empty(), "no milestones: the page leaves the section out");
}

#[test]
fn the_status_button_completes_active_and_reopens_completed_or_archived_quests() {
    assert_eq!(next_status(QuestStatus::Active), QuestStatus::Completed);
    assert_eq!(next_status(QuestStatus::Completed), QuestStatus::Active);
    assert_eq!(next_status(QuestStatus::Archived), QuestStatus::Active);
}

#[test]
fn titles_are_trimmed_to_one_line_and_unchanged_ones_not_saved() {
    assert_eq!(retitle("Old", " New\nname "), Some("New name".into()));
    assert_eq!(retitle("Old", " Old "), None);
    assert_eq!(retitle("Old", "  "), None);
    assert_eq!(new_title("  "), Err(MobileError::EmptyTitle));
    assert_eq!(new_title(" Garden "), Ok("Garden".into()));
}

#[test]
fn hues_map_both_ways() {
    for c in QuestColor::ALL {
        assert_eq!(color(hue(c)), c);
    }
}
