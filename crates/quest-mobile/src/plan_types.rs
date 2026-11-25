//! The Plan screen's records: a window of days, each with its QUEST WORK and OTHER cards.

use crate::types::{HomeTask, QuestChip, UndoToast};

/// The days built for the plan: `count` days from `first_day` ("2026-11-24").
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct PlanWindow {
    pub first_day: String,
    pub count: u32,
}

/// One day of the plan: a section of the compact list, or a column.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct PlanDay {
    /// "2026-11-24"; also the key of the day's section or column.
    pub day_iso: String,
    /// The column head: "Tue" and "24", as on the desktop.
    pub weekday: String,
    pub day_number: String,
    /// The compact list's sticky head: "Tuesday, November 24".
    pub label: String,
    /// "November 2026", for the month header.
    pub month_label: String,
    pub is_today: bool,
    /// Before today: dimmed, without an add field.
    pub is_past: bool,
    /// Open cards first, then done ones; repeats planned for the day included.
    pub quest_work: Vec<HomeTask>,
    pub other: Vec<HomeTask>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct PlanData {
    pub window: PlanWindow,
    pub days: Vec<PlanDay>,
    pub today_iso: String,
    /// The filter's chip. None: no filter, or its quest is gone.
    pub quest_filter: Option<QuestChip>,
    /// Active quests, for the filter menu.
    pub quests: Vec<QuestChip>,
    pub toast: Option<UndoToast>,
}

/// One entry of "Move to…": "Today", "Tomorrow", the next five days ("Thursday, Nov 26"), and
/// "Backlog · someday" (no day).
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MoveTarget {
    pub label: String,
    pub day_iso: Option<String>,
}
