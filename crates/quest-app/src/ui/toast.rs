//! The toast at the bottom right: "Deleted “…” · Undo ⌘Z" or "Completed quest “…” · Undo ⌘Z".
//! `UndoState` decides what shows and for how long.

use quest_domain::QuestStatus;
use rune::{Color, Edge, El, Transition, Tx, row, text};

use crate::icons;
use crate::state::State;
use crate::theme::{Pal, ty};
use crate::ui::chrome::keycap;
use crate::ui::parts;
use crate::undo::{Toast, UndoEvent};

pub fn layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    if let Some(shown) = &s.undo.shown {
        let (message, action, event) = match &shown.toast {
            Toast::Rebooked => ("Booked as Organizing".into(), "Undo", UndoEvent::Undo),
            Toast::Done => ("Done".into(), "Undo", UndoEvent::Undo),
            Toast::DateChanged => ("Date change".into(), "Undo", UndoEvent::Undo),
            Toast::QuestChanged => ("Quest change".into(), "Undo", UndoEvent::Undo),
            Toast::Deleted { title, .. } => (format!("Deleted “{title}”"), "Undo", UndoEvent::Undo),
            Toast::QuestStatus { title, status, .. } => {
                let verb = if *status == QuestStatus::Completed { "Completed" } else { "Reopened" };
                (format!("{verb} quest “{title}”"), "Undo", UndoEvent::Undo)
            }
        };
        let act = move |s: &mut State, _: &mut Tx| s.step_undo(event.clone());
        let dismiss = |s: &mut State, _: &mut Tx| s.step_undo(UndoEvent::Dismiss);
        layer.push(toast(message, action, true, act, dismiss, p));
        return;
    }
    // What the last AI capture created: "Created 1 quest and 2 tasks · Undo ⌘Z".
    if let Some(undo) = s.capture_undo.as_ref().filter(|u| u.until.is_some()) {
        let act = |s: &mut State, _: &mut Tx| s.undo_capture();
        let dismiss = |s: &mut State, _: &mut Tx| {
            if let Some(undo) = &mut s.capture_undo {
                undo.until = None;
            }
        };
        layer.push(toast(undo.message.clone(), "Undo", true, act, dismiss, p));
    }
}

fn toast(
    message: String,
    action: &str,
    cmd_z: bool,
    act: impl Fn(&mut State, &mut Tx) + 'static,
    dismiss: impl Fn(&mut State, &mut Tx) + 'static,
    p: &Pal,
) -> El {
    let mut toast = row()
        .key("toast")
        .role("status")
        .label(message.clone())
        .pin(None, None, Some(24.0), Some(24.0))
        .max_w(520.0)
        .h(52.0)
        .pl(18.0)
        .pr(8.0)
        .gap(12.0)
        .center_y()
        .radius(12.0)
        .fill(p.card_raised)
        .border(1.0, p.border_strong)
        .shadow(0.0, 24.0, Color::BLACK.alpha(0.35))
        .child(
            text(message)
                .key("toast-message")
                .font(ty::BODY)
                .color(p.text)
                .single_line()
                .truncate()
                .min_w(0.0)
                .shrink(),
        )
        .child(parts::button("toast-action", action, None, p, act).h(32.0));
    if cmd_z {
        toast = toast.child(keycap("⌘Z", p));
    }
    toast = toast.child(parts::icon_button("toast-close", "Dismiss", &icons::CLOSE, p.muted, p, dismiss));
    toast.transition(Transition::move_edge(Edge::Bottom).combined(Transition::opacity()))
}
