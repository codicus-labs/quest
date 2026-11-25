use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use quest_domain::{Command, EntityId, FieldStore, HlcClock, NodeId, Position, Snapshot, plan};

use super::*;

fn at(h: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 25, h, 0, 0).unwrap()
}

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

    fn task(&mut self, title: &str) -> EntityId {
        let id = EntityId::new();
        let planned_on = NaiveDate::from_ymd_opt(2026, 11, 25);
        self.run(Command::CreateTask {
            id,
            title: title.into(),
            quest_id: None,
            planned_on,
            due_on: None,
            position: Position::Bottom,
        });
        id
    }
}

#[test]
fn nothing_tracked_is_empty_then_resume_offers_the_last_open_task() {
    let mut b = Board::new();
    assert_eq!(player(&b.snapshot), Player::Empty);
    let (first, second) = (b.task("First"), b.task("Second"));
    b.run(Command::LogTime { entry_id: EntityId::new(), task_id: Some(first), started_at: at(8), ended_at: at(9) });
    b.run(Command::LogTime { entry_id: EntityId::new(), task_id: Some(second), started_at: at(9), ended_at: at(10) });
    assert_eq!(player(&b.snapshot), Player::Resume(TaskRef { task: second, title: "Second".into() }));
    b.run(Command::CompleteTask { id: second, at: at(11) });
    assert_eq!(player(&b.snapshot), Player::Resume(TaskRef { task: first, title: "First".into() }));
}

#[test]
fn a_running_task_and_organizing_with_its_way_back() {
    let mut b = Board::new();
    let task = b.task("Design");
    b.run(Command::StartFocus { entry_id: EntityId::new(), task_id: task, at: at(9) });
    assert_eq!(
        player(&b.snapshot),
        Player::Running { task: Some(task), title: "Design".into(), quest: None, started_at: at(9) }
    );
    b.run(Command::StartOrganizing { entry_id: EntityId::new(), origin_task_id: Some(task), at: at(10) });
    let back_to = Some(TaskRef { task, title: "Design".into() });
    assert_eq!(player(&b.snapshot), Player::Organizing { started_at: at(10), back_to });
}
