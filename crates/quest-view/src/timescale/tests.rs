use super::*;

const SCALE: TimeScale = TimeScale { first_hour: 7, last_hour: 23, hour_height: 48.0 };

#[test]
fn minutes_and_points_round_trip() {
    assert_eq!(SCALE.y(7 * 60), 0.0);
    assert_eq!(SCALE.y(9 * 60 + 30), 120.0);
    assert_eq!(SCALE.minute_at(120.0), 570.0);
    assert_eq!(SCALE.height(), 16.0 * 48.0);
    assert_eq!(SCALE.minutes(12.0), 15.0);
}

#[test]
fn snapping_goes_to_the_nearest_quarter_hour() {
    assert_eq!(snap(570.0), 570);
    assert_eq!(snap(577.0), 570);
    assert_eq!(snap(578.0), 585);
    assert_eq!(snap(-8.0), -15);
}

#[test]
fn spans_stay_inside_the_grid() {
    assert_eq!(SCALE.clamp_start(6 * 60, 30), 7 * 60);
    assert_eq!(SCALE.clamp_start(22 * 60 + 45, 30), 22 * 60 + 30);
    assert_eq!(Span::new(6 * 60, 7 * 60 + 30).visible(&SCALE), Some(Span::new(7 * 60, 7 * 60 + 30)));
    assert_eq!(Span::new(5 * 60, 6 * 60).visible(&SCALE), None);
    assert_eq!(Span::new(600, 630).moved_to(660), Span::new(660, 690));
}

#[test]
fn separate_items_take_the_full_width() {
    let lanes = lanes(&[Span::new(540, 600), Span::new(600, 660)]);
    assert_eq!(lanes, vec![Lane { index: 0, count: 1 }, Lane { index: 0, count: 1 }]);
}

#[test]
fn overlapping_items_sit_side_by_side() {
    // 9–10 and 9:30–11 overlap; 10:30–11 overlaps the second but fits in the first lane again.
    let spans = [Span::new(540, 600), Span::new(570, 660), Span::new(630, 660), Span::new(720, 750)];
    let lanes = lanes(&spans);
    assert_eq!(lanes[0], Lane { index: 0, count: 2 });
    assert_eq!(lanes[1], Lane { index: 1, count: 2 });
    assert_eq!(lanes[2], Lane { index: 0, count: 2 });
    assert_eq!(lanes[3], Lane { index: 0, count: 1 }, "a later, separate item is alone again");
}

#[test]
fn three_at_once_need_three_lanes_whatever_the_input_order() {
    let spans = [Span::new(600, 660), Span::new(540, 700), Span::new(610, 620)];
    let lanes = lanes(&spans);
    let mut indexes: Vec<usize> = lanes.iter().map(|l| l.index).collect();
    indexes.sort_unstable();
    assert_eq!(indexes, vec![0, 1, 2]);
    assert!(lanes.iter().all(|l| l.count == 3));
    assert_eq!(lanes[1].index, 0, "the earliest start takes the first lane");
}

#[test]
fn zero_length_items_still_get_a_lane() {
    let lanes = lanes(&[Span::new(600, 600), Span::new(600, 630)]);
    assert_eq!(lanes[0].count, 2);
    assert_ne!(lanes[0].index, lanes[1].index);
}
