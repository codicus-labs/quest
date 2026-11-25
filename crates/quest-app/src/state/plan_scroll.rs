//! Where the planner's day strip is scrolled to. Rune owns the scroll (wheel, trackpad,
//! momentum, glides) and tells the board's `on_scroll_offset` where it is before each frame
//! draws; that is the offset the app reads. The app scrolls it itself to restore the place
//! when the view mounts again, after a resize, for Earlier / Later / Today, and while a drag
//! rests at the board's edge.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use chrono::{Days, NaiveDate};
use rune::{ScrollPos, Tx};

use super::{State, View};
use crate::dnd::Rect;
use crate::plan_strip::{self, Strip, Window};
use crate::ui::drag::{self, Zone};

/// The key of the strip's horizontal scroll viewport.
pub const BOARD: &str = "plan-board";
/// How often an edge drag scrolls on while the pointer holds still.
const EDGE_TICK: Duration = Duration::from_millis(16);

/// The offset as Rune draws it, shared with the board's `on_scroll_offset`, which can't reach
/// the state.
#[derive(Default)]
struct Reported {
    offset: Cell<f32>,
    /// An offset asked of Rune that it hasn't drawn yet: reports until then are of the old
    /// place (a board just mounted at 0) and don't count.
    pending: Cell<Option<f32>>,
}

impl Reported {
    fn report(&self, offset: f32) {
        if let Some(pending) = self.pending.get() {
            if (offset - pending).abs() > 0.5 {
                return;
            }
            self.pending.set(None);
        }
        self.offset.set(offset);
    }
}

pub struct PlanScroll {
    pub strip: Strip,
    reported: Rc<Reported>,
    /// The month the header shows, as the view last built it (its first day).
    built_month: NaiveDate,
    /// The days the view last built.
    pub built: Window,
    /// Whether the viewport (newly mounted) still has to be put at the offset.
    restore: bool,
    edge: EdgeScroll,
}

/// A drag resting in the board's edge zone, scrolling on the clock.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum EdgeScroll {
    /// No tick is due: no drag at the edge, or one just arrived and its first tick is asked.
    #[default]
    Off,
    /// It scrolled at this time on the app clock, and the next tick is asked.
    Ticking { last: f64 },
    /// A tick moved nothing (the strip's end, or too slow to move a point): no more ticks
    /// until the pointer moves.
    Stalled,
}

impl PlanScroll {
    /// A strip showing `from` first, in a viewport about `viewport_w` wide (the real width
    /// comes with the first layout).
    pub fn new(from: NaiveDate, viewport_w: f32) -> PlanScroll {
        let strip = Strip::around(from, viewport_w);
        let reported = Rc::new(Reported::default());
        reported.offset.set(strip.offset_for(from));
        let offset = reported.offset.get();
        let (built, built_month) = (strip.window(offset, None), strip.month_at(offset));
        PlanScroll { strip, reported, built_month, built, restore: true, edge: EdgeScroll::Off }
    }

    /// How far the strip is scrolled, in points from its start.
    pub fn offset(&self) -> f32 {
        self.reported.offset.get()
    }

    /// The day shown first: the one most of whose column shows at the board's left.
    pub fn from(&self) -> NaiveDate {
        self.strip.first_visible(self.offset())
    }

    /// For the board's `on_scroll_offset`.
    pub fn on_scroll(&self) -> impl Fn(ScrollPos) + 'static {
        let reported = Rc::clone(&self.reported);
        move |at: ScrollPos| reported.report(at.offset[0])
    }

    /// Scrolls to `offset` at once; reports of the old place don't count until it shows.
    fn jump(&mut self, offset: f32, tx: &mut Tx) {
        self.reported.offset.set(offset);
        self.reported.pending.set(Some(offset));
        tx.scroll_to_x(BOARD, offset);
    }
}

impl State {
    /// Mounting the planner again starts it where it was.
    pub(super) fn plan_mounts(&mut self) {
        self.plan.restore = true;
    }

    /// The board viewport was laid out (mounted, moved or resized). A new width keeps the
    /// first day where it was.
    pub fn plan_board_laid_out(&mut self, rect: Rect, tx: &mut Tx) {
        self.zones.insert(Zone::PlanBoard, rect);
        let plan = &mut self.plan;
        if (rect.w - plan.strip.viewport_w).abs() <= 0.5 {
            return tx.live_only();
        }
        let (strip, offset) = plan.strip.resized(plan.offset(), rect.w);
        plan.strip = strip;
        plan.jump(offset, tx);
    }

    /// Switches views from a handler. A scroll request lands in the commit of the update that
    /// asked for it, so asking here puts a planner that mounts at its place in the frame that
    /// first draws it.
    pub fn go_with(&mut self, view: View, tx: &mut Tx) {
        if self.view != view && View::ALL.contains(&view) {
            tx.instant();
        }
        self.go(view);
        self.restore_plan_scroll(tx);
    }

    /// Puts a newly mounted board at the strip's offset. `go_with` asks for it as the board
    /// mounts; `poll` catches the rest (a scene's first frame), a frame late.
    fn restore_plan_scroll(&mut self, tx: &mut Tx) {
        if self.view != View::Plan || !self.plan.restore {
            return;
        }
        self.plan.restore = false;
        let offset = self.plan.offset();
        self.plan.jump(offset, tx);
    }

    /// The strip moved (its leading spacer reports every frame of a scroll, a frame late).
    /// Re-runs the view only when the days to build or the month change: Rune moves the
    /// built days itself.
    pub fn plan_strip_moved(&mut self, tx: &mut Tx) {
        let keep = drag::dragged_day(self);
        let plan = &mut self.plan;
        let offset = plan.offset();
        let (built, month) = (plan.strip.next_window(plan.built, offset, keep), plan.strip.month_at(offset));
        if (built, month) == (plan.built, plan.built_month) {
            return tx.live_only();
        }
        plan.built = built;
        plan.built_month = month;
    }

    /// Glides the strip so `day` shows first (Earlier, Later, Today).
    pub fn glide_plan_to(&mut self, day: NaiveDate, tx: &mut Tx) {
        let offset = self.plan.strip.offset_for(day);
        self.plan.reported.pending.set(None);
        tx.scroll_to(BOARD, offset, 0.0);
    }

    /// Glides the strip by `days` from the day shown first.
    pub fn glide_plan_by(&mut self, days: i64, tx: &mut Tx) {
        let from = self.plan.from();
        let day = if days < 0 { from - Days::new(days.unsigned_abs()) } else { from + Days::new(days as u64) };
        self.glide_plan_to(day, tx);
    }

    /// How fast a drag resting where it is scrolls the strip: only a task over the board's
    /// edge zones, on Plan.
    fn plan_edge_speed(&self) -> f32 {
        let Some(pointer) = drag::task_pointer(self) else { return 0.0 };
        if self.view != View::Plan {
            return 0.0;
        }
        let Some(board) = self.zones.get(&Zone::PlanBoard) else { return 0.0 };
        if pointer.y < board.y || pointer.y > board.y + board.h {
            return 0.0;
        }
        plan_strip::edge_speed(pointer.x, board.x, board.x + board.w)
    }

    /// Keeps an edge drag scrolling on the clock while the pointer holds still, and the drop
    /// target on the day now under it. Stops as soon as the drag leaves the edge or ends.
    /// True when it scrolled.
    pub fn plan_edge_scroll(&mut self, clock: f64, tx: &mut Tx) -> bool {
        self.restore_plan_scroll(tx);
        let speed = self.plan_edge_speed();
        if speed == 0.0 {
            self.plan.edge = EdgeScroll::Off;
            return false;
        }
        let dt = match self.plan.edge {
            EdgeScroll::Stalled => return false,
            EdgeScroll::Off => 0.0,
            EdgeScroll::Ticking { last } => (clock - last).clamp(0.0, 0.1),
        };
        let offset = plan_strip::scrolled(&self.plan.strip, self.plan.offset(), speed, dt);
        if offset == self.plan.offset() && dt > 0.0 {
            self.plan.edge = EdgeScroll::Stalled;
            return false;
        }
        self.plan.edge = EdgeScroll::Ticking { last: clock };
        tx.wake_after(EDGE_TICK);
        if offset == self.plan.offset() {
            return false;
        }
        self.plan.jump(offset, tx);
        drag::follow_scroll(self);
        true
    }

    /// Asks for an edge tick when a drag move reaches an edge zone, or moves within one that
    /// stalled (the new speed may move it).
    pub fn plan_edge_check(&mut self, tx: &mut Tx) {
        if matches!(self.plan.edge, EdgeScroll::Ticking { .. }) || self.plan_edge_speed() == 0.0 {
            return;
        }
        self.plan.edge = EdgeScroll::Off;
        tx.wake_after(EDGE_TICK);
    }
}
