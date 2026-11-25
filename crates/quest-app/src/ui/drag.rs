use quest_domain::{EntityId, Position, Task, TaskList};
use rune::{Color, Drag, El, Phase, Transition, Tx, col, el};

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

pub fn source(el: El, list: TaskList, id: EntityId) -> El {
    let row = Zone::Row(list, id);
    zone(
        el.on_drag(move |s: &mut State, d: &mut Drag, tx: &mut Tx| {
            let at = Point::new(d.window_point()[0], d.window_point()[1]);
            let event = match d.phase {
                Phase::Down => DragEvent::Down { payload: Payload { task: id }, at, lift: s.zones.get(&row).copied() },
                Phase::Move => DragEvent::Move { at, target: target_at(s, at) },
                Phase::Up if d.cancelled() => DragEvent::Cancel,
                Phase::Up => DragEvent::Up { at, target: target_at(s, at) },
            };
            let room = s.drag.target();
            let (next, effect) = dnd::step(s.drag, event);
            s.drag = next;
            if room != s.drag.target() || matches!(effect, Effect::Drop(..) | Effect::SnapBack(_)) {
                tx.animated();
            }
            match effect {
                Effect::None => {}
                Effect::Click(payload) => task_row::open(s, payload.task, tx),
                Effect::SnapBack(payload) => land(s, payload.task, None),
                Effect::Drop(payload, target) => {
                    let file = if let ListId::File(list) = target.list { Some(list) } else { None };
                    land(s, payload.task, file);
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
        let own_in_flow =
            s.drag.dragging().is_none() || s.drag.target().is_some_and(|target| target.list == ListId::Rows(list));
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
    let landing = s.landing.and_then(|(id, n, file)| {
        (file.map(ListId::File) == Some(list)).then(|| {
            el().key(format!("sidebar-landing-{id}-{n}"))
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .radius(7.0)
                .border(1.0, p.accent)
                .opacity(0.0)
                .matched(matched_id(id))
                .inert()
        })
    });
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
        let row = task_row::in_list(s, task, shows, p, list);
        Some(if own == Some(id) && room.is_some() { ghost(row, p) } else { row })
    };
    let hidden = own.filter(|_| room.is_none());
    let slots = dnd::slots(&order, carried, room);
    col()
        .w_full()
        .children(slots.into_iter().filter_map(|slot| match slot {
            Slot::Card(id) if Some(id) == hidden => None,
            Slot::Card(id) => render(id),
            Slot::Room => {
                own.and_then(render).or_else(|| Some(ghost(el().key("drag-room").w_full().h(38.0).radius(7.0), p)))
            }
        }))
        .children(hidden.and_then(render).map(|row| row.absolute(0.0, 0.0).opacity(0.0)))
}

fn ghost(row: El, p: &Pal) -> El {
    row.fill(p.accent.alpha(0.06)).border(1.5, p.accent.alpha(0.7)).border_dash(6.0, 4.0).opacity(0.45)
}

fn matched_id(id: EntityId) -> String {
    format!("task-lift-{id}")
}

pub fn face(s: &State, id: EntityId, row: El) -> El {
    let key = match s.landing {
        Some((landed, n, _)) if landed == id => format!("face-{id}-{n}"),
        _ => format!("face-{id}"),
    };
    let row = row.key(key).transition(Transition::none());
    if s.drag.dragging().is_some_and(|payload| payload.task == id)
        || s.landing.is_some_and(|(landed, _, file)| landed == id && file.is_some())
    {
        row
    } else {
        row.matched(matched_id(id))
    }
}

fn land(s: &mut State, task: EntityId, file: Option<TaskList>) {
    s.landing = Some((task, s.landing.map_or(0, |(_, n, _)| n.wrapping_add(1)), file));
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
        .matched(matched_id(id));
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
