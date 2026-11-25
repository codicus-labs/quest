//! `--demo`: quests, a week of tasks and tracked time like the reference screenshots, as plain
//! commands so they go through the same policy and sync as anything a user does.

use chrono::{DateTime, Days, Duration, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use quest_domain::{Command, EntityId, Position, QuestColor, TimeBlock};

/// One task in the seed: title, quest index (into `QUESTS`), day offset from today, done.
type SeedTask = (&'static str, Option<usize>, Option<i64>, bool);

const QUESTS: [(&str, QuestColor); 4] = [
    ("Launch my idea", QuestColor::Red),
    ("Write every week", QuestColor::Orange),
    ("Run a half marathon", QuestColor::Lime),
    ("Keep learning", QuestColor::Green),
];

const TASKS: [SeedTask; 22] = [
    ("Design the next chapter", Some(0), Some(0), false),
    ("Make the first impression feel effortless", Some(0), Some(0), false),
    ("Draft this week’s essay", Some(1), Some(0), false),
    ("Easy 5 km. Leave the headphones at home.", Some(2), Some(0), false),
    ("Read a chapter, take a few notes", Some(3), Some(0), true),
    ("Map the ideas worth pursuing", Some(0), Some(0), true),
    ("Outline the launch plan", Some(0), Some(-3), true),
    ("Tempo run", Some(2), Some(-3), true),
    ("Collect essay notes", Some(1), Some(-3), true),
    ("Explore three directions", Some(0), Some(-2), true),
    ("Read for 30 minutes", Some(3), Some(-2), true),
    ("Build the first prototype", Some(0), Some(-1), true),
    ("Hill repeats – stay consistent", Some(2), Some(-1), true),
    ("Answer emails", None, Some(-1), true),
    ("Polish the details that matter", Some(0), Some(1), false),
    ("Publish this week’s essay", Some(1), Some(1), false),
    ("Long run · enjoy the route", Some(2), Some(2), false),
    ("A slow morning with a good book", Some(3), Some(2), false),
    ("Sketch a pricing page", Some(0), None, false),
    ("Find a beta reader", Some(1), None, false),
    ("Pick up the dry cleaning", None, Some(0), false),
    ("Call the landlord", None, Some(1), false),
];

/// Tracked time on past days: task index, day offset, minutes. Each day starts at 09:00.
const PAST_ENTRIES: [(usize, i64, i64); 11] = [
    (6, -3, 150),
    (7, -3, 60),
    (8, -3, 60),
    (13, -3, 60),
    (9, -2, 180),
    (10, -2, 60),
    (2, -2, 72),
    (13, -2, 48),
    (11, -1, 150),
    (12, -1, 120),
    (13, -1, 48),
];

/// Today's sessions, counted back from now: task index, minutes ago it started, minutes long.
/// The last one is still running.
const TODAY_ENTRIES: [(usize, i64, i64); 3] = [(5, 340, 90), (1, 220, 60), (4, 130, 30)];
const RUNNING: (usize, i64) = (0, 36);
/// Today's plan for the rest of the day: task index, local start (hour, minute), minutes.
const PLANNED_TODAY: [(usize, (u32, u32), u32); 2] = [(2, (16, 0), 60), (3, (18, 0), 45)];
const COMPLETED_TODAY: [usize; 2] = [4, 5];

/// The commands that seed the demo. `to_utc` reads a local wall-clock time.
pub fn commands(now: DateTime<Utc>, today: NaiveDate, to_utc: impl Fn(NaiveDateTime) -> DateTime<Utc>) -> Vec<Command> {
    let quest_ids: Vec<EntityId> = QUESTS.iter().map(|_| EntityId::new()).collect();
    let task_ids: Vec<EntityId> = TASKS.iter().map(|_| EntityId::new()).collect();
    let day = |offset: i64| today.checked_add_signed(Duration::days(offset)).unwrap_or(today);
    let mut out = Vec::new();

    for ((title, color), id) in QUESTS.iter().zip(&quest_ids) {
        out.push(Command::CreateQuest { id: *id, title: (*title).to_owned(), color: *color });
    }
    let milestones = [
        (0, "First prototype in a friend’s hands", 9),
        (2, "First 10 km without stopping", 20),
        (1, "Four essays in a row", 30),
    ];
    for (quest, title, days_ago) in milestones {
        let achieved_on = today.checked_sub_days(Days::new(days_ago)).unwrap_or(today);
        out.push(Command::AddMilestone {
            id: EntityId::new(),
            quest_id: quest_ids[quest],
            title: title.to_owned(),
            achieved_on,
        });
    }
    for ((title, quest, offset, _), id) in TASKS.iter().zip(&task_ids) {
        out.push(Command::CreateTask {
            id: *id,
            title: (*title).to_owned(),
            quest_id: quest.map(|q| quest_ids[q]),
            day: offset.map(day),
            position: Position::Bottom,
        });
    }

    let mut cursor: Option<(i64, DateTime<Utc>)> = None;
    for (task, offset, minutes) in PAST_ENTRIES {
        let start = match cursor {
            Some((d, end)) if d == offset => end + Duration::minutes(15),
            _ => to_utc(day(offset).and_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap_or_default())),
        };
        let end = start + Duration::minutes(minutes);
        out.push(Command::StartFocus { entry_id: EntityId::new(), task_id: task_ids[task], at: start });
        out.push(Command::StopFocus { at: end });
        cursor = Some((offset, end));
    }
    for (task, ago, minutes) in TODAY_ENTRIES {
        let start = now - Duration::minutes(ago);
        let end = start + Duration::minutes(minutes);
        out.push(Command::StartFocus { entry_id: EntityId::new(), task_id: task_ids[task], at: start });
        out.push(Command::StopFocus { at: end });
    }
    for (task, (hour, minute), minutes) in PLANNED_TODAY {
        let start = NaiveTime::from_hms_opt(hour, minute, 0).unwrap_or_default();
        out.push(Command::SetTaskBlock { id: task_ids[task], block: Some(TimeBlock { start, minutes }) });
    }
    for (task, _, offset, done) in TASKS.iter().enumerate().map(|(i, t)| (i, t.0, t.2, t.3)) {
        if done && !COMPLETED_TODAY.contains(&task) {
            let at = to_utc(day(offset.unwrap_or(0)).and_time(NaiveTime::from_hms_opt(18, 0, 0).unwrap_or_default()));
            out.push(Command::CompleteTask { id: task_ids[task], at });
        }
    }
    for task in COMPLETED_TODAY {
        out.push(Command::CompleteTask { id: task_ids[task], at: now - Duration::minutes(60) });
    }
    let (running, ago) = RUNNING;
    let start = now - Duration::minutes(ago);
    out.push(Command::StartFocus { entry_id: EntityId::new(), task_id: task_ids[running], at: start });
    out
}

/// A proposal like the AI returns for the brief's example, against the demo data: a new quest with
/// a task, a task for an existing quest (guessed), and work on an existing task. For snapshots.
pub fn capture_proposal(snapshot: &quest_domain::Snapshot, today: NaiveDate) -> quest_protocol::CaptureProposal {
    use quest_protocol::{
        CaptureProposal, DropReason, Dropped, ProposedQuest, ProposedTask, ProposedWork, QuestRef, TempKey, WorkTarget,
    };
    let key = |k: &str| TempKey(k.to_owned());
    let time = |h, m| NaiveTime::from_hms_opt(h, m, 0).unwrap_or_default();
    let launch = snapshot.active_quests().next().map(|q| QuestRef::Existing { id: q.id });
    let worked_on = snapshot.tasks_on(today).next().map(|t| t.id);
    let task = |k: &str, title: &str, quest: Option<QuestRef>, uncertain: bool, day: Option<NaiveDate>| ProposedTask {
        key: key(k),
        title: title.to_owned(),
        description: String::new(),
        quest,
        quest_uncertain: uncertain,
        day,
        block: None,
        repeat: None,
    };
    let mut offer = task("t1", "Write the offer for Müller", launch, true, Some(today + Days::new(1)));
    offer.block = Some(TimeBlock { start: time(9, 0), minutes: 120 });
    let receipts =
        task("t2", "Collect receipts", Some(QuestRef::New { key: key("q1") }), false, Some(today + Days::new(3)));
    CaptureProposal {
        quests: vec![ProposedQuest {
            key: key("q1"),
            title: "Tax return".to_owned(),
            description: "File the 2026 tax return".to_owned(),
            milestones: Vec::new(),
        }],
        tasks: vec![offer, receipts],
        work: worked_on
            .map(|id| ProposedWork {
                key: key("w1"),
                task: WorkTarget::Existing { id },
                uncertain: false,
                day: today,
                start: time(9, 0),
                end: time(11, 0),
            })
            .into_iter()
            .collect(),
        dropped: vec![Dropped {
            title: "Call the bank yesterday at 25:00".to_owned(),
            reason: DropReason::InvalidDate,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quest_domain::{FieldStore, Hlc, HlcClock, NodeId, Snapshot, WeeklyReview, plan};

    #[test]
    fn the_demo_plans_cleanly_and_reads_like_the_reference() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        let now = today.and_hms_opt(14, 40, 0).unwrap().and_utc();
        let mut store = FieldStore::default();
        let mut clock = HlcClock::new(NodeId(1));
        let mut snapshot = Snapshot::default();
        for (i, command) in commands(now, today, |t| t.and_utc()).iter().enumerate() {
            let writes = plan(command, &snapshot).unwrap_or_else(|e| panic!("command {i} {command:?}: {e}"));
            for write in writes {
                let hlc: Hlc = clock.now(1_000 + i as u64);
                store.apply(&write.stamp(hlc));
            }
            snapshot = Snapshot::from_store(&store, snapshot.workspace);
        }
        assert_eq!(snapshot.quests.len(), 4);
        assert_eq!(snapshot.tasks_on(today).count(), 7);
        assert!(snapshot.running_entry().is_some());
        let review = WeeklyReview::build(&snapshot, today, &Utc, now);
        let quest_hours = review.quest_ms as f64 / 3_600_000.0;
        assert!((15.0..20.0).contains(&quest_hours), "quest hours {quest_hours}");
        assert!(review.other_ms > 0);
    }
}
