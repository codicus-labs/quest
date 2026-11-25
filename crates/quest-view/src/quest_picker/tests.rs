use chrono::{DateTime, TimeZone, Utc};
use quest_domain::{
    Command, EntityId, FieldStore, Frequency, HlcClock, NodeId, Position, QuestColor, RepeatPin, RepeatRule, Snapshot,
    Weekdays, plan,
};

use super::{filter, matches, recent_quests};

/// A store that applies commands at a given wall time, as a device would.
struct Device {
    store: FieldStore,
    clock: HlcClock,
    snapshot: Snapshot,
}

fn at(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 15, hour, minute, 0).unwrap()
}

impl Device {
    fn new() -> Self {
        Device { store: FieldStore::default(), clock: HlcClock::new(NodeId(1)), snapshot: Snapshot::default() }
    }

    fn run(&mut self, when: DateTime<Utc>, command: Command) {
        let ms = u64::try_from(when.timestamp_millis()).unwrap();
        for write in plan(&command, &self.snapshot).unwrap() {
            self.store.apply(&write.stamp(self.clock.now(ms)));
        }
        self.snapshot = Snapshot::from_store(&self.store, self.snapshot.workspace);
    }

    fn quest(&mut self, title: &str) -> EntityId {
        let id = EntityId::new();
        self.run(at(8, 0), Command::CreateQuest { id, title: title.into(), color: QuestColor::Blue });
        id
    }

    fn task(&mut self, when: DateTime<Utc>, quest: Option<EntityId>) -> EntityId {
        let id = EntityId::new();
        let command =
            Command::CreateTask { id, title: "Task".into(), quest_id: quest, day: None, position: Position::Bottom };
        self.run(when, command);
        id
    }

    fn log(&mut self, task: EntityId, from: DateTime<Utc>, to: DateTime<Utc>) {
        let command =
            Command::LogTime { entry_id: EntityId::new(), task_id: Some(task), started_at: from, ended_at: to };
        // Logged long after the fact, so the session's own times decide, not this write.
        self.run(at(7, 0), command);
    }

    fn order(&self, now: DateTime<Utc>) -> Vec<String> {
        recent_quests(&self.snapshot, now).into_iter().map(|q| q.title.clone()).collect()
    }
}

#[test]
fn never_used_quests_keep_the_quests_page_order() {
    let mut device = Device::new();
    for title in ["Alpha", "Beta", "Gamma"] {
        device.quest(title);
    }
    assert_eq!(device.order(at(12, 0)), ["Alpha", "Beta", "Gamma"]);
}

#[test]
fn the_newest_session_puts_its_quest_first() {
    let mut device = Device::new();
    let [_, beta, gamma] = ["Alpha", "Beta", "Gamma"].map(|t| device.quest(t));
    let (beta_task, gamma_task) = (device.task(at(7, 0), Some(beta)), device.task(at(7, 0), Some(gamma)));
    device.log(beta_task, at(9, 0), at(10, 0));
    device.log(gamma_task, at(10, 0), at(11, 0));
    assert_eq!(device.order(at(12, 0)), ["Gamma", "Beta", "Alpha"]);
}

#[test]
fn a_running_session_counts_as_used_now() {
    let mut device = Device::new();
    let [alpha, beta] = ["Alpha", "Beta"].map(|t| device.quest(t));
    let (alpha_task, beta_task) = (device.task(at(7, 0), Some(alpha)), device.task(at(7, 0), Some(beta)));
    device.log(beta_task, at(9, 0), at(11, 0));
    device.run(at(7, 0), Command::StartFocus { entry_id: EntityId::new(), task_id: alpha_task, at: at(10, 0) });
    assert_eq!(device.order(at(12, 0)), ["Alpha", "Beta"]);
}

#[test]
fn creating_or_changing_a_task_uses_its_quest() {
    let mut device = Device::new();
    let [alpha, beta, gamma] = ["Alpha", "Beta", "Gamma"].map(|t| device.quest(t));
    let beta_task = device.task(at(9, 0), Some(beta));
    device.task(at(10, 0), Some(gamma));
    assert_eq!(device.order(at(12, 0)), ["Gamma", "Beta", "Alpha"]);

    device.run(at(11, 0), Command::RenameTask { id: beta_task, title: "Renamed".into() });
    assert_eq!(device.order(at(12, 0)), ["Beta", "Gamma", "Alpha"]);

    // Moving a task to a quest uses that quest.
    device.run(at(11, 30), Command::AssignTask { id: beta_task, quest_id: Some(alpha) });
    assert_eq!(device.order(at(12, 0)), ["Alpha", "Gamma", "Beta"]);
}

#[test]
fn the_later_of_a_session_and_a_task_change_counts() {
    let mut device = Device::new();
    let [alpha, beta] = ["Alpha", "Beta"].map(|t| device.quest(t));
    let alpha_task = device.task(at(7, 0), Some(alpha));
    device.log(alpha_task, at(9, 0), at(9, 30));
    device.task(at(10, 0), Some(beta));
    assert_eq!(device.order(at(12, 0)), ["Beta", "Alpha"]);
    device.log(alpha_task, at(10, 0), at(10, 30));
    assert_eq!(device.order(at(12, 0)), ["Alpha", "Beta"]);
}

#[test]
fn generated_instances_of_a_repeating_task_do_not_count() {
    let mut device = Device::new();
    let [alpha, beta] = ["Alpha", "Beta"].map(|t| device.quest(t));
    let daily = device.task(at(7, 0), Some(beta));
    let rule = RepeatRule {
        every: 1,
        frequency: Frequency::Day,
        starts: at(7, 0).date_naive(),
        ends: None,
        time_zone: chrono_tz::UTC,
        pin: RepeatPin::None,
        weekdays: Weekdays::default(),
        month_day: None,
    };
    device.run(at(7, 0), Command::SetTaskRepeat { id: daily, rule: Some(rule), now: at(7, 0) });
    device.task(at(9, 0), Some(alpha));
    device.run(at(11, 0), Command::GenerateDueTasks { now: at(11, 0) + chrono::Days::new(1) });
    assert!(device.snapshot.tasks.iter().any(|t| t.series == Some(daily)), "an instance was generated");
    assert_eq!(device.order(at(12, 0)), ["Alpha", "Beta"]);
}

#[test]
fn a_tie_keeps_the_quests_page_order() {
    let mut device = Device::new();
    let [alpha, _, gamma] = ["Alpha", "Beta", "Gamma"].map(|t| device.quest(t));
    let gamma_task = device.task(at(7, 0), Some(gamma));
    let alpha_task = device.task(at(7, 0), Some(alpha));
    device.log(gamma_task, at(9, 0), at(10, 0));
    device.log(alpha_task, at(9, 30), at(10, 0));
    assert_eq!(device.order(at(12, 0)), ["Alpha", "Gamma", "Beta"]);
}

#[test]
fn only_active_quests_are_offered() {
    let mut device = Device::new();
    let [alpha, _] = ["Alpha", "Beta"].map(|t| device.quest(t));
    let command = Command::SetQuestStatus { id: alpha, status: quest_domain::QuestStatus::Completed };
    device.run(at(9, 0), command);
    assert_eq!(device.order(at(12, 0)), ["Beta"]);
}

#[test]
fn work_in_another_workspace_does_not_count() {
    let mut device = Device::new();
    let [alpha, beta] = ["Alpha", "Beta"].map(|t| device.quest(t));
    let alpha_task = device.task(at(7, 0), Some(alpha));
    device.log(alpha_task, at(8, 0), at(8, 30));
    let beta_task = device.task(at(7, 0), Some(beta));
    device.log(beta_task, at(9, 0), at(9, 30));

    // A second workspace with its own quest, used later than everything above.
    let other = EntityId::new();
    device.run(at(7, 0), Command::CreateWorkspace { id: other, name: "Other".into() });
    device.snapshot = Snapshot::from_store(&device.store, other);
    let elsewhere = device.quest("Elsewhere");
    let task = device.task(at(11, 0), Some(elsewhere));
    device.log(task, at(11, 0), at(11, 30));
    assert_eq!(device.order(at(12, 0)), ["Elsewhere"]);

    device.snapshot = Snapshot::from_store(&device.store, EntityId::DEFAULT_WORKSPACE);
    assert_eq!(device.order(at(12, 0)), ["Beta", "Alpha"]);
}

#[test]
fn matching_is_a_substring_ignoring_case() {
    assert!(matches("Write the Book", "book"));
    assert!(matches("Write the Book", " THE b "));
    assert!(matches("Write the Book", ""));
    assert!(matches("Ärger klären", "äRGER"));
    assert!(!matches("Write the Book", "wtb"), "no fuzzy subsequences");
    assert!(!matches("Write the Book", "books"));
}

#[test]
fn filtering_keeps_the_order() {
    let mut device = Device::new();
    for title in ["Garden", "Book", "Guitar", "Run"] {
        device.quest(title);
    }
    let quests = recent_quests(&device.snapshot, at(12, 0));
    let titles = |query: &str| filter(&quests, query).into_iter().map(|q| q.title.as_str()).collect::<Vec<_>>();
    assert_eq!(titles("g"), ["Garden", "Guitar"]);
    assert_eq!(titles("UI"), ["Guitar"]);
    assert!(titles("xyz").is_empty());
    assert_eq!(titles("").len(), 4);
}
