use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use quest_domain::{Command, EntityId, FieldStore, HlcClock, NodeId, Position, QuestColor, Snapshot, plan};

use super::*;

fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
}

fn at(day: u32, h: u32, m: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, day, h, m, 0).unwrap()
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

    fn quest(&mut self, title: &str) -> EntityId {
        let id = EntityId::new();
        self.run(Command::CreateQuest { id, title: title.into(), color: QuestColor::Red });
        id
    }

    fn task(&mut self, title: &str, quest_id: Option<EntityId>) -> EntityId {
        let id = EntityId::new();
        let (planned_on, due_on) = (Some(d(25)), None);
        self.run(Command::CreateTask {
            id,
            title: title.into(),
            quest_id,
            planned_on,
            due_on,
            position: Position::Bottom,
        });
        id
    }

    fn log(&mut self, task: Option<EntityId>, from: DateTime<Utc>, to: DateTime<Utc>) {
        self.run(Command::LogTime { entry_id: EntityId::new(), task_id: task, started_at: from, ended_at: to });
    }
}

#[test]
fn the_day_shows_its_sessions_cut_to_the_day_with_now_and_a_running_one() {
    let mut b = Board::new();
    let quest = b.quest("Launch");
    let task = b.task("Map the ideas", Some(quest));
    b.log(Some(task), at(24, 23, 0), at(25, 1, 0));
    b.log(None, at(25, 11, 0), at(25, 11, 30));
    b.run(Command::StartOrganizing { entry_id: EntityId::new(), origin_task_id: Some(task), at: at(25, 14, 0) });

    let day = day_timeline(&b.snapshot, d(25), &Utc, at(25, 14, 40));
    let shown: Vec<_> =
        day.blocks.iter().map(|b| (b.title.as_str(), b.start_minute, b.end_minute, b.running)).collect();
    assert_eq!(shown, [("Map the ideas", 0, 60, false), ("No task", 660, 690, false), ("Organizing", 840, 880, true)]);
    assert_eq!(day.blocks[0].kind, BlockKind::Task { task, quest: Some(quest) });
    assert_eq!(day.blocks[0].color, Some(QuestColor::Red));
    assert_eq!(day.blocks[2].color, None);
    assert_eq!((day.first_hour, day.last_hour, day.now_minute), (0, 18, Some(880)));
    assert_eq!(day.tracked_ms, (60 + 30 + 40) * 60_000);
}

#[test]
fn an_empty_day_spans_eight_to_six_and_another_day_has_no_now() {
    let b = Board::new();
    let day = day_timeline(&b.snapshot, d(24), &Utc, at(25, 20, 30));
    assert_eq!((day.first_hour, day.last_hour, day.now_minute, day.tracked_ms), (8, 18, None, 0));
    let late = day_timeline(&b.snapshot, d(25), &Utc, at(25, 20, 30));
    assert_eq!((late.first_hour, late.last_hour), (8, 21));
}

#[test]
fn the_week_balances_quests_organizing_no_quest_and_postponements() {
    let mut b = Board::new();
    let (launch, write) = (b.quest("Launch"), b.quest("Write"));
    let (task, loose) = (b.task("Map", Some(launch)), b.task("Call", None));
    b.log(Some(task), at(23, 9, 0), at(23, 11, 0));
    b.log(Some(loose), at(24, 9, 0), at(24, 9, 30));
    b.run(Command::StartOrganizing { entry_id: EntityId::new(), origin_task_id: None, at: at(25, 9, 0) });
    b.run(Command::StopFocus { at: at(25, 9, 15) });
    b.run(Command::MoveTask { id: loose, planned_on: Some(d(27)), position: Position::Bottom, at: at(25, 10, 0) });

    let week = week_balance(&b.snapshot, d(25), &Utc, at(25, 12, 0));
    assert_eq!(week.quest_ms, 2 * 3_600_000);
    let quests: Vec<_> = week.quests.iter().map(|q| (q.quest, q.ms)).collect();
    assert_eq!(quests, [(launch, 2 * 3_600_000), (write, 0)]);
    assert_eq!((week.organizing_ms, week.no_quest_ms, week.postponed), (15 * 60_000, 30 * 60_000, 1));
    assert_eq!(week.quest_ms_of(launch), 2 * 3_600_000);
    assert_eq!(fraction(1, 4), 0.25);
    assert_eq!(fraction(5, 0), 0.0);
}
