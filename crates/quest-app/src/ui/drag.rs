use std::{cell::Cell, rc::Rc, time::Duration};

use quest_domain::{EntityId, Position, Task, TaskList};
use rune::{Color, Drag, El, Phase, ScrollPos, Transition, Tx, col, el};

use super::task_row::{self, Shows};
use crate::dnd::{
    self, DragEvent, DragState, Effect, Geometry, ListGeom, ListId, Payload, Point, Rect, Room, Slot, Target,
};
use crate::state::{State, View};
use crate::theme::Pal;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Zone {
    Viewport,
    List(ListId),
    Row(TaskList, EntityId),
}

pub fn zone(el: El, zone: Zone) -> El {
    el.on_layout_tx(move |s: &mut State, rect, tx| {
        s.zones.insert(zone, Rect::from(rect));
        tx.live_only();
    })
}

#[derive(Default)]
pub struct AutoScroll {
    position: Rc<Cell<ScrollPos>>,
    last_frame: Option<f64>,
    last_offset: Option<f32>,
}

pub fn viewport(s: &State, el: El) -> El {
    let position = s.drag_scroll.position.clone();
    zone(el.on_scroll_offset(move |pos| position.set(pos)), Zone::Viewport)
}

pub fn auto_scroll(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    let DragState::Dragging { pointer, .. } = s.drag else {
        s.drag_scroll.last_frame = None;
        s.drag_scroll.last_offset = None;
        return false;
    };
    let Some(viewport) = s.zones.get(&Zone::Viewport).copied() else { return false };
    let pos = s.drag_scroll.position.get();
    let y = pos.offset[1];
    let changed = s.drag_scroll.last_offset.replace(y).is_some_and(|old| old != y) && update_target(s, pointer, tx);
    let speed = dnd::edge_speed(viewport, pointer);
    if (speed < 0.0 && y > 0.0) || (speed > 0.0 && y < pos.max[1]) {
        let dt = s.drag_scroll.last_frame.replace(now).map_or(0.0, |last| (now - last).clamp(0.0, 0.05));
        let key = match s.view {
            View::Plan => "plan-page-scroll",
            View::Quest(_) => "quest-detail-scroll",
            _ => "list-page-scroll",
        };
        if dt > 0.0 {
            tx.scroll_to_y(key, (y + speed * dt as f32).clamp(0.0, pos.max[1]));
        }
        tx.wake_after(Duration::ZERO);
    } else {
        s.drag_scroll.last_frame = None;
    }
    changed
}

fn update_target(s: &mut State, pointer: Point, tx: &mut Tx) -> bool {
    let target = target_at(s, pointer);
    let DragState::Dragging { target: current, .. } = &mut s.drag else { return false };
    if *current == target {
        return false;
    }
    *current = target;
    tx.animated();
    true
}

pub fn source(el: El, list: TaskList, id: EntityId, tray_alias: bool) -> El {
    let row = Zone::Row(list, id);
    zone(
        el.on_drag(move |s: &mut State, d: &mut Drag, tx: &mut Tx| {
            let at = Point::new(d.window_point()[0], d.window_point()[1]);
            let event = match d.phase {
                Phase::Down => {
                    s.landing = None;
                    s.drag_tray_alias = tray_alias;
                    s.drag_scroll.last_frame = None;
                    s.drag_scroll.last_offset = None;
                    DragEvent::Down { payload: Payload { task: id }, at, lift: s.zones.get(&row).copied() }
                }
                Phase::Move => DragEvent::Move { at, target: target_at(s, at) },
                Phase::Up if d.cancelled() => DragEvent::Cancel,
                Phase::Up => {
                    let target = match s.drag {
                        DragState::Dragging { pointer, target, .. } if pointer == at => target,
                        _ => target_at(s, at),
                    };
                    DragEvent::Up { at, target }
                }
            };
            let was_dragging = s.drag.dragging().is_some();
            let (next, effect) = dnd::step(s.drag, event);
            s.drag = next;
            if was_dragging || s.drag.dragging().is_some() {
                // A lift can remove the source gap even when both targets are None.
                tx.animated();
            }
            if s.drag.dragging().is_some() {
                tx.wake_after(Duration::ZERO);
            }
            match effect {
                Effect::None => {}
                Effect::Click(payload) => task_row::open(s, payload.task, tray_alias, tx),
                Effect::SnapBack(_) => {}
                Effect::Drop(payload, target) => {
                    s.landing = match target.list {
                        ListId::File(list) => Some(Landing::Sidebar(payload.task, list)),
                        ListId::Rows(_) if tray_alias => Some(Landing::FromTray(payload.task)),
                        ListId::Rows(_) => None,
                    };
                    place(s, payload.task, target, tx);
                }
            }
        }),
        row,
    )
}

fn target_at(s: &State, at: Point) -> Option<Target> {
    let (payload, origin, lift) = match s.drag {
        DragState::Idle => return None,
        DragState::Pressed { payload, origin, lift } | DragState::Dragging { payload, origin, lift, .. } => {
            (payload, origin, lift)
        }
    };
    if let Some(list) = sidebar_lists(s)
        .map(ListId::File)
        .find(|list| s.zones.get(&Zone::List(*list)).is_some_and(|rect| rect.contains(at)))
    {
        return Some(Target { list, before: None });
    }
    if !s.zones.get(&Zone::Viewport).is_some_and(|rect| rect.contains(at)) {
        return None;
    }
    let mut geometry = Geometry {
        room: s.drag.target().map(|target| Room { list: target.list, before: target.before }),
        ..Geometry::default()
    };
    for (list, order) in orders(s) {
        let Some(rect) = s.zones.get(&Zone::List(ListId::Rows(list))).copied() else { continue };
        if !rect.contains(at) {
            continue;
        }
        let own_in_flow = s.drag.dragging().is_none();
        let cards = order
            .into_iter()
            .filter(|id| *id != payload.task || own_in_flow)
            .filter_map(|id| s.zones.get(&Zone::Row(list, id)).map(|rect| (id, *rect)))
            .collect();
        geometry.lists.push(ListGeom { list: ListId::Rows(list), rect, cards, spacing: 0.0 });
    }
    dnd::hit(&geometry, &payload, at, lift.map(|r| r.offset(at.x - origin.x, at.y - origin.y)))
}

fn sidebar_lists(s: &State) -> impl Iterator<Item = TaskList> + '_ {
    [TaskList::Today, TaskList::Inbox].into_iter().chain(s.snap.active_quests().map(|q| TaskList::Quest(q.id)))
}

pub fn sidebar(s: &State, list: TaskList, item: El, p: &Pal) -> El {
    let list = ListId::File(list);
    let over = s.drag.target().is_some_and(|target| target.list == list);
    let landing = match s.landing {
        Some(Landing::Sidebar(id, file)) if ListId::File(file) == list => Some(
            el().key(format!("sidebar-landing-{id}"))
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .radius(7.0)
                .border(1.0, p.accent)
                .opacity(0.0)
                .matched(matched_id(id, s.drag_tray_alias))
                .inert(),
        ),
        _ => None,
    };
    let item = if over { item.fill(p.selected).border(1.0, p.accent) } else { item };
    zone(item.children(landing), Zone::List(list))
}

fn orders(s: &State) -> Vec<(TaskList, Vec<EntityId>)> {
    if s.view == View::Plan {
        return super::plan::model(s)
            .all_days()
            .map(|day| {
                let list = if day.date == s.today() { TaskList::Today } else { TaskList::Day(day.date) };
                (list, day.tasks.iter().map(|task| task.id).collect())
            })
            .collect();
    }
    super::lists::sort_list(s.view)
        .map(|list| {
            (
                list,
                super::lists::shown_tasks(s)
                    .into_iter()
                    .filter(|id| s.snap.task(*id).is_some_and(|task| !task.is_done()))
                    .collect(),
            )
        })
        .into_iter()
        .collect()
}

pub fn nudge(s: &mut State, up: bool, tx: &mut Tx) -> bool {
    let Some(id) = s.selected_task else { return false };
    let Some((list, order)) = orders(s).into_iter().find(|(_, order)| order.contains(&id)) else { return false };
    if let Some(position) = dnd::nudge(&order, id, up) {
        let before = if let Position::Before(id) = position { Some(id) } else { None };
        place(s, id, Target { list: ListId::Rows(list), before }, tx);
    }
    true
}

fn place(s: &mut State, id: EntityId, target: Target, tx: &mut Tx) {
    if let ListId::Rows(list) = target.list
        && orders(s)
            .into_iter()
            .find(|(current, _)| *current == list)
            .is_some_and(|(_, order)| !dnd::changes_order(&order, id, target.before))
    {
        return;
    }
    s.dispatch(dnd::drop_command(id, target, s.today(), s.clock.now()));
    s.selected_task = Some(id);
    tx.animated();
}

pub fn rows(s: &State, p: &Pal, list: TaskList, tasks: &[&Task], shows: Shows) -> El {
    let id = ListId::Rows(list);
    let carried = s.drag.dragging().map(|payload| payload.task);
    let room = s.drag.target().filter(|target| target.list == id).map(|target| target.before);
    let order: Vec<_> = tasks.iter().map(|task| task.id).collect();
    let own = carried.filter(|id| order.contains(id));
    let render = |id| {
        let task = tasks.iter().find(|task| task.id == id)?;
        Some(task_row::in_list(s, task, shows, p, list))
    };
    let slots = dnd::slots(&order, carried, room);
    col()
        .key("sortable-rows")
        .w_full()
        .children(slots.into_iter().filter_map(|slot| match slot {
            Slot::Card(id) if Some(id) == own => None,
            Slot::Card(id) => render(id),
            Slot::Room => Some(ghost(el().key("drag-room").w_full().h(38.0).radius(7.0), p)),
        }))
        .children(own.and_then(render).map(|row| row.absolute(0.0, 0.0).opacity(0.0)))
}

fn ghost(row: El, p: &Pal) -> El {
    row.fill(p.accent.alpha(0.06)).border(1.5, p.accent.alpha(0.7)).border_dash(6.0, 4.0).opacity(0.45)
}

pub enum Landing {
    Sidebar(EntityId, TaskList),
    FromTray(EntityId),
}

fn matched_id(id: EntityId, tray_alias: bool) -> String {
    if tray_alias { format!("tray-lift-{id}") } else { format!("task-lift-{id}") }
}

pub fn face(s: &State, id: EntityId, tray_alias: bool, row: El) -> El {
    let carrying = s.drag.dragging().is_some_and(|payload| payload.task == id);
    let from_tray = matches!(s.landing, Some(Landing::FromTray(task)) if task == id);
    // Rune matches an unmounted face to a newly mounted one; keep the gesture's parent.
    let prefix = match (tray_alias, carrying, from_tray) {
        (true, true, _) if s.drag_tray_alias => "tray-face-ghost",
        (true, _, _) => "tray-face",
        (false, true, _) => "face-ghost",
        (false, false, true) => "face-from-tray",
        (false, false, false) => "face",
    };
    let row = row.key(format!("{prefix}-{id}")).transition(Transition::none());
    let filed = matches!(s.landing, Some(Landing::Sidebar(task, _)) if task == id);
    if (carrying && (!tray_alias || s.drag_tray_alias)) || filed || (tray_alias && from_tray) {
        return row;
    }
    row.matched(matched_id(id, tray_alias || from_tray))
}

pub fn layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let DragState::Dragging { payload: Payload { task: id }, origin, pointer, lift: Some(rect), .. } = s.drag else {
        return;
    };
    let Some(task) = s.plan_task(id) else { return };
    let shows =
        Shows { quest: !matches!(s.view, View::Quest(_)), plan_ahead: !matches!(s.view, View::Plan | View::Today) };
    let at = rect.offset(pointer.x - origin.x, pointer.y - origin.y);
    let left = s.zones.get(&Zone::List(ListId::File(TaskList::Today))).map_or(0.0, |nav| nav.x + nav.w + 12.0);
    let face = task_row::face(s, &task, shows, p, "lift")
        .fill(p.card)
        .border(1.0, p.accent.alpha(0.6))
        .key("drag-lift-face")
        .animated_changes()
        .transition(Transition::none())
        .matched(matched_id(id, s.drag_tray_alias));
    layer.push(
        el().key("drag-lift")
            .absolute(at.x.max(left), at.y)
            .w(rect.w)
            .child(face)
            .radius(7.0)
            .opacity(0.93)
            .shadow(0.0, 20.0, Color::BLACK.alpha(0.35))
            .instant_changes()
            .transition(Transition::none())
            .inert(),
    );
}
