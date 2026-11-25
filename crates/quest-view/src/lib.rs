//! Quest's presentation logic, free of any UI framework: formatting, the lists (Today, Inbox,
//! Logbook), dialogs' parsing and validation, the palette, menus and undo.
//! Pure: no IO, no clock (time is passed in), so every client (Rune desktop, Android) can share it.

pub mod calendar;
pub mod entry_editor;
pub mod focus;
pub mod format;
pub mod home;
pub mod lists;
pub mod log_work;
pub mod menu;
pub mod nav;
pub mod palette;
pub mod plan_strip;
pub mod player;
pub mod quest_picker;
pub mod quests;
pub mod quick_add;
pub mod reorder;
pub mod repeat;
pub mod row_meta;
pub mod tracked;
pub mod undo;
pub mod workspaces;
