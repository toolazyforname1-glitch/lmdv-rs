//! Shared line model, inline styling and word wrapping.
//!
//! The renderer pre-wraps every line so the viewport is a plain slice: that is
//! what makes scrolling, the sticky section bar and find-jumping exact, and it
//! keeps ratatui's `Paragraph` out of the layout decisions.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use crate::md::{CalloutKind, TSpan};
use crate::theme::Palette;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Text,
    Heading(u8),
    Code,
    CodeLabel,
    Quote,
    Callout(CalloutKind),
    Table,
    Rule,
    Meta,
    Math,
    Image,
    Footnote,
    Html,
    Blank,
}

#[derive(Clone, Debug)]
pub struct VLine {
    pub spans: Vec<Span<'static>>,
    /// The unstyled text, used by find and by the sticky section bar.
    pub plain: String,
    /// Set on a heading's first line: the outline index it opens.
    pub heading: Option<usize>,
    /// The outline index in force at this line, for scroll-spy.
    pub section: usize,
    pub kind: LineKind,
    /// A task item's source line and checked state, for the checkbox write.
    pub task: Option<(usize, bool)>,
    /// A fenced code block's text, so its label row can offer Copy.
    pub copy: Option<String>,
}

impl VLine {
    pub fn new(spans: Vec<Span<'static>>, kind: LineKind, section: usize) -> Self {
        let plain = spans.iter().map(|span| span.content.as_ref()).collect();
        Self { spans, plain, heading: None, section, kind, task: None, copy: None }
    }

    pub fn is_blank(&self) -> bool {
        self.kind == LineKind::Blank || self.plain.trim().is_empty()
    }
}

pub fn width_of(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|span| span.content.width()).sum()
}

/// One inline run's terminal style.
pub fn span_style(span: &TSpan, p: &Palette, muted: bool) -> Style {
    let mut style = Style::new();
    let fg = if span.code {
        p.text
    } else if span.math {
        p.syn_fun
    } else if span.link.is_some() {
        p.link
    } else if muted {
        p.muted
    } else {
        p.text
    };
    style = style.fg(fg);
    if span.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if span.italic || span.math || (muted && span.link.is_none()) {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if span.strike {
        style = style.add_modifier(Modifier::CROSSED_OUT);
    }
    if span.code {
        style = style.bg(p.chip_bg);
    }
    if span.link.is_some() {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

enum Atom {
    Word { text: String, style: Style },
    Space,
    Break,
}

fn atoms(spans: &[TSpan], p: &Palette, muted: bool) -> Vec<Atom> {
    let mut out = Vec::new();
    for span in spans {
        let style = span_style(span, p, muted);
        for (index, chunk) in span.text.split('\n').enumerate() {
            if index > 0 {
                out.push(Atom::Break);
            }
            for (word_index, word) in chunk.split(' ').enumerate() {
                if word_index > 0 {
                    out.push(Atom::Space);
                }
                if word.is_empty() {
                    continue;
                }
                out.push(Atom::Word { text: word.to_owned(), style });
            }
        }
    }
    out
}

/// Wrap inline spans into lines of at most `width` cells.
///
/// `first` and `rest` are the line prefixes: list bullets, quote bars or plain
/// indentation. Their width counts against the budget, so the right edge stays
/// aligned for every line of a block.
pub fn wrap_spans(
    spans: &[TSpan],
    width: usize,
    p: &Palette,
    muted: bool,
    first: Vec<Span<'static>>,
    rest: Vec<Span<'static>>,
) -> Vec<Vec<Span<'static>>> {
    let width = width.max(8);
    let mut lines: Vec<Vec<Span<'static>>> = Vec::new();
    let mut current = first;
    let mut used = width_of(&current);
    let mut empty = current.is_empty();
    let mut pending_space = false;

    let flush = |lines: &mut Vec<Vec<Span<'static>>>,
                 current: &mut Vec<Span<'static>>,
                 used: &mut usize,
                 empty: &mut bool,
                 pending_space: &mut bool,
                 rest: &[Span<'static>]| {
        lines.push(std::mem::replace(current, rest.to_vec()));
        *used = width_of(current);
        *empty = true;
        let _ = pending_space;
        *pending_space = false;
    };

    for atom in atoms(spans, p, muted) {
        match atom {
            Atom::Break => {
                if !empty || !lines.is_empty() {
                    flush(&mut lines, &mut current, &mut used, &mut empty, &mut pending_space, &rest);
                }
            }
            Atom::Space => {
                if !empty {
                    pending_space = true;
                }
            }
            Atom::Word { text, style } => {
                let word_width = text.width();
                let need = word_width + usize::from(pending_space);
                if !empty && need > width.saturating_sub(used) {
                    flush(&mut lines, &mut current, &mut used, &mut empty, &mut pending_space, &rest);
                }
                if pending_space {
                    current.push(Span::styled(" ", style));
                    used += 1;
                    pending_space = false;
                }
                if word_width > width.saturating_sub(used) {
                    let mut chunk = String::new();
                    for ch in text.chars() {
                        let ch_width = UnicodeWidthStr::width(ch.to_string().as_str());
                        if !chunk.is_empty() && width.saturating_sub(used) < ch_width {
                            current.push(Span::styled(std::mem::take(&mut chunk), style));
                            flush(&mut lines, &mut current, &mut used, &mut empty, &mut pending_space, &rest);
                        }
                        chunk.push(ch);
                        used += ch_width;
                    }
                    if !chunk.is_empty() {
                        current.push(Span::styled(chunk, style));
                        empty = false;
                    }
                } else {
                    current.push(Span::styled(text, style));
                    used += word_width;
                    empty = false;
                }
            }
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

/// Split a span run at `needle` so matches can be painted without losing the
/// original styling.
pub fn highlight_spans(
    spans: &[Span<'static>],
    needle: &str,
    hit: Style,
    case_sensitive: bool,
) -> Vec<Span<'static>> {
    if needle.is_empty() {
        return spans.to_vec();
    }
    let lowered = needle.to_lowercase();
    let mut out = Vec::new();
    for span in spans {
        let text = span.content.as_ref();
        let hay = if case_sensitive { text.to_owned() } else { text.to_lowercase() };
        if hay.len() != text.len() || !hay.contains(&lowered) {
            out.push(span.clone());
            continue;
        }
        let mut cursor = 0usize;
        let mut offset = 0usize;
        while let Some(found) = hay[offset..].find(&lowered) {
            let start = offset + found;
            let end = start + lowered.len();
            if start > cursor {
                out.push(Span::styled(text[cursor..start].to_owned(), span.style));
            }
            out.push(Span::styled(
                text[start..end].to_owned(),
                hit.patch(span.style),
            ));
            cursor = end;
            offset = end;
        }
        if cursor < text.len() {
            out.push(Span::styled(text[cursor..].to_owned(), span.style));
        }
    }
    out
}
