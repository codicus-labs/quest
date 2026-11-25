use super::*;

#[test]
fn blocks_and_inline_marks() {
    let parsed = parse(
        "# One\n## Two\n### Three\n\n**bold**, *italic*, `code`, [web](HTTPS://example.org), [mail](mailto:a@example.org)\n\n- bullet\n\n3. ordered\n\n> quote\n\n```rust\ncode\n```",
    );
    for (index, level) in (1..=3).enumerate() {
        assert!(matches!(&parsed[index], Block::Heading { level: actual, .. } if *actual == level));
    }
    let Block::Paragraph(content) = &parsed[3] else { panic!("expected paragraph") };
    assert!(content.iter().any(|i| i.text == "bold" && i.bold));
    assert!(content.iter().any(|i| i.text == "italic" && i.italic));
    assert!(content.iter().any(|i| i.text == "code" && i.kind == InlineKind::Code));
    assert!(content.iter().any(|i| i.kind == InlineKind::Link("HTTPS://example.org".into())));
    assert!(content.iter().any(|i| i.kind == InlineKind::Link("mailto:a@example.org".into())));
    assert!(matches!(&parsed[4], Block::List { start: None, items } if items.len() == 1));
    assert!(matches!(&parsed[5], Block::List { start: Some(3), items } if items.len() == 1));
    assert!(matches!(&parsed[6], Block::Quote(blocks) if matches!(blocks.first(), Some(Block::Paragraph(_)))));
    assert_eq!(parsed[7], Block::Code("code".into()));
}

#[test]
fn html_and_unsafe_links_are_text() {
    for (source, expected) in [
        ("<b>x</b>", "<b>x</b>"),
        ("<script>x</script>", "<script>x</script>"),
        ("[bad](javascript:alert(1))", "bad"),
        ("[relative](//example.org)", "relative"),
    ] {
        assert_eq!(
            parse(source),
            [Block::Paragraph(vec![Inline {
                text: expected.into(),
                bold: false,
                italic: false,
                kind: InlineKind::Text
            },])]
        );
    }
}

#[test]
fn deep_nesting_falls_back_to_plain_text() {
    let source = format!("{}text", "> ".repeat(64));
    assert_eq!(
        parse(&source),
        [Block::Paragraph(vec![Inline { text: source, bold: false, italic: false, kind: InlineKind::Text },])]
    );
}

#[test]
fn task_markers_use_utf8_source_offsets_and_ignore_code() {
    let source = "Élan\r\n\r\n- [ ] first\r\n- [X] next\r\n\r\n```\r\n- [ ] code\r\n```";
    let Block::List { items, .. } = &parse(source)[1] else { panic!("expected list") };
    let first = source.find("[ ]").unwrap();
    let second = source.find("[X]").unwrap();
    assert_eq!(items[0].marker, Some(TaskMarker { checked: false, offset: first }));
    assert_eq!(items[1].marker, Some(TaskMarker { checked: true, offset: second }));
    assert_eq!(toggle_task_marker(source, first), source.replacen("[ ]", "[x]", 1));
    assert_eq!(toggle_task_marker(source, second), source.replace("[X]", "[ ]"));
    assert_eq!(toggle_task_marker(source, source.rfind("[ ]").unwrap()), source);
    assert_eq!(toggle_task_marker(source, 1), source);
}
