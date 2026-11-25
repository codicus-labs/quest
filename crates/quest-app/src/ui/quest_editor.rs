//! The quest dialog over the Quests page: title, description (Markdown, as in the task dialog)
//! and colour; Complete quest / Reopen quest, Open quest page and Delete. A new quest shows
//! Create quest instead. Esc, the close button and a click outside return to the page as it was.

use quest_domain::{Quest, QuestColor, QuestStatus};
use rune::{
    ClipboardContent, Color, Cursor, El, Key, KeyEvent, TextStyle, Tx, col, el, icon, row, spacer, text, text_input,
};

use crate::format;
use crate::icons;
use crate::state::{QuestEditor, State};
use crate::theme::{Pal, color_name, quest_color, ty};
use crate::ui::parts;
use crate::ui::quests::{total_ms, week_ms};

const TITLE: TextStyle = TextStyle::new(24.0, 500.0).lh(32.0);

pub fn layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let Some(form) = &s.quest_editor else { return };
    let quest = form.quest.and_then(|id| s.snap.quest(id));
    if form.quest.is_some() && quest.is_none() {
        return;
    }
    let (heading, status) = match quest {
        Some(q) => ("Edit quest", if is_saved(q, form) { "Saved" } else { "Saving…" }),
        None => ("New quest", ""),
    };
    let header = row()
        .w_full()
        .h(56.0)
        .pl(24.0)
        .pr(14.0)
        .gap(12.0)
        .center_y()
        .child(parts::caps(heading, p))
        .child(spacer())
        .child(text(status).key("quest-editor-status").font(ty::SMALL).color(p.faint))
        .child(parts::icon_button("quest-editor-close", "Close", &icons::CLOSE, p.muted, p, close));
    let body = row().w_full().grow(1.0).min_h(0.0).child(main(form, p)).child(side(s, quest, form, p));
    let mut panel = col()
        .key("quest-editor")
        .role("dialog")
        .label(heading)
        .w_full()
        .max_w(860.0)
        .h_full()
        .max_h(520.0)
        .radius(14.0)
        .fill(p.card_raised)
        .border(1.0, p.border_strong)
        // Inside the border: the right column's fill would cover it.
        .padding(1.0)
        .clip_rounded(14.0)
        .shadow(20.0, 60.0, Color::BLACK.alpha(0.45))
        .child(header)
        .child(el().w_full().h(1.0).fill(p.border))
        .child(body)
        .modal_tx(true, close);
    if let Some(q) = quest {
        panel = panel.focus_return(format!("quest-{}", q.id));
    }
    layer.push(el().key("quest-editor-scrim").absolute(0.0, 0.0).w_full().h_full().fill(p.scrim).a11y_hidden());
    layer
        .push(row().key("quest-editor-layer").absolute(0.0, 0.0).w_full().h_full().padding(40.0).center().child(panel));
}

fn close(s: &mut State, tx: &mut Tx) {
    tx.blur();
    s.close_quest_editor();
}

/// Whether everything typed is stored (an empty title is not).
fn is_saved(quest: &Quest, form: &QuestEditor) -> bool {
    form.title.text.trim() == quest.title && form.description.text == quest.description
}

/// The colour dot, the title (one line however it wraps) and the description.
fn main(form: &QuestEditor, p: &Pal) -> El {
    let title = text_input(&form.title, TITLE)
        .key("quest-editor-title")
        .label("Quest title")
        .grow(1.0)
        .min_w(0.0)
        .color(p.text)
        .placeholder("Name a long-term goal", p.faint)
        .on_edit(|s: &mut State| &mut s.quest_editor_fields().title)
        .on_input_change(|s: &mut State, _: &mut Tx| s.save_quest_title())
        .on_submit(|s: &mut State, tx: &mut Tx| {
            if s.quest_editor.as_ref().is_some_and(|f| f.quest.is_none()) {
                s.create_quest_from_editor();
            }
            tx.blur();
        })
        // Shift/⌥ Return would break the line.
        .on_input_key(|_: &mut State, ev: &KeyEvent, _: &mut Tx| ev.key == Key::Enter && (ev.mods.shift || ev.mods.alt))
        .on_input_paste(|s: &mut State, content: &ClipboardContent, _: &mut Tx| {
            s.quest_editor_fields().title.insert(&quest_view::home::one_line(content.text()));
            s.save_quest_title();
            true
        });
    let description = super::notes::field(
        "quest-editor-description",
        &form.description,
        p,
        |s| &mut s.quest_editor_fields().description,
        State::save_quest_description,
    )
    .grow(1.0)
    .min_h(0.0);
    col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .padding(28.0)
        .gap(20.0)
        .child(row().w_full().gap(14.0).center_y().child(parts::dot(quest_color(form.color), 14.0)).child(title))
        .child(el().w_full().h(1.0).fill(p.border))
        .child(description)
}

fn side(s: &State, quest: Option<&Quest>, form: &QuestEditor, p: &Pal) -> El {
    let side = col()
        .w(300.0)
        .h_full()
        .padding(22.0)
        .gap(12.0)
        .fill(p.bar)
        .border_sides([0.0, 0.0, 0.0, 1.0], p.border)
        .child(parts::caps("Color", p))
        .child(swatches(form.color, p));
    let Some(quest) = quest else {
        let can_create = !form.title.text.trim().is_empty();
        let create =
            wide_button("quest-editor-create", "Create quest", &icons::PLUS, p, |s: &mut State, _: &mut Tx| {
                s.create_quest_from_editor()
            });
        return side.child(if can_create { create } else { create.opacity(0.5) });
    };
    let completed = quest.status == QuestStatus::Completed;
    let (label, glyph) = if completed { ("Reopen quest", &icons::REPEAT) } else { ("Complete quest", &icons::CHECK) };
    let open = s.snap.tasks.iter().filter(|t| t.quest_id == Some(quest.id) && !t.is_done()).count();
    let figures = format!(
        "{} this week · {} total · {open} open {}",
        format::hours(week_ms(s, quest.id)),
        format::hours(total_ms(s, quest.id)),
        if open == 1 { "task" } else { "tasks" }
    );
    side.child(wide_button("quest-editor-status-action", label, glyph, p, |s: &mut State, tx: &mut Tx| {
        tx.blur();
        s.toggle_editor_quest_completed();
    }))
    .child(wide_button("quest-editor-page", "Open quest page", &icons::OPEN, p, |s: &mut State, tx: &mut Tx| {
        tx.blur();
        s.open_editor_quest_page();
    }))
    .child(text(figures).key("quest-editor-figures").font(ty::SMALL).color(p.faint).wrap())
    .child(spacer())
    .child(delete(p))
}

fn swatches(current: QuestColor, p: &Pal) -> El {
    let mut swatches = row().w_full().flex_wrap().gap(10.0);
    for color in QuestColor::ALL {
        let on = color == current;
        swatches = swatches.child(
            el().key(format!("quest-editor-color-{}", color_name(color).to_lowercase()))
                .role("radio")
                .label(color_name(color))
                .checked(on)
                .focusable(true)
                .size(30.0, 30.0)
                .radius(15.0)
                .fill(quest_color(color))
                .cursor(Cursor::Pointer)
                .border(2.0, if on { p.accent } else { p.bar })
                .on_press(move |s: &mut State| s.set_quest_editor_color(color)),
        );
    }
    swatches
}

fn wide_button(
    key: &str,
    label: &str,
    glyph: &'static rune::Icon,
    p: &Pal,
    on_press: impl Fn(&mut State, &mut Tx) + 'static,
) -> El {
    row()
        .key(key.to_owned())
        .role("button")
        .label(label)
        .focusable(true)
        .w_full()
        .h(40.0)
        .gap(8.0)
        .center()
        .radius(8.0)
        .fill(p.card)
        .border(1.0, p.border_strong)
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .child(icon(glyph, 15.0).color(p.text))
        .child(text(label.to_owned()).font(ty::BODY_MEDIUM).color(p.text))
        .on_press_tx(on_press)
}

fn delete(p: &Pal) -> El {
    row()
        .key("quest-editor-delete")
        .role("button")
        .label("Delete quest")
        .focusable(true)
        .h(36.0)
        .px(10.0)
        .gap(10.0)
        .center_y()
        .radius(8.0)
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .child(icon(&icons::TRASH, 15.0).color(p.error))
        .child(text("Delete").font(ty::BODY).color(p.error))
        .on_press_tx(|s: &mut State, tx: &mut Tx| {
            tx.blur();
            s.delete_editor_quest();
        })
}
