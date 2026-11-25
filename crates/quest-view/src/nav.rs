//! The app's screens and how the top bar's tabs cycle through them. Pure.

use quest_domain::EntityId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Home,
    Plan,
    Calendar,
    Focus,
    Quests,
    Review,
    /// Deleted and archived things, to restore.
    Archive,
    /// One quest's detail page.
    Quest(EntityId),
}

impl View {
    pub const ALL: [View; 6] = [View::Home, View::Plan, View::Calendar, View::Focus, View::Quests, View::Review];

    /// The tab after (or before) this view's, wrapping around: Tab and ⇧Tab.
    pub fn cycled(self, back: bool) -> View {
        let index = View::ALL.iter().position(|v| *v == self.tab()).unwrap_or(0);
        let step = if back { View::ALL.len() - 1 } else { 1 };
        View::ALL[(index + step) % View::ALL.len()]
    }

    pub fn title(self) -> &'static str {
        match self {
            View::Home => "Home",
            View::Plan => "Plan",
            View::Calendar => "Calendar",
            View::Focus => "Focus",
            View::Quests => "Quests",
            View::Review => "Review",
            View::Archive => "Archive",
            View::Quest(_) => "Quest",
        }
    }

    /// The top bar's tab this view belongs to.
    pub fn tab(self) -> View {
        match self {
            View::Archive | View::Quest(_) => View::Quests,
            view => view,
        }
    }
}
