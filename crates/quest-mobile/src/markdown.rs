use quest_view::markdown::{self, Block, Inline, InlineKind};

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum MarkdownBlock {
    Heading { level: u8, content: Vec<MarkdownInline> },
    Paragraph { content: Vec<MarkdownInline> },
    ItemList { start: Option<u64>, items: Vec<MarkdownListItem> },
    Quote { blocks: Vec<MarkdownBlock> },
    Code { text: String },
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MarkdownListItem {
    pub marker: Option<MarkdownTaskMarker>,
    pub blocks: Vec<MarkdownBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MarkdownTaskMarker {
    pub checked: bool,
    /// UTF-8 byte offset, not a Kotlin string index.
    pub offset: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MarkdownInline {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub kind: MarkdownInlineKind,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum MarkdownInlineKind {
    Text,
    Code,
    Link { url: String },
}

#[uniffi::export]
pub fn parse_markdown(text: String) -> Vec<MarkdownBlock> {
    convert_blocks(markdown::parse(&text))
}

#[uniffi::export]
pub fn toggle_task_marker(text: String, offset: u64) -> String {
    match usize::try_from(offset) {
        Ok(offset) => markdown::toggle_task_marker(&text, offset),
        Err(_) => text,
    }
}

fn convert_blocks(blocks: Vec<Block>) -> Vec<MarkdownBlock> {
    blocks
        .into_iter()
        .map(|block| match block {
            Block::Heading { level, content } => MarkdownBlock::Heading { level, content: convert_inlines(content) },
            Block::Paragraph(content) => MarkdownBlock::Paragraph { content: convert_inlines(content) },
            Block::List { start, items } => MarkdownBlock::ItemList {
                start,
                items: items
                    .into_iter()
                    .map(|item| MarkdownListItem {
                        marker: item
                            .marker
                            .map(|marker| MarkdownTaskMarker { checked: marker.checked, offset: marker.offset as u64 }),
                        blocks: convert_blocks(item.blocks),
                    })
                    .collect(),
            },
            Block::Quote(blocks) => MarkdownBlock::Quote { blocks: convert_blocks(blocks) },
            Block::Code(text) => MarkdownBlock::Code { text },
        })
        .collect()
}

fn convert_inlines(inlines: Vec<Inline>) -> Vec<MarkdownInline> {
    inlines
        .into_iter()
        .map(|inline| MarkdownInline {
            text: inline.text,
            bold: inline.bold,
            italic: inline.italic,
            kind: match inline.kind {
                InlineKind::Text => MarkdownInlineKind::Text,
                InlineKind::Code => MarkdownInlineKind::Code,
                InlineKind::Link(url) => MarkdownInlineKind::Link { url },
            },
        })
        .collect()
}
