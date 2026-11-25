//! A task card's "…" button and the menu it opens (rune-kit's `Menu`, drawn in the overlay layer
//! and anchored to the button). `rows` lists them; `pick` carries one out.

use quest_domain::{EntityId, Task};
use rune::{El, Transition, Tx};
use rune_kit::{Menu, MenuItem};

use crate::icons;
use crate::menu::{self, Step, TaskAction};
use crate::state::{State, TaskMenu, View};
use crate::theme::Pal;
use crate::ui::parts;
use crate::ui::topbar::open_palette_in;

const MENU_KEY: &str = "card-menu";
/// rune-kit anchors a menu to `{key}-trigger`: the open menu's button takes that key.
const TRIGGER_KEY: &str = "card-menu-trigger";

/// The "…" button on a task card.
pub fn button(s: &State, prefix: &'static str, id: EntityId, p: &Pal) -> El {
    let this = TaskMenu { task: id, prefix };
    let open = s.menu == Some(this);
    let key = if open { TRIGGER_KEY.to_owned() } else { format!("{prefix}-menu-{id}") };
    parts::icon_button(key, "More actions", &icons::MORE, p.muted, p, move |s: &mut State, _: &mut Tx| {
        s.menu = if s.menu == Some(this) { None } else { Some(this) };
    })
    .transition(Transition::none())
}

/// The menu's rows (`None` is a separator).
pub fn rows(task: &Task, has_quest: bool) -> Vec<Option<TaskAction>> {
    menu::task_actions(task, has_quest)
}

/// The open menu, if any.
pub fn layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let Some(open) = s.menu else { return };
    let Some(task) = s.plan_task(open.task) else { return };
    let has_quest = task.quest_id.and_then(|q| s.snap.quest(q)).is_some();
    let id = task.id;
    let mut menu = Menu::new(MENU_KEY).open(true).min_width(210.0).on_close(|s: &mut State| s.menu = None);
    for row in rows(&task, has_quest) {
        let Some(action) = row else {
            menu = menu.separator();
            continue;
        };
        let mut item = MenuItem::new(action.label(&task)).danger(action == TaskAction::Delete);
        if let Some(hint) = action.hint() {
            item = item.shortcut(hint);
        }
        menu = menu.item(item.on_select_tx(move |s: &mut State, tx: &mut Tx| pick(s, id, action, tx)));
    }
    // The trigger it hands back is the button's stand-in; the real button is in the card.
    let _ = menu.build(&p.kit, layer);
}

pub fn pick(s: &mut State, id: EntityId, action: TaskAction, tx: &mut Tx) {
    s.menu = None;
    let Some(task) = s.plan_task(id) else { return };
    let Some(step) = menu::task_step(action, &task) else { return };
    match step {
        Step::Open(task) => s.open_task_modal(task),
        Step::Repeat(task) => s.open_repeat_editor(task),
        Step::Focus(task) => {
            s.start_focus(task);
            s.go_with(View::Focus, tx);
        }
        Step::ToggleDone(task) => s.toggle_done(task),
        Step::Palette(mode) => open_palette_in(s, mode, tx),
        Step::OpenQuest(quest) => s.go(View::Quest(quest)),
        Step::Delete(item) => {
            tx.blur();
            s.delete(item);
        }
    }
}
