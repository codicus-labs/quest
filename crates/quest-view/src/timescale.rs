//! Time-grid math: minutes of the day ↔ points, 15-minute snapping, and side-by-side lanes for
//! items that overlap. Pure; the grid view and drag hit-testing share it.

#[cfg(test)]
mod tests;

/// Drops and drags land on quarter hours.
pub const SNAP_MINUTES: i32 = 15;
/// A task dropped onto the grid without a block of its own is planned for this long.
pub const DEFAULT_BLOCK_MINUTES: i32 = 30;

/// A grid's visible hours and how tall an hour is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeScale {
    pub first_hour: u32,
    pub last_hour: u32,
    pub hour_height: f32,
}

impl TimeScale {
    pub fn first_minute(&self) -> i32 {
        self.first_hour as i32 * 60
    }

    pub fn last_minute(&self) -> i32 {
        self.last_hour as i32 * 60
    }

    /// Points from the grid's top to `minute` (minutes after midnight).
    pub fn y(&self, minute: i32) -> f32 {
        (minute - self.first_minute()) as f32 / 60.0 * self.hour_height
    }

    /// The minute of the day at `y` points from the grid's top, unsnapped and unclamped.
    pub fn minute_at(&self, y: f32) -> f32 {
        self.first_minute() as f32 + y / self.hour_height * 60.0
    }

    /// How many minutes `dy` points span.
    pub fn minutes(&self, dy: f32) -> f32 {
        dy / self.hour_height * 60.0
    }

    pub fn height(&self) -> f32 {
        self.y(self.last_minute())
    }

    /// Where a span of `length` minutes may start so it stays inside the grid.
    pub fn clamp_start(&self, start: i32, length: i32) -> i32 {
        start.clamp(self.first_minute(), (self.last_minute() - length).max(self.first_minute()))
    }

    pub fn clamp(&self, minute: i32) -> i32 {
        minute.clamp(self.first_minute(), self.last_minute())
    }
}

/// The nearest quarter hour.
pub fn snap(minute: f32) -> i32 {
    (minute / SNAP_MINUTES as f32).round() as i32 * SNAP_MINUTES
}

/// From `start` to `end`, in minutes after midnight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: i32,
    pub end: i32,
}

impl Span {
    pub fn new(start: i32, end: i32) -> Span {
        Span { start, end }
    }

    #[expect(clippy::len_without_is_empty, reason = "a span's length in minutes, not a collection")]
    pub fn len(&self) -> i32 {
        self.end - self.start
    }

    /// The same length, starting at `start`.
    pub fn moved_to(&self, start: i32) -> Span {
        Span { start, end: start + self.len() }
    }

    /// The part inside the grid's hours, or None when nothing of it shows.
    pub fn visible(&self, scale: &TimeScale) -> Option<Span> {
        let span = Span { start: self.start.max(scale.first_minute()), end: self.end.min(scale.last_minute()) };
        (span.end > span.start).then_some(span)
    }
}

/// Which of `count` side-by-side lanes an item takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lane {
    pub index: usize,
    pub count: usize,
}

/// Lanes for items in one day column: items that overlap (directly or through a chain) share a
/// group whose width splits into as many lanes as it needs at once; each item takes the first
/// lane free at its start. Returns one lane per input span, in input order.
pub fn lanes(spans: &[Span]) -> Vec<Lane> {
    let mut order: Vec<usize> = (0..spans.len()).collect();
    order.sort_by_key(|&i| (spans[i].start, -spans[i].end));
    let mut result = vec![Lane { index: 0, count: 1 }; spans.len()];
    let mut group: Vec<usize> = Vec::new();
    let mut lane_ends: Vec<i32> = Vec::new();
    let mut group_end = i32::MIN;
    for i in order {
        // A zero-length item still takes room.
        let span = Span { start: spans[i].start, end: spans[i].end.max(spans[i].start + 1) };
        if span.start >= group_end {
            close_group(&group, lane_ends.len(), &mut result);
            group.clear();
            lane_ends.clear();
        }
        let lane = match lane_ends.iter().position(|&end| end <= span.start) {
            Some(free) => free,
            None => {
                lane_ends.push(span.end);
                lane_ends.len() - 1
            }
        };
        lane_ends[lane] = span.end;
        result[i].index = lane;
        group.push(i);
        group_end = group_end.max(span.end);
    }
    close_group(&group, lane_ends.len(), &mut result);
    result
}

fn close_group(group: &[usize], count: usize, result: &mut [Lane]) {
    for &i in group {
        result[i].count = count.max(1);
    }
}
