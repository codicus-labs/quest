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
fn today_keeps_manual_order_regardless_of_urgency() {
    let mut b = Board::new();
    b.task("planned today", None, Some(25), None);
    b.task("due today", None, None, Some(25));
    b.task("carried over", None, Some(23), None);
    b.task("overdue", None, None, Some(24));
    b.task("tomorrow", None, Some(26), None);
    let done = b.task("done", None, Some(25), None);
    b.run(Command::CompleteTask { id: done, at: at(25) });
    assert_eq!(titles(&today(&b.snapshot, d(25))), ["planned today", "due today", "carried over", "overdue"]);
}

#[test]
fn placement_between_tied_keys_keeps_other_order_and_undo_restores_the_tie() {
    use crate::undo::TaskEdit;
    use quest_domain::{Placement, TaskList};

    let mut board = Board::new();
    let today_date = d(25);
    let moved = board.task("moved", None, Some(23), None);
    let a = board.task("first tie", None, Some(25), None);
    let b = board.task("second tie", None, Some(25), None);
    let (first, second) = (a.min(b), a.max(b));
    for id in [a, b] {
        board.run(Command::RestoreTaskEdit {
            id,
            dates: None,
            sort_key: None,
            day_order: Some("k".into()),
            rekeyed: Vec::new(),
            quest_id: None,
            completed_at: None,
            postponements: Vec::new(),
            at: at(25),
        });
    }
    let old = board.snapshot.clone();
    let list = TaskList::Today;
    board.run(Command::PlaceTask {
        id: moved,
        placement: Placement::InList { list, position: Position::Before(second) },
        today: today_date,
        at: at(25),
    });
    let ids = |snap: &Snapshot| today(snap, today_date).into_iter().map(|task| task.id).collect::<Vec<_>>();
    assert_eq!(ids(&board.snapshot), [first, moved, second]);
    let before = old.task(moved).unwrap();
    let after = board.snapshot.task(moved).unwrap();
    assert_eq!(after.planned_on, before.planned_on, "carried-over is still carried over");
    assert_eq!(after.sort_key, before.sort_key, "Today does not move the task in the quest or Inbox");

    let undo = TaskEdit::placement(before, after, &old, &board.snapshot, list, today_date).unwrap();
    board.run(undo.command(at(25)));
    assert_eq!(ids(&board.snapshot), ids(&old));
    assert_eq!(board.snapshot.task(second).unwrap().day_order, "k");
}

#[test]
fn sidebar_inbox_clears_quest_and_plan_but_keeps_due_and_undo_restores_the_placement() {
    use quest_domain::{Placement, TaskList};

    let mut board = Board::new();
    let quest = EntityId::new();
    board.run(Command::CreateQuest { id: quest, title: "Q".into(), color: QuestColor::Red });
    let id = board.task("Filed", Some(quest), Some(24), Some(25));
    let other = board.task("Inbox", None, None, None);
    let old = board.snapshot.clone();
    board.run(Command::PlaceTask { id, placement: Placement::FileIn(TaskList::Inbox), today: d(25), at: at(25) });
    let filed = board.snapshot.task(id).unwrap();
    assert_eq!((filed.quest_id, filed.planned_on, filed.due_on), (None, None, Some(d(25))));
    assert_eq!(inbox(&board.snapshot).iter().map(|t| t.id).collect::<Vec<_>>(), [other, id]);
    assert!(board.snapshot.postponements.is_empty());

    let before = old.task(id).unwrap();
    board.run(
        crate::undo::TaskEdit::placement(before, filed, &old, &board.snapshot, TaskList::Inbox, d(25))
            .unwrap()
            .command(at(25)),
    );
    assert_eq!(board.snapshot.task(id), Some(before));
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

#[test]
fn done_today_and_the_logbook_by_day_follow_the_local_day() {
    let mut b = Board::new();
    let (early, late, before) =
        (b.task("early", None, None, None), b.task("late", None, None, None), b.task("before", None, None, None));
    b.run(Command::CompleteTask { id: early, at: at(25) });
    b.run(Command::CompleteTask { id: late, at: at(25) + chrono::Duration::hours(3) });
    b.run(Command::CompleteTask { id: before, at: at(24) });
    assert_eq!(titles(&done_today(&b.snapshot, d(25), &Utc)), ["late", "early"]);
    let days: Vec<(NaiveDate, Vec<String>)> =
        logbook_days(&b.snapshot, &Utc).into_iter().map(|(day, tasks)| (day, titles(&tasks))).collect();
    assert_eq!(days, [(d(25), vec!["late".to_owned(), "early".to_owned()]), (d(24), vec!["before".to_owned()])]);
}

#[test]
fn a_quest_page_splits_open_and_done_and_the_sidebar_counts() {
    let mut b = Board::new();
    let quest = EntityId::new();
    b.run(Command::CreateQuest { id: quest, title: "Q".into(), color: QuestColor::Red });
    b.task("open", Some(quest), Some(27), None);
    let done = b.task("done", Some(quest), None, None);
    b.task("elsewhere", None, None, None);
    b.task("today", None, Some(25), None);
    b.run(Command::CompleteTask { id: done, at: at(25) });
    assert_eq!(titles(&quest_open(&b.snapshot, quest)), ["open"]);
    assert_eq!(titles(&quest_done(&b.snapshot, quest)), ["done"]);
    assert_eq!(nav_counts(&b.snapshot, d(25)), NavCounts { inbox: 1, today: 1, plan: 1 });
}
