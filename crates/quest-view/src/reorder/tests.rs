use quest_domain::{EntityId, Position};

use super::nudge;

#[test]
fn nudges_move_one_place() {
    let ids: Vec<EntityId> = (0..3).map(|_| EntityId::new()).collect();
    assert_eq!(nudge(&ids, ids[1], true), Some(Position::Before(ids[0])));
    assert_eq!(nudge(&ids, ids[0], true), None);
    assert_eq!(nudge(&ids, ids[0], false), Some(Position::Before(ids[2])));
    assert_eq!(nudge(&ids, ids[1], false), Some(Position::Bottom));
    assert_eq!(nudge(&ids, ids[2], false), None);
    assert_eq!(nudge(&ids, EntityId::new(), false), None);
}
