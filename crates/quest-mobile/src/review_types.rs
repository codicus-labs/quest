//! Records of the weekly review. Bars come as fractions (0 to 1) of their chart, so the UI only
//! scales them to its size.

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ReviewSegment {
    pub quest_id: String,
    pub color_hex: String,
    /// Its share of the chart's height.
    pub fraction: f32,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ReviewDay {
    /// "Mon".
    pub label: String,
    /// "0.3h".
    pub hours: String,
    pub is_today: bool,
    /// The bar's height: the largest day (at least 1 h, like the desktop) is 1.
    pub fraction: f32,
    /// Stacked from the bottom, in `ReviewData::by_quest`'s order.
    pub segments: Vec<ReviewSegment>,
    /// "Monday: 0.3h. Photo series 0.2h, Garden 0.1h".
    pub a11y_label: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ReviewQuest {
    pub id: String,
    pub name: String,
    pub color_hex: String,
    /// "0.3h".
    pub hours: String,
    /// Of the quest with the most time.
    pub fraction: f32,
    /// "Photo series: 2h".
    pub a11y_label: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ReviewWeek {
    /// Its Monday, "2026-11-23": tap to review that week.
    pub week_start: String,
    /// "Nov 23".
    pub label: String,
    /// "0.3h".
    pub hours: String,
    /// Of the week with the most Quest Hours (at least 1 h).
    pub fraction: f32,
    pub selected: bool,
    /// "Week of Nov 23: 3.5h".
    pub a11y_label: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ReviewData {
    /// The week's Monday, "2026-11-23".
    pub week_start: String,
    /// "Nov 23 – 29".
    pub week_label: String,
    /// Mondays of the weeks before and after.
    pub previous_week: String,
    pub next_week: String,
    /// Whether this is the current week (the UI hides "This week" then).
    pub is_this_week: bool,
    /// "0.3h".
    pub quest_hours: String,
    /// "/ 20h".
    pub target: String,
    /// "1% of target".
    pub target_note: String,
    /// 0 to 1.
    pub target_progress: f32,
    /// "0.3h".
    pub daily_average: String,
    /// "100%".
    pub quest_share: String,
    /// Monday to Sunday.
    pub days: Vec<ReviewDay>,
    /// Most time first; empty: "No quest time this week".
    pub by_quest: Vec<ReviewQuest>,
    /// "Hours by quest: Photo series 2h, Garden 1.5h", or "No quest time this week".
    pub by_quest_a11y: String,
    /// "Quest work · 0.3h".
    pub quest_work: String,
    /// "Other · 0h".
    pub other: String,
    /// Quest work's share of the split bar; 0 without tracked time.
    pub quest_fraction: f32,
    /// "Quest work 0.3h, other 0h: 100% quest work".
    pub split_a11y: String,
    /// The 8 weeks up to this one, oldest first.
    pub weeks: Vec<ReviewWeek>,
    /// "Quest hours of recent weeks: Oct 5 2h, …".
    pub weeks_a11y: String,
}
