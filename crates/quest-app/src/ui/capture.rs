//! The AI capture panel: a text to structure, and the proposal grouped as New quests, Tasks and
//! Time entries, each with a checkbox and an editable title. Nothing is created before "Create (n)".

use quest_capture::review;
use quest_domain::Frequency;
use quest_protocol::{ProposedRepeat, QuestRef, TempKey, WorkTarget};
use rune::{Color, El, Key, KeyEvent, ScrollSpec, TextEdit, Tx, col, row, spacer, text, text_input};
use rune_kit::{Button, ButtonVariant, Checkbox, Dialog};

use super::quest_picker;
use crate::state::capture_draft::Review;
use crate::state::quest_picker::PickFor;
use crate::state::{CaptureStep, State};
use crate::theme::{Pal, quest_color, ty};
use crate::ui::parts;

pub fn layer(s: &State, p: &Pal, layer: &mut Vec<El>) {
    let (description, body) = match &s.capture.step {
        CaptureStep::Compose { error } => (
            "Type or dictate tasks, quests and work done. AI turns it into a proposal.",
            compose(s, error.as_deref(), p),
        ),
        CaptureStep::Structuring => ("Structuring your text…", structuring(p)),
        CaptureStep::Review(r) => ("Check the proposal. Only what is selected is created.", review_body(s, r, p)),
    };
    Dialog::new("capture", "Capture with AI")
        .description(description)
        .open(true)
        .fit()
        .width(620.0)
        .on_close(|s: &mut State| s.capture_back())
        .child(body)
        .build(&p.kit, layer);
}

fn compose(s: &State, error: Option<&str>, p: &Pal) -> El {
    let field = text_input(&s.capture.text, ty::TASK)
        .key("capture-text")
        .label("What to capture")
        .w_full()
        .min_h(140.0)
        .color(p.text)
        .placeholder("e.g. Tomorrow write the offer for Müller, for the website relaunch. Today 9 to 11 worked on the relaunch concept.", p.faint)
        .on_edit(|s: &mut State| &mut s.capture.text)
        .on_submit(|s: &mut State, _: &mut Tx| s.capture.text.insert("\n"))
        .on_input_key(structure_on_command_return);
    let mut body = col()
        .w_full()
        .gap(10.0)
        .px(20.0)
        .pb(20.0)
        .child(col().w_full().padding(14.0).radius(8.0).fill(p.bg).border(1.0, p.border_strong).child(field));
    if let Some(error) = error {
        body = body
            .child(text(error.to_owned()).key("capture-error").role("alert").font(ty::SMALL).color(p.accent).wrap());
    }
    let k = &p.kit;
    body.child(
        row()
            .w_full()
            .gap(10.0)
            .center_y()
            .child(text("Enter adds a line · ⌘↵ structures").font(ty::SMALL).color(p.faint))
            .child(spacer())
            .child(
                Button::new("Cancel")
                    .variant(ButtonVariant::Ghost)
                    .on_press(|s: &mut State| s.close_capture())
                    .build(k)
                    .key("capture-cancel"),
            )
            .child(
                Button::new("Structure")
                    .variant(ButtonVariant::Contrast)
                    .on_press(|s: &mut State| s.structure_capture())
                    .build(k)
                    .key("capture-structure"),
            ),
    )
}

fn structure_on_command_return(s: &mut State, ev: &KeyEvent, _: &mut Tx) -> bool {
    if ev.key != Key::Enter || !ev.mods.cmd {
        return false;
    }
    s.structure_capture();
    true
}

fn structuring(p: &Pal) -> El {
    let k = &p.kit;
    col()
        .key("capture-structuring")
        .role("status")
        .label("Structuring")
        .w_full()
        .gap(16.0)
        .px(20.0)
        .pb(20.0)
        .child(text("Asking the AI… this can take up to a minute.").font(ty::BODY).color(p.muted))
        .child(
            row().w_full().child(spacer()).child(
                Button::new("Stop")
                    .variant(ButtonVariant::Outline)
                    .on_press(|s: &mut State| s.capture_back())
                    .build(k)
                    .key("capture-stop"),
            ),
        )
}

fn review_body(s: &State, r: &Review, p: &Pal) -> El {
    let draft = &r.draft;
    let today = s.today();
    let mut list = col().key("capture-review").w_full().gap(6.0);
    if r.is_empty() {
        list = list.child(
            text("The AI found nothing to create. Edit the text and try again.")
                .key("capture-empty")
                .font(ty::BODY)
                .color(p.muted)
                .wrap(),
        );
    }
    if !draft.proposal.quests.is_empty() {
        list = list.child(parts::caps("New quests", p).mt(6.0));
        for quest in &draft.proposal.quests {
            let empty = quest.title.trim().is_empty();
            let error = (draft.is_selected(&quest.key) && empty).then_some("Type a title.");
            list = list.child(item(r, &quest.key, Body::Title, String::new(), None, error, p));
        }
    }
    if !draft.proposal.tasks.is_empty() {
        list = list.child(parts::caps("Tasks", p).mt(6.0));
        for task in &draft.proposal.tasks {
            let mut meta = vec![review::task_when(task.planned_on, task.due_on, today)];
            meta.extend(task.repeat.as_ref().map(repeat_label));
            let quest = task_quest(s, r, &task.key, draft.effective_quest(&task.quest), task.quest_uncertain, p);
            let error = draft.is_selected(&task.key).then(|| draft.task_error(task, &s.snap)).flatten();
            list = list.child(item(r, &task.key, Body::Title, join(meta), Some(quest), error, p));
        }
    }
    if !draft.proposal.work.is_empty() {
        list = list.child(parts::caps("Time entries", p).mt(6.0));
        for work in &draft.proposal.work {
            let mut meta = vec![review::work_when(work, today), review::work_duration(work)];
            let body = match &work.task {
                WorkTarget::Existing { .. } => {
                    Body::Existing(review::work_task_title(&work.task, &s.snap).to_owned(), work.uncertain)
                }
                WorkTarget::New { quest, .. } => {
                    if let Some(title) = draft.effective_quest(quest).and_then(|q| quest_title(s, r, q)) {
                        meta.push(title);
                    }
                    meta.push("new task, done".into());
                    Body::Title
                }
            };
            let error = draft.is_selected(&work.key).then(|| draft.work_error(work, &s.snap)).flatten();
            list = list.child(item(r, &work.key, body, join(meta), None, error, p));
        }
    }
    let mut body =
        col().w_full().gap(10.0).px(20.0).pb(20.0).child(list.max_h(420.0).scroll(ScrollSpec { stick_bottom: false }));
    for (i, dropped) in draft.proposal.dropped.iter().enumerate() {
        body = body.child(
            text(review::dropped_message(dropped))
                .key(format!("capture-dropped-{i}"))
                .font(ty::SMALL)
                .color(p.muted)
                .wrap(),
        );
    }
    if let Some(error) = &r.error {
        body =
            body.child(text(error.clone()).key("capture-error").role("alert").font(ty::SMALL).color(p.accent).wrap());
    }
    let k = &p.kit;
    body.child(
        row()
            .w_full()
            .gap(10.0)
            .center_y()
            .child(
                Button::new("Edit text")
                    .variant(ButtonVariant::Ghost)
                    .on_press_tx(|s: &mut State, tx: &mut Tx| {
                        s.edit_capture_text();
                        tx.focus_when_shown("capture-text", false);
                    })
                    .build(k)
                    .key("capture-edit-text"),
            )
            .child(spacer())
            .child(
                Button::new("Cancel")
                    .variant(ButtonVariant::Ghost)
                    .on_press(|s: &mut State| s.discard_capture())
                    .build(k)
                    .key("capture-cancel"),
            )
            .child(
                Button::new(review::apply_label(draft.selected_count()))
                    .variant(ButtonVariant::Contrast)
                    .disabled(!draft.can_apply(&s.snap))
                    .on_press(|s: &mut State| s.create_from_capture())
                    .build(k)
                    .key("capture-create"),
            ),
    )
}

/// What an item shows after its checkbox: its editable title, or the existing task a session is
/// logged on (marked when guessed).
enum Body {
    Title,
    Existing(String, bool),
}

fn join(parts: Vec<String>) -> String {
    parts.into_iter().filter(|p| !p.trim().is_empty()).collect::<Vec<_>>().join(" · ")
}

/// A proposed item: checkbox, title (dimmed when deselected), its details, a trailing chip and
/// what keeps it from being created.
fn item(r: &Review, key: &TempKey, body: Body, meta: String, trailing: Option<El>, error: Option<&str>, p: &Pal) -> El {
    let id = format!("capture-item-{}", key.0);
    let selected = r.draft.is_selected(key);
    let toggled = key.clone();
    let check = Checkbox::new(selected)
        .label("Create")
        .hide_label()
        .on_toggle(move |s: &mut State| {
            if let Some(review) = s.capture.review_mut() {
                review.toggle(&toggled);
            }
        })
        .build(&p.kit)
        .key(format!("{id}-check"));
    let ink = if selected { p.text } else { p.faint };
    let head: El = match body {
        Body::Title => match r.titles.get(key) {
            Some(edit) => title_field(&id, edit, key, ink, p),
            None => col(),
        },
        Body::Existing(title, uncertain) => {
            row().gap(6.0).center_y().min_w(0.0).child(text("Log work on").font(ty::BODY).color(p.muted)).child(
                text(if uncertain { format!("{title}?") } else { title })
                    .key(format!("{id}-task"))
                    .font(ty::BODY_MEDIUM)
                    .color(if uncertain { p.accent } else { ink })
                    .single_line()
                    .truncate()
                    .min_w(0.0)
                    .shrink(),
            )
        }
    };
    let mut line = row().w_full().gap(10.0).center_y().child(check).child(head.grow(1.0).min_w(0.0));
    if let Some(trailing) = trailing {
        line = line.child(trailing);
    }
    let mut card = col()
        .key(id.clone())
        .w_full()
        .gap(2.0)
        .px(12.0)
        .py(8.0)
        .radius(8.0)
        .fill(if selected { p.card } else { Color::TRANSPARENT })
        .border(1.0, p.border)
        .child(line);
    if !meta.is_empty() {
        card = card.child(
            text(meta).key(format!("{id}-meta")).font(ty::SMALL).color(p.faint).single_line().truncate().pl(34.0),
        );
    }
    if let Some(error) = error {
        card = card.child(text(error).key(format!("{id}-error")).font(ty::SMALL).color(p.accent).pl(34.0));
    }
    card
}

fn title_field(id: &str, edit: &TextEdit, key: &TempKey, ink: Color, p: &Pal) -> El {
    let bound = key.clone();
    text_input(edit, ty::BODY)
        .key(format!("{id}-title"))
        .label("Title")
        .single_line()
        .min_w(0.0)
        .color(ink)
        .placeholder("Title", p.faint)
        .on_edit(move |s: &mut State| s.capture.item_title(&bound))
        .on_input_change(|s: &mut State, _: &mut Tx| {
            if let Some(review) = s.capture.review_mut() {
                review.sync_titles();
            }
        })
}

fn quest_title(s: &State, r: &Review, quest: &QuestRef) -> Option<String> {
    match quest {
        QuestRef::Existing { id } => s.snap.quest(*id).map(|q| q.title.clone()),
        QuestRef::New { key } => r.titles.get(key).map(|t| t.text.clone()),
    }
}

/// The task's quest as a chip that opens the quest picker; a guessed quest is marked.
fn task_quest(s: &State, r: &Review, task: &TempKey, quest: Option<&QuestRef>, uncertain: bool, p: &Pal) -> El {
    let title = quest.and_then(|q| quest_title(s, r, q));
    let color = match quest {
        Some(QuestRef::Existing { id }) => s.snap.quest(*id).map(|q| quest_color(q.color)),
        Some(QuestRef::New { key }) => {
            r.draft.new_quest_colors(&s.snap).into_iter().find(|(k, _)| k == key).map(|(_, c)| quest_color(c))
        }
        None => None,
    };
    let label = title.unwrap_or_else(|| "No quest".to_owned());
    let tone = if uncertain { p.accent } else { p.muted };
    let mut chip = row()
        .h(28.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .radius(14.0)
        .border(1.0, if uncertain { p.accent } else { p.border_strong })
        .child(parts::dot(color.unwrap_or(Color::TRANSPARENT), 7.0))
        .child(text(label.clone()).font(ty::SMALL).color(tone).single_line().truncate().max_w(180.0));
    if uncertain {
        chip = chip.child(text("guess").font(ty::SMALL_MEDIUM).color(p.accent));
    }
    let name = if uncertain { format!("Quest: {label} (a guess, check it)") } else { format!("Quest: {label}") };
    quest_picker::trigger(chip, PickFor::CaptureTask(task.clone()), name, s, p)
}

/// "Every week", "Every 2 months".
fn repeat_label(repeat: &ProposedRepeat) -> String {
    let unit = match repeat.frequency {
        Frequency::Day => "day",
        Frequency::Week => "week",
        Frequency::Month => "month",
        Frequency::Year => "year",
    };
    if repeat.every <= 1 { format!("Every {unit}") } else { format!("Every {} {unit}s", repeat.every) }
}
