//! The planner's day strip, pure: one long row of day columns, `VISIBLE_DAYS` to a viewport,
//! that scrolls sideways years either way. Only a window of days around the scroll offset is
//! built (`window`); a spacer as wide as the days left out keeps every built day exactly where
//! the full strip has it (`x_of`), so days that come and go never move what shows. The drag's
//! column under the pointer (`day_at`) and its auto-scroll at the board's edges (`edge_speed`)
//! are read from the offset, never from rects reported a frame ago.

#[cfg(test)]
mod tests;

use chrono::{Datelike, Days, NaiveDate};

/// How many day columns fill the viewport.
pub const VISIBLE_DAYS: usize = 5;
/// The space between two day columns.
pub const GAP: f32 = 12.0;
/// How far the strip reaches either side of the day it was centred on.
pub const REACH_DAYS: u64 = 3 * 366;
/// Days built beyond each visible edge.
const OVERSCAN_DAYS: i64 = 4;
/// The window moves a week at a time, so a scroll re-runs the view rarely.
const WINDOW_STEP: i64 = 7;
/// A built window holds while at least this many built days stay beyond each visible edge, so
/// a scroll that turns back near where the window last moved doesn't move it again.
pub const HOLD_DAYS: i64 = 2;
/// A column is never narrower than this, however narrow the window.
const MIN_COLUMN_W: f32 = 120.0;
/// How far into the board an edge zone reaches, and the fastest a drag there scrolls (points a
/// second, right at the edge).
pub const EDGE_ZONE: f32 = 80.0;
pub const EDGE_MAX_SPEED: f32 = 1400.0;

/// The strip's geometry: the day at its very start and the width of the viewport showing it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strip {
    pub start: NaiveDate,
    pub viewport_w: f32,
}

/// The days built: `count` days from `first`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub first: NaiveDate,
    pub count: usize,
}

impl Window {
    pub fn days(self) -> impl Iterator<Item = NaiveDate> {
        (0..self.count as u64).map(move |i| self.first + Days::new(i))
    }
}

impl Strip {
    /// A strip reaching `REACH_DAYS` either side of `center`.
    pub fn around(center: NaiveDate, viewport_w: f32) -> Strip {
        Strip { start: center - Days::new(REACH_DAYS), viewport_w }
    }

    pub fn len_days(&self) -> i64 {
        2 * REACH_DAYS as i64 + 1
    }

    pub fn column_w(&self) -> f32 {
        let gaps = GAP * (VISIBLE_DAYS - 1) as f32;
        ((self.viewport_w - gaps) / VISIBLE_DAYS as f32).max(MIN_COLUMN_W)
    }

    pub fn stride(&self) -> f32 {
        self.column_w() + GAP
    }

    /// Every column with the gap after it, the last one's too.
    pub fn content_w(&self) -> f32 {
        self.len_days() as f32 * self.stride()
    }

    pub fn max_offset(&self) -> f32 {
        (self.content_w() - self.viewport_w).max(0.0)
    }

    pub fn clamp(&self, offset: f32) -> f32 {
        offset.clamp(0.0, self.max_offset())
    }

    fn index(&self, day: NaiveDate) -> i64 {
        (day - self.start).num_days()
    }

    fn day(&self, index: i64) -> NaiveDate {
        let index = index.clamp(0, self.len_days() - 1);
        self.start + Days::new(index as u64)
    }

    /// Where `day`'s column starts in the strip.
    pub fn x_of(&self, day: NaiveDate) -> f32 {
        self.index(day) as f32 * self.stride()
    }

    /// The offset that shows `day` as the first column.
    pub fn offset_for(&self, day: NaiveDate) -> f32 {
        self.clamp(self.x_of(day))
    }

    /// The day shown first at `offset`: the one most of whose column is on screen at the left.
    pub fn first_visible(&self, offset: f32) -> NaiveDate {
        self.day((offset / self.stride()).round() as i64)
    }

    /// The day whose column (or the gap after it) is `x` points into the viewport at `offset`.
    pub fn day_at(&self, offset: f32, x: f32) -> NaiveDate {
        self.day(((offset + x) / self.stride()).floor() as i64)
    }

    /// The days to build at `offset`: what shows plus a few either side, in whole weeks of the
    /// strip, and `keep` (a dragged card's own day, which holds the drag) however far off.
    pub fn window(&self, offset: f32, keep: Option<NaiveDate>) -> Window {
        let stride = self.stride();
        let shown_from = (offset / stride).floor() as i64;
        let shown = (self.viewport_w / stride).ceil() as i64 + 1;
        // Both ends move together, once a week: from a week boundary, enough whole weeks to
        // cover what shows and the overscan wherever in its week the scroll is.
        let mut from = (shown_from - OVERSCAN_DAYS).div_euclid(WINDOW_STEP) * WINDOW_STEP;
        let span = shown + 2 * OVERSCAN_DAYS + WINDOW_STEP - 1;
        let mut to = from + (span + WINDOW_STEP - 1).div_euclid(WINDOW_STEP) * WINDOW_STEP;
        if let Some(keep) = keep.map(|d| self.index(d)) {
            from = from.min(keep);
            to = to.max(keep + 1);
        }
        let from = from.clamp(0, self.len_days());
        let to = to.clamp(from, self.len_days());
        Window { first: self.day(from), count: (to - from) as usize }
    }

    /// The days to build at `offset` when `built` are built: `built` while it still holds
    /// `HOLD_DAYS` either side of what shows (and `keep`), else a fresh `window`.
    pub fn next_window(&self, built: Window, offset: f32, keep: Option<NaiveDate>) -> Window {
        let stride = self.stride();
        let shown_from = (offset / stride).floor() as i64;
        let shown_to = ((offset + self.viewport_w) / stride).ceil() as i64;
        let (from, to) = (self.index(built.first), self.index(built.first) + built.count as i64);
        let wanted_from = (shown_from - HOLD_DAYS).max(0);
        let wanted_to = (shown_to + HOLD_DAYS).min(self.len_days());
        let holds_keep = keep.map(|d| self.index(d)).is_none_or(|k| from <= k && k < to);
        if from <= wanted_from && wanted_to <= to && holds_keep {
            return built;
        }
        self.window(offset, keep)
    }

    /// The month the middle of the board shows at `offset`, as its first day.
    pub fn month_at(&self, offset: f32) -> NaiveDate {
        let middle = self.first_visible(offset) + Days::new(VISIBLE_DAYS as u64 / 2);
        middle.with_day(1).expect("every month has a first day")
    }

    /// The same strip in a viewport `viewport_w` wide, and the offset that keeps the same point
    /// of the same day at the viewport's left edge.
    pub fn resized(&self, offset: f32, viewport_w: f32) -> (Strip, f32) {
        let next = Strip { viewport_w, ..*self };
        (next, next.clamp(offset / self.stride() * next.stride()))
    }
}

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

/// The offset after scrolling at `speed` for `dt` seconds.
pub fn scrolled(strip: &Strip, offset: f32, speed: f32, dt: f64) -> f32 {
    strip.clamp(offset + speed * dt as f32)
}
