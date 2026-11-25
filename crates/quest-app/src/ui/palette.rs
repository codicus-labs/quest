//! The ⌘K command palette: a field and the entries `palette::entries` finds for it.

use quest_domain::{Command, EntityId, Position};
use rune::{Color, Cursor, El, Key, KeyEvent, Tx, col, el, row, text, text_input};

use crate::clock;
use crate::palette::{self, Action, Entry};
use crate::state::{Overlay, State, View};
use crate::theme::{Pal, ty};
use crate::ui::chrome::{keycap, open_palette_in};

/// The entries for the query; "Capture with AI" only while it is on in the settings.
pub(super) fn current_entries(s: &State) -> Vec<Entry> {
    let mut entries = palette::entries_for(
        &View::SIDEBAR,
        s.palette.mode,
        &s.palette.query.text,
        &s.snap,
        clock::local(s.now),
        s.now,
    );
    if !s.ai_capture_enabled() {
        entries.retain(|e| e.action != Action::CaptureWithAi);
    }
    entries
}

pub fn layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let entries = current_entries(s);
    let picked = s.palette.picked.min(entries.len().saturating_sub(1));
    let input = text_input(&s.palette.query, ty::TASK)
        .key("palette-input")
        .label("Search or jump")
        .single_line()
        .grow(1.0)
        .color(p.text)
        .placeholder(palette::placeholder(s.palette.mode), p.faint)
        .on_edit(|s: &mut State| &mut s.palette.query)
        .on_input_change(|s: &mut State, _: &mut Tx| s.palette.picked = 0)
        .on_input_key(navigate)
        .on_submit(|s: &mut State, tx: &mut Tx| {
            let entries = current_entries(s);
            let index = s.palette.picked.min(entries.len().saturating_sub(1));
            if let Some(entry) = entries.into_iter().nth(index) {
                run(s, entry.action, tx);
            }
        });
    let mut list = col().key("palette-results").w_full().padding(6.0).gap(2.0);
    for (i, entry) in entries.iter().enumerate().take(12) {
        list = list.child(entry_row(i, entry, i == picked, p));
    }
    let panel = col()
        .key("palette")
        .role("dialog")
        .label("Command palette")
        .w(620.0)
        .radius(14.0)
        .fill(p.card_raised)
        .border(1.0, p.border_strong)
        .clip_rounded(14.0)
        .shadow(20.0, 60.0, Color::BLACK.alpha(0.45))
        .child(
            row()
                .h(58.0)
                .px(18.0)
                .gap(12.0)
                .center_y()
                .child(rune::icon(&crate::icons::SEARCH, 18.0).color(p.muted))
                .child(input)
                .child(keycap("esc", p)),
        )
        .child(el().w_full().h(1.0).fill(p.border))
        .child(list)
        .modal(true, |s: &mut State| s.overlay = Overlay::None);
    layer.push(el().key("palette-scrim").absolute(0.0, 0.0).w_full().h_full().fill(p.scrim).a11y_hidden());
    layer.push(col().key("palette-layer").absolute(0.0, 0.0).w_full().h_full().items_center().pt(110.0).child(panel));
}

fn entry_row(i: usize, entry: &Entry, picked: bool, p: &Pal) -> El {
    let action = entry.action.clone();
    row()
        .key(format!("palette-entry-{i}"))
        .role("option")
        .label(entry.title.clone())
        .w_full()
        .h(40.0)
        .px(12.0)
        .gap(12.0)
        .center_y()
        .radius(8.0)
        .fill(if picked { p.selected } else { Color::TRANSPARENT })
        .hover(p.hover)
        .cursor(Cursor::Pointer)
        .child(text(entry.title.clone()).font(ty::BODY).color(p.text).grow(1.0).single_line().truncate())
        .child(text(entry.hint.clone()).font(ty::SMALL).color(p.faint))
        .on_press_tx(move |s: &mut State, tx: &mut Tx| run(s, action.clone(), tx))
}

fn navigate(s: &mut State, ev: &KeyEvent, _: &mut Tx) -> bool {
    let count = current_entries(s).len().min(12);
    match ev.key {
        Key::Down if count > 0 => s.palette.picked = (s.palette.picked + 1) % count,
        Key::Up if count > 0 => s.palette.picked = (s.palette.picked + count - 1) % count,
        Key::Escape => s.overlay = Overlay::None,
        _ => return false,
    }
    true
}

/// Carries out a palette entry and closes the palette.
pub fn run(s: &mut State, action: Action, tx: &mut Tx) {
    s.overlay = Overlay::None;
    match action {
        Action::Go(view) => s.go_with(view, tx),
        Action::CreateTask(title) => {
            let planned_on = Some(s.today());
            s.dispatch(Command::CreateTask {
                id: EntityId::new(),
                title,
                quest_id: None,
                planned_on,
                due_on: None,
                position: Position::Top,
            });
            s.go_with(View::Today, tx);
        }
        Action::CreateQuest(title) => {
            let color = quest_domain::QuestColor::ALL[s.snap.quests.len() % quest_domain::QuestColor::ALL.len()];
            s.dispatch(Command::CreateQuest { id: EntityId::new(), title, color });
            s.go_with(View::Quests, tx);
        }
        Action::Focus(task) => {
            s.start_focus(task);
            s.go_with(View::Focus, tx);
        }
        Action::ShowQuest(quest) => s.go(View::Quest(quest)),
        Action::ToggleTheme => s.dark = !s.dark,
        Action::Account => s.overlay = Overlay::Account,
        Action::Choose(mode) => open_palette_in(s, mode, tx),
        Action::MoveTask { task, planned_on } => {
            let at = s.clock.now();
            s.dispatch(Command::MoveTask { id: task, planned_on, position: Position::Bottom, at });
        }
        Action::AssignQuest { task, quest } => s.dispatch(Command::AssignTask { id: task, quest_id: quest }),
        Action::Delete(item) => s.delete(item),
        Action::LogWork => {
            s.open_log_work_now();
            tx.focus_when_shown("log-work-task", false);
        }
        Action::CaptureWithAi => {
            s.open_capture();
            tx.focus_when_shown("capture-text", false);
        }
        Action::FilterQuest(quest) => s.quest_filter = quest,
        Action::SwitchWorkspace(workspace) => s.switch_workspace(workspace),
        Action::ManageWorkspaces => {
            s.open_workspaces();
            tx.focus_when_shown("workspace-new", false);
        }
    }
}
