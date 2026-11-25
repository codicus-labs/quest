//! Cleaning text that comes from the user's data or the model.

/// Collapses whitespace (including line breaks) and drops control characters.
pub(crate) fn single_line(text: &str) -> String {
    let visible: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    visible.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Trims, keeps line breaks, drops other control characters.
pub(crate) fn multi_line(text: &str) -> String {
    text.chars().filter(|c| *c == '\n' || !c.is_control()).collect::<String>().trim().to_owned()
}

/// At most `max` characters, with "…" if shortened.
pub(crate) fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut short: String = text.chars().take(max.saturating_sub(1)).collect();
    short.push('…');
    short
}
