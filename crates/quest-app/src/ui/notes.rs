use pulldown_cmark::{Event, HeadingLevel, Parser, Tag};
use rune::{Cursor, El, ScrollSpec, Span, TextEdit, Tx, col, text_input};
use rune_kit::Markdown;

use crate::state::State;
use crate::theme::{Pal, ty};

pub(super) fn field(
    key: &'static str,
    edit: &TextEdit,
    p: &Pal,
    bind: fn(&mut State) -> &mut TextEdit,
    save: fn(&mut State),
) -> El {
    if edit.focused || edit.text.is_empty() {
        return text_input(edit, ty::BODY)
            .key(key)
            .label("Description")
            .w_full()
            .color(p.text)
            .placeholder("Add a description…", p.faint)
            .spans(source_spans(&edit.text, p))
            .on_edit(bind)
            .on_input_change(move |s: &mut State, _: &mut Tx| save(s))
            .on_submit(move |s: &mut State, _: &mut Tx| {
                bind(s).insert("\n");
                save(s);
            });
    }
    col()
        .key(format!("{key}-preview"))
        .role("button")
        .label("Edit description")
        .w_full()
        .min_h(0.0)
        .focusable(true)
        .cursor(Cursor::Text)
        .child(Markdown::new(&edit.text).build(&p.kit))
        .scroll(ScrollSpec { stick_bottom: false })
        .on_press_tx(move |s: &mut State, tx: &mut Tx| {
            bind(s).focused = true;
            tx.focus_when_shown(key, false);
        })
}

fn source_spans(source: &str, p: &Pal) -> Vec<Span> {
    let mut spans = Vec::new();
    for (event, range) in Parser::new(source).into_offset_iter() {
        let span = Span::new(range.start, range.end);
        let styled = match event {
            Event::Start(Tag::Heading { level, .. }) => span.bold().size(match level {
                HeadingLevel::H1 => 25.0,
                HeadingLevel::H2 => 21.0,
                HeadingLevel::H3 => 18.0,
                _ => 16.0,
            }),
            Event::Start(Tag::Strong) => span.bold(),
            Event::Start(Tag::Emphasis) => span.italic(),
            Event::Code(_) | Event::Start(Tag::CodeBlock(_)) => span.mono().color(p.muted),
            Event::Start(Tag::Link { .. }) => span.underline(p.accent),
            Event::Start(Tag::BlockQuote(_)) => span.italic().color(p.muted),
            _ => continue,
        };
        spans.push(styled);
    }
    if source.ends_with('#') {
        let start = source.rfind('\n').map_or(0, |i| i + 1);
        let marker = &source[start..];
        if marker == "#" {
            spans.push(Span::new(start, source.len()).bold().size(25.0));
        }
    }
    spans
}
