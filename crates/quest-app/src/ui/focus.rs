//! Focus: one task (the running one, else the one started or picked last) with its quest chip,
//! editable title, a big timer, RUNNING/PAUSED, Pause/Resume and Complete, and its editable
//! description. With no such task, a list of today's to start one.

use rune::{Align, Color, Cursor, El, Tx, col, el, icon, row, spacer, text, text_input};

use crate::format;
use crate::icons;
use crate::state::quest_picker::PickFor;
use crate::state::{FocusTarget, State, View};
use crate::theme::{Pal, quest_color, ty};
use crate::ui::chrome;
use crate::ui::workspaces;
use crate::ui::{parts, quest_picker};

/// `leading`: what the window's controls take at the left; the top row starts past them, like the
/// top bar the other views have.
pub fn view(s: &State, p: &Pal, leading: f32) -> El {
    let Some(draft) = &s.focus_draft else { return pick(s, p, leading) };
    let (task, running) = match s.focus_target() {
        FocusTarget::Running { task, .. } => (task, true),
        FocusTarget::Paused { task } => (task, false),
        FocusTarget::Nothing => return pick(s, p, leading),
    };
    let quest = parts::quest_of(s, task);
    let color = parts::accent_of(quest, p);
    let id = task.id;
    let label = quest.map_or("Focus".to_owned(), |q| format!("Focus · {}", q.title));
    let top = row()
        .w_full()
        .h(64.0)
        .pl(chrome::leading_pad(leading))
        .pr(26.0)
        .center_y()
        .gap(10.0)
        .child(back_button(p))
        .children(workspaces::timer_note(s, p))
        .child(spacer())
        .child(parts::dot(p.accent.alpha(if running { 1.0 } else { 0.35 }), 8.0))
        .child(parts::caps(label, p));
    let chip = row()
        .h(30.0)
        .px(14.0)
        .gap(8.0)
        .center_y()
        .radius(15.0)
        .border(1.0, color.alpha(0.5))
        .child(parts::dot(color, 8.0))
        .child(text(quest.map_or("No quest".to_owned(), |q| q.title.clone())).font(ty::BODY).color(p.muted));
    let chip = quest_picker::trigger(chip, PickFor::Focus(id), "Change quest", s, p);
    let title = text_input(&draft.title, ty::FOCUS_TITLE)
        .key("focus-title")
        .label("Task title")
        .single_line()
        .align(Align::Center)
        .w(640.0)
        .color(p.text)
        .on_edit(|s: &mut State| &mut s.focus_fields().title)
        .on_input_change(|s: &mut State, _: &mut Tx| s.save_focus_title())
        .on_submit(|_: &mut State, tx: &mut Tx| tx.blur());
    let notes = super::notes::field(
        "focus-description",
        &draft.description,
        p,
        |s| &mut s.focus_fields().description,
        State::save_focus_description,
    )
    .max_h(150.0);
    let description = col().w(520.0).min_h(60.0).padding(12.0).radius(10.0).border(1.0, p.border).child(notes);
    let tracked = s.snap.task_tracked_ms(id, s.now);
    let toggle = if running {
        parts::button("focus-toggle", "Pause", Some(&icons::PAUSE), p, |s: &mut State, _: &mut Tx| s.stop_focus())
    } else {
        parts::button("focus-toggle", "Resume", Some(&icons::PLAY), p, move |s: &mut State, _: &mut Tx| {
            s.start_focus(id)
        })
    };
    let complete =
        parts::button("focus-complete", "Complete", Some(&icons::CHECK), p, move |s: &mut State, tx: &mut Tx| {
            s.complete_focus(id, tx)
        });
    let center = col()
        .grow(1.0)
        .w_full()
        .center()
        .gap(0.0)
        .child(chip)
        .child(title.mt(26.0))
        .child(text(format::timer(tracked)).key("focus-timer").font(ty::TIMER).color(p.text).mt(24.0))
        .child(
            text(if running { "RUNNING" } else { "PAUSED" })
                .key("focus-state")
                .font(ty::CAPS)
                .color(if running { p.muted } else { p.accent })
                .mt(10.0),
        )
        .child(row().gap(14.0).mt(40.0).child(toggle.h(44.0).px(20.0)).child(complete.h(44.0).px(20.0)))
        .child(description.mt(36.0));
    col().key("focus").w_full().h_full().fill(p.bg).child(top).child(center).child(el().h(64.0))
}

fn back_button(p: &Pal) -> El {
    parts::icon_button("focus-back", "Back to Home", &icons::ARROW_LEFT, p.muted, p, |s: &mut State, tx: &mut Tx| {
        s.go_with(View::Home, tx)
    })
}

/// No session: choose a task to start one.
fn pick(s: &State, p: &Pal, leading: f32) -> El {
    let today = s.today();
    let mut list = col().w(460.0).gap(8.0);
    let mut any = false;
    for task in s.snap.tasks_on(today).filter(|t| !t.is_done()) {
        any = true;
        let id = task.id;
        let color = parts::quest_of(s, task).map_or(p.faint, |q| quest_color(q.color));
        list = list.child(
            row()
                .key(format!("focus-pick-{id}"))
                .role("button")
                .label(task.title.clone())
                .focusable(true)
                .w_full()
                .h(48.0)
                .px(14.0)
                .gap(12.0)
                .center_y()
                .radius(10.0)
                .fill(p.card)
                .border(1.0, p.border)
                .cursor(Cursor::Pointer)
                .hover(p.hover)
                .child(el().w(3.0).h(24.0).radius(2.0).fill(color))
                .child(text(task.title.clone()).font(ty::BODY).color(p.text).grow(1.0).single_line().truncate())
                .child(icon(&icons::PLAY, 14.0).color(p.muted))
                .on_press(move |s: &mut State| s.start_focus(id)),
        );
    }
    if !any {
        list = list.child(parts::empty_note("No open tasks today. Add one on Home (N).", p));
    }
    let top = row()
        .w_full()
        .h(64.0)
        .pl(chrome::leading_pad(leading))
        .pr(26.0)
        .gap(10.0)
        .center_y()
        .child(back_button(p))
        .children(workspaces::timer_note(s, p))
        .child(spacer())
        .child(parts::caps("Focus", p));
    col()
        .key("focus")
        .w_full()
        .h_full()
        .child(top)
        .child(
            col()
                .grow(1.0)
                .w_full()
                .center()
                .gap(18.0)
                .child(text("What are you working on?").font(ty::HEADING).color(p.text))
                .child(list),
        )
        .child(el().h(64.0).fill(Color::TRANSPARENT))
}
