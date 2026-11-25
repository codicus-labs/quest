//! Quest's presentation logic, free of any UI framework: formatting, the time grid's geometry,
//! dialogs' parsing and validation, the palette, menus and undo. Pure: no IO, no clock (time is
//! passed in), so every client (Rune desktop, Android) can share it.

pub mod entry_editor;
pub mod focus;
pub mod format;
pub mod home;
pub mod log_work;
pub mod menu;
pub mod nav;
pub mod palette;
pub mod plan_strip;
pub mod quest_picker;
pub mod quests;
pub mod reorder;
pub mod repeat;
pub mod timescale;
pub mod undo;
pub mod workspaces;
