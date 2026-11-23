//! Markdown → block tree, modelled on lmdv's `lmdv-md` surface.
//!
//! The renderer needs more structure than a flat event stream gives it, so the
//! parser folds pulldown-cmark events into [`Block`]s: headings with outline
//! indexes, lists (task lists included), tables with alignments, fenced code,
//! GitHub alert callouts, display math, images and YAML front matter.

use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, MetadataBlockKind, Options, Parser, Tag,
    TagEnd,
};

/// One run of inline text and the styles it carries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub math: bool,
    pub link: Option<String>,
}

impl TSpan {
    fn plain(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Self::default() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalloutKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl CalloutKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Note => "NOTE",
            Self::Tip => "TIP",
            Self::Important => "IMPORTANT",
            Self::Warning => "WARNING",
            Self::Caution => "CAUTION",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Self::Note => "ℹ",
            Self::Tip => "✦",
            Self::Important => "◆",
            Self::Warning => "▲",
            Self::Caution => "⛔",
        }
    }

    fn from_gfm(kind: BlockQuoteKind) -> Self {
        match kind {
            BlockQuoteKind::Note => Self::Note,
            BlockQuoteKind::Tip => Self::Tip,
            BlockQuoteKind::Important => Self::Important,
            BlockQuoteKind::Warning => Self::Warning,
            BlockQuoteKind::Caution => Self::Caution,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ListItem {
    pub depth: usize,
    /// `1.`, `2.` or a bullet.
    pub marker: String,
    pub checked: Option<bool>,
    pub source_line: Option<usize>,
    pub spans: Vec<TSpan>,
}

#[derive(Clone, Debug)]
pub enum Block {
    Heading { level: u8, spans: Vec<TSpan>, index: usize },
    Para(Vec<TSpan>),
    Quote(Vec<Block>),
    Callout { kind: CalloutKind, blocks: Vec<Block> },
    Code { lang: String, lines: Vec<String> },
    List { items: Vec<ListItem> },
    Table { aligns: Vec<Align>, head: Vec<Vec<TSpan>>, rows: Vec<Vec<Vec<TSpan>>> },
    Rule,
    Image { alt: String, src: String },
    Math { text: String },
    Meta(Vec<(String, String)>),
    FootnoteDef { label: String, spans: Vec<TSpan> },
    Html(String),
}

#[derive(Clone, Debug)]
pub struct OutlineEntry {
    pub level: u8,
    pub title: String,
}

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub blocks: Vec<Block>,
    pub outline: Vec<OutlineEntry>,
    pub words: usize,
    pub meta: Vec<(String, String)>,
}

impl Doc {
    pub fn title(&self) -> Option<&str> {
        self.meta
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("title"))
            .map(|(_, value)| value.as_str())
    }

    /// The reading estimate lmdv shows next to the tab title (R-05).
    pub fn reading_minutes(&self) -> usize {
        ((self.words.max(1) + 109) / 220).max(1)
    }
}

/// Parse a whole document.
pub fn parse(source: &str) -> Doc {
    let mut line_starts = vec![0usize];
    for (index, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            line_starts.push(index + 1);
        }
    }
    let mut state = State { line_starts, ..State::default() };
    for (event, range) in Parser::new_ext(source, options()).into_offset_iter() {
        state.event(event, range.start);
    }
    state.finish()
}

fn options() -> Options {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_GFM);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_MATH);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    options
}

/// Render a Markdown source to HTML for the share/export command.
pub fn to_html(source: &str) -> String {
    let parser = Parser::new_ext(source, options());
    let mut body = String::new();
    pulldown_cmark::html::push_html(&mut body, parser);
    body
}

#[derive(Default)]
struct Styles {
    bold: usize,
    italic: usize,
    strike: usize,
    links: Vec<String>,
}

#[derive(Default)]
struct ListCtx {
    items: Vec<ListItem>,
    levels: Vec<Level>,
    current: Option<usize>,
    /// The item that owns the list being opened, per nesting level.
    owners: Vec<Option<usize>>,
}

struct Level {
    ordered: bool,
    next: u64,
}

struct TableCtx {
    aligns: Vec<Align>,
    head: Vec<Vec<TSpan>>,
    rows: Vec<Vec<Vec<TSpan>>>,
    in_head: bool,
    row: Vec<Vec<TSpan>>,
    cell: Vec<TSpan>,
    in_cell: bool,
}

#[derive(Default)]
struct State {
    blocks: Vec<Block>,
    /// Open quote frames: `(callout, blocks)`. A `None` callout is a plain quote.
    quotes: Vec<(Option<CalloutKind>, Vec<Block>)>,
    list: Option<ListCtx>,
    inline: Vec<TSpan>,
    styles: Styles,
    heading: Option<u8>,
    code: Option<(String, String)>,
    table: Option<TableCtx>,
    meta: Option<String>,
    footnote: Option<(String, Vec<TSpan>)>,
    image: Option<(String, String)>,
    html: Option<String>,
    outline: Vec<OutlineEntry>,
    words: usize,
    line_starts: Vec<usize>,
    item_line: Option<usize>,
}

impl State {
    fn event(&mut self, event: Event<'_>, offset: usize) {
        match event {
            Event::Start(tag) => self.start(tag, offset),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(code) => {
                let mut span = TSpan::plain(code.as_ref());
                span.code = true;
                self.push_span(span);
            }
            Event::InlineMath(math) => {
                let mut span = TSpan::plain(math.as_ref());
                span.math = true;
                span.italic = true;
                self.push_span(span);
            }
            Event::DisplayMath(math) => {
                self.flush_inline();
                self.push_block(Block::Math { text: math.trim().to_owned() });
            }
            Event::SoftBreak => {
                if let Some((_, buffer)) = self.code.as_mut() {
                    buffer.push('\n');
                } else if self.table.as_ref().is_some_and(|table| table.in_cell) {
                    self.push_span(TSpan::plain(" "));
                } else if !self.inline.is_empty() {
                    self.push_span(TSpan::plain(" "));
                }
            }
            Event::HardBreak => {
                if let Some((_, buffer)) = self.code.as_mut() {
                    buffer.push('\n');
                } else if self.table.as_ref().is_some_and(|table| table.in_cell) {
                    self.push_span(TSpan::plain(" "));
                } else {
                    self.push_span(TSpan::plain("\n"));
                }
            }
            Event::Rule => {
                self.flush_inline();
                self.push_block(Block::Rule);
            }
            Event::TaskListMarker(checked) => {
                let source_line = self.item_line;
                if let Some(list) = self.list.as_mut() {
                    if let Some(index) = list.current {
                        if let Some(item) = list.items.get_mut(index) {
                            item.checked = Some(checked);
                            item.source_line = source_line;
                        }
                    }
                }
            }
            Event::FootnoteReference(label) => {
                let mut span = TSpan::plain(format!("[^{label}]"));
                span.link = Some(String::new());
                self.push_span(span);
                self.words += 1;
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                if self.html.is_some() {
                    if let Some(buffer) = self.html.as_mut() {
                        buffer.push_str(&html);
                    }
                }
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>, offset: usize) {
        match tag {
            Tag::Paragraph => {
                if self.inline.is_empty() {
                    self.inline.clear();
                }
            }
            Tag::Heading { level, .. } => {
                self.heading = Some(heading_level(level));
                self.inline.clear();
            }
            Tag::BlockQuote(kind) => {
                self.flush_inline();
                self.quotes.push((kind.map(CalloutKind::from_gfm), Vec::new()));
            }
            Tag::CodeBlock(kind) => {
                self.flush_inline();
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split(|c: char| c == ',' || c.is_whitespace())
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_lowercase(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((lang, String::new()));
            }
            Tag::List(start) => {
                let list = self.list.get_or_insert_with(ListCtx::default);
                let owner = list.current;
                list.owners.push(owner);
                list.levels.push(Level {
                    ordered: start.is_some(),
                    next: start.unwrap_or(1),
                });
            }
            Tag::Item => {
                self.item_line = Some(self.line_of(offset));
                let depth = self.list.as_ref().map_or(0, |list| list.levels.len().saturating_sub(1));
                let (marker, ordered) = {
                    let Some(list) = self.list.as_mut() else { return };
                    let Some(level) = list.levels.last_mut() else { return };
                    if level.ordered {
                        let marker = format!("{}.", level.next);
                        level.next += 1;
                        (marker, true)
                    } else {
                        (bullet_for(depth).to_owned(), false)
                    }
                };
                let _ = ordered;
                if let Some(list) = self.list.as_mut() {
                    list.items.push(ListItem {
                        depth,
                        marker,
                        checked: None,
                        source_line: None,
                        spans: Vec::new(),
                    });
                    list.current = Some(list.items.len() - 1);
                }
            }
            Tag::Table(aligns) => {
                self.flush_inline();
                self.table = Some(TableCtx {
                    aligns: aligns.iter().map(|a| align_of(*a)).collect(),
                    head: Vec::new(),
                    rows: Vec::new(),
                    in_head: false,
                    row: Vec::new(),
                    cell: Vec::new(),
                    in_cell: false,
                });
            }
            Tag::TableHead => {
                if let Some(table) = self.table.as_mut() {
                    table.in_head = true;
                }
            }
            Tag::TableRow => {
                if let Some(table) = self.table.as_mut() {
                    table.row.clear();
                }
            }
            Tag::TableCell => {
                if let Some(table) = self.table.as_mut() {
                    table.cell.clear();
                    table.in_cell = true;
                }
            }
            Tag::Emphasis => self.styles.italic += 1,
            Tag::Strong => self.styles.bold += 1,
            Tag::Strikethrough => self.styles.strike += 1,
            Tag::Link { dest_url, .. } => {
                self.styles.links.push(dest_url.to_string());
            }
            Tag::Image { dest_url, .. } => {
                self.image = Some((String::new(), dest_url.to_string()));
                self.styles.links.push(String::new());
            }
            Tag::FootnoteDefinition(label) => {
                self.flush_inline();
                self.footnote = Some((label.to_string(), Vec::new()));
            }
            Tag::MetadataBlock(MetadataBlockKind::YamlStyle) => {
                self.meta = Some(String::new());
            }
            Tag::HtmlBlock => {
                self.html = Some(String::new());
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                let in_item = self.list.as_ref().is_some_and(|list| list.current.is_some());
                if !in_item && self.table.is_none() && self.footnote.is_none() {
                    self.flush_inline();
                }
            }
            TagEnd::Heading(_) => {
                let level = self.heading.take().unwrap_or(1);
                let spans = std::mem::take(&mut self.inline);
                if !spans.is_empty() {
                    let index = self.outline.len();
                    let title = spans.iter().map(|s| s.text.as_str()).collect::<String>();
                    self.outline.push(OutlineEntry { level, title: title.trim().to_owned() });
                    self.push_block(Block::Heading { level, spans, index });
                }
            }
            TagEnd::BlockQuote(_) => {
                self.flush_inline();
                if let Some((kind, blocks)) = self.quotes.pop() {
                    let block = match kind {
                        Some(kind) => Block::Callout { kind, blocks },
                        None => Block::Quote(blocks),
                    };
                    self.push_block(block);
                }
            }
            TagEnd::CodeBlock => {
                if let Some((lang, body)) = self.code.take() {
                    let mut lines: Vec<String> = body.lines().map(|line| line.to_owned()).collect();
                    while lines.last().is_some_and(|line| line.trim().is_empty()) {
                        lines.pop();
                    }
                    self.push_block(Block::Code { lang, lines });
                }
            }
            TagEnd::List(_) => {
                let Some(list) = self.list.as_mut() else { return };
                list.levels.pop();
                list.owners.pop();
                if list.levels.is_empty() {
                    if let Some(list) = self.list.take() {
                        if !list.items.is_empty() {
                            self.push_block(Block::List { items: list.items });
                        }
                    }
                } else {
                    let owner = list.owners.last().copied().flatten();
                    list.current = owner;
                }
            }
            TagEnd::Item => {}
            TagEnd::TableCell => {
                if let Some(table) = self.table.as_mut() {
                    let cell = std::mem::take(&mut table.cell);
                    table.row.push(cell);
                    table.in_cell = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(table) = self.table.as_mut() {
                    let row = std::mem::take(&mut table.row);
                    if table.in_head {
                        table.head = row;
                    } else {
                        table.rows.push(row);
                    }
                }
            }
            TagEnd::TableHead => {
                if let Some(table) = self.table.as_mut() {
                    // pulldown-cmark emits the head cells without a `TableRow`
                    // wrapper, so the accumulated row becomes the head here.
                    if table.head.is_empty() && !table.row.is_empty() {
                        table.head = std::mem::take(&mut table.row);
                    }
                    table.in_head = false;
                }
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    self.push_block(Block::Table {
                        aligns: table.aligns,
                        head: table.head,
                        rows: table.rows,
                    });
                }
            }
            TagEnd::Emphasis => self.styles.italic = self.styles.italic.saturating_sub(1),
            TagEnd::Strong => self.styles.bold = self.styles.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.styles.strike = self.styles.strike.saturating_sub(1),
            TagEnd::Link => {
                self.styles.links.pop();
            }
            TagEnd::Image => {
                self.styles.links.pop();
                if let Some((alt, src)) = self.image.take() {
                    let sp = alt.trim().to_owned();
                    self.push_block(Block::Image { alt: sp, src });
                }
            }
            TagEnd::FootnoteDefinition => {
                if let Some((label, spans)) = self.footnote.take() {
                    self.push_block(Block::FootnoteDef { label, spans });
                }
            }
            TagEnd::MetadataBlock(_) => {
                if let Some(raw) = self.meta.take() {
                    self.push_block(Block::Meta(parse_meta(&raw)));
                }
            }
            TagEnd::HtmlBlock => {
                if let Some(raw) = self.html.take() {
                    let text = strip_tags(&raw);
                    if !text.trim().is_empty() {
                        self.push_block(Block::Html(text.trim().to_owned()));
                    }
                }
            }
            _ => {}
        }
    }

    fn text(&mut self, text: &str) {
        if let Some((_, buffer)) = self.code.as_mut() {
            buffer.push_str(text);
            return;
        }
        if let Some(raw) = self.meta.as_mut() {
            raw.push_str(text);
            return;
        }
        if self.table.as_ref().is_some_and(|table| table.in_cell) {
            let span = self.span(text);
            if let Some(table) = self.table.as_mut() {
                table.cell.push(span);
            }
            self.words += text.split_whitespace().count();
            return;
        }
        if let Some((alt, _)) = self.image.as_mut() {
            alt.push_str(text);
            self.words += text.split_whitespace().count();
            return;
        }
        if self.footnote.is_some() {
            let span = self.span(text);
            if let Some((_, spans)) = self.footnote.as_mut() {
                spans.push(span);
            }
            self.words += text.split_whitespace().count();
            return;
        }
        let span = self.span(text);
        self.push_span(span);
        self.words += text.split_whitespace().count();
    }

    fn line_of(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        }
    }

    fn span(&self, text: &str) -> TSpan {
        TSpan {
            text: text.to_owned(),
            bold: self.styles.bold > 0,
            italic: self.styles.italic > 0,
            strike: self.styles.strike > 0,
            code: false,
            math: false,
            link: self.styles.links.last().cloned(),
        }
    }

    fn push_span(&mut self, span: TSpan) {
        if span.text.is_empty() {
            return;
        }
        if self.table.as_ref().is_some_and(|table| table.in_cell) {
            if let Some(table) = self.table.as_mut() {
                table.cell.push(span);
            }
            return;
        }
        if self.list.as_ref().is_some_and(|list| list.current.is_some()) && self.heading.is_none() {
            if let Some(list) = self.list.as_mut() {
                if let Some(index) = list.current {
                    if let Some(item) = list.items.get_mut(index) {
                        item.spans.push(span);
                        return;
                    }
                }
            }
        }
        self.inline.push(span);
    }

    fn flush_inline(&mut self) {
        let spans = std::mem::take(&mut self.inline);
        if spans.is_empty() {
            return;
        }
        self.push_block(Block::Para(spans));
    }

    fn push_block(&mut self, block: Block) {
        if let Some((_, blocks)) = self.quotes.last_mut() {
            blocks.push(block);
        } else {
            self.blocks.push(block);
        }
    }

    fn finish(mut self) -> Doc {
        self.flush_inline();
        while let Some((kind, blocks)) = self.quotes.pop() {
            let block = match kind {
                Some(kind) => Block::Callout { kind, blocks },
                None => Block::Quote(blocks),
            };
            self.push_block(block);
        }
        let meta = self
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Meta(meta) => Some(meta.clone()),
                _ => None,
            })
            .unwrap_or_default();
        Doc { blocks: self.blocks, outline: self.outline, words: self.words, meta }
    }
}

fn heading_level(level: pulldown_cmark::HeadingLevel) -> u8 {
    use pulldown_cmark::HeadingLevel::*;
    match level {
        H1 => 1,
        H2 => 2,
        H3 => 3,
        H4 => 4,
        H5 => 5,
        H6 => 6,
    }
}

fn align_of(align: Alignment) -> Align {
    match align {
        Alignment::Center => Align::Center,
        Alignment::Right => Align::Right,
        _ => Align::Left,
    }
}

fn bullet_for(depth: usize) -> &'static str {
    match depth % 3 {
        0 => "•",
        1 => "◦",
        _ => "▪",
    }
}

fn parse_meta(raw: &str) -> Vec<(String, String)> {
    raw.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once(':')?;
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'').to_owned();
            Some((key.trim().to_owned(), value))
        })
        .collect()
}

fn strip_tags(raw: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"---
title: Test document
author: Jane Doe
---

# Test document

Text in **bold** and *italic* and `code`.

## Table

| System | Role |
|---|---|
| PIM | Current |

## Task list

- [x] done
- [ ] todo

```rust
fn main() {}
```

> [!NOTE]
> Useful information.

![Alt text](./img/test.png)
"#;

    #[test]
    fn parses_blocks_and_outline() {
        let doc = parse(SAMPLE);
        assert_eq!(doc.title(), Some("Test document"));
        assert_eq!(doc.outline.len(), 3);
        assert_eq!(doc.outline[1].title, "Table");
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Table { .. })));
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Code { lang, .. } if lang == "rust")));
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Callout { kind: CalloutKind::Note, .. })));
        assert!(doc.blocks.iter().any(|b| matches!(b, Block::Image { alt, .. } if alt == "Alt text")));
        let tasks = doc.blocks.iter().find_map(|b| match b {
            Block::List { items, .. } => Some(items),
            _ => None,
        });
        let tasks = tasks.expect("task list");
        assert_eq!(tasks[0].checked, Some(true));
        assert_eq!(tasks[1].checked, Some(false));
    }
}
