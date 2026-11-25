use std::rc::Rc;

use quest_domain::{EntityId, Task};
use quest_view::markdown::{self, Block, Inline, InlineKind};
use rune::{Cursor, El, Key, Link, Mods, Span, TextStyle, Transition, Tx, col, row, text, text_input};

use crate::state::State;
use crate::state::notes::NoteHost;
use crate::theme::{Pal, ty};

pub(super) fn field(s: &State, task: &Task, host: NoteHost, p: &Pal) -> El {
    let id = task.id;
    if let Some(note) = s.note.as_ref().filter(|note| note.host == host && note.task == id) {
        return text_input(&note.edit, ty::BODY)
            .key(host.source())
            .label("Notes (Markdown)")
            .w_full()
            .min_h(96.0)
            .padding(8.0)
            .radius(8.0)
            .fill(p.card)
            .border(1.0, p.accent)
            .color(p.text)
            .placeholder("Notes · Markdown", p.faint)
            .on_edit(State::note_source)
            .on_input_change(State::note_changed)
            .on_input_key(|s: &mut State, event, tx| {
                if event.key != Key::Escape {
                    return false;
                }
                s.close_note(tx);
                true
            })
            .on_submit(|s: &mut State, tx: &mut Tx| {
                s.note_source().insert("\n");
                s.note_changed(tx);
            })
            .transition(Transition::none());
    }

    let source: Rc<str> = Rc::from(task.description.as_str());
    let blocks = markdown::parse(&source);
    let preview = col()
        .key(host.preview())
        .role("button")
        .label("Notes · Edit (E)")
        .focusable(true)
        .w_full()
        .gap(8.0)
        .padding(8.0)
        .radius(8.0)
        .hover(p.hover)
        .cursor(Cursor::Text)
        .transition(Transition::none())
        .on_press_tx(move |s: &mut State, tx: &mut Tx| {
            s.open_note(host, id, tx);
        })
        .on_key(move |s: &mut State, event, tx| {
            let edit = event.key == Key::Enter || matches!(&event.key, Key::Text(t) if t.eq_ignore_ascii_case("e"));
            tx.focused() == Some(host.preview()) && event.mods == Mods::default() && edit && s.open_note(host, id, tx)
        });
    if blocks.is_empty() {
        return preview.child(text("Notes").font(ty::BODY).color(p.faint));
    }
    preview.children(
        blocks
            .into_iter()
            .enumerate()
            .map(|(index, block)| render_block(block, id, &source, p).key(format!("note-block-{index}"))),
    )
}

fn render_block(block: Block, id: EntityId, source: &Rc<str>, p: &Pal) -> El {
    match block {
        Block::Paragraph(content) => inline(content, ty::BODY, false, p),
        Block::Heading { level, content } => {
            let font = TextStyle::new(17.0 - f32::from(level), 650.0).lh(20.0);
            inline(content, font, false, p).role("heading")
        }
        Block::Quote(blocks) => col()
            .w_full()
            .gap(4.0)
            .pl(10.0)
            .border_sides([0.0, 0.0, 0.0, 2.0], p.border_strong)
            .children(blocks.into_iter().map(|block| render_block(block, id, source, p))),
        Block::Code(code) => {
            let end = code.len();
            text(code)
                .font(ty::SMALL)
                .color(p.muted)
                .spans(vec![Span::new(0, end).mono()])
                .w_full()
                .wrap_anywhere()
                .padding(8.0)
                .radius(6.0)
                .fill(p.bg)
                .border(1.0, p.border)
        }
        Block::List { start, items } => {
            col().w_full().gap(2.0).children(items.into_iter().enumerate().map(|(i, item)| {
                let marker = if let Some(marker) = item.marker {
                    let source = Rc::clone(source);
                    super::checklist::checkbox(
                        format!("note-check-{}", marker.offset),
                        if marker.checked { "Uncheck note item".into() } else { "Check note item".into() },
                        marker.checked,
                        p,
                        move |s| s.toggle_note_marker(id, &source, marker.offset),
                    )
                    .mt(2.0)
                } else {
                    let marker =
                        start.map_or_else(|| "•".into(), |start| format!("{}.", start.saturating_add(i as u64)));
                    text(marker).font(ty::BODY).color(p.muted).min_w(16.0)
                };
                let body =
                    col().grow(1.0).min_w(0.0).gap(4.0).children(item.blocks.into_iter().map(|block| match block {
                        Block::Paragraph(content) if item.marker.is_some_and(|marker| marker.checked) => {
                            inline(content, ty::BODY, true, p)
                        }
                        block => render_block(block, id, source, p),
                    }));
                row().w_full().gap(6.0).child(marker).child(body)
            }))
        }
    }
}

fn inline(content: Vec<Inline>, font: TextStyle, done: bool, p: &Pal) -> El {
    let mut value = String::new();
    let mut spans = Vec::new();
    let mut links = Vec::new();
    for span in content {
        let start = value.len();
        value.push_str(&span.text);
        let end = value.len();
        let mut style = Span::new(start, end);
        if span.bold {
            style = style.bold();
        }
        if span.italic {
            style = style.italic();
        }
        match span.kind {
            InlineKind::Text => {}
            InlineKind::Code => style = style.mono().color(p.muted),
            InlineKind::Link(url) => {
                style = style.color(p.accent).underline(p.accent);
                links.push(Link::new(start..end, url));
            }
        }
        spans.push(style);
    }
    let end = value.len();
    let mut line =
        text(value).font(font).color(if done { p.faint } else { p.text }).w_full().wrap_anywhere().spans(spans);
    if done {
        line = line.strike(0, end, None);
    }
    if links.is_empty() {
        return line;
    }
    links.into_iter().fold(line, El::link).on_link(|s: &mut State, url, _| {
        if markdown::safe_link(url)
            && let Ok(url) = quest_store::Url::parse(url)
        {
            let browser = s.auth.browser.clone();
            std::thread::spawn(move || {
                if let Err(error) = browser.open(&url) {
                    eprintln!("{error}");
                }
            });
        }
    })
}
