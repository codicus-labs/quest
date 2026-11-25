use super::*;

#[test]
fn edges_scroll_faster_nearer_the_edge_and_not_in_the_middle() {
    let (left, right) = (100.0, 1100.0);
    assert_eq!(edge_speed(600.0, left, right), 0.0);
    assert_eq!(edge_speed(left + EDGE_ZONE + 1.0, left, right), 0.0, "just inside the zone's border");
    let near = edge_speed(right - 10.0, left, right);
    let far = edge_speed(right - 60.0, left, right);
    assert!(near > far && far > 0.0, "later, faster at the edge: {far} < {near}");
    assert_eq!(edge_speed(right, left, right), EDGE_MAX_SPEED);
    assert_eq!(edge_speed(right + 40.0, left, right), EDGE_MAX_SPEED, "past the edge is the edge");
    assert!(edge_speed(left + 10.0, left, right) < 0.0, "earlier at the left");
    assert_eq!(edge_speed(left + 10.0, left, right), -near);
}
