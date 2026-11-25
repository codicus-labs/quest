//! The archive: archived quests (put away, also those an older version completed; Unarchive) and
//! everything deleted, grouped by kind, newest
//! first, each with Restore.

use quest_domain::{Command, EntityKind, QuestStatus, Trashed};
use rune::{El, ScrollSpec, Tx, col, row, spacer, text};

use crate::icons;
use crate::state::{State, View};
use crate::theme::{Pal, quest_color, ty};
use crate::ui::parts;
use crate::undo::Deletable;

/// The groups, in the order they show.
const GROUPS: [(EntityKind, &str); 3] = [
    (EntityKind::Quest, "Deleted quests"),
    (EntityKind::Task, "Deleted tasks"),
    (EntityKind::TimeEntry, "Deleted sessions"),
];

pub fn view(s: &State, p: &Pal) -> El {
    let header = row()
        .w_full()
        .center_y()
        .gap(16.0)
        .pb(28.0)
        .child(
            parts::button("archive-back", "Quests", Some(&icons::ARROW_LEFT), p, |s: &mut State, _: &mut Tx| {
                s.go(View::Quests)
            })
            .h(32.0),
        )
        .child(text("Archive").font(ty::HEADING).color(p.text))
        .child(spacer())
        .child(text("Deleted things stay here until you restore them.").font(ty::SMALL).color(p.faint));
    let mut list = col().key("archive-list").w_full().gap(10.0).pb(24.0);
    let archived: Vec<_> = s.snap.quests.iter().filter(|q| q.status == QuestStatus::Archived).collect();
    let mut empty = archived.is_empty();
    if !archived.is_empty() {
        list = list.child(parts::caps("Archived quests", p).mt(8.0));
        for quest in archived {
            let id = quest.id;
            let unarchive = parts::button(
                format!("archive-unarchive-{id}"),
                "Unarchive",
                None,
                p,
                move |s: &mut State, _: &mut Tx| {
                    s.dispatch(Command::SetQuestStatus { id, status: QuestStatus::Active });
                },
            );
            let open =
                parts::button(format!("archive-open-{id}"), "Open", None, p, move |s: &mut State, _: &mut Tx| {
                    s.go(View::Quest(id))
                });
            let actions = row().gap(8.0).child(open.h(30.0)).child(unarchive.h(30.0));
            list = list.child(item_row(
                format!("archive-quest-{id}"),
                &quest.title,
                "Archived quest",
                Some(quest_color(quest.color)),
                actions,
                p,
            ));
        }
    }
    for (kind, label) in GROUPS {
        let items: Vec<&Trashed> = s.snap.trash.iter().filter(|t| t.kind == kind).collect();
        if items.is_empty() {
            continue;
        }
        empty = false;
        list = list.child(parts::caps(label, p).mt(8.0));
        for item in items {
            list = list.child(trashed_row(s, item, p));
        }
    }
    if empty {
        list = list.child(parts::empty_note("Nothing archived or deleted.", p));
    }
    col()
        .key("archive")
        .w_full()
        .h_full()
        .px(30.0)
        .pt(40.0)
        .pb(16.0)
        .child(header)
        .child(list.scroll(ScrollSpec { stick_bottom: false }).grow(1.0).min_h(0.0))
}

fn trashed_row(s: &State, item: &Trashed, p: &Pal) -> El {
    let quest = item.quest_id.and_then(|q| s.snap.quest(q));
    let title = match (item.kind, &item.title) {
        (_, Some(title)) => title.clone(),
        (EntityKind::TimeEntry, None) => "Other work".to_owned(),
        (_, None) => "Untitled".to_owned(),
    };
    let deleted = crate::clock::local(item.deleted_at).format("Deleted %b %-d, %H:%M").to_string();
    let meta = match quest.filter(|q| q.id != item.id) {
        Some(q) => format!("{} · {deleted}", q.title),
        None => deleted,
    };
    let color = quest.map(|q| quest_color(q.color));
    let Some(target) = Deletable::from_kind(item.kind, item.id) else { return row() };
    let restore =
        parts::button(format!("archive-restore-{}", item.id), "Restore", None, p, move |s: &mut State, _: &mut Tx| {
            s.restore(target)
        })
        .h(30.0);
    item_row(format!("archive-item-{}", item.id), &title, &meta, color, restore, p)
}

fn item_row(key: String, title: &str, meta: &str, color: Option<rune::Color>, actions: El, p: &Pal) -> El {
    row()
        .key(key)
        .w_full()
        .h(60.0)
        .px(16.0)
        .gap(14.0)
        .center_y()
        .radius(10.0)
        .fill(p.card)
        .border(1.0, p.border)
        .child(parts::dot(color.unwrap_or(p.faint), 9.0))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(3.0)
                .child(text(title.to_owned()).font(ty::BODY_MEDIUM).color(p.text).single_line().truncate())
                .child(text(meta.to_owned()).font(ty::SMALL).color(p.faint).single_line().truncate()),
        )
        .child(actions)
}
