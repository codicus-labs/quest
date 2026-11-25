//! The app's screens, how the mobile tabs cycle through them, and the desktop sidebar's order. Pure.

use quest_domain::EntityId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Home,
    /// Desktop sidebar: open tasks without a quest and a plan date.
    Inbox,
    /// Desktop sidebar: open tasks planned or due today or earlier.
    Today,
    /// Desktop sidebar: done tasks by day.
    Logbook,
    Plan,
    Focus,
    Quests,
    Review,
    /// Deleted and archived things, to restore.
    Archive,
    /// One quest's detail page.
    Quest(EntityId),
}

impl View {
    pub const ALL: [View; 5] = [View::Home, View::Plan, View::Focus, View::Quests, View::Review];
    /// The desktop sidebar's views, in ⌘1… order.
    pub const SIDEBAR: [View; 5] = [View::Inbox, View::Today, View::Plan, View::Logbook, View::Review];

    /// The tab after (or before) this view's, wrapping around: Tab and ⇧Tab.
    pub fn cycled(self, back: bool) -> View {
        let index = View::ALL.iter().position(|v| *v == self.tab()).unwrap_or(0);
        let step = if back { View::ALL.len() - 1 } else { 1 };
        View::ALL[(index + step) % View::ALL.len()]
    }

    pub fn title(self) -> &'static str {
        match self {
            View::Home => "Home",
            View::Inbox => "Inbox",
            View::Today => "Today",
            View::Logbook => "Logbook",
            View::Plan => "Plan",
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
