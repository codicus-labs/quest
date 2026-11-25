use quest_domain::{ChecklistItem, EntityId, Task};
use rune::{
    Chord, ChordKey, ChordMod, Color, Cursor, El, Key, KeyEvent, Mods, TextEdit, Tx, col, el, icon, row, text,
    text_input,
};

use crate::icons;
use crate::state::notes::NoteHost;
use crate::state::{State, TaskModal};
use crate::theme::{self, Pal, ty};
use crate::undo::Deletable;

pub(super) fn detail(s: &State, task: &Task, form: &TaskModal, p: &Pal) -> El {
    let id = task.id;
    let items = s.snap.checklist_of(id).map(|item| {
        let edit = form.checklist.items.get(&item.id).filter(|edit| edit.text == item.text || edit.focused);
        let default;
        let edit = match edit {
            Some(edit) => edit,
            None => {
                default = TextEdit::new(&item.text);
                &default
            }
        };
        let item_id = item.id;
        let field = text_input(edit, ty::BODY)
            .key(item_key(item_id))
            .label("Checklist item")
            .grow(1.0)
            .min_w(0.0)
            .single_line()
            .color(if item.done { p.faint } else { p.text })
            .placeholder("Item", p.faint)
            .on_edit(move |s: &mut State| s.checklist_edit(item_id))
            .on_input_change(move |s: &mut State, _: &mut Tx| s.save_checklist_text(item_id))
            .on_input_key(move |s: &mut State, event, tx| item_keypress(s, id, Some(item_id), event, tx));
        let field = if item.done { field.strike(0, edit.text.len(), None) } else { field };
        line().child(item_box(item, false, p)).child(field)
    });
    let add = text_input(&form.checklist.add, ty::BODY)
        .key("checklist-add")
        .label("Add checklist item")
        .grow(1.0)
        .min_w(0.0)
        .single_line()
        .color(p.text)
        .placeholder("Add checklist item", p.faint)
        .on_edit(|s: &mut State| &mut s.task_modal_fields().checklist.add)
        .on_input_key(move |s: &mut State, event, tx| item_keypress(s, id, None, event, tx));
    col()
        .key("checklist")
        .w_full()
        .gap(3.0)
        .children(items)
        .child(line().child(icon(&icons::PLUS, 16.0).color(p.faint)).child(add))
}

pub(super) fn focus(s: &State, task: &Task, p: &Pal) -> El {
    col().key("focus-checklist").w_full().gap(3.0).children(s.snap.checklist_of(task.id).map(|item| {
        let label = text(item.text.clone()).font(ty::BODY).color(if item.done { p.faint } else { p.text }).wrap();
        let label = if item.done { label.strike(0, item.text.len(), None) } else { label };
        line().child(item_box(item, true, p)).child(label.grow(1.0).min_w(0.0))
    }))
}

fn line() -> El {
    row().w_full().min_h(28.0).gap(8.0).center_y()
}

fn item_key(id: EntityId) -> String {
    format!("checklist-item-{id}")
}

fn item_box(item: &ChecklistItem, focus: bool, p: &Pal) -> El {
    let id = item.id;
    checkbox(
        format!("{}checklist-check-{id}", if focus { "focus-" } else { "" }),
        if item.text.is_empty() { "Checklist item".into() } else { item.text.clone() },
        item.done,
        p,
        move |s| s.toggle_checklist_item(id),
    )
}

pub(super) fn checkbox(
    key: String,
    label: String,
    done: bool,
    p: &Pal,
    action: impl Fn(&mut State) + Clone + 'static,
) -> El {
    let key_action = action.clone();
    el().key(key)
        .role("checkbox")
        .label(label)
        .checked(done)
        .focusable(true)
        .size(16.0, 16.0)
        .radius(4.0)
        .fill(if done { p.accent_fill } else { Color::TRANSPARENT })
        .border(1.0, if done { p.accent_fill } else { p.border_strong })
        .center()
        .cursor(Cursor::Pointer)
        .children(done.then(|| icon(&icons::CHECK, 12.0).color(theme::on_accent_fill())))
        .on_press(action)
        .on_key(move |s: &mut State, event, tx| {
            if event.key != Key::Enter || event.mods != Mods::default() {
                return false;
            }
            if !tx.key_repeat() {
                key_action(s);
            }
            true
        })
}

fn item_keypress(s: &mut State, task: EntityId, item: Option<EntityId>, event: &KeyEvent, tx: &mut Tx) -> bool {
    let plain = event.mods == Mods::default();
    let ids: Vec<_> = s.snap.checklist_of(task).map(|item| item.id).collect();
    let index = item.and_then(|id| ids.iter().position(|candidate| *candidate == id)).unwrap_or(ids.len());
    match (&event.key, item) {
        (Key::Escape, _) => tx.focus_when_shown(NoteHost::Detail.preview(), true),
        (Key::Up, _) if plain => {
            if let Some(previous) = index.checked_sub(1).and_then(|i| ids.get(i)) {
                focus_item(s, Some(*previous), tx);
            }
        }
        (Key::Down, Some(_)) if plain => focus_item(s, ids.get(index + 1).copied(), tx),
        (Key::Down, None) if plain => {}
        (Key::Up | Key::Down, Some(id)) if event.mods == (Mods { alt: true, ..Mods::default() }) => {
            s.move_checklist_item(id, event.key == Key::Down);
        }
        (Key::Enter, Some(id)) if Chord::new(ChordKey::Enter).with(ChordMod::Primary).matches(event) => {
            if !tx.key_repeat() {
                s.toggle_checklist_item(id);
            }
        }
        (Key::Enter, Some(id)) if plain => {
            if !tx.key_repeat()
                && !s.checklist_edit(id).text.trim().is_empty()
                && let Some(new) = s.insert_checklist_item(task, Some(id), String::new())
            {
                focus_item(s, Some(new), tx);
            }
        }
        (Key::Enter, None) if plain => {
            let value = s.task_modal_fields().checklist.add.text.trim().to_owned();
            if !value.is_empty() && s.insert_checklist_item(task, None, value).is_some() {
                let edit = &mut s.task_modal_fields().checklist.add;
                *edit = TextEdit::default();
                edit.focused = true;
            }
        }
        (Key::Backspace, Some(id)) if plain && s.checklist_edit(id).text.is_empty() => {
            s.delete(Deletable::ChecklistItem(id));
            s.task_modal_fields().checklist.items.remove(&id);
            let neighbor = index.checked_sub(1).or(Some(index + 1)).and_then(|i| ids.get(i)).copied();
            focus_item(s, neighbor, tx);
        }
        _ => return false,
    }
    true
}

fn focus_item(s: &mut State, item: Option<EntityId>, tx: &mut Tx) {
    let key = match item {
        Some(id) => {
            let edit = s.checklist_edit(id);
            let end = edit.text.len();
            edit.set_selection(end, end);
            item_key(id)
        }
        None => "checklist-add".into(),
    };
    tx.focus_when_shown(key.clone(), true);
    tx.scroll_into_view(key);
}
