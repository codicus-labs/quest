use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use quest_domain::{Command, EntityId, FieldStore, HlcClock, NodeId, Position, QuestColor, Snapshot, plan};

use super::*;

fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
}

fn at(day: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, day, 9, 0, 0).unwrap()
}

/// Runs commands against an in-memory store, like a client does.
struct Board {
    store: FieldStore,
    clock: HlcClock,
    snapshot: Snapshot,
}

impl Board {
    fn new() -> Self {
        Board { store: FieldStore::default(), clock: HlcClock::new(NodeId(1)), snapshot: Snapshot::default() }
    }

    fn run(&mut self, command: Command) {
        for write in plan(&command, &self.snapshot).unwrap() {
            self.store.apply(&write.stamp(self.clock.now(1)));
        }
        self.snapshot = Snapshot::from_store(&self.store, self.snapshot.workspace);
    }

    fn task(&mut self, title: &str, quest_id: Option<EntityId>, plan: Option<u32>, due: Option<u32>) -> EntityId {
        let id = EntityId::new();
        self.run(Command::CreateTask {
            id,
            title: title.into(),
            quest_id,
            planned_on: plan.map(d),
            due_on: due.map(d),
            position: Position::Bottom,
        });
        id
    }
}

fn titles(tasks: &[&Task]) -> Vec<String> {
    tasks.iter().map(|t| t.title.clone()).collect()
}

#[test]
fn today_ranks_overdue_carried_over_due_today_then_the_rest() {
    let mut b = Board::new();
    b.task("planned today", None, Some(25), None);
    b.task("due today", None, None, Some(25));
    b.task("carried over", None, Some(23), None);
    b.task("overdue", None, None, Some(24));
    b.task("tomorrow", None, Some(26), None);
    let done = b.task("done", None, Some(25), None);
    b.run(Command::CompleteTask { id: done, at: at(25) });
    assert_eq!(titles(&today(&b.snapshot, d(25))), ["overdue", "carried over", "due today", "planned today"]);
}

#[test]
fn the_inbox_holds_open_tasks_without_quest_and_plan_date() {
    let mut b = Board::new();
    let quest = EntityId::new();
    b.run(Command::CreateQuest { id: quest, title: "Q".into(), color: QuestColor::Red });
    b.task("loose", None, None, None);
    b.task("due only", None, None, Some(30));
    b.task("filed", Some(quest), None, None);
    b.task("planned", None, Some(26), None);
    assert_eq!(titles(&inbox(&b.snapshot)), ["loose", "due only"]);
}

#[test]
fn the_logbook_lists_done_tasks_newest_first_and_postponements_are_counted() {
    let mut b = Board::new();
    let (first, second) = (b.task("first", None, Some(20), Some(21)), b.task("second", None, None, None));
    b.run(Command::CompleteTask { id: first, at: at(20) });
    b.run(Command::CompleteTask { id: second, at: at(22) });
    assert_eq!(titles(&logbook(&b.snapshot)), ["second", "first"]);

    b.run(Command::MoveTask { id: first, planned_on: Some(d(23)), position: Position::Bottom, at: at(22) });
    assert_eq!(postponed(&b.snapshot, first), Postponed { plan: 1, due: 1 });
    assert_eq!(postponed(&b.snapshot, second).total(), 0);
}
