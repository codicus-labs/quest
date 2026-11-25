//! Workspaces on screen: the switcher between the logo and the view tabs, its menu, "Manage
//! workspaces", and the note on a focus timer that runs in another workspace.

use quest_domain::{EntityId, Workspace};
use quest_view::format;
use quest_view::workspaces::{content_summary, delete_question, timer_elsewhere};
use rune::{Cursor, El, TextEdit, Tx, col, icon, row, spacer, text};
use rune_kit::{Button, ButtonVariant, Dialog, Menu, MenuItem};

use crate::icons;
use crate::state::State;
use crate::theme::{Pal, ty};
use crate::ui::parts;

const MENU_KEY: &str = "workspace-menu";
/// rune-kit anchors a menu to `{key}-trigger`: the switcher takes that key while it is open.
const TRIGGER_KEY: &str = "workspace-menu-trigger";

/// The active workspace's name with a chevron; opens the switcher menu.
pub fn switcher(s: &State, p: &Pal) -> El {
    let name = s.snap.active_workspace().name.clone();
    row()
        .key(if s.workspace_menu { TRIGGER_KEY } else { "workspace-switcher" })
        .role("button")
        .label(format!("Switch workspace · {name}"))
        .focusable(true)
        .h(36.0)
        .px(10.0)
        .gap(7.0)
        .center_y()
        .radius(7.0)
        .border(1.0, p.border_strong)
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .child(icon(&icons::LAYERS, 14.0).color(p.muted))
        .child(text(name).key("workspace-name").font(ty::NAV).color(p.text).max_w(180.0).truncate())
        .child(icon(&icons::CHEVRON_DOWN, 14.0).color(p.muted))
        .on_press(|s: &mut State| s.toggle_workspace_menu())
}

/// The switcher's menu: the live workspaces (the shown one checked), then "Manage workspaces…".
pub fn menu_layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    if !s.workspace_menu {
        return;
    }
    let mut menu = Menu::new(MENU_KEY).open(true).min_width(220.0).on_close(|s: &mut State| s.workspace_menu = false);
    for workspace in s.snap.live_workspaces() {
        let id = workspace.id;
        menu = menu.item(
            MenuItem::new(workspace.name.clone())
                .checked(id == s.snap.workspace)
                .on_select(move |s: &mut State| s.switch_workspace(id)),
        );
    }
    menu = menu.separator().item(MenuItem::new("Manage workspaces…").on_select_tx(|s: &mut State, tx: &mut Tx| {
        s.open_workspaces();
        tx.focus_when_shown("workspace-new", false);
    }));
    // The trigger it hands back is the switcher's stand-in; the real one is in the top bar.
    let _ = menu.build(&p.kit, layer);
}

/// The running focus timer when it is in another workspace: its time, task and workspace.
/// Pressing it shows that workspace's Focus.
pub fn timer_note(s: &State, p: &Pal) -> Option<El> {
    let (_, workspace, title) = timer_elsewhere(&s.snap)?;
    let running = s.snap.running_focus()?;
    let time = format::timer(running.entry.tracked_ms(s.now));
    Some(
        row()
            .key("timer-elsewhere")
            .role("button")
            .label(format!("Timer running in {workspace}: {title}"))
            .focusable(true)
            .h(32.0)
            .px(10.0)
            .gap(8.0)
            .center_y()
            .radius(16.0)
            .fill(p.card)
            .border(1.0, p.border_strong)
            .cursor(Cursor::Pointer)
            .hover(p.hover)
            .help("Show it in Focus")
            .child(parts::dot(p.accent, 8.0))
            .child(text(time).font(ty::MONO_TIME).color(p.text))
            .child(text(format!("in {workspace}")).font(ty::SMALL).color(p.muted).max_w(120.0).truncate())
            .on_press(|s: &mut State| s.go_to_running_timer()),
    )
}

pub fn manage_layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let mut body = col().key("workspaces").w_full().gap(14.0).px(20.0).pb(20.0);
    let can_delete = s.snap.live_workspaces().count() > 1;
    for workspace in s.snap.live_workspaces() {
        body = body.child(if s.workspaces.confirm_delete == Some(workspace.id) {
            confirm_row(workspace, p)
        } else {
            live_row(s, workspace, can_delete, p)
        });
    }
    if let Some(error) = &s.workspaces.error {
        body =
            body.child(text(error.clone()).key("workspace-error").role("alert").font(ty::SMALL).color(p.accent).wrap());
    }
    body = body.child(create_row(s, p));
    let deleted = s.snap.deleted_workspaces();
    if !deleted.is_empty() {
        body = body.child(parts::caps("Deleted", p).pt(6.0));
        for workspace in deleted {
            body = body.child(deleted_row(workspace, p));
        }
    }
    Dialog::new("workspaces-dialog", "Manage workspaces")
        .open(true)
        .fit()
        .width(560.0)
        .on_close(|s: &mut State| s.close_workspaces())
        .child(body)
        .build(&p.kit, layer);
}

fn name_box(field: El, p: &Pal) -> El {
    row()
        .grow(1.0)
        .min_w(0.0)
        .h(36.0)
        .px(10.0)
        .center_y()
        .radius(7.0)
        .fill(p.bg)
        .border(1.0, p.border_strong)
        .child(field)
}

fn live_row(s: &State, workspace: &Workspace, can_delete: bool, p: &Pal) -> El {
    let k = &p.kit;
    let id = workspace.id;
    let unfilled = TextEdit::new(&workspace.name);
    let field = parts::field(
        &format!("workspace-name-{id}"),
        s.workspaces.names.get(&id).unwrap_or(&unfilled),
        "Name",
        ty::BODY,
        p,
        move |s: &mut State| s.workspaces.name_field(id),
        move |s: &mut State, _: &mut Tx| s.rename_workspace(id),
    );
    let mut summary = content_summary(&workspace.content);
    if id == s.snap.workspace {
        summary = format!("Current · {summary}");
    }
    col()
        .key(format!("workspace-{id}"))
        .w_full()
        .gap(4.0)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(name_box(field, p))
                .child(
                    Button::new("Rename")
                        .variant(ButtonVariant::Ghost)
                        .on_press(move |s: &mut State| s.rename_workspace(id))
                        .build(k)
                        .key(format!("workspace-rename-{id}")),
                )
                .child(
                    Button::new("Delete")
                        .variant(ButtonVariant::Ghost)
                        .disabled(!can_delete)
                        .help(if can_delete { "Delete this workspace" } else { "The last workspace stays" })
                        .on_press(move |s: &mut State| s.ask_delete_workspace(id))
                        .build(k)
                        .key(format!("workspace-delete-{id}")),
                ),
        )
        .child(text(summary).key(format!("workspace-summary-{id}")).font(ty::SMALL).color(p.muted))
}

fn confirm_row(workspace: &Workspace, p: &Pal) -> El {
    let k = &p.kit;
    col()
        .key(format!("workspace-confirm-{}", workspace.id))
        .w_full()
        .gap(10.0)
        .padding(12.0)
        .radius(8.0)
        .fill(p.card)
        .border(1.0, p.border_strong)
        .child(text(delete_question(workspace)).key("workspace-delete-question").font(ty::BODY).color(p.text).wrap())
        .child(
            row()
                .w_full()
                .gap(10.0)
                .child(spacer())
                .child(
                    Button::new("Cancel")
                        .variant(ButtonVariant::Ghost)
                        .on_press(|s: &mut State| s.workspaces.confirm_delete = None)
                        .build(k)
                        .key("workspace-delete-cancel"),
                )
                .child(
                    Button::new("Delete workspace")
                        .variant(ButtonVariant::Danger)
                        .on_press_tx(|s: &mut State, tx: &mut Tx| {
                            tx.blur();
                            s.delete_workspace();
                        })
                        .build(k)
                        .key("workspace-delete-confirm"),
                ),
        )
}

fn create_row(s: &State, p: &Pal) -> El {
    let field = parts::field(
        "workspace-new",
        &s.workspaces.new_name,
        "New workspace",
        ty::BODY,
        p,
        |s: &mut State| &mut s.workspaces.new_name,
        |s: &mut State, _: &mut Tx| s.create_workspace(),
    );
    row().w_full().gap(8.0).center_y().pt(6.0).child(name_box(field, p)).child(
        Button::new("Create")
            .variant(ButtonVariant::Contrast)
            .on_press(|s: &mut State| s.create_workspace())
            .build(&p.kit)
            .key("workspace-create"),
    )
}

fn deleted_row(workspace: &Workspace, p: &Pal) -> El {
    let id: EntityId = workspace.id;
    row()
        .key(format!("workspace-deleted-{id}"))
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .child(text(workspace.name.clone()).font(ty::BODY).color(p.text).truncate())
                .child(text(content_summary(&workspace.content)).font(ty::SMALL).color(p.muted)),
        )
        .child(
            Button::new("Restore")
                .variant(ButtonVariant::Ghost)
                .on_press(move |s: &mut State| s.restore_workspace(id))
                .build(&p.kit)
                .key(format!("workspace-restore-{id}")),
        )
}
