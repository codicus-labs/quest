use quest_domain::{EntityId, Task};
use rune::{El, Tx, el};
use rune_kit::{Menu, MenuItem};

use crate::menu::{self, Step, TaskAction};
use crate::state::{State, TaskMenu, View};
use crate::theme::Pal;
use crate::ui::chrome::open_palette_in;

const MENU_KEY: &str = "card-menu";
pub fn anchor(s: &State, id: EntityId) -> Option<El> {
    (s.menu == Some(TaskMenu { task: id }))
        .then(|| el().key("card-menu-trigger").absolute(12.0, 0.0).size(1.0, 38.0).inert())
}

pub fn toggle(s: &mut State, id: EntityId) {
    let item = TaskMenu { task: id };
    s.menu = (s.menu != Some(item)).then_some(item);
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
    // The row provides the trigger's anchor.
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
