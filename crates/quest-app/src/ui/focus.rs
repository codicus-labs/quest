use chrono::Local;
use quest_view::tracked;
use rune::{Align, Color, Cursor, El, Tx, col, el, icon, row, spacer, text, text_input};

use crate::format;
use crate::icons;
use crate::state::quest_picker::PickFor;
use crate::state::{FocusTarget, State};
use crate::theme::{Pal, quest_color, ty};
use crate::ui::chrome;
use crate::ui::workspaces;
use crate::ui::{parts, quest_picker};

/// `leading`: what the window's controls take at the left; the top row starts past them, like the
/// top bar the other views have.
pub fn view(s: &State, p: &Pal, leading: f32) -> El {
    let (FocusTarget::Running { task }, Some(entry), Some(draft)) =
        (s.focus_target(), s.snap.running_entry(), &s.focus_draft)
    else {
        return pick(s, p, leading);
    };
    let quest = parts::quest_of(s, task);
    let color = parts::accent_of(quest, p);
    let id = task.id;
    let top = row()
        .w_full()
        .h(60.0)
        .pl(chrome::leading_pad(leading))
        .pr(26.0)
        .center_y()
        .gap(10.0)
        .child(parts::caps("Focus", p).key("focus-heading"))
        .child(spacer())
        .child(
            parts::icon_button("focus-close", "Leave focus", &icons::CLOSE, p.muted, p, State::leave_focus)
                .help("Back to lists  Esc"),
        );
    let chip = row()
        .gap(8.0)
        .center_y()
        .child(parts::dot(color, 7.0))
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
    let notes = super::notes::field(s, task, crate::state::notes::NoteHost::Focus, p);
    let description = col()
        .w(520.0)
        .gap(8.0)
        .padding(12.0)
        .radius(10.0)
        .border(1.0, p.border)
        .child(notes)
        .child(super::checklist::editor(s, task, &draft.checklist, crate::state::notes::NoteHost::Focus, p));
    let today = tracked::task_today_ms(&s.snap, id, s.today(), &Local, s.now);
    let mut summary = format!("{} today", quest_view::entry_editor::duration_label(today));
    if let Some(quest) = quest {
        let week = tracked::week_balance(&s.snap, s.today(), &Local, s.now);
        summary.push_str(&format!(" · {} this week", format::hours(week.quest_ms_of(quest.id))));
    }
    let stop = parts::button("focus-toggle", "Stop", Some(&icons::STOP), p, |s: &mut State, _| s.stop_focus())
        .help("Stop  Space");
    let done = row()
        .key("focus-complete")
        .role("button")
        .label("Done")
        .focusable(true)
        .h(36.0)
        .px(16.0)
        .gap(8.0)
        .center_y()
        .radius(7.0)
        .cursor(Cursor::Pointer)
        .fill(p.accent_fill)
        .child(icon(&icons::CHECK, 15.0).color(crate::theme::on_accent_fill()))
        .child(text("Done").font(ty::BODY_MEDIUM).color(crate::theme::on_accent_fill()))
        .help("Done  X")
        .on_press_tx(move |s: &mut State, tx: &mut Tx| s.complete_focus(id, tx));
    let organize = parts::button("focus-organize", "Organize", None, p, |s: &mut State, tx| {
        s.organize(tx);
    })
    .help("Organize  O");
    let center = col()
        .w_full()
        .center()
        .child(chip)
        .child(title.mt(14.0))
        .child(text(format::timer(entry.tracked_ms(s.now))).key("focus-timer").font(ty::TIMER).color(p.text).mt(12.0))
        .child(text(summary).key("focus-summary").font(ty::SMALL).color(p.muted).mt(4.0))
        .child(description.mt(24.0))
        .child(row().gap(10.0).mt(20.0).child(stop).child(done))
        .child(row().gap(10.0).mt(10.0).child(back_button(p)).child(organize));
    col().key("focus").w_full().h_full().fill(p.bg).child(top).child(
        col()
            .key("focus-scroll")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(rune::ScrollSpec { stick_bottom: false })
            .child(center.grow(0.0).pt(24.0).pb(64.0)),
    )
}

fn back_button(p: &Pal) -> El {
    parts::button("focus-back", "Back to lists", Some(&icons::ARROW_LEFT), p, State::leave_focus)
        .help("The timer keeps running  Esc")
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
