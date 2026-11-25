//! The view: top bar, the current view, and overlays (command palette, account, Log work, shortcuts,
//! a task's dialog, a quest's dialog).

mod account;
mod archive;
mod auth_dialogs;
mod capture;
pub(crate) mod date_picker;
pub mod drag;
mod focus;
mod home;
mod log_work;
mod menu;
mod notes;
mod palette;
pub mod parts;
mod plan;
mod quest_detail;
mod quest_editor;
mod quest_picker;
mod quests;
mod recurrence;
mod review;
mod shortcuts;
mod task_modal;
mod task_picker;
mod toast;
pub mod topbar;
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
mod dnd_room_tests;
#[cfg(test)]
mod dnd_scroll_tests;
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
mod plan_scroll_tests;
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
mod shortcut_tests;
#[cfg(test)]
mod task_modal_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod title_click_tests;
#[cfg(test)]
mod workspaces_tests;

use rune::{Cx, El, col, row, text};

use crate::state::{Overlay, State, View};
use crate::theme::{self, Pal, ty};

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = theme::pal(s.dark);
    let page = match s.view {
        View::Home => home::view(s, p),
        View::Plan => plan::view(s, p, cx),
        View::Focus => focus::view(s, p, cx.chrome_insets().leading),
        View::Quests => quests::view(s, p),
        View::Review => review::view(s, p),
        View::Archive => archive::view(s, p),
        View::Quest(id) => quest_detail::view(s, id, p),
    };
    let mut base = col().key("app").w_full().h_full().fill(p.bg);
    if s.view != View::Focus {
        base = base.child(topbar::topbar(s, p, cx)).child(rune::el().w_full().h(1.0).fill(p.border));
    }
    base = base.child(page.grow(1.0).min_h(0.0));
    if let Some(notice) = &s.notice {
        base = base.child(notice_bar(notice, p));
    }
    let mut layer = Vec::new();
    match s.overlay {
        Overlay::None => {}
        Overlay::Palette => palette::layer(s, p, &mut layer),
        Overlay::Account => account::layer(s, p, &mut layer),
        Overlay::LogWork => log_work::layer(s, p, &mut layer),
        Overlay::Shortcuts => shortcuts::layer(p, &mut layer),
        Overlay::Task => task_modal::layer(s, p, &mut layer),
        Overlay::RepeatEditor => recurrence::editor_layer(s, p, &mut layer),
        Overlay::RecurringTasks => recurrence::overview_layer(s, p, &mut layer),
        Overlay::Capture => capture::layer(s, p, &mut layer),
        Overlay::GuestImport => auth_dialogs::guest_import_layer(s, p, &mut layer),
        Overlay::AiNotice => auth_dialogs::ai_notice_layer(p, &mut layer),
        Overlay::Quest => quest_editor::layer(s, p, &mut layer),
        Overlay::Workspaces => workspaces::manage_layer(s, p, &mut layer),
    }
    quest_picker::layer(s, p, cx, &mut layer);
    toast::layer(s, p, &mut layer);
    menu::layer(s, p, &mut layer);
    if s.view != View::Focus {
        workspaces::menu_layer(s, p, &mut layer);
    }
    drag::layer(s, p, &mut layer);
    rune_kit::root(base, layer)
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
