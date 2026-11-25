//! A quest's page: rename it inline, change its colour, complete, reopen or delete it; its hours this
//! week and in total; and its tasks (open ones by plan date, then the unplanned ones, then
//! what's done) with a field to add one.

use quest_domain::{Command, EntityId, Position, Quest, QuestColor, QuestStatus};
use rune::{
    ClipboardContent, Color, Cursor, El, Key, KeyEvent, ScrollSpec, TextStyle, Tx, col, el, row, spacer, text,
    text_input,
};

use crate::format;
use crate::icons;
use crate::state::{AddAt, QuestEdit, State, View};
use crate::theme::{Pal, color_name, quest_color, ty};
use crate::ui::parts::{self, CardOptions};
use crate::ui::quests::{total_ms, week_ms};
use crate::undo::Deletable;

pub fn view(s: &State, id: EntityId, p: &Pal) -> El {
    let back = parts::button("quest-back", "Quests", Some(&icons::ARROW_LEFT), p, |s: &mut State, _: &mut Tx| {
        s.go(View::Quests)
    })
    .h(32.0);
    let page = col().key("quest-detail").w_full().h_full().px(30.0).pt(28.0).pb(16.0).gap(22.0);
    let Some(quest) = s.snap.quest(id) else {
        return page
            .child(row().child(back))
            .child(parts::empty_note("This quest is gone. It may be in the archive.", p));
    };
    let (status_label, next) = match quest.status {
        QuestStatus::Active => ("Complete quest", QuestStatus::Completed),
        QuestStatus::Completed => ("Reopen quest", QuestStatus::Active),
        QuestStatus::Archived => ("Unarchive", QuestStatus::Active),
    };
    let top = row()
        .w_full()
        .center_y()
        .gap(10.0)
        .child(back)
        .child(spacer())
        .child(
            parts::button("quest-status", status_label, Some(&icons::CHECK), p, move |s: &mut State, _: &mut Tx| {
                s.set_quest_status(id, next);
            })
            .h(32.0),
        )
        .child(
            parts::button("quest-delete", "Delete", Some(&icons::TRASH), p, move |s: &mut State, tx: &mut Tx| {
                tx.blur();
                s.delete(Deletable::Quest(id));
            })
            .h(32.0),
        );
    let body = row().w_full().grow(1.0).min_h(0.0).gap(28.0).child(tasks(s, quest, p).grow(1.3).min_w(0.0));
    page.child(top).child(heading(s, quest, p)).child(body)
}

/// Colour dot, name (click to rename), a Completed or Archived badge, swatches, and the hours.
fn heading(s: &State, quest: &Quest, p: &Pal) -> El {
    let id = quest.id;
    let title: El = match s.quest_edit {
        QuestEdit::Rename(editing) if editing == id => {
            rename_field("quest-rename", ty::HEADING, s, p, move |s, title| {
                s.dispatch(Command::RenameQuest { id, title });
            })
        }
        _ => text(quest.title.clone())
            .key("quest-title")
            .role("button")
            .label("Rename quest")
            .font(ty::HEADING)
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
        .gap(14.0)
        .child(parts::dot(quest_color(quest.color), 14.0))
        .child(title.grow(1.0).min_w(0.0));
    let badge = match quest.status {
        QuestStatus::Active => None,
        QuestStatus::Completed => Some("Completed"),
        QuestStatus::Archived => Some("Archived"),
    };
    if let Some(badge) = badge {
        name = name.child(parts::caps(badge, p).px(10.0).py(4.0).radius(6.0).border(1.0, p.border_strong));
    }
    let open = s.snap.tasks.iter().filter(|t| t.quest_id == Some(id) && !t.is_done()).count();
    let done = s.snap.tasks.iter().filter(|t| t.quest_id == Some(id) && t.is_done()).count();
    let stat = |key: &str, value: String, label: &str, color: Color| {
        row()
            .gap(8.0)
            .items_end()
            .child(text(value).key(key.to_owned()).font(ty::STAT).color(color))
            .child(text(label.to_owned()).font(ty::SMALL.lh(22.0)).color(p.muted))
    };
    let stats = row()
        .w_full()
        .center_y()
        .gap(28.0)
        .child(stat("quest-week", format::hours(week_ms(s, id)), "this week", p.accent))
        .child(stat("quest-total", format::hours(total_ms(s, id)), "total", p.text))
        .child(stat("quest-open", open.to_string(), "open", p.text))
        .child(stat("quest-done", done.to_string(), "done", p.text))
        .child(spacer())
        .child(swatches(id, quest.color, p));
    col().w_full().gap(16.0).child(name).child(stats)
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

fn tasks(s: &State, quest: &Quest, p: &Pal) -> El {
    let id = quest.id;
    let add = parts::add_box_with(
        "quest-add-task",
        &s.quest_add,
        "Add a task to this quest…",
        s,
        |s: &mut State| &mut s.quest_add,
        move |s: &mut State, _: &mut Tx| add_task(s, id),
        false,
        Some(destination(s, p)),
    );
    let options = CardOptions { show_time: true, key_prefix: "quest", wrap_title: false, list: None };
    let mut list = col().key("quest-tasks").w_full().gap(10.0).pb(24.0);
    let groups = quest_view::quests::task_groups(&s.snap, id, s.today());
    if groups.is_empty() {
        list = list.child(parts::empty_note("No tasks yet. Add the next step above.", p));
    }
    for (label, tasks) in groups {
        list = list.child(parts::caps(label, p).mt(8.0));
        for task in tasks {
            list = list.child(parts::task_card(s, task, p, options));
        }
    }
    col()
        .h_full()
        .min_h(0.0)
        .gap(14.0)
        .child(parts::caps("Tasks", p))
        .child(add)
        .child(list.scroll(ScrollSpec { stick_bottom: false }).grow(1.0).min_h(0.0).scroll_edge_fade(12.0))
}

/// Where a task added here goes: Today (the default, so it shows on Home) or the Backlog.
fn destination(s: &State, p: &Pal) -> El {
    let choice = |key: &str, label: &str, backlog: bool| {
        let on = s.quest_add_backlog == backlog;
        row()
            .key(key.to_owned())
            .role("radio")
            .label(label)
            .checked(on)
            .focusable(true)
            .h(30.0)
            .px(10.0)
            .center_y()
            .radius(6.0)
            .cursor(Cursor::Pointer)
            .fill(if on { p.selected } else { Color::TRANSPARENT })
            .border(1.0, if on { p.border_strong } else { Color::TRANSPARENT })
            .child(text(label).font(ty::BODY).color(if on { p.text } else { p.muted }))
            .on_press(move |s: &mut State| s.quest_add_backlog = backlog)
    };
    row()
        .gap(2.0)
        .padding(2.0)
        .radius(8.0)
        .fill(p.bar)
        .border(1.0, p.border)
        .child(choice("quest-add-today", "Today", false))
        .child(choice("quest-add-backlog", "Backlog", true))
}

/// New tasks from here go on this quest, today or to the backlog.
fn add_task(s: &mut State, quest: EntityId) {
    let title = s.quest_add.text.trim().to_owned();
    if title.is_empty() {
        return;
    }
    let position = match s.add_at {
        AddAt::Top => Position::Top,
        AddAt::Bottom => Position::Bottom,
    };
    let planned_on = (!s.quest_add_backlog).then(|| s.today());
    s.dispatch(Command::CreateTask {
        id: EntityId::new(),
        title,
        quest_id: Some(quest),
        planned_on,
        due_on: None,
        position,
    });
    s.quest_add.clear();
}
