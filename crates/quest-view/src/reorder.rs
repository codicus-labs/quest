//! Reordering a list one step at a time, pure: the keyboard's ⌥↑ / ⌥↓ on the desktop and the
//! accessibility actions "Move up" / "Move down" on mobile.

#[cfg(test)]
mod tests;

use quest_domain::{EntityId, Position};

/// The keyboard's reorder (⌥↑ / ⌥↓): where `id` goes to move one place up or down in `order`,
/// the list as shown. None at either end.
pub fn nudge(order: &[EntityId], id: EntityId, up: bool) -> Option<Position> {
    let index = order.iter().position(|t| *t == id)?;
    if up {
        return index.checked_sub(1).map(|before| Position::Before(order[before]));
    }
    match order.get(index + 2) {
        Some(after_next) => Some(Position::Before(*after_next)),
        None if index + 1 < order.len() => Some(Position::Bottom),
        None => None,
    }
}
