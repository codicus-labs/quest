use quest_domain::ChecklistItem;

use super::*;

#[test]
fn positions_and_progress_follow_the_task_order() {
    let task = EntityId::new();
    let other = EntityId::new();
    let item = |task_id, done| ChecklistItem {
        id: EntityId::new(),
        task_id,
        text: String::new(),
        done,
        sort_key: String::new(),
    };
    let checklist = vec![item(task, true), item(other, true), item(task, false), item(task, true)];
    let (first, second, third) = (checklist[0].id, checklist[2].id, checklist[3].id);
    let snapshot = Snapshot { checklist, ..Snapshot::default() };
    assert_eq!(progress(&snapshot, task), Some(Progress { done: 2, total: 3 }));
    assert_eq!(progress(&snapshot, EntityId::new()), None);
    assert_eq!(after(&snapshot, first), Some(Position::Before(second)));
    assert_eq!(after(&snapshot, third), Some(Position::Bottom));
    assert_eq!(move_one(&snapshot, first, false), None);
    assert_eq!(move_one(&snapshot, first, true), Some(Position::Before(third)));
    assert_eq!(move_one(&snapshot, third, false), Some(Position::Before(second)));
    assert_eq!(move_one(&snapshot, third, true), None);
}
