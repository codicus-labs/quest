use std::{iter::Peekable, ops::Range};

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading { level: u8, content: Vec<Inline> },
    Paragraph(Vec<Inline>),
    List { start: Option<u64>, items: Vec<ListItem> },
    Quote(Vec<Block>),
    Code(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
    pub marker: Option<TaskMarker>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskMarker {
    pub checked: bool,
    /// UTF-8 byte offset of the opening bracket in the source.
    pub offset: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inline {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub kind: InlineKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum InlineKind {
    #[default]
    Text,
    Code,
    Link(String),
}

pub fn parse(text: &str) -> Vec<Block> {
    let mut too_deep = false;
    let mut depth = 0;
    let parsed = blocks(
        &mut events(text)
            .map_while(|entry| {
                match entry.0 {
                    Event::Start(_) => depth += 1,
                    Event::End(_) => depth -= 1,
                    _ => {}
                }
                too_deep = depth > 32;
                (!too_deep).then_some(entry)
            })
            .fuse()
            .peekable(),
        None,
    )
    .0;
    if too_deep {
        return vec![Block::Paragraph(vec![Inline {
            text: text.to_owned(),
            bold: false,
            italic: false,
            kind: InlineKind::Text,
        }])];
    }
    parsed
}

pub fn safe_link(url: &str) -> bool {
    ["https://", "http://", "mailto:"]
        .iter()
        .any(|prefix| url.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix)))
}

pub fn toggle_task_marker(text: &str, offset: usize) -> String {
    let mut toggled = text.to_owned();
    if let Some((Event::TaskListMarker(checked), range)) =
        events(text).find(|(event, range)| range.start == offset && matches!(event, Event::TaskListMarker(_)))
    {
        toggled.replace_range(range.start + 1..range.start + 2, if checked { " " } else { "x" });
    }
    toggled
}

fn events(text: &str) -> impl Iterator<Item = (Event<'_>, Range<usize>)> {
    Parser::new_ext(text, Options::ENABLE_TASKLISTS).into_offset_iter()
}

type SourceEvent<'a> = (Event<'a>, Range<usize>);

fn blocks<'a>(
    events: &mut Peekable<impl Iterator<Item = SourceEvent<'a>>>,
    end: Option<TagEnd>,
) -> (Vec<Block>, Option<TaskMarker>) {
    let mut blocks = Vec::new();
    let mut inline = InlineBuilder::default();
    let mut marker = None;
    while let Some((event, range)) = events.next() {
        if matches!(event, Event::End(tag) if Some(tag) == end) {
            break;
        }
        let block = match event {
            Event::Start(Tag::Paragraph | Tag::HtmlBlock) => {
                let end =
                    if matches!(event, Event::Start(Tag::HtmlBlock)) { TagEnd::HtmlBlock } else { TagEnd::Paragraph };
                let (content, task) = inlines(events, end);
                marker = marker.or(task);
                Block::Paragraph(content)
            }
            Event::Start(Tag::Heading { level, .. }) => {
                let (content, _) = inlines(events, TagEnd::Heading(level));
                if level as u8 <= 3 {
                    Block::Heading { level: level as u8, content }
                } else {
                    Block::Paragraph(content)
                }
            }
            Event::Start(Tag::BlockQuote(kind)) => Block::Quote(self::blocks(events, Some(TagEnd::BlockQuote(kind))).0),
            Event::Start(Tag::CodeBlock(_)) => {
                let (content, _) = inlines(events, TagEnd::CodeBlock);
                Block::Code(
                    content.into_iter().map(|span| span.text).collect::<String>().trim_end_matches('\n').to_owned(),
                )
            }
            Event::Start(Tag::List(start)) => {
                let mut items = Vec::new();
                while let Some((Event::Start(Tag::Item), _)) = events.peek() {
                    events.next();
                    let (blocks, marker) = self::blocks(events, Some(TagEnd::Item));
                    items.push(ListItem { marker, blocks });
                }
                events.next_if(|(event, _)| *event == Event::End(TagEnd::List(start.is_some())));
                Block::List { start, items }
            }
            Event::TaskListMarker(checked) => {
                marker = Some(TaskMarker { checked, offset: range.start });
                continue;
            }
            event => {
                inline.push(event);
                continue;
            }
        };
        inline.flush_into(&mut blocks);
        blocks.push(block);
    }
    inline.flush_into(&mut blocks);
    (blocks, marker)
}

fn inlines<'a>(events: &mut impl Iterator<Item = SourceEvent<'a>>, end: TagEnd) -> (Vec<Inline>, Option<TaskMarker>) {
    let mut inline = InlineBuilder::default();
    let mut marker = None;
    for (event, range) in events {
        match event {
            Event::End(tag) if tag == end => break,
            Event::TaskListMarker(checked) => marker = Some(TaskMarker { checked, offset: range.start }),
            event => inline.push(event),
        }
    }
    (inline.content, marker)
}

#[derive(Default)]
struct InlineBuilder {
    content: Vec<Inline>,
    bold: usize,
    italic: usize,
    link: Option<String>,
}

impl InlineBuilder {
    fn push(&mut self, event: Event<'_>) {
        match event {
            Event::Start(Tag::Strong) => self.bold += 1,
            Event::End(TagEnd::Strong) => self.bold -= 1,
            Event::Start(Tag::Emphasis) => self.italic += 1,
            Event::End(TagEnd::Emphasis) => self.italic -= 1,
            Event::Start(Tag::Link { dest_url, .. }) => {
                self.link = safe_link(&dest_url).then(|| dest_url.into_string());
            }
            Event::End(TagEnd::Link) => self.link = None,
            Event::Code(code) => self.span(&code, InlineKind::Code),
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => self.span(&text, InlineKind::Text),
            Event::SoftBreak | Event::HardBreak => self.span("\n", InlineKind::Text),
            Event::Rule => self.span("—", InlineKind::Text),
            _ => {}
        }
    }

    fn span(&mut self, text: &str, kind: InlineKind) {
        let kind = self.link.as_ref().map_or(kind, |url| InlineKind::Link(url.clone()));
        let (bold, italic) = (self.bold > 0, self.italic > 0);
        if let Some(last) = self.content.last_mut()
            && last.kind == kind
            && last.bold == bold
            && last.italic == italic
        {
            last.text.push_str(text);
        } else {
            self.content.push(Inline { text: text.to_owned(), bold, italic, kind });
        }
    }

    fn flush_into(&mut self, blocks: &mut Vec<Block>) {
        if !self.content.is_empty() {
            blocks.push(Block::Paragraph(std::mem::take(&mut self.content)));
        }
    }
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;
