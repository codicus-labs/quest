//! Drag and drop between Rune and `dnd`: drop zones report their rects into `State::zones`,
//! drag sources feed pointer phases into `dnd::step`, and a finished drop runs its commands.
//! The views read `State::drag` to draw the lifted card and the room a list opens for it (its
//! siblings spring aside).

use std::borrow::Cow;

use chrono::NaiveDate;
use quest_domain::{EntityId, Task};
use rune::{Color, Drag, El, Phase, Transition, Tx, row, text};

use crate::dnd::{
    self, DragEvent, DragState, Effect, Geometry, ListGeom, ListId, Payload, Point, Rect, Room, Section, Slot, Target,
};
use crate::plan_strip::VISIBLE_DAYS;
use crate::state::{State, View};
use crate::theme::{Pal, ty};
use crate::ui::parts::{self, CardOptions};

/// An element whose laid-out rect drag and drop needs, named as its key is: Rune reports a rect
/// only when the element moves, so a zone named by data on an element keyed by position goes
/// stale when the data changes. `geometry` picks the zones the view shows now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Zone {
    /// Home's list viewport (QUEST WORK and OTHER scroll together).
    HomeList,
    /// The planner's backlog column. Day columns take drops where the strip's offset puts them
    /// (`plan_list_rects`), not where they last reported.
    PlanColumn(ListId),
    /// The planner's horizontal scroll viewport over the day strip.
    PlanBoard,
    /// A card, keyed by its task: it moves (and reports) whenever it changes lists.
    Card(ListId, EntityId),
}

/// The space between cards in a list that takes drops.
pub const LIST_GAP: f32 = 10.0;

/// Reports `el`'s rect as `zone` whenever layout moves it (scrolling included). The view
/// doesn't read zones, so the report needn't re-run it.
pub fn zone(el: El, zone: Zone) -> El {
    el.on_layout_tx(move |s: &mut State, rect, tx| {
        s.zones.insert(zone, Rect::from(rect));
        tx.live_only();
    })
}

/// Makes `el` a drag source carrying `payload`; `lift` names the zone drawn under the pointer
/// while dragging.
pub fn source(el: El, lift: Option<Zone>, payload: Payload) -> El {
    el.on_drag(move |s: &mut State, d: &mut Drag, tx: &mut Tx| {
        let at = Point::new(d.window_point()[0], d.window_point()[1]);
        let event = match d.phase {
            Phase::Down => {
                let lift = lift.and_then(|z| s.zones.get(&z).copied());
                DragEvent::Down { payload, at, lift }
            }
            Phase::Move => DragEvent::Move { at, target: target_at(s, at) },
            Phase::Up if d.cancelled() => DragEvent::Cancel,
            Phase::Up => DragEvent::Up { at, target: target_at(s, at) },
        };
        let room_before = room(s);
        let moved = matches!(event, DragEvent::Move { .. });
        let (next, effect) = dnd::step(s.drag, event);
        s.drag = next;
        if moved {
            s.plan_edge_check(tx);
        }
        // Moves are instant, so the card follows the pointer; a room opening or closing, a
        // drop and a snap back animate, so the cards around it spring.
        if room(s) != room_before || matches!(effect, Effect::Drop(..) | Effect::SnapBack(_)) {
            tx.animated();
        }
        apply(s, effect, tx);
    })
}

fn target_at(s: &State, at: Point) -> Option<Target> {
    let (payload, origin, lift) = match &s.drag {
        DragState::Idle => return None,
        DragState::Pressed { payload, origin, lift } | DragState::Dragging { payload, origin, lift, .. } => {
            (payload, origin, lift)
        }
    };
    let lifted = lift.map(|r| r.offset(at.x - origin.x, at.y - origin.y));
    dnd::hit(&geometry(s), payload, at, lifted)
}

/// The room a list has open for the dragged card.
fn room(s: &State) -> Option<Room> {
    let Target { list, before } = s.drag.target()?;
    Some(Room { list, before })
}

fn apply(s: &mut State, effect: Effect, tx: &mut Tx) {
    match effect {
        Effect::None => {}
        Effect::SnapBack(payload) => land(s, payload),
        Effect::Click(payload) => {
            s.open_task_modal(payload.task);
            tx.scroll_into_view("task-modal");
        }
        Effect::Drop(payload, target) => {
            land(s, payload);
            let id = payload.task;
            let Some(section) = s.plan_task(id).map(|t| s.section_of(&t)) else { return };
            let order: Vec<EntityId> = list_tasks(s, target.list, section).iter().map(|t| t.id).collect();
            if !dnd::changes_order(&order, id, target.before) {
                return;
            }
            let command = dnd::drop_command(id, target, s.today(), s.clock.now());
            s.dispatch(command);
        }
    }
}

/// Plan keeps a locally completed card at its original date and sort key until its hold expires.
pub fn list_tasks(s: &State, list: ListId, section: Section) -> Vec<Cow<'_, Task>> {
    let shown = |t: &Cow<'_, Task>| {
        s.section_of(t) == section
            && (section == Section::Other || s.quest_filter.is_none_or(|q| t.quest_id == Some(q)))
    };
    let open = |task: &&Task| !task.is_done();
    let tasks: Vec<_> = match list {
        ListId::Day(day) => {
            let mut tasks: Vec<_> = s
                .snap
                .tasks_on(day)
                .filter(open)
                .chain(
                    s.lingering_tasks
                        .keys()
                        .filter_map(|id| s.snap.task(*id))
                        .filter(|task| s.lingering(task).is_some_and(|before| before.planned_on == Some(day))),
                )
                .map(Cow::Borrowed)
                .chain(s.snap.planned_occurrences(day, s.now).map(Cow::Owned))
                .filter(shown)
                .collect();
            tasks.sort_by(|a, b| {
                let a = s.lingering(a).unwrap_or(a);
                let b = s.lingering(b).unwrap_or(b);
                a.sort_key.cmp(&b.sort_key).then(a.id.cmp(&b.id))
            });
            tasks
        }
        ListId::Home => s.snap.tasks_on(s.today()).map(Cow::Borrowed).filter(shown).collect(),
        ListId::Backlog => s
            .snap
            .tasks
            .iter()
            .filter(|t| {
                let before = s.lingering(t).unwrap_or(t);
                before.planned_on.is_none() && !before.is_done()
            })
            .map(Cow::Borrowed)
            .filter(shown)
            .collect(),
    };
    tasks
}

/// The planner's lists that take drops, left to right, with where each shows: the day columns
/// the strip's current offset puts in the board (each with the gap after it), then the backlog.
pub fn plan_list_rects(s: &State) -> Vec<(ListId, Rect)> {
    let mut lists = Vec::new();
    if let Some(board) = s.zones.get(&Zone::PlanBoard) {
        let (strip, offset) = (s.plan.strip, s.plan.offset());
        let first = strip.day_at(offset, 0.0);
        for day in (0..=VISIBLE_DAYS as u64).map(|i| first + chrono::Days::new(i)) {
            let x = board.x + strip.x_of(day) - offset;
            let (left, right) = (x.max(board.x), (x + strip.stride()).min(board.x + board.w));
            if right > left {
                lists.push((ListId::Day(day), Rect { x: left, y: board.y, w: right - left, h: board.h }));
            }
        }
    }
    if s.show_backlog
        && let Some(rect) = s.zones.get(&Zone::PlanColumn(ListId::Backlog))
    {
        lists.push((ListId::Backlog, *rect));
    }
    lists
}

/// The day of the task a drag carries: its column has to stay built, as its card holds the drag.
pub fn dragged_day(s: &State) -> Option<NaiveDate> {
    let payload = match &s.drag {
        DragState::Idle => return None,
        DragState::Pressed { payload, .. } | DragState::Dragging { payload, .. } => payload,
    };
    s.plan_task(payload.task)?.planned_on
}

/// Where the pointer of a drag carrying a task is.
pub fn task_pointer(s: &State) -> Option<Point> {
    match s.drag {
        DragState::Dragging { pointer, .. } => Some(pointer),
        _ => None,
    }
}

/// The view scrolled under a resting drag: the target becomes what is under the pointer now.
pub fn follow_scroll(s: &mut State) {
    let Some(at) = task_pointer(s) else { return };
    let target = target_at(s, at);
    s.drag = dnd::step(s.drag, DragEvent::Move { at, target }).0;
}

/// The section of the task a press or drag carries: its cards are the ones it can drop among.
fn carried_section(s: &State) -> Option<Section> {
    let payload = match &s.drag {
        DragState::Idle => return None,
        DragState::Pressed { payload, .. } | DragState::Dragging { payload, .. } => payload,
    };
    s.plan_task(payload.task).map(|t| s.section_of(&t))
}

/// What takes drops in the current view. Zones from other views (or cards that moved away)
/// are left out by building from what the view shows now. A list's cards are those of the
/// carried task's section, so a card drops only among its own kind.
pub fn geometry(s: &State) -> Geometry {
    let mut geometry = Geometry { room: room(s), ..Geometry::default() };
    let section = carried_section(s).unwrap_or(Section::Quest);
    let lists = match s.view {
        View::Home => s.zones.get(&Zone::HomeList).map(|r| (ListId::Home, *r)).into_iter().collect(),
        View::Plan => plan_list_rects(s),
        _ => Vec::new(),
    };
    for (list, rect) in lists {
        let cards = list_tasks(s, list, section)
            .iter()
            .filter_map(|t| s.zones.get(&Zone::Card(list, t.id)).map(|r| (t.id, *r)))
            .collect();
        geometry.lists.push(ListGeom { list, rect, cards, spacing: LIST_GAP });
    }
    geometry
}

/// The item a drag lifted, so its source can fade.
pub fn dragged(s: &State) -> Option<EntityId> {
    s.drag.dragging().map(|p| p.task)
}

/// Where in `list` the dragged task would drop: before this card, or at the end (`Some(None)`).
pub fn list_drop(s: &State, list: ListId) -> Option<Option<EntityId>> {
    let target = s.drag.target()?;
    (target.list == list).then_some(target.before)
}

/// The cards of one section of `list`, with the room a dragged card of that section would drop
/// into. In the card's own list the card itself moves into the room (its element keeps the
/// drag's pointer), drawn as a ghost; in another list a ghost of it opens the room.
pub fn list_cards(s: &State, p: &Pal, list: ListId, section: Section, options: CardOptions) -> Vec<El> {
    let tasks = list_tasks(s, list, section);
    let order: Vec<EntityId> = tasks.iter().map(|t| t.id).collect();
    let dragged =
        s.drag.dragging().map(|p| p.task).filter(|id| s.plan_task(*id).is_some_and(|t| s.section_of(&t) == section));
    let room = if dragged.is_some() { list_drop(s, list) } else { None };
    let card = |id: EntityId| {
        let task = tasks.iter().find(|t| t.id == id)?;
        let face = parts::task_card(s, task, p, options);
        Some(match super::task_modal::inline(s, task, p, true) {
            Some(detail) => rune::col().w_full().child(face).child(detail),
            None => face,
        })
    };
    dnd::slots(&order, dragged, room)
        .into_iter()
        .filter_map(|slot| match slot {
            Slot::Card(id) => card(id),
            Slot::Room if order.contains(&dragged?) => card(dragged?),
            Slot::Room => ghost_card(s, p, dragged?, options),
        })
        .collect()
}

/// A dragged card's ghost in another list than its own: the room it would drop into.
fn ghost_card(s: &State, p: &Pal, id: EntityId, options: CardOptions) -> Option<El> {
    let task = s.plan_task(id)?;
    let card = parts::task_card(s, &task, p, CardOptions { key_prefix: "ghost", list: None, ..options });
    Some(ghost(card.key("drag-room"), p).inert())
}

/// How a list card shows while a drag may be running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardLook {
    Normal,
    /// Dragged and moved into its list's room.
    Room,
    /// Dragged to another list or off every list: left behind, faded.
    Left,
}

pub fn card_look(s: &State, list: ListId, id: EntityId) -> CardLook {
    if dragged(s) != Some(id) {
        return CardLook::Normal;
    }
    if list_drop(s, list).is_some() { CardLook::Room } else { CardLook::Left }
}

/// Draws `card` as the room a dragged card would drop into: faint, with a dashed outline.
pub fn ghost(card: El, p: &Pal) -> El {
    card.fill(p.accent.alpha(0.06)).border(1.5, p.accent.alpha(0.7)).border_dash(6.0, 4.0).opacity(0.45).shadow(
        0.0,
        0.0,
        Color::TRANSPARENT,
    )
}

/// The id a card's face and the lifted card share, so a drop or a snap back flies the lifted
/// card into the list. It names the view (`prefix`): Home's and Plan's cards of one task must
/// not match, or switching views flies the card from one to the other.
fn matched_id(prefix: &str, id: EntityId) -> String {
    format!("card-{prefix}-{id}")
}

/// The key of a card's face (everything inside its frame). It changes when the card lands from
/// a drag, so the face mounts anew and Rune flies the leaving lifted card into it.
pub fn card_face(s: &State, prefix: &str, id: EntityId, face: El) -> El {
    let key = match s.landing {
        Some((landed, n)) if landed == id => format!("{prefix}-face-{id}-{n}"),
        _ => format!("{prefix}-face-{id}"),
    };
    // A face that mounts with nothing to fly from (the card that landed before) shows at once.
    let face = face.key(key).transition(Transition::none());
    if dragged(s) == Some(id) { face } else { face.matched(matched_id(prefix, id)) }
}

/// Notes that a dragged card was dropped or sent back, for its flight home.
fn land(s: &mut State, payload: Payload) {
    let id = payload.task;
    let n = s.landing.map_or(0, |(_, n)| n.wrapping_add(1));
    s.landing = Some((id, n));
}

/// The lifted card (or, without its rect, a title pill) following the pointer.
pub fn layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let DragState::Dragging { payload: Payload { task: id }, origin, pointer, lift, .. } = s.drag else {
        return;
    };
    let Some(task) = s.plan_task(id) else { return };
    let lifted = match lift {
        Some(rect) => {
            let at = rect.offset(pointer.x - origin.x, pointer.y - origin.y);
            // Laid out as in its list, so it keeps the card's exact size.
            let options = lift_options(s);
            let card = parts::task_card(s, &task, p, CardOptions { key_prefix: "lift", list: None, ..options });
            let card = card.fill(p.card).border(1.0, p.accent.alpha(0.6)).matched(matched_id(options.key_prefix, id));
            rune::el().absolute(at.x, at.y).w(rect.w).child(card)
        }
        None => row()
            .absolute(pointer.x + 8.0, pointer.y - 16.0)
            .h(32.0)
            .px(12.0)
            .center_y()
            .radius(8.0)
            .fill(p.card)
            .border(1.0, p.accent.alpha(0.6))
            .child(text(task.title.clone()).font(ty::SMALL_MEDIUM).color(p.text).single_line())
            .matched(matched_id(lift_options(s).key_prefix, id)),
    };
    layer.push(
        lifted
            .key("drag-lift")
            .radius(10.0)
            .shadow(0.0, 28.0, Color::BLACK.alpha(0.45))
            .instant_changes()
            .transition(Transition::none())
            .inert(),
    );
}

/// How cards in `list` are drawn: Home's wide rows show tracked time on one line; the
/// planner's narrow columns wrap the title and give the controls their own row.
pub fn list_card_options(list: ListId) -> CardOptions {
    match list {
        ListId::Home => CardOptions { show_time: true, key_prefix: "home", wrap_title: false, list: Some(list) },
        ListId::Day(_) | ListId::Backlog => {
            CardOptions { show_time: false, key_prefix: "plan", wrap_title: true, list: Some(list) }
        }
    }
}

/// How the dragged card is drawn in the current view: every planner list draws cards alike.
fn lift_options(s: &State) -> CardOptions {
    list_card_options(if s.view == View::Plan { ListId::Backlog } else { ListId::Home })
}
