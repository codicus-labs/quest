//! The Quest theme: colour tokens as data (`assets/theme.json`, read with Rune's `Theme`), the
//! quest accent colours, type styles, and a rune-kit `Kit` seeded from the same tokens so kit
//! components (text fields, dialogs) match the app.

use std::sync::OnceLock;

use quest_domain::QuestColor;
use rune::color::hex;
use rune::{Color, TextStyle, Theme};
use rune_kit::{Kit, PaletteSeed};

const THEME_JSON: &str = include_str!("../assets/theme.json");

/// Resolved colours for one appearance.
pub struct Pal {
    pub bg: Color,
    pub bar: Color,
    pub card: Color,
    pub card_raised: Color,
    pub border: Color,
    pub border_strong: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub chip: Color,
    pub hover: Color,
    pub selected: Color,
    pub grid: Color,
    pub grid_line: Color,
    pub bar_track: Color,
    pub bar_idle: Color,
    pub accent: Color,
    /// Errors and failures (sync failed).
    pub error: Color,
    pub scrim: Color,
    pub kit: Kit,
}

fn build(dark: bool) -> Pal {
    let theme = Theme::from_json(THEME_JSON, dark).unwrap_or_else(|e| panic!("assets/theme.json is invalid: {e}"));
    let c = |name: &str| theme.color(name);
    let kit = Kit::from_palette(PaletteSeed { bg: c("card"), fg: c("text"), accent: c("accent") });
    Pal {
        bg: c("bg"),
        bar: c("bar"),
        card: c("card"),
        card_raised: c("card-raised"),
        border: c("border"),
        border_strong: c("border-strong"),
        text: c("text"),
        muted: c("text-muted"),
        faint: c("text-faint"),
        chip: c("chip"),
        hover: c("hover"),
        selected: c("selected"),
        grid: c("grid"),
        grid_line: c("grid-line"),
        bar_track: c("bar-track"),
        bar_idle: c("bar-idle"),
        accent: c("accent"),
        error: c("error"),
        scrim: c("scrim"),
        kit,
    }
}

pub fn pal(dark: bool) -> &'static Pal {
    static DARK: OnceLock<Pal> = OnceLock::new();
    static LIGHT: OnceLock<Pal> = OnceLock::new();
    if dark { DARK.get_or_init(|| build(true)) } else { LIGHT.get_or_init(|| build(false)) }
}

/// A quest's accent colour.
pub fn quest_color(color: QuestColor) -> Color {
    let code = match color {
        QuestColor::Red => "#d9625c",
        QuestColor::Orange => "#d98a4e",
        QuestColor::Yellow => "#d9b84a",
        QuestColor::Lime => "#b3c94c",
        QuestColor::Green => "#62b87a",
        QuestColor::Teal => "#4cb5a8",
        QuestColor::Blue => "#5c8fd9",
        QuestColor::Violet => "#906bd9",
        QuestColor::Pink => "#d96ba8",
    };
    hex(code).unwrap_or(Color::WHITE)
}

pub use quest_view::quests::color_name;

/// Type scale (Inter, which Rune embeds).
pub mod ty {
    use super::TextStyle;

    pub const BODY: TextStyle = TextStyle::new(14.0, 400.0).lh(20.0);
    pub const BODY_MEDIUM: TextStyle = TextStyle::new(14.0, 500.0).lh(20.0);
    pub const SMALL: TextStyle = TextStyle::new(12.0, 400.0).lh(16.0);
    pub const SMALL_MEDIUM: TextStyle = TextStyle::new(12.0, 500.0).lh(16.0);
    pub const CAPS: TextStyle = TextStyle::new(11.0, 500.0).lh(14.0).tracking(1.4);
    pub const NAV: TextStyle = TextStyle::new(14.0, 500.0).lh(20.0);
    pub const TASK: TextStyle = TextStyle::new(15.0, 400.0).lh(21.0);
    pub const HEADING: TextStyle = TextStyle::new(30.0, 500.0).lh(38.0).tracking(-0.4);
    pub const STAT: TextStyle = TextStyle::new(20.0, 500.0).lh(26.0);
    pub const BIG_STAT: TextStyle = TextStyle::new(30.0, 500.0).lh(36.0);
    pub const FOCUS_TITLE: TextStyle = TextStyle::new(26.0, 400.0).lh(32.0);
    pub const TIMER: TextStyle = TextStyle::new(120.0, 500.0).lh(130.0).tracking(-2.0).features("'tnum' 1");
    pub const MONO_TIME: TextStyle = TextStyle::new(14.0, 500.0).lh(20.0).features("'tnum' 1");
}
