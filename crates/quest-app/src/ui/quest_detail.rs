//! A quest's page: its colour, name (click to rename) and hours this week; colour swatches,
//! complete or reopen, delete; a quick-add that files tasks under it; its open tasks, then what's
//! done.

use quest_domain::{Command, EntityId, Quest, QuestColor, QuestStatus};
use rune::{ClipboardContent, Cursor, El, Key, KeyEvent, TextStyle, Tx, col, el, row, spacer, text, text_input};

use crate::format;
use crate::icons;
use crate::state::{QuestEdit, State};
use crate::theme::{Pal, color_name, quest_color, ty};
use crate::ui::lists;
use crate::ui::parts;
use crate::ui::quests::week_ms;
use crate::ui::task_row::Shows;
use crate::undo::Deletable;

pub fn view(s: &State, id: EntityId, p: &Pal) -> El {
    let Some(quest) = s.snap.quest(id) else {
        let gone = text("This quest is gone. It may be in the archive.").font(ty::BODY).color(p.faint);
        return lists::page("quest-detail", gone, None, el());
    };
    let add = lists::quick_add(s, p, &format!("Add to {}", quest.title), None);
    let shows = Shows { quest: false, plan_ahead: true };
    lists::page("quest-detail", heading(s, quest, p), Some(add), lists::rows(s, shows, "No tasks yet", p))
}

/// Colour dot, name (click to rename), a Completed or Archived badge, complete and delete; under
/// it the hours this week and the colour swatches.
fn heading(s: &State, quest: &Quest, p: &Pal) -> El {
    let id = quest.id;
    let title: El = match s.quest_edit {
        QuestEdit::Rename(editing) if editing == id => rename_field("quest-rename", ty::H1, s, p, move |s, title| {
            s.dispatch(Command::RenameQuest { id, title });
        }),
        _ => text(quest.title.clone())
            .key("quest-title")
            .role("button")
            .label("Rename quest")
            .font(ty::H1)
            .color(p.text)
            .cursor(Cursor::Text)
            .single_line()
            .truncate()
            .min_w(0.0)
            .on_press_tx(move |s: &mut State, tx: &mut Tx| {
                let title = s.snap.quest(id).map(|q| q.title.clone()).unwrap_or_default();
                s.quest_text = rune::TextEdit::new(title);
                s.quest_edit = QuestEdit::Rename(id);
                tx.focus_when_shown("quest-rename", true);
            }),
    };
    let mut name = row()
        .w_full()
        .center_y()
        .gap(12.0)
        .child(el().size(26.0, 26.0).center().child(parts::dot(quest_color(quest.color), 14.0)))
        .child(title.grow(1.0).min_w(0.0));
    let badge = match quest.status {
        QuestStatus::Active => None,
        QuestStatus::Completed => Some("Completed"),
        QuestStatus::Archived => Some("Archived"),
    };
    if let Some(badge) = badge {
        name = name.child(parts::caps(badge, p).px(10.0).py(4.0).radius(6.0).border(1.0, p.border_strong));
    }
    let (status_label, next) = match quest.status {
        QuestStatus::Active => ("Complete quest", QuestStatus::Completed),
        QuestStatus::Completed => ("Reopen quest", QuestStatus::Active),
        QuestStatus::Archived => ("Unarchive", QuestStatus::Active),
    };
    name = name
        .child(
            parts::icon_button("quest-status", status_label, &icons::COMPLETED, p.muted, p, move |s: &mut State, _| {
                s.set_quest_status(id, next);
            })
            .size(32.0, 32.0)
            .help(status_label),
        )
        .child(
            parts::icon_button("quest-delete", "Delete quest", &icons::TRASH, p.muted, p, move |s: &mut State, tx| {
                tx.blur();
                s.delete(Deletable::Quest(id));
            })
            .size(32.0, 32.0)
            .help("Delete quest"),
        );
    let sub = row()
        .w_full()
        .center_y()
        .pl(38.0)
        .child(
            text(format!("{} this week", format::hours(week_ms(s, id))))
                .key("quest-week")
                .font(ty::BODY)
                .color(p.muted),
        )
        .child(spacer())
        .child(swatches(id, quest.color, p));
    col().w_full().gap(4.0).child(name).child(sub)
}

fn swatches(id: EntityId, current: QuestColor, p: &Pal) -> El {
    let mut swatches = row().gap(6.0);
    for color in QuestColor::ALL {
        let on = color == current;
        swatches = swatches.child(
            el().key(format!("quest-color-{}", color_name(color).to_lowercase()))
                .role("radio")
                .label(color_name(color))
                .checked(on)
                .focusable(true)
                .size(20.0, 20.0)
                .radius(10.0)
                .fill(quest_color(color))
                .cursor(Cursor::Pointer)
                .border(2.0, if on { p.text } else { p.bg })
                .on_press(move |s: &mut State| s.dispatch(Command::SetQuestColor { id, color })),
        );
    }
    swatches
}

/// A one-line field for a new name, in the `style` of the name it replaces, so nothing moves or
/// resizes: Return saves (unless empty), Esc cancels. No Shift/⌥ Return; a paste's breaks become
/// spaces.
fn rename_field(key: &str, style: TextStyle, s: &State, p: &Pal, save: impl Fn(&mut State, String) + 'static) -> El {
    text_input(&s.quest_text, style)
        .key(key.to_owned())
        .label("Name")
        .single_line()
        .grow(1.0)
        .color(p.text)
        .placeholder("Name", p.faint)
        .on_edit(|s: &mut State| &mut s.quest_text)
        .on_submit(move |s: &mut State, _: &mut Tx| {
            let title = s.quest_text.text.trim().to_owned();
            if !title.is_empty() {
                save(s, title);
            }
            s.quest_edit = QuestEdit::None;
        })
        .on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| match ev.key {
            Key::Escape => {
                s.quest_edit = QuestEdit::None;
                true
            }
            // Shift/⌥ Return would break the line.
            Key::Enter => ev.mods.shift || ev.mods.alt,
            _ => false,
        })
        .on_input_paste(|s: &mut State, content: &ClipboardContent, _: &mut Tx| {
            s.quest_text.insert(&quest_view::home::one_line(content.text()));
            true
        })
}
