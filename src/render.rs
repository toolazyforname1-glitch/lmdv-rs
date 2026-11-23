//! Block tree → pre-wrapped, styled lines.
//!
//! Everything the viewport needs is decided here: wrapping width, indentation,
//! heading rules, code panels, callout bars and the section index every line
//! belongs to. The UI only slices this vector.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use crate::highlight;
use crate::md::{Block, CalloutKind, Doc, TSpan};
use crate::prefs::Spacing;
use crate::style::{width_of, wrap_spans, LineKind, VLine};
use crate::table;
use crate::theme::Palette;

pub fn render(doc: &Doc, width: usize, p: &Palette, spacing: Spacing) -> Vec<VLine> {
    let mut renderer = Renderer {
        p,
        width: width.max(16),
        out: Vec::new(),
        section: 0,
        muted: 0,
        spacing,
    };
    renderer.out.push(VLine::new(Vec::new(), LineKind::Blank, 0));
    for block in &doc.blocks {
        renderer.block(block);
    }
    renderer.out
}

/// The outline index in force at a given rendered line.
pub fn section_at(lines: &[VLine], index: usize) -> usize {
    lines.get(index.min(lines.len().saturating_sub(1))).map_or(0, |line| line.section)
}

struct Renderer<'a> {
    p: &'a Palette,
    width: usize,
    out: Vec<VLine>,
    section: usize,
    muted: usize,
    spacing: Spacing,
}

impl<'a> Renderer<'a> {
    fn block(&mut self, block: &Block) {
        match block {
            Block::Meta(meta) => self.meta(meta),
            Block::Heading { level, spans, index } => self.heading(*level, spans, *index),
            Block::Para(spans) => {
                self.space();
                self.wrapped(spans, Vec::new(), LineKind::Text);
            }
            Block::Quote(blocks) => {
                self.space();
                let mut body = self.inner(blocks, true);
                let bar = Span::styled("│ ", Style::new().fg(self.p.accent));
                prefix_lines(&mut body, bar, LineKind::Quote);
                self.out.append(&mut body);
            }
            Block::Callout { kind, blocks } => {
                self.space();
                let color = callout_color(*kind, self.p);
                let tint = crate::theme::mix(self.p.surface, color, 0.12);
                let mut label = vec![Span::styled(
                    format!("  {} {} ", kind.glyph(), kind.label()),
                    Style::new().fg(color).bg(tint).add_modifier(Modifier::BOLD),
                )];
                pad_to(&mut label, self.width, Style::new().bg(tint));
                for span in &mut label {
                    if span.style.bg.is_none() {
                        span.style = span.style.bg(tint);
                    }
                }
                self.out.push(VLine::new(label, LineKind::Callout(*kind), self.section));
                let mut body = self.inner(blocks, false);
                for line in &mut body {
                    line.spans.insert(0, Span::styled("  ", Style::new().bg(tint)));
                    for span in &mut line.spans {
                        span.style = span.style.bg(tint);
                    }
                    let used = width_of(&line.spans);
                    if used < self.width {
                        line.spans.push(Span::styled(
                            " ".repeat(self.width - used),
                            Style::new().bg(tint),
                        ));
                    }
                    line.plain.insert_str(0, "  ");
                    line.kind = LineKind::Callout(*kind);
                }
                self.out.append(&mut body);
            }
            Block::Code { lang, lines } => self.code(lang, lines),
            Block::List { items, .. } => {
                self.space();
                for item in items {
                    let indent = item.depth * 2;
                    let mut first = vec![Span::raw(" ".repeat(indent))];
                    let marker = match item.checked {
                        Some(true) => {
                            let box_marker = "☑ ";
                            first.push(Span::styled(box_marker, Style::new().fg(self.p.accent)));
                            box_marker.width()
                        }
                        Some(false) => {
                            let box_marker = "☐ ";
                            first.push(Span::styled(box_marker, Style::new().fg(p_marker(self.p))));
                            box_marker.width()
                        }
                        None => {
                            let color = self.p.muted;
                            let marker = format!("{} ", item.marker);
                            first.push(Span::styled(marker.clone(), Style::new().fg(color)));
                            marker.width()
                        }
                    };
                    let rest = vec![Span::raw(" ".repeat(indent + marker))];
                    let muted = self.muted > 0;
                    let task = item.source_line.zip(item.checked);
                    for (line_index, line) in wrap_spans(&item.spans, self.width, self.p, muted, first, rest)
                        .into_iter()
                        .enumerate()
                    {
                        let mut vline = VLine::new(line, LineKind::Text, self.section);
                        if line_index == 0 {
                            vline.task = task;
                        }
                        self.out.push(vline);
                    }
                }
            }
            Block::Table { aligns, head, rows } => {
                self.space();
                let mut lines = table::build(aligns, head, rows, self.width, self.p, self.section);
                self.out.append(&mut lines);
            }
            Block::Rule => {
                self.space();
                self.out.push(VLine::new(
                    vec![Span::styled("─".repeat(self.width), Style::new().fg(self.p.border))],
                    LineKind::Rule,
                    self.section,
                ));
            }
            Block::Image { alt, src } => {
                self.space();
                let name = if alt.trim().is_empty() { "image" } else { alt.trim() };
                let mut spans = vec![
                    Span::styled("▣ ", Style::new().fg(self.p.info)),
                    Span::styled(name.to_owned(), Style::new().fg(self.p.text)),
                    Span::styled("   ", Style::new()),
                    Span::styled(src.clone(), Style::new().fg(self.p.faint)),
                ];
                pad_to(&mut spans, self.width, Style::new());
                self.out.push(VLine::new(spans, LineKind::Image, self.section));
            }
            Block::Math { text } => {
                self.space();
                let bg = Style::new().bg(self.p.surface_2);
                for line in text.lines() {
                    let body = line.trim();
                    if body.is_empty() {
                        continue;
                    }
                    let mut spans = vec![Span::styled("  ", bg)];
                    spans.push(Span::styled(
                        body.to_owned(),
                        Style::new()
                            .fg(self.p.syn_fun)
                            .bg(self.p.surface_2)
                            .add_modifier(Modifier::ITALIC),
                    ));
                    pad_to(&mut spans, self.width, bg);
                    self.out.push(VLine::new(spans, LineKind::Math, self.section));
                }
            }
            Block::FootnoteDef { label, spans } => {
                self.space();
                let first = vec![
                    Span::styled("[", Style::new().fg(self.p.muted)),
                    Span::styled(label.clone(), Style::new().fg(self.p.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("] ", Style::new().fg(self.p.muted)),
                ];
                let rest = vec![Span::raw(" ".repeat(width_of(&first)))];
                for line in wrap_spans(spans, self.width, self.p, true, first, rest) {
                    self.out.push(VLine::new(line, LineKind::Footnote, self.section));
                }
            }
            Block::Html(text) => {
                self.space();
                let spans = vec![TSpan { text: text.clone(), ..TSpan::default() }];
                self.wrapped(&spans, Vec::new(), LineKind::Html);
            }
        }
    }

    /// Render nested blocks (quote or callout bodies) in isolation.
    fn inner(&mut self, blocks: &[Block], muted: bool) -> Vec<VLine> {
        let saved = std::mem::take(&mut self.out);
        if muted {
            self.muted += 1;
        }
        for block in blocks {
            self.block(block);
        }
        if muted {
            self.muted -= 1;
        }
        let mut body = std::mem::replace(&mut self.out, saved);
        while body.first().is_some_and(|line| line.is_blank()) {
            body.remove(0);
        }
        while body.last().is_some_and(|line| line.is_blank()) {
            body.pop();
        }
        body
    }

    fn heading(&mut self, level: u8, spans: &[TSpan], index: usize) {
        let wanted = if self.spacing == Spacing::Compact {
            1
        } else {
            match self.spacing {
                Spacing::Relaxed => 2,
                _ => 1,
            }
        };
        self.ensure_blanks(wanted);
        let muted = level >= 5;
        let lines = wrap_spans(spans, self.width, self.p, muted, Vec::new(), Vec::new());
        for (line_index, line) in lines.into_iter().enumerate() {
            let mut line_spans = line;
            for span in &mut line_spans {
                span.style = span.style.add_modifier(Modifier::BOLD);
                if level == 1 {
                    span.style = span.style.fg(self.p.text);
                }
            }
            let mut vline = VLine::new(line_spans, LineKind::Heading(level), index);
            if line_index == 0 {
                vline.heading = Some(index);
            }
            self.out.push(vline);
        }
        self.section = index;
    }

    fn code(&mut self, lang: &str, lines: &[String]) {
        self.space();
        let fill = Style::new().bg(self.p.code_bg);
        let body = lines.join("\n");
        let mut label = vec![Span::styled(
            format!("  {}", if lang.is_empty() { "code" } else { lang }),
            Style::new().fg(self.p.muted).bg(self.p.code_bg),
        )];
        let copy = " copy ";
        let copy_width = copy.width();
        let used = width_of(&label);
        if used + copy_width < self.width {
            label.push(Span::styled(" ".repeat(self.width - used - copy_width), fill));
            label.push(Span::styled(copy, Style::new().bg(self.p.surface_2).fg(self.p.muted)));
        }
        let mut label_line = VLine::new(label, LineKind::CodeLabel, self.section);
        label_line.copy = Some(body);
        self.out.push(label_line);
        for line in lines {
            let expanded = line.replace('\t', "    ");
            let mut spans = vec![Span::styled("  ", fill)];
            for span in highlight::code_line(lang, &expanded, self.p) {
                spans.push(Span::styled(span.content.to_string(), span.style.bg(self.p.code_bg)));
            }
            pad_to(&mut spans, self.width, fill);
            self.out.push(VLine::new(spans, LineKind::Code, self.section));
        }
    }

    fn meta(&mut self, meta: &[(String, String)]) {
        if meta.is_empty() {
            return;
        }
        self.space();
        let title = meta
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("title"))
            .map(|(_, value)| value.clone());
        let mut spans = vec![Span::styled("  ◆ ", Style::new().fg(self.p.accent))];
        if let Some(title) = &title {
            spans.push(Span::styled(
                title.clone(),
                Style::new().fg(self.p.text).add_modifier(Modifier::BOLD),
            ));
        }
        let rest: Vec<String> = meta
            .iter()
            .filter(|(key, _)| !key.eq_ignore_ascii_case("title"))
            .map(|(_, value)| value.trim_matches(['[', ']']).to_owned())
            .collect();
        if !rest.is_empty() {
            if title.is_some() {
                spans.push(Span::styled("   ", Style::new()));
            }
            let mut tail = rest.join("  ·  ");
            let used = width_of(&spans);
            let room = self.width.saturating_sub(used + 2);
            if tail.width() > room {
                let mut cut = String::new();
                let mut taken = 0usize;
                for ch in tail.chars() {
                    let cw = ch.to_string().width();
                    if taken + cw > room.saturating_sub(1) {
                        break;
                    }
                    cut.push(ch);
                    taken += cw;
                }
                cut.push('…');
                tail = cut;
            }
            spans.push(Span::styled(tail, Style::new().fg(self.p.muted)));
        }
        let bg = Style::new().bg(self.p.surface_2);
        pad_to(&mut spans, self.width, bg);
        for span in &mut spans {
            if span.style.bg.is_none() {
                span.style = span.style.bg(self.p.surface_2);
            }
        }
        self.out.push(VLine::new(spans, LineKind::Meta, self.section));
    }

    fn wrapped(&mut self, spans: &[TSpan], prefix: Vec<Span<'static>>, kind: LineKind) {
        let rest = vec![Span::raw(" ".repeat(width_of(&prefix)))];
        let muted = self.muted > 0;
        for line in wrap_spans(spans, self.width, self.p, muted, prefix, rest) {
            self.out.push(VLine::new(line, kind, self.section));
        }
    }

    fn space(&mut self) {
        let wanted = match self.spacing {
            Spacing::Compact => 0,
            Spacing::Normal => 1,
            Spacing::Relaxed => 2,
        };
        self.ensure_blanks(wanted);
    }

    fn ensure_blanks(&mut self, wanted: usize) {
        if self.out.is_empty() {
            return;
        }
        let trailing = self
            .out
            .iter()
            .rev()
            .take_while(|line| line.is_blank())
            .count();
        for _ in trailing..wanted {
            self.out.push(VLine::new(Vec::new(), LineKind::Blank, self.section));
        }
    }
}

fn callout_color(kind: CalloutKind, p: &Palette) -> ratatui::style::Color {
    match kind {
        CalloutKind::Note => p.info,
        CalloutKind::Tip => p.success,
        CalloutKind::Important => p.syn_fun,
        CalloutKind::Warning => p.warning,
        CalloutKind::Caution => p.danger,
    }
}

fn prefix_lines(lines: &mut [VLine], prefix: Span<'static>, kind: LineKind) {
    let pad = " ".repeat(width_of(std::slice::from_ref(&prefix)));
    for line in lines {
        if line.is_blank() {
            line.spans = vec![prefix.clone()];
            line.plain = pad.clone();
        } else {
            line.spans.insert(0, prefix.clone());
            line.plain.insert_str(0, &pad);
        }
        line.kind = kind;
    }
}

/// Fill a line to the full reading width so a background band stays continuous;
/// overlong lines are clipped instead of wrapped.
fn pad_to(spans: &mut Vec<Span<'static>>, width: usize, fill: Style) {
    let used = width_of(spans);
    if used < width {
        spans.push(Span::styled(" ".repeat(width - used), fill));
        return;
    }
    if used == width {
        return;
    }
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut budget = width;
    for span in spans.drain(..) {
        let text = span.content.as_ref();
        let span_width = text.width();
        if span_width <= budget {
            budget -= span_width;
            out.push(span);
        } else {
            let mut kept = String::new();
            let mut taken = 0usize;
            for ch in text.chars() {
                let cw = ch.to_string().width();
                if taken + cw > budget {
                    break;
                }
                kept.push(ch);
                taken += cw;
            }
            if !kept.is_empty() {
                out.push(Span::styled(kept, span.style));
            }
            break;
        }
    }
    *spans = out;
}

fn p_marker(palette: &Palette) -> ratatui::style::Color {
    palette.faint
}
