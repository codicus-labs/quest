//! The left sidebar: workspace switcher and account, the views with their counts, the active
//! quests with their hours this week (a ring against the weekly goal), completed quests and a
//! new quest; the player card at the bottom.

use chrono::Local;
use quest_domain::{EntityId, TaskList};
use quest_view::lists;
use quest_view::player::{self, Player};
use quest_view::tracked::{self, QuestWeek};
use rune::{Color, Cursor, El, Icon, ScrollSpec, Shape, Tx, canvas, col, el, icon, row, spacer, text};

use crate::format;
use crate::icons;
use crate::state::{QuestsTab, State, View};
use crate::theme::{self, Pal, quest_color, ty};
use crate::ui::{chrome, parts, workspaces};

pub const WIDTH: f32 = 250.0;
/// The band the window's controls sit in.
pub const TITLEBAR: f32 = 52.0;

pub fn view(s: &State, p: &Pal) -> El {
    let counts = lists::nav_counts(&s.snap, s.today());
    let week = tracked::week_balance(&s.snap, s.today(), &Local, s.now);
    let nav = col()
        .w_full()
        .gap(1.0)
        .child(nav_item(s, View::Inbox, &icons::INBOX, p.icon_inbox, counts.inbox, p))
        .child(nav_item(s, View::Today, &icons::TODAY, p.accent, counts.today, p))
        .child(nav_item(s, View::Plan, &icons::PLAN, p.icon_plan, counts.plan, p))
        .child(nav_item(s, View::Review, &icons::REVIEW, p.muted, 0, p))
        .child(nav_item(s, View::Logbook, &icons::LOGBOOK, p.icon_logbook, 0, p));
    let mut quests = col().w_full().gap(1.0);
    for quest in &week.quests {
        quests = quests.child(quest_item(s, quest, week.target_ms, p));
    }
    quests = quests
        .child(link("nav-completed", &icons::COMPLETED, "Completed", p, |s: &mut State, tx: &mut Tx| {
            s.go_with(View::Quests, tx);
            s.quests_tab = QuestsTab::Completed;
        }))
        .child(link("nav-new-quest", &icons::ADD, "New quest", p, |s: &mut State, _: &mut Tx| s.open_new_quest()));
    let scroller = col()
        .w_full()
        .child(nav)
        .child(text("QUESTS").font(ty::CAPS).color(p.faint).px(10.0).mt(22.0).mb(6.0))
        .child(quests)
        .scroll(ScrollSpec { stick_bottom: false })
        .grow(1.0)
        .min_h(0.0);
    let account = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .mb(12.0)
        .child(workspaces::switcher(s, p).grow(1.0).min_w(0.0))
        .child(chrome::avatar(s, p));
    let mut side = col()
        .key("sidebar")
        .w(WIDTH)
        .h_full()
        .px(10.0)
        .pb(10.0)
        .fill(p.side)
        .child(el().w_full().h(TITLEBAR).window_drag())
        .child(account)
        .child(scroller);
    if let Some(note) = workspaces::timer_note(s, p) {
        side = side.child(row().mt(10.0).child(note));
    }
    side.children(player_card(s, p))
}

fn nav_item(s: &State, view: View, glyph: &'static Icon, ink: Color, count: usize, p: &Pal) -> El {
    let on = s.view == view;
    let mut item = row()
        .key(format!("nav-{}", view.title().to_lowercase()))
        .role("tab")
        .label(view.title())
        .checked(on)
        .focusable(true)
        .w_full()
        .h(32.0)
        .px(10.0)
        .gap(10.0)
        .center_y()
        .radius(7.0)
        .cursor(Cursor::Pointer)
        .fill(if on { p.selected } else { Color::TRANSPARENT })
        .hover(p.hover)
        .child(icon(glyph, 18.0).color(ink))
        .child(text(view.title()).font(if on { ty::NAV_ON } else { ty::NAV }).color(p.text).grow(1.0))
        .on_press_tx(move |s: &mut State, tx: &mut Tx| s.go_with(view, tx));
    if count > 0 {
        item = item.child(text(count.to_string()).font(ty::SMALL).color(p.faint));
    }
    match view {
        View::Today => super::drag::sidebar(s, TaskList::Today, item, p),
        View::Inbox => super::drag::sidebar(s, TaskList::Inbox, item, p),
        _ => item,
    }
}

fn quest_item(s: &State, quest: &QuestWeek, target_ms: i64, p: &Pal) -> El {
    let view = View::Quest(quest.quest);
    let on = s.view == view;
    let item = row()
        .key(format!("nav-quest-{}", quest.quest))
        .role("tab")
        .label(quest.title.clone())
        .checked(on)
        .focusable(true)
        .w_full()
        .h(32.0)
        .px(10.0)
        .gap(10.0)
        .center_y()
        .radius(7.0)
        .cursor(Cursor::Pointer)
        .fill(if on { p.selected } else { Color::TRANSPARENT })
        .hover(p.hover)
        .help(format!("{} of {} this week", format::hours(quest.ms), format::hours(target_ms)))
        .child(ring(tracked::fraction(quest.ms, target_ms), quest_color(quest.color), p.border_strong))
        .child(
            text(quest.title.clone())
                .font(if on { ty::NAV_ON } else { ty::NAV })
                .color(p.text)
                .single_line()
                .truncate()
                .grow(1.0)
                .min_w(0.0),
        )
        .child(text(format::hours(quest.ms)).font(ty::SMALL).color(p.faint))
        .on_press_tx(move |s: &mut State, tx: &mut Tx| s.go_with(view, tx));
    super::drag::sidebar(s, TaskList::Quest(quest.quest), item, p)
}

/// An 18-point ring: the track, and the share `fraction` drawn clockwise from the top.
fn ring(fraction: f32, color: Color, track: Color) -> El {
    const SIZE: f32 = 18.0;
    const WIDTH: f32 = 2.4;
    let radius = SIZE / 2.0 - WIDTH;
    let point = |turn: f32| {
        let angle = turn * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        [SIZE / 2.0 + radius * angle.cos(), SIZE / 2.0 + radius * angle.sin()]
    };
    let circle: Vec<[f32; 2]> = (0..48).map(|i| point(i as f32 / 48.0)).collect();
    let mut shape = Shape::new().view_box(SIZE, SIZE).stroke_with(circle, true, WIDTH, rune::LineCap::Round, track);
    if fraction > 0.0 {
        let steps = (fraction * 48.0).ceil().max(2.0) as usize;
        let arc: Vec<[f32; 2]> = (0..=steps).map(|i| point(fraction * i as f32 / steps as f32)).collect();
        shape = shape.stroke(arc, WIDTH, color);
    }
    canvas(shape).size(SIZE, SIZE)
}

fn link(key: &str, glyph: &'static Icon, label: &str, p: &Pal, press: fn(&mut State, &mut Tx)) -> El {
    row()
        .key(key.to_owned())
        .role("button")
        .label(label)
        .focusable(true)
        .w_full()
        .h(32.0)
        .px(10.0)
        .gap(10.0)
        .center_y()
        .radius(7.0)
        .cursor(Cursor::Pointer)
        .hover(p.hover)
        .child(icon(glyph, 18.0).color(p.muted))
        .child(text(label.to_owned()).font(ty::NAV).color(p.muted))
        .on_press_tx(press)
}

/// What runs (with Stop and Focus), Organizing (with "Back to" its task), or the task to resume.
fn player_card(s: &State, p: &Pal) -> Option<El> {
    let card = col().key("player").w_full().mt(14.0).padding(12.0).radius(12.0).fill(p.card).border(1.0, p.border);
    let since = |at: chrono::DateTime<chrono::Utc>| format::timer((s.now - at).num_milliseconds());
    Some(match player::player(&s.snap) {
        Player::Empty => return None,
        Player::Resume(task) => card.child(text("Resume").font(ty::SMALL).color(p.muted)).child(
            row().w_full().gap(8.0).center_y().child(title(&task.title, p)).child(round_button(
                "player-resume",
                "Resume",
                &icons::RESUME,
                p,
                task.task,
            )),
        ),
        Player::Running { task, title: name, quest, started_at } => {
            let quest = quest.and_then(|q| s.snap.quest(q));
            let label = row()
                .gap(6.0)
                .center_y()
                .child(parts::dot(quest.map_or(p.accent, |q| quest_color(q.color)), 7.0))
                .child(
                    text(quest.map_or_else(|| "No quest".to_owned(), |q| q.title.clone()))
                        .font(ty::SMALL)
                        .color(p.muted)
                        .single_line()
                        .truncate(),
                );
            let mut controls = row().w_full().gap(6.0).center_y().child(clock(since(started_at), p)).child(spacer());
            if task.is_some() {
                controls = controls.child(
                    parts::icon_button("player-focus", "Open focus", &icons::FOCUS, p.muted, p, |s: &mut State, tx| {
                        s.go_with(View::Focus, tx)
                    })
                    .help("Focus  F"),
                );
            }
            card.child(label).child(title(&name, p).mt(2.0)).child(controls.child(stop_button(true, p)))
        }
        Player::Organizing { started_at, back_to } => {
            let label = row()
                .gap(6.0)
                .center_y()
                .child(parts::dot(p.faint, 7.0))
                .child(text("Organizing").font(ty::SMALL).color(p.muted));
            let controls = row()
                .w_full()
                .center_y()
                .child(clock(since(started_at), p))
                .child(spacer())
                .child(stop_button(false, p));
            let mut card = card.child(label).child(controls.mt(4.0));
            if let Some(task) = back_to {
                let id = task.task;
                card = card.child(
                    row()
                        .key("player-back-to")
                        .role("button")
                        .label(format!("Back to {}", task.title))
                        .w_full()
                        .mt(10.0)
                        .pt(9.0)
                        .gap(6.0)
                        .center_y()
                        .border_sides([1.0, 0.0, 0.0, 0.0], p.border)
                        .cursor(Cursor::Pointer)
                        .child(icon(&icons::BACK_TO, 15.0).color(p.accent))
                        .child(
                            text(format!("Back to “{}”", task.title))
                                .font(ty::SMALL_MEDIUM)
                                .color(p.accent)
                                .single_line()
                                .truncate()
                                .min_w(0.0),
                        )
                        .on_press(move |s: &mut State| s.start_focus(id)),
                );
            }
            card
        }
    })
}

fn title(name: &str, p: &Pal) -> El {
    text(name.to_owned())
        .key("player-title")
        .font(ty::NAV_ON)
        .color(p.text)
        .single_line()
        .truncate()
        .grow(1.0)
        .min_w(0.0)
}

fn clock(time: String, p: &Pal) -> El {
    text(time).key("player-time").font(ty::TOTAL).color(p.text)
}

/// Stop: the accent pill for a task's session, a grey one for Organizing.
fn stop_button(accent: bool, p: &Pal) -> El {
    let (fill, ink) = if accent { (p.accent_fill, theme::on_accent_fill()) } else { (p.hover, p.text) };
    row()
        .key("player-stop")
        .role("button")
        .label("Stop")
        .focusable(true)
        .h(28.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .radius(14.0)
        .fill(fill)
        .cursor(Cursor::Pointer)
        .help("Stop  Space")
        .child(icon(&icons::STOP, 14.0).color(ink))
        .child(text("Stop").font(ty::SMALL_MEDIUM).color(ink))
        .on_press(|s: &mut State| s.stop_focus())
}

fn round_button(key: &str, label: &str, glyph: &'static Icon, p: &Pal, task: EntityId) -> El {
    el().key(key.to_owned())
        .role("button")
        .label(label)
        .focusable(true)
        .size(32.0, 32.0)
        .radius(16.0)
        .center()
        .fill(p.accent_fill)
        .cursor(Cursor::Pointer)
        .child(icon(glyph, 16.0).color(theme::on_accent_fill()))
        .on_press(move |s: &mut State| s.start_focus(task))
}
