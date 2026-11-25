//! The app's icons, as SVG bodies on a 16-point grid drawn in the element's colour.

use rune::Icon;

macro_rules! icon {
    ($name:ident, $body:expr) => {
        pub static $name: Icon = Icon { name: concat!("quest-", stringify!($name)), view: 16.0, body: $body };
    };
}

icon!(
    SEARCH,
    r#"<circle cx="7" cy="7" r="4.5" stroke="currentColor" stroke-width="1.2" fill="none"/><path d="M10.4 10.4L14 14" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>"#
);
icon!(PLAY, r#"<path d="M5 3.2L12.5 8L5 12.8Z" fill="currentColor"/>"#);
icon!(
    PAUSE,
    r#"<rect x="4" y="3" width="2.6" height="10" rx="0.6" stroke="currentColor" stroke-width="1.1" fill="none"/><rect x="9.4" y="3" width="2.6" height="10" rx="0.6" stroke="currentColor" stroke-width="1.1" fill="none"/>"#
);
icon!(
    CHECK,
    r#"<path d="M3.5 8.4L6.5 11.2L12.5 4.8" stroke="currentColor" stroke-width="1.4" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    FILTER,
    r#"<path d="M2.5 3h11L9.2 8.2V13L6.8 11.8V8.2Z" stroke="currentColor" stroke-width="1.1" fill="none" stroke-linejoin="round"/>"#
);
icon!(
    TO_TOP,
    r#"<path d="M3.5 2.5h9M8 13.5V5.5M4.8 8.5L8 5.3L11.2 8.5" stroke="currentColor" stroke-width="1.2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    TO_BOTTOM,
    r#"<path d="M3.5 13.5h9M8 2.5v8M4.8 7.5L8 10.7L11.2 7.5" stroke="currentColor" stroke-width="1.2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    ARROW_LEFT,
    r#"<path d="M13 8H3.5M7.5 4L3.5 8L7.5 12" stroke="currentColor" stroke-width="1.2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    CHEVRON_LEFT,
    r#"<path d="M10 3.5L5.5 8L10 12.5" stroke="currentColor" stroke-width="1.2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    CHEVRON_RIGHT,
    r#"<path d="M6 3.5L10.5 8L6 12.5" stroke="currentColor" stroke-width="1.2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    DATE,
    r#"<rect x="2.5" y="3.5" width="11" height="10" rx="1.5" stroke="currentColor" stroke-width="1.1" fill="none"/><path d="M2.5 6.5h11M5.5 2v3M10.5 2v3" stroke="currentColor" stroke-width="1.1" stroke-linecap="round"/>"#
);
icon!(CLOSE, r#"<path d="M4 4L12 12M12 4L4 12" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>"#);
icon!(PLUS, r#"<path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>"#);
icon!(
    EYE,
    r#"<path d="M1.5 8C3 5 5.3 3.5 8 3.5S13 5 14.5 8C13 11 10.7 12.5 8 12.5S3 11 1.5 8Z" stroke="currentColor" stroke-width="1.1" fill="none"/><circle cx="8" cy="8" r="2" stroke="currentColor" stroke-width="1.1" fill="none"/>"#
);
icon!(
    EYE_OFF,
    r#"<path d="M6.3 3.7A6.9 6.9 0 0 1 8 3.5C10.7 3.5 13 5 14.5 8a9.6 9.6 0 0 1-1.6 2.3M10.1 12.1A6.4 6.4 0 0 1 8 12.5C5.3 12.5 3 11 1.5 8a9.4 9.4 0 0 1 2.4-3M6.6 6.6a2 2 0 0 0 2.8 2.8M2.5 2.5l11 11" stroke="currentColor" stroke-width="1.1" fill="none" stroke-linecap="round"/>"#
);
icon!(
    SUN,
    r#"<circle cx="8" cy="8" r="2.8" stroke="currentColor" stroke-width="1.1" fill="none"/><path d="M8 1.5v1.8M8 12.7v1.8M1.5 8h1.8M12.7 8h1.8M3.4 3.4l1.3 1.3M11.3 11.3l1.3 1.3M3.4 12.6l1.3-1.3M11.3 4.7l1.3-1.3" stroke="currentColor" stroke-width="1.1" stroke-linecap="round"/>"#
);
icon!(
    MOON,
    r#"<path d="M13 9.5A5.5 5.5 0 1 1 6.5 3a4.5 4.5 0 0 0 6.5 6.5Z" stroke="currentColor" stroke-width="1.1" fill="none" stroke-linejoin="round"/>"#
);
icon!(
    MORE,
    r#"<circle cx="3.5" cy="8" r="1.3" fill="currentColor"/><circle cx="8" cy="8" r="1.3" fill="currentColor"/><circle cx="12.5" cy="8" r="1.3" fill="currentColor"/>"#
);
icon!(
    ARCHIVE,
    r#"<rect x="2" y="3" width="12" height="3" rx="0.8" stroke="currentColor" stroke-width="1.1" fill="none"/><path d="M3 6v6.2c0 .5.4.8.8.8h8.4c.4 0 .8-.3.8-.8V6M6.5 8.5h3" stroke="currentColor" stroke-width="1.1" fill="none" stroke-linecap="round"/>"#
);
icon!(
    TRASH,
    r#"<path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8.3c0 .4.4.7.8.7h4.2c.4 0 .8-.3.8-.7l.6-8.3" stroke="currentColor" stroke-width="1.1" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    REPEAT,
    r#"<path d="M3 7V6.2C3 5 4 4 5.2 4H13M11 2l2 2-2 2M13 9v.8C13 11 12 12 10.8 12H3M5 14l-2-2 2-2" stroke="currentColor" stroke-width="1.1" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    TROPHY,
    r#"<path d="M5 2.5h6v3.8a3 3 0 0 1-6 0V2.5ZM5 3.5H3v1.2A2.2 2.2 0 0 0 5.2 7M11 3.5h2v1.2A2.2 2.2 0 0 1 10.8 7M8 9.3v2.7M5.5 13.5h5M6.5 12h3" stroke="currentColor" stroke-width="1.1" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    OPEN,
    r#"<path d="M6 3.5H4a.5.5 0 0 0-.5.5v8c0 .3.2.5.5.5h8a.5.5 0 0 0 .5-.5v-2M9 3h4v4M13 3L7.5 8.5" stroke="currentColor" stroke-width="1.1" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    LAYERS,
    r#"<path d="M8 2.5L14 5.5L8 8.5L2 5.5Z" stroke="currentColor" stroke-width="1.2" fill="none" stroke-linejoin="round"/><path d="M2 8.25L8 11.25L14 8.25M2 10.75L8 13.75L14 10.75" stroke="currentColor" stroke-width="1.2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#
);
icon!(
    CHEVRON_DOWN,
    r#"<path d="M4.5 6.5L8 10L11.5 6.5" stroke="currentColor" stroke-width="1.2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#
);

/// The redesign's line icons, on a 24-point grid with 1.8-point strokes.
macro_rules! line_icon {
    ($name:ident, $paths:expr) => {
        pub static $name: Icon = Icon {
            name: concat!("quest-", stringify!($name)),
            view: 24.0,
            body: concat!(
                r#"<g fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">"#,
                $paths,
                "</g>"
            ),
        };
    };
}

line_icon!(INBOX, r#"<path d="M4 13h4l2 3h4l2-3h4"/><path d="M6 5h12l2 8v6H4v-6z"/>"#);
line_icon!(
    TODAY,
    r#"<circle cx="12" cy="12" r="4"/><path d="M12 2.5v2M12 19.5v2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M2.5 12h2M19.5 12h2M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4"/>"#
);
line_icon!(PLAN, r#"<rect x="4" y="5" width="16" height="15" rx="2"/><path d="M4 10h16M9 3v4M15 3v4"/>"#);
line_icon!(REVIEW, r#"<path d="M5 20V11M12 20V4M19 20v-6"/>"#);
line_icon!(LOGBOOK, r#"<path d="M5 4h11a3 3 0 0 1 3 3v13H8a3 3 0 0 1-3-3z"/><path d="M9 11.5l2 2 4-4"/>"#);
line_icon!(ADD, r#"<path d="M12 5v14M5 12h14"/>"#);
line_icon!(STOP, r#"<rect x="6.5" y="6.5" width="11" height="11" rx="2" fill="currentColor" stroke="none"/>"#);
line_icon!(RESUME, r#"<path d="M8 5.5v13l10.5-6.5z" fill="currentColor" stroke="none"/>"#);
line_icon!(BACK_TO, r#"<path d="M9 14L4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/>"#);
line_icon!(FOCUS, r#"<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="3"/>"#);
line_icon!(PANEL, r#"<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M15 4.5v15"/>"#);
line_icon!(COMPLETED, r#"<circle cx="12" cy="12" r="8"/><path d="M8.5 12.2l2.4 2.4 4.6-4.8"/>"#);
