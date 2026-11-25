//! The ⌘K command palette's entries for a query. Pure: the view lists them, `run` acts on one.
//! Besides commands, the palette picks a task for a new session, a day to move a task to, and a
//! time to schedule it at: the keyboard way to do what drag and drop does.

use chrono::{DateTime, Datelike, Days, NaiveDate, NaiveDateTime, Timelike, Utc};
use quest_domain::{EntityId, Snapshot, Workspace};

use crate::nav::View;
use crate::quest_picker;
use crate::timescale::SNAP_MINUTES;
use crate::undo::Deletable;

/// What the palette is choosing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Commands,
    /// A day (or the backlog) for a task.
    MoveTo(EntityId),
    /// A time today for a task's block.
    ScheduleAt(EntityId),
    /// A quest (or none) for a task.
    AssignQuest(EntityId),
    /// The quest Quest Work is filtered to (or all).
    FilterQuest,
    /// The workspace to show.
    SwitchWorkspace,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Go(View),
    CreateTask(String),
    CreateQuest(String),
    Focus(EntityId),
    ShowQuest(EntityId),
    ToggleTheme,
    Account,
    /// Switches the palette to another mode with an empty query.
    Choose(Mode),
    MoveTask {
        task: EntityId,
        day: Option<NaiveDate>,
    },
    /// Plans `task` on `day` at `start` (minutes after midnight).
    Schedule {
        task: EntityId,
        day: NaiveDate,
        start: i32,
    },
    AssignQuest {
        task: EntityId,
        quest: Option<EntityId>,
    },
    Delete(Deletable),
    LogWork,
    /// Opens the AI capture panel: free text structured into a proposal to confirm.
    CaptureWithAi,
    FilterQuest(Option<EntityId>),
    SwitchWorkspace(EntityId),
    ManageWorkspaces,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub title: String,
    /// A short tag on the right: "Jump", "Task", "Quest", the shortcut.
    pub hint: String,
    pub action: Action,
}

fn matches(haystack: &str, query: &str) -> bool {
    query.is_empty() || haystack.to_lowercase().contains(&query.to_lowercase())
}

const MAX_SEARCH_HITS: usize = 8;

/// Entries for `query` in `mode`; `now` is the local time (for "today" and upcoming times), `at`
/// the same moment in UTC (for which quests were used last).
pub fn entries(mode: Mode, query: &str, snapshot: &Snapshot, now: NaiveDateTime, at: DateTime<Utc>) -> Vec<Entry> {
    let query = query.trim();
    match mode {
        Mode::Commands => commands(query, snapshot),
        Mode::MoveTo(task) => move_to(task, query, now.date()),
        Mode::ScheduleAt(task) => schedule_at(task, query, now),
        Mode::AssignQuest(task) => assign_quest(task, query, snapshot, at),
        Mode::FilterQuest => filter_quest(query, snapshot),
        Mode::SwitchWorkspace => switch_workspace(query, snapshot),
    }
}

/// What the palette field says in `mode`.
pub fn placeholder(mode: Mode) -> &'static str {
    match mode {
        Mode::Commands => "Search tasks and quests, or jump to…",
        Mode::MoveTo(_) => "Move to which day?",
        Mode::ScheduleAt(_) => "Schedule at what time today? (e.g. 9:30)",
        Mode::AssignQuest(_) => "Which quest is this task for?",
        Mode::FilterQuest => "Show quest work for which quest?",
        Mode::SwitchWorkspace => "Switch to which workspace?",
    }
}

/// Create actions first when there is text, then jumps, open tasks to focus on (and to move or
/// schedule, when searching), and quests.
fn commands(query: &str, snapshot: &Snapshot) -> Vec<Entry> {
    let mut out = Vec::new();
    for (i, view) in View::ALL.iter().enumerate() {
        let title = format!("Go to {}", view.title());
        if matches(&title, query) {
            out.push(Entry { title, hint: format!("⌘{}", i + 1), action: Action::Go(*view) });
        }
    }
    let archive = "Archive · deleted and archived";
    if matches(archive, query) {
        out.push(Entry { title: archive.into(), hint: "Jump".into(), action: Action::Go(View::Archive) });
    }
    for (title, hint, action) in [
        ("Switch workspace…", "Workspace", Action::Choose(Mode::SwitchWorkspace)),
        ("Manage workspaces", "Workspace", Action::ManageWorkspaces),
        ("Log work", "Calendar", Action::LogWork),
        ("Capture with AI", "AI", Action::CaptureWithAi),
        ("Toggle light / dark theme", "Theme", Action::ToggleTheme),
        ("Account and sync", "⌘,", Action::Account),
    ] {
        if matches(title, query) {
            out.push(Entry { title: title.into(), hint: hint.into(), action });
        }
    }
    if !query.is_empty() {
        out.extend(other_workspaces(snapshot).filter(|w| matches(&w.name, query)).map(|w| Entry {
            title: format!("Switch to workspace “{}”", w.name),
            hint: "Workspace".into(),
            action: Action::SwitchWorkspace(w.id),
        }));
    }
    let tasks = snapshot.tasks.iter().filter(|t| !t.is_done() && matches(&t.title, query)).take(MAX_SEARCH_HITS);
    for t in tasks {
        out.push(Entry {
            title: format!("Focus on “{}”", t.title), hint: "Task".into(), action: Action::Focus(t.id)
        });
        if !query.is_empty() {
            out.push(Entry {
                title: format!("Move “{}” to…", t.title),
                hint: "⌥← ⌥→".into(),
                action: Action::Choose(Mode::MoveTo(t.id)),
            });
            out.push(Entry {
                title: format!("Schedule “{}” at…", t.title),
                hint: "Calendar".into(),
                action: Action::Choose(Mode::ScheduleAt(t.id)),
            });
            out.push(Entry {
                title: format!("Delete task “{}”", t.title),
                hint: "⌫".into(),
                action: Action::Delete(Deletable::Task(t.id)),
            });
        }
    }
    let quests = snapshot.active_quests().filter(|q| matches(&q.title, query)).take(MAX_SEARCH_HITS);
    out.extend(quests.map(|q| Entry { title: q.title.clone(), hint: "Quest".into(), action: Action::ShowQuest(q.id) }));
    // Matches come first so Enter jumps; with nothing matching, Enter creates.
    if !query.is_empty() {
        out.push(Entry {
            title: format!("Create task “{query}”"),
            hint: "Today".into(),
            action: Action::CreateTask(query.into()),
        });
        out.push(Entry {
            title: format!("Create quest “{query}”"),
            hint: "Quest".into(),
            action: Action::CreateQuest(query.into()),
        });
    }
    out
}

/// Where "Move to…" offers to move a task, labelled: today, tomorrow, the five days after, and
/// the backlog (no day).
pub fn move_days(today: NaiveDate) -> Vec<(String, Option<NaiveDate>)> {
    let mut days: Vec<(String, Option<NaiveDate>)> = vec![("Today".into(), Some(today))];
    days.push(("Tomorrow".into(), Some(today + Days::new(1))));
    days.extend((2..7).map(|i| today + Days::new(i)).map(|d| (d.format("%A, %b %-d").to_string(), Some(d))));
    days.push(("Backlog · someday".into(), None));
    days
}

fn move_to(task: EntityId, query: &str, today: NaiveDate) -> Vec<Entry> {
    move_days(today)
        .into_iter()
        .filter(|(title, _)| matches(title, query))
        .map(|(title, day)| Entry { title, hint: "Move".into(), action: Action::MoveTask { task, day } })
        .collect()
}

/// The quests most recently used first, as the quest picker lists them.
fn assign_quest(task: EntityId, query: &str, snapshot: &Snapshot, at: DateTime<Utc>) -> Vec<Entry> {
    let quests = quest_picker::recent_quests(snapshot, at).into_iter().filter(|q| matches(&q.title, query));
    let mut out: Vec<Entry> = quests
        .map(|q| Entry {
            title: q.title.clone(),
            hint: "Quest".into(),
            action: Action::AssignQuest { task, quest: Some(q.id) },
        })
        .collect();
    let none = "No quest";
    if matches(none, query) || out.is_empty() {
        out.push(Entry { title: none.into(), hint: "Other".into(), action: Action::AssignQuest { task, quest: None } });
    }
    out
}

fn filter_quest(query: &str, snapshot: &Snapshot) -> Vec<Entry> {
    let mut out = Vec::new();
    let all = "All quests";
    if matches(all, query) {
        out.push(Entry { title: all.into(), hint: "Filter".into(), action: Action::FilterQuest(None) });
    }
    let quests = snapshot.active_quests().filter(|q| matches(&q.title, query));
    out.extend(quests.map(|q| Entry {
        title: q.title.clone(),
        hint: "Quest".into(),
        action: Action::FilterQuest(Some(q.id)),
    }));
    out
}

fn other_workspaces(snapshot: &Snapshot) -> impl Iterator<Item = &Workspace> {
    snapshot.live_workspaces().filter(move |w| w.id != snapshot.workspace)
}

/// The other live workspaces, then managing them.
fn switch_workspace(query: &str, snapshot: &Snapshot) -> Vec<Entry> {
    let mut out: Vec<Entry> = other_workspaces(snapshot)
        .filter(|w| matches(&w.name, query))
        .map(|w| Entry { title: w.name.clone(), hint: "Switch".into(), action: Action::SwitchWorkspace(w.id) })
        .collect();
    let manage = "Manage workspaces…";
    if matches(manage, query) || out.is_empty() {
        out.push(Entry { title: manage.into(), hint: "Workspace".into(), action: Action::ManageWorkspaces });
    }
    out
}

/// "2026-10-01", or "Oct 1" in `today`'s year.
pub fn parse_date(text: &str, today: NaiveDate) -> Option<NaiveDate> {
    let text = text.trim();
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .ok()
        .or_else(|| NaiveDate::parse_from_str(&format!("{text} {}", today.year()), "%b %d %Y").ok())
}

fn schedule_at(task: EntityId, query: &str, now: NaiveDateTime) -> Vec<Entry> {
    let today = now.date();
    let entry = |start: i32| Entry {
        title: format!("Today at {}", crate::format::hh_mm(start as u32)),
        hint: "Schedule".into(),
        action: Action::Schedule { task, day: today, start },
    };
    if let Some(start) = parse_time(query) {
        return vec![entry(start)];
    }
    // The next quarter hour, then every half hour until late evening.
    let minute = (now.hour() * 60 + now.minute()) as i32;
    let next = (minute + SNAP_MINUTES - 1) / SNAP_MINUTES * SNAP_MINUTES;
    (0..8).map(|i| next + i * 30).filter(|m| *m < 24 * 60).map(entry).collect()
}

/// "9", "9:30", "0930", "17.15" → minutes after midnight, on the quarter hour it names.
pub fn parse_time(text: &str) -> Option<i32> {
    let digits: String = text.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() != text.chars().filter(|c| !matches!(c, ':' | '.')).count() {
        return None;
    }
    let (h, m) = match digits.len() {
        1 | 2 => (digits.parse::<i32>().ok()?, 0),
        3 | 4 => {
            let split = digits.len() - 2;
            (digits[..split].parse::<i32>().ok()?, digits[split..].parse::<i32>().ok()?)
        }
        _ => return None,
    };
    (h < 24 && m < 60).then_some(h * 60 + m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quest_domain::{Quest, QuestColor, QuestStatus, Task};

    fn now() -> NaiveDateTime {
        "2026-10-15T14:40:00".parse().unwrap()
    }

    fn at() -> DateTime<Utc> {
        now().and_utc()
    }

    fn entries(query: &str, snapshot: &Snapshot) -> Vec<Entry> {
        super::entries(Mode::Commands, query, snapshot, now(), at())
    }

    fn snapshot() -> Snapshot {
        let quest = Quest {
            id: EntityId::new(),
            title: "Launch my idea".into(),
            color: QuestColor::Red,
            status: QuestStatus::Active,
            sort_key: "a".into(),
            description: String::new(),
        };
        let task = |title: &str, done: bool| Task {
            id: EntityId::new(),
            title: title.into(),
            quest_id: Some(quest.id),
            day: None,
            sort_key: "a".into(),
            block: None,
            completed_at: done.then(|| "2026-10-15T14:40:00Z".parse().unwrap()),
            description: String::new(),
            repeat: None,
            series: None,
        };
        Snapshot {
            tasks: vec![task("Design the next chapter", false), task("Old idea", true)],
            quests: vec![quest.clone()],
            ..Snapshot::default()
        }
    }

    #[test]
    fn empty_query_lists_jumps_and_open_work() {
        let entries = entries("", &snapshot());
        assert_eq!(entries[0].action, Action::Go(View::Home));
        assert!(entries.iter().any(|e| matches!(e.action, Action::Focus(_))));
        assert!(!entries.iter().any(|e| e.title.contains("Old idea")), "done tasks are not offered");
        assert!(!entries.iter().any(|e| matches!(e.action, Action::CreateTask(_))));
    }

    #[test]
    fn a_query_offers_creation_and_filters_case_insensitively() {
        let entries = entries("  design ", &snapshot());
        let titles: Vec<&str> = entries.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Focus on “Design the next chapter”",
                "Move “Design the next chapter” to…",
                "Schedule “Design the next chapter” at…",
                "Delete task “Design the next chapter”",
                "Create task “design”",
                "Create quest “design”"
            ]
        );
    }

    #[test]
    fn with_no_match_creating_comes_first() {
        let entries = entries("stretch", &snapshot());
        assert_eq!(entries[0].action, Action::CreateTask("stretch".into()));
    }

    #[test]
    fn jumps_match_view_names() {
        let entries = entries("review", &snapshot());
        assert!(entries.iter().any(|e| e.action == Action::Go(View::Review)));
    }

    #[test]
    fn moving_offers_the_coming_week_and_the_backlog() {
        let task = EntityId::new();
        let days = super::entries(Mode::MoveTo(task), "", &snapshot(), now(), at());
        assert_eq!(days.len(), 8);
        assert_eq!(days[1].action, Action::MoveTask { task, day: Some(now().date() + Days::new(1)) });
        assert_eq!(days[7].action, Action::MoveTask { task, day: None });
        let filtered = super::entries(Mode::MoveTo(task), "back", &snapshot(), now(), at());
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn scheduling_offers_upcoming_times_or_the_typed_one() {
        let task = EntityId::new();
        let day = now().date();
        let times = super::entries(Mode::ScheduleAt(task), "", &snapshot(), now(), at());
        assert_eq!(times[0].action, Action::Schedule { task, day, start: 14 * 60 + 45 });
        assert_eq!(times[1].action, Action::Schedule { task, day, start: 15 * 60 + 15 });
        let typed = super::entries(Mode::ScheduleAt(task), "9:30", &snapshot(), now(), at());
        assert_eq!(typed[0].action, Action::Schedule { task, day, start: 570 });
    }

    #[test]
    fn assigning_a_quest_offers_active_quests_and_none() {
        let task = EntityId::new();
        let all = super::entries(Mode::AssignQuest(task), "", &snapshot(), now(), at());
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].action, Action::AssignQuest { task, quest: None });
    }

    #[test]
    fn assigning_a_quest_lists_the_most_recently_used_first() {
        let mut snap = snapshot();
        let mut used = snap.quests[0].clone();
        used.id = EntityId::new();
        used.title = "Run a half marathon".into();
        used.sort_key = "b".into();
        snap.quests.push(used.clone());
        snap.tasks[0].quest_id = Some(used.id);
        snap.task_written.insert(snap.tasks[0].id, "2026-10-15T09:00:00Z".parse().unwrap());
        let task = EntityId::new();
        let all = super::entries(Mode::AssignQuest(task), "", &snap, now(), at());
        let titles: Vec<&str> = all.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["Run a half marathon", "Launch my idea", "No quest"]);
    }

    #[test]
    fn filtering_offers_all_quests_and_each_active_one() {
        let snap = snapshot();
        let all = super::entries(Mode::FilterQuest, "", &snap, now(), at());
        assert_eq!(all[0].action, Action::FilterQuest(None));
        assert_eq!(all[1].action, Action::FilterQuest(Some(snap.quests[0].id)));
        let log = entries("log", &snap);
        assert_eq!(log[0].action, Action::LogWork);
    }

    #[test]
    fn capture_with_ai_is_a_command() {
        let entries = entries("capture", &snapshot());
        assert_eq!(entries[0].action, Action::CaptureWithAi);
        assert_eq!(entries[0].title, "Capture with AI");
        assert!(
            super::entries(Mode::Commands, "", &snapshot(), now(), at())
                .iter()
                .any(|e| e.action == Action::CaptureWithAi)
        );
    }

    #[test]
    fn switching_workspaces_offers_the_other_live_ones() {
        let mut snap = snapshot();
        let work = EntityId::new();
        let gone = EntityId::new();
        let workspace = |id, name: &str| quest_domain::Workspace {
            id,
            name: name.into(),
            deleted_at: None,
            content: Default::default(),
        };
        snap.workspaces = vec![
            workspace(EntityId::DEFAULT_WORKSPACE, "Home"),
            workspace(work, "Work"),
            quest_domain::Workspace {
                deleted_at: Some("2026-10-15T14:40:00Z".parse().unwrap()),
                ..workspace(gone, "Gone")
            },
        ];
        let listed = super::entries(Mode::SwitchWorkspace, "", &snap, now(), at());
        let actions: Vec<&Action> = listed.iter().map(|e| &e.action).collect();
        assert_eq!(actions, [&Action::SwitchWorkspace(work), &Action::ManageWorkspaces]);
        let found = entries("wor", &snap);
        assert!(found.iter().any(|e| e.action == Action::SwitchWorkspace(work)));
        assert_eq!(entries("switch", &snap)[0].action, Action::Choose(Mode::SwitchWorkspace));
        assert!(
            !entries("home", &snap).iter().any(|e| matches!(e.action, Action::SwitchWorkspace(_))),
            "not the current one"
        );
    }

    #[test]
    fn the_archive_is_a_jump() {
        let entries = entries("archive", &snapshot());
        assert_eq!(entries[0].action, Action::Go(View::Archive));
    }

    #[test]
    fn times_parse_loosely() {
        assert_eq!(parse_time("9"), Some(540));
        assert_eq!(parse_time("9:30"), Some(570));
        assert_eq!(parse_time("0930"), Some(570));
        assert_eq!(parse_time("17.15"), Some(17 * 60 + 15));
        assert_eq!(parse_time("25"), None);
        assert_eq!(parse_time("soon"), None);
    }
}
