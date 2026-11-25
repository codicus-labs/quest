//! The view: the sidebar, the current view with its top band, the tracked-time column, and
//! overlays (command palette, account, Log work, shortcuts, a quest's dialog,
//! a session's editor). Focus fills the window on its own.

mod account;
mod archive;
mod auth_dialogs;
mod capture;
mod checklist;
#[cfg(test)]
mod checklist_tests;
pub mod chrome;
pub(crate) mod date_picker;
pub mod drag;
mod entry_editor;
mod focus;
pub(crate) mod lists;
mod log_work;
mod menu;
mod notes;
mod palette;
pub mod parts;
pub(crate) mod plan;
mod quest_detail;
mod quest_editor;
pub(crate) mod quest_picker;
mod quests;
pub(crate) mod recurrence;
pub(crate) mod review;
mod shortcuts;
pub mod sidebar;
pub(crate) mod task_fields;
mod task_modal;
mod task_picker;
mod task_row;
mod toast;
mod tracked;
pub(crate) mod workspaces;

#[cfg(test)]
mod ai_connections_tests;
#[cfg(test)]
mod auth_tests;
#[cfg(test)]
mod capture_tests;
#[cfg(test)]
mod delete_tests;
#[cfg(test)]
mod dnd_tests;
#[cfg(test)]
mod idle_tests;
#[cfg(test)]
mod jump_tests;
#[cfg(test)]
mod log_work_tests;
#[cfg(test)]
mod motion_tests;
#[cfg(test)]
mod quest_editor_tests;
#[cfg(test)]
mod quest_picker_tests;
#[cfg(test)]
mod quest_tests;
#[cfg(test)]
mod recurrence_tests;
#[cfg(test)]
mod sections_tests;
#[cfg(test)]
mod shell_tests;
#[cfg(test)]
mod shortcut_tests;
#[cfg(test)]
mod task_fields_tests;
#[cfg(test)]
mod task_modal_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod workspaces_tests;

use rune::{Cx, El, Tx, col, el, row, spacer, text};

use crate::icons;
use crate::state::{Overlay, State, View};
use crate::theme::{self, Pal, ty};

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = theme::pal(s.dark);
    let base = if s.view == View::Focus {
        col().key("app").w_full().h_full().fill(p.bg).child(focus::view(s, p, cx.chrome_insets().leading))
    } else {
        shell(s, p, cx)
    };
    let mut layer = Vec::new();
    match s.overlay {
        Overlay::None => {}
        Overlay::Palette => palette::layer(s, p, &mut layer),
        Overlay::Account => account::layer(s, p, &mut layer),
        Overlay::LogWork => log_work::layer(s, p, &mut layer),
        Overlay::Shortcuts => shortcuts::layer(p, &mut layer),
        Overlay::RepeatEditor => recurrence::editor_layer(s, p, &mut layer),
        Overlay::RecurringTasks => recurrence::overview_layer(s, p, &mut layer),
        Overlay::Capture => capture::layer(s, p, &mut layer),
        Overlay::GuestImport => auth_dialogs::guest_import_layer(s, p, &mut layer),
        Overlay::AiNotice => auth_dialogs::ai_notice_layer(p, &mut layer),
        Overlay::Quest => quest_editor::layer(s, p, &mut layer),
        Overlay::Workspaces => workspaces::manage_layer(s, p, &mut layer),
        Overlay::EntryEditor => entry_editor::layer(s, p, &mut layer),
    }
    task_fields::layer(s, p, cx, &mut layer);
    quest_picker::layer(s, p, cx, &mut layer);
    quest_detail::color_layer(s, p, &mut layer);
    toast::layer(s, p, &mut layer);
    menu::layer(s, p, &mut layer);
    if s.view != View::Focus {
        workspaces::menu_layer(s, p, &mut layer);
    }
    drag::layer(s, p, &mut layer);
    rune_kit::root(base, layer).on_background_press_tx(task_modal::close)
}

/// Sidebar, the main column (its top band, the view, a notice), and the tracked column when on.
fn shell(s: &State, p: &Pal, cx: &mut Cx<State>) -> El {
    let page = match s.view {
        View::Plan => plan::view(s, p),
        View::Quests => quests::view(s, p),
        View::Review => review::view(s, p),
        View::Archive => archive::view(s, p),
        View::Quest(id) => quest_detail::view(s, id, p),
        View::Home | View::Today | View::Inbox | View::Logbook | View::Focus => lists::view(s, p),
    };
    let band = row()
        .key("main-band")
        .w_full()
        .h(sidebar::TITLEBAR)
        .pr(14.0)
        .gap(4.0)
        .center_y()
        .window_drag()
        .child(spacer())
        .child(chrome::search_button(p))
        .children((s.view != View::Review).then(|| right_column_toggle(s, p)));
    let mut main =
        col().key("main").grow(1.0).min_w(0.0).h_full().fill(p.bg).child(band).child(page.grow(1.0).min_h(0.0));
    if let Some(notice) = &s.hotkey_notice {
        main = main.child(notice_bar(notice, p).key("hotkey-notice"));
    }
    if let Some(notice) = &s.notice {
        main = main.child(notice_bar(notice, p));
    }
    row()
        .key("app")
        .w_full()
        .h_full()
        .fill(p.bg)
        .child(sidebar::view(s, p))
        .child(main)
        .children((s.right_column || s.view == View::Review).then(|| tracked::column(s, p, cx)))
}

fn right_column_toggle(s: &State, p: &Pal) -> El {
    let label = if s.right_column { "Hide tracked time" } else { "Show tracked time" };
    el().key("right-column-toggle")
        .role("button")
        .label(label)
        .checked(s.right_column)
        .focusable(true)
        .size(32.0, 32.0)
        .center()
        .radius(8.0)
        .cursor(rune::Cursor::Pointer)
        .hover(p.hover)
        .help(format!("{label}  T  ⌥⌘I"))
        .child(rune::icon(&icons::PANEL, 18.0).color(if s.right_column { p.muted } else { p.faint }))
        .on_press_tx(|s: &mut State, _: &mut Tx| crate::keys::toggle_tracked(s))
}

fn notice_bar(notice: &str, p: &Pal) -> El {
    row()
        .key("notice")
        .w_full()
        .h(34.0)
        .px(24.0)
        .center_y()
        .fill(p.card)
        .border_sides([1.0, 0.0, 0.0, 0.0], p.border)
        .child(text(notice.to_owned()).font(ty::SMALL).color(p.muted))
}
