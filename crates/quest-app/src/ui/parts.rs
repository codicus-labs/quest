//! Small pieces the views share: chips, checks, buttons, cards, the add-task field.

use chrono::Days;
use quest_domain::{Command, EntityId, Position, Quest, Task};
use rune::{
    Align, Color, Cursor, El, Icon, Key, KeyEvent, TextEdit, TextStyle, Tx, col, el, icon, row, spacer, text,
    text_input,
};

use crate::dnd::{self, ListId, Payload};
use crate::format;
use crate::icons;
use crate::keys::{self, CardKey};
use crate::palette::Mode;
use crate::state::{AddAt, State};
use crate::theme::{self, Pal, quest_color, ty};
use crate::ui::drag::{self, CardLook, Zone};
use crate::ui::menu;
use crate::ui::topbar::open_palette_in;
use crate::undo::Deletable;

pub fn quest_of<'a>(s: &'a State, task: &Task) -> Option<&'a Quest> {
    task.quest_id.and_then(|id| s.snap.quest(id))
}

pub fn accent_of(quest: Option<&Quest>, p: &Pal) -> Color {
    quest.map_or(p.faint, |q| quest_color(q.color))
}

/// The text between a period's back and forward buttons, as wide for every period ("Sep 28 – Oct 4"
/// as "Oct 5 – 11"), so the buttons around it stay put while one steps through the weeks.
pub fn period_label(key: &str, label: String, font: TextStyle, color: Color) -> El {
    text(label).key(key).font(font).color(color).single_line().w(PERIOD_LABEL_W).align(Align::Center)
}

/// Fits the widest week range of `format::week_range` with room to spare.
const PERIOD_LABEL_W: f32 = 130.0;

pub fn caps(label: impl Into<String>, p: &Pal) -> El {
    text(label.into().to_uppercase()).font(ty::CAPS).color(p.muted)
}

pub fn dot(color: Color, size: f32) -> El {
    el().size(size, size).radius(size / 2.0).fill(color)
}

/// A quest's name with its colour dot, in a pill.
pub fn chip(quest: Option<&Quest>, p: &Pal) -> El {
    let (label, color) = match quest {
        Some(q) => (q.title.clone(), quest_color(q.color)),
        None => ("No quest".to_owned(), p.faint),
    };
    row()
        .h(26.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .radius(13.0)
        .fill(p.chip)
        .border(1.0, p.border)
        .min_w(0.0)
        .shrink()
        .child(dot(color, 7.0))
        .child(text(label).font(ty::SMALL_MEDIUM.lh(16.0)).color(p.muted).single_line().truncate().min_w(0.0).shrink())
}

/// The round "complete" toggle.
pub fn check(key: String, done: bool, color: Color, p: &Pal, on_press: impl Fn(&mut State) + 'static) -> El {
    let (fill, border, ink) =
        if done { (color.alpha(0.85), color, Color::WHITE) } else { (Color::TRANSPARENT, p.border_strong, p.muted) };
    el().key(key)
        .role("checkbox")
        .label("Complete")
        .checked(done)
        .focusable(true)
        .size(24.0, 24.0)
        .radius(12.0)
        .fill(fill)
        .border(1.0, border)
        .center()
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .child(icon(&icons::CHECK, 14.0).color(ink))
        .on_press(on_press)
}

pub fn icon_button(
    key: impl Into<String>,
    label: &str,
    glyph: &'static Icon,
    ink: Color,
    p: &Pal,
    on_press: impl Fn(&mut State, &mut Tx) + 'static,
) -> El {
    el().key(key.into())
        .role("button")
        .label(label)
        .focusable(true)
        .size(28.0, 28.0)
        .radius(6.0)
        .center()
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .child(icon(glyph, 16.0).color(ink))
        .on_press_tx(on_press)
}

/// A bordered button with an optional leading icon.
pub fn button(
    key: impl Into<String>,
    label: &str,
    glyph: Option<&'static Icon>,
    p: &Pal,
    on_press: impl Fn(&mut State, &mut Tx) + 'static,
) -> El {
    let mut b = row()
        .key(key.into())
        .role("button")
        .label(label)
        .focusable(true)
        .h(36.0)
        .px(14.0)
        .gap(8.0)
        .center_y()
        .radius(7.0)
        .fill(p.card)
        .border(1.0, p.border_strong)
        .cursor(Cursor::Pointer)
        .hover(p.hover);
    if let Some(glyph) = glyph {
        b = b.child(icon(glyph, 15.0).color(p.text));
    }
    b.child(text(label).font(ty::BODY_MEDIUM).color(p.text)).on_press_tx(on_press)
}

pub fn card(p: &Pal) -> El {
    col().radius(12.0).fill(p.card).border(1.0, p.border)
}

/// A single-line field. `edit` picks the `TextEdit` it binds; `submit` runs on Return.
pub fn field(
    key: &str,
    edit: &TextEdit,
    placeholder: &str,
    style: TextStyle,
    p: &Pal,
    bind: impl for<'a> Fn(&'a mut State) -> &'a mut TextEdit + 'static,
    submit: impl Fn(&mut State, &mut Tx) + 'static,
) -> El {
    text_input(edit, style)
        .key(key.to_owned())
        .label(placeholder)
        .single_line()
        .grow(1.0)
        .color(p.text)
        .placeholder(placeholder, p.faint)
        .on_edit(bind)
        .on_submit(submit)
}

/// The dashed "Add …" box with Top/Bottom placement.
pub fn add_box(
    key: &str,
    edit: &TextEdit,
    placeholder: &str,
    s: &State,
    bind: impl for<'a> Fn(&'a mut State) -> &'a mut TextEdit + 'static,
    submit: impl Fn(&mut State, &mut Tx) + 'static,
    compact: bool,
) -> El {
    add_box_with(key, edit, placeholder, s, bind, submit, compact, None)
}

/// `add_box` with something (a quest chip) between the field and Top/Bottom.
#[allow(clippy::too_many_arguments)]
pub fn add_box_with(
    key: &str,
    edit: &TextEdit,
    placeholder: &str,
    s: &State,
    bind: impl for<'a> Fn(&'a mut State) -> &'a mut TextEdit + 'static,
    submit: impl Fn(&mut State, &mut Tx) + 'static,
    compact: bool,
    extra: Option<El>,
) -> El {
    let p = theme::pal(s.dark);
    let toggle = |at: AddAt, glyph: &'static Icon, label: &str| {
        let on = s.add_at == at;
        let mut b = row()
            .key(format!("{key}-{}", label.to_lowercase()))
            .role("radio")
            .label(label)
            .checked(on)
            .focusable(true)
            .h(30.0)
            .px(if compact { 8.0 } else { 10.0 })
            .gap(6.0)
            .center_y()
            .radius(6.0)
            .cursor(Cursor::Pointer)
            .fill(if on { p.selected } else { Color::TRANSPARENT })
            .border(1.0, if on { p.border_strong } else { Color::TRANSPARENT })
            .child(icon(glyph, 14.0).color(if on { p.text } else { p.muted }));
        if !compact {
            b = b.child(text(label).font(ty::BODY).color(if on { p.text } else { p.muted }));
        }
        b.on_press(move |s: &mut State| s.add_at = at)
    };
    let switch = row()
        .gap(2.0)
        .padding(2.0)
        .radius(8.0)
        .fill(p.bar)
        .border(1.0, p.border)
        .child(toggle(AddAt::Top, &icons::TO_TOP, "Top"))
        .child(toggle(AddAt::Bottom, &icons::TO_BOTTOM, "Bottom"));
    row()
        .h(if compact { 48.0 } else { 54.0 })
        .pl(16.0)
        .pr(8.0)
        .gap(8.0)
        .center_y()
        .radius(10.0)
        .border(1.0, p.border_strong)
        .border_dash(4.0, 3.0)
        .child(field(key, edit, placeholder, ty::TASK, p, bind, submit))
        .children(extra)
        .child(switch)
}

/// What a task card shows besides title and quest.
#[derive(Clone, Copy)]
pub struct CardOptions {
    pub show_time: bool,
    pub key_prefix: &'static str,
    pub wrap_title: bool,
    /// The list the card sits in: the card drags into others, and ⌥ + arrows move it.
    pub list: Option<ListId>,
}

/// A task card: quest colour bar, title, complete check, quest chip, tracked time, play/pause.
pub fn task_card(s: &State, task: &Task, p: &Pal, options: CardOptions) -> El {
    let id = task.id;
    let quest = quest_of(s, task);
    let color = accent_of(quest, p);
    let done = task.is_done();
    let running = s.snap.running_entry().is_some_and(|e| e.task_id == Some(id));
    let tracked = s.snap.task_tracked_ms(id, s.now);
    let prefix = options.key_prefix;

    let title = card_title(task, p, options);
    // The chip picks the task's quest.
    let chip = chip(quest, p)
        .key(format!("{prefix}-chip-{id}"))
        .role("button")
        .label("Assign quest")
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .on_press_tx(move |s: &mut State, tx: &mut Tx| open_palette_in(s, Mode::AssignQuest(id), tx));
    let mut meta = row()
        .gap(8.0)
        .center_y()
        .min_w(0.0)
        .child(check(format!("{prefix}-check-{id}"), done, color, p, move |s: &mut State| s.toggle_done(id)))
        .child(chip);
    if prefix == "plan" && s.series_of(task).repeat.is_some() {
        meta = meta.child(icon(&icons::REPEAT, 14.0).role("img").color(p.muted).label("Recurring task"));
    }
    let mut controls = row().gap(10.0).center_y();
    if running || (options.show_time && tracked > 0) {
        let mut time = row().gap(8.0).center_y();
        if running {
            time = time.child(dot(color, 7.0));
        }
        controls = controls.child(time.child(text(format::timer(tracked)).font(ty::MONO_TIME).color(if done {
            p.faint
        } else {
            p.text
        })));
    }
    if !done {
        let (glyph, label) = if running { (&icons::PAUSE, "Pause") } else { (&icons::PLAY, "Start focus") };
        controls = controls.child(icon_button(
            format!("{prefix}-play-{id}"),
            label,
            glyph,
            p.muted,
            p,
            move |s: &mut State, _: &mut Tx| {
                if running { s.stop_focus() } else { s.start_focus(id) }
            },
        ));
    }
    let more = menu::button(s, prefix, id, p);
    let mut body = col().grow(1.0).min_w(0.0).gap(10.0).py(14.0).pl(14.0).pr(8.0).child(title);
    body = if options.wrap_title {
        // Narrow planner cards: the controls get their own row so the handle still fits.
        body.child(meta).child(row().w_full().center_y().child(controls).child(spacer()).child(more))
    } else {
        body.child(row().w_full().center_y().child(meta.child(controls)).child(spacer()).child(more))
    };
    let bar = el().w(3.0).my(14.0).radius(2.0).fill(color.alpha(if done { 0.45 } else { 1.0 }));
    let face = row().grow(1.0).min_w(0.0).child(bar).child(body);
    let face = match options.list {
        Some(_) => drag::card_face(s, prefix, id, face),
        None => face,
    };
    let edge = if running { p.accent.alpha(0.7) } else { p.border };
    let mut card = row()
        .key(format!("{prefix}-task-{id}"))
        .w_full()
        .radius(10.0)
        .fill(p.card)
        .pl(8.0)
        .hover_group()
        .border(1.0, edge)
        .child(face)
        .on_hover_change(move |s: &mut State, inside: bool, tx: &mut Tx| {
            // The task shortcuts read this; nothing on screen does.
            if inside {
                s.hovered_task = Some(id);
            } else if s.hovered_task == Some(id) {
                s.hovered_task = None;
            }
            tx.live_only();
        });
    if running {
        card = card.shadow(0.0, 18.0, p.accent.alpha(0.18));
    }
    if done {
        card = card.opacity(0.75);
    }
    let Some(list) = options.list else {
        return card
            .role("listitem")
            .label(task.title.clone())
            .focusable(true)
            .on_press(move |s: &mut State| s.open_task_modal(id));
    };
    card = match drag::card_look(s, list, id) {
        CardLook::Normal => card,
        CardLook::Room => drag::ghost(card, p),
        CardLook::Left => card.opacity(0.35),
    };
    // The whole card drags once the pointer travels; a click opens its dialog. Its buttons take
    // their own presses.
    let card = card
        .role("listitem")
        .label(task.title.clone())
        .focusable(true)
        .on_key(move |s: &mut State, ev: &KeyEvent, tx: &mut Tx| card_key(s, list, id, prefix, ev, tx));
    let card = drag::source(card.cursor(Cursor::Grab), Some(Zone::Card(list, id)), Payload { task: id });
    drag::zone(card, Zone::Card(list, id))
}

/// A card's title. A click on it opens the task's dialog, like one on the rest of the card.
fn card_title(task: &Task, p: &Pal, options: CardOptions) -> El {
    let done = task.is_done();
    let mut title = text(task.title.clone()).font(ty::TASK).color(if done { p.faint } else { p.text });
    if done {
        title = title.strike(0, task.title.len(), Some(p.faint));
    }
    title = if options.wrap_title { title.wrap() } else { title.single_line().truncate() };
    title.key(format!("{}-title-{}", options.key_prefix, task.id)).self_start()
}

/// ↵ opens the dialog, ⌫ deletes, ⌥↑ / ⌥↓ reorder within the list, ⌥← / ⌥→ move to the day
/// before or after. A click focuses the card but doesn't pick it: its keys act only while the
/// keyboard drives focus, or on the card under the pointer.
fn card_key(s: &mut State, list: ListId, id: EntityId, prefix: &str, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if !(tx.focus_visible() || s.hovered_task == Some(id)) {
        return false;
    }
    if ev.key == Key::Enter && !ev.mods.cmd && !ev.mods.alt && !ev.mods.shift {
        s.open_task_modal(id);
        return true;
    }
    if matches!(ev.key, Key::Backspace | Key::Delete) && !ev.mods.cmd && !ev.mods.alt {
        // The card goes; focus mustn't move on into a field that would take ⌘Z.
        tx.blur();
        s.delete(Deletable::Task(id));
        return true;
    }
    let Some(key) = keys::card_key(ev) else { return false };
    let today = s.today();
    let command = match key {
        CardKey::Nudge { up } => {
            let Some(section) = s.plan_task(id).map(|t| s.section_of(&t)) else { return true };
            let order: Vec<EntityId> = drag::list_tasks(s, list, section).iter().map(|t| t.id).collect();
            let Some(position) = dnd::nudge(&order, id, up) else { return true };
            Command::MoveTask { id, planned_on: list.day(today), position, at: s.clock.now() }
        }
        CardKey::Day { later } => {
            let from = list.day(today).unwrap_or(today);
            let day = if later { from + Days::new(1) } else { from - Days::new(1) };
            Command::MoveTask { id, planned_on: Some(day), position: Position::Bottom, at: s.clock.now() }
        }
    };
    s.dispatch(command);
    tx.focus_when_shown(format!("{prefix}-task-{id}"), true);
    true
}

/// A thin horizontal progress bar; `fraction` is clamped to 0..=1.
pub fn progress_bar(key: &str, width: f32, height: f32, fraction: f32, fill: Color, track: Color) -> El {
    let filled = (width * fraction.clamp(0.0, 1.0)).max(if fraction > 0.0 { height } else { 0.0 });
    el().key(key.to_owned())
        .w(width)
        .h(height)
        .radius(height / 2.0)
        .fill(track)
        .child(el().w(filled).h(height).radius(height / 2.0).fill(fill))
}

pub fn empty_note(message: &str, p: &Pal) -> El {
    row().w_full().py(18.0).center().child(text(message).font(ty::BODY).color(p.faint))
}
