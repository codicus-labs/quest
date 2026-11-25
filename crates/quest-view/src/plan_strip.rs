//! Bounds and edge scrolling for the legacy mobile Plan board.

#[cfg(test)]
mod tests;

pub const REACH_DAYS: u64 = 3 * 366;

const EDGE_ZONE: f32 = 80.0;
const EDGE_MAX_SPEED: f32 = 1400.0;

/// How fast a drag at `x` scrolls a board spanning `left..right`: negative towards earlier days
/// in the left edge zone, positive in the right one, faster nearer the edge (and fastest past
/// it), else 0.
pub fn edge_speed(x: f32, left: f32, right: f32) -> f32 {
    let zone = EDGE_ZONE.min((right - left) / 4.0);
    if zone <= 0.0 {
        return 0.0;
    }
    let depth = |from_edge: f32| ((zone - from_edge) / zone).clamp(0.0, 1.0);
    let speed = |d: f32| EDGE_MAX_SPEED * d * d;
    if x < left + zone {
        return -speed(depth(x - left));
    }
    if x > right - zone {
        return speed(depth(right - x));
    }
    0.0
}
