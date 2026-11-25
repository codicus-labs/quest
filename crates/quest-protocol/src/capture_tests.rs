use chrono::{NaiveDate, NaiveTime};
use quest_domain::{EntityId, TimeBlock};

use super::*;

fn day(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

#[test]
fn proposal_round_trips_as_tagged_json() {
    let quest = TempKey("q1".into());
    let proposal = CaptureProposal {
        quests: vec![ProposedQuest {
            key: quest.clone(),
            title: "Steuererklärung".into(),
            description: String::new(),
            milestones: vec![],
        }],
        tasks: vec![ProposedTask {
            key: TempKey("t1".into()),
            title: "Belege sammeln".into(),
            description: String::new(),
            quest: Some(QuestRef::New { key: quest }),
            quest_uncertain: false,
            day: Some(day("2026-11-27")),
            block: Some(TimeBlock { start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(), minutes: 60 }),
            repeat: None,
        }],
        work: vec![ProposedWork {
            key: TempKey("w1".into()),
            task: WorkTarget::Existing { id: EntityId::SETTINGS },
            uncertain: true,
            day: day("2026-11-24"),
            start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            end: NaiveTime::from_hms_opt(11, 0, 0).unwrap(),
        }],
        dropped: vec![Dropped { title: "x".into(), reason: DropReason::InvalidDate }],
    };
    let json = serde_json::to_value(&proposal).unwrap();
    assert_eq!(json["tasks"][0]["quest"], serde_json::json!({"type": "new", "key": "q1"}));
    assert_eq!(json["work"][0]["task"]["type"], "existing");
    assert_eq!(json["dropped"][0]["reason"], "invalid_date");
    assert_eq!(serde_json::from_value::<CaptureProposal>(json).unwrap(), proposal);
}
