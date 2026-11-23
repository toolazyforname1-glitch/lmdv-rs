//! Markdown tables in the stock style: a filled header band, thin horizontal
//! rules, and no vertical grid. Column widths fit the reading column and every
//! cell wraps inside its column.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use crate::md::{Align, TSpan};
use crate::style::{width_of, wrap_spans, LineKind, VLine};
use crate::theme::Palette;

const MIN_COL: usize = 4;
const MAX_COL: usize = 44;

pub fn build(
    aligns: &[Align],
    head: &[Vec<TSpan>],
    rows: &[Vec<Vec<TSpan>>],
    width: usize,
    p: &Palette,
    section: usize,
) -> Vec<VLine> {
    let cols = head.len().max(aligns.len()).max(rows.iter().map(Vec::len).max().unwrap_or(0));
    if cols == 0 {
        return Vec::new();
    }

    let cell_text = |cell: &Vec<TSpan>| -> String {
        cell.iter().map(|span| span.text.as_str()).collect::<String>().trim().to_owned()
    };

    let mut widths = vec![MIN_COL; cols];
    for (index, cell) in head.iter().enumerate() {
        if index < cols {
            widths[index] = widths[index].max(cell_text(cell).width().min(MAX_COL));
        }
    }
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            if index < cols {
                widths[index] = widths[index].max(cell_text(cell).width().min(MAX_COL));
            }
        }
    }

    let fits = |widths: &[usize]| widths.iter().sum::<usize>() + cols * 4 <= width + 2;
    while !fits(&widths) && widths.iter().any(|w| *w > MIN_COL) {
        let widest = widths
            .iter()
            .enumerate()
            .filter(|(_, w)| **w > MIN_COL)
            .max_by_key(|(_, w)| **w)
            .map(|(index, _)| index);
        if let Some(index) = widest {
            widths[index] -= 1;
        }
    }

    let mut out = Vec::new();
    if !head.iter().all(|cell| cell_text(cell).is_empty()) {
        let cells: Vec<Vec<Vec<Span<'static>>>> = (0..cols)
            .map(|index| {
                let empty = Vec::new();
                let cell = head.get(index).unwrap_or(&empty);
                wrap_spans(cell, widths[index], p, false, Vec::new(), Vec::new())
            })
            .collect();
        render_row(&mut out, &cells, aligns, &widths, cols, true, width, p, section);
        out.push(rule(width, p, section, true));
    }

    for (row_index, row) in rows.iter().enumerate() {
        let cells: Vec<Vec<Vec<Span<'static>>>> = (0..cols)
            .map(|index| {
                let empty = Vec::new();
                let cell = row.get(index).unwrap_or(&empty);
                wrap_spans(cell, widths[index], p, false, Vec::new(), Vec::new())
            })
            .collect();
        render_row(&mut out, &cells, aligns, &widths, cols, false, width, p, section);
        if row_index + 1 < rows.len() {
            out.push(rule(width, p, section, false));
        }
    }
    out
}

fn rule(width: usize, p: &Palette, section: usize, strong: bool) -> VLine {
    VLine::new(
        vec![Span::styled(
            "─".repeat(width),
            Style::new().fg(if strong { p.border_strong } else { p.border }),
        )],
        LineKind::Table,
        section,
    )
}

fn render_row(
    out: &mut Vec<VLine>,
    cells: &[Vec<Vec<Span<'static>>>],
    aligns: &[Align],
    widths: &[usize],
    cols: usize,
    header: bool,
    width: usize,
    p: &Palette,
    section: usize,
) {
    let height = cells.iter().map(Vec::len).max().unwrap_or(1);
    for line in 0..height {
        let mut spans = Vec::new();
        for (index, cell_lines) in cells.iter().enumerate().take(cols) {
            if index > 0 {
                spans.push(Span::styled(
                    "  ",
                    if header { Style::new().bg(p.surface_2) } else { Style::new() },
                ));
            }
            let content = cell_lines.get(line).cloned().unwrap_or_default();
            push_cell(
                &mut spans,
                &content,
                widths[index],
                aligns.get(index).copied().unwrap_or_default(),
                header,
                p,
            );
        }
        if header {
            let used = width_of(&spans);
            if used < width {
                spans.push(Span::styled(
                    " ".repeat(width - used),
                    Style::new().bg(p.surface_2),
                ));
            }
        }
        out.push(VLine::new(spans, LineKind::Table, section));
    }
}

fn push_cell(
    spans: &mut Vec<Span<'static>>,
    content: &[Span<'static>],
    width: usize,
    align: Align,
    header: bool,
    p: &Palette,
) {
    let used = width_of(content);
    let pad = width.saturating_sub(used);
    let (left, right) = match align {
        Align::Left => (0, pad),
        Align::Center => (pad / 2, pad - pad / 2),
        Align::Right => (pad, 0),
    };
    let pad_style = if header { Style::new().bg(p.surface_2) } else { Style::new() };
    spans.push(Span::styled(" ", pad_style));
    if left > 0 {
        spans.push(Span::styled(" ".repeat(left), pad_style));
    }
    for span in content {
        let mut style = span.style;
        if header {
            style = style.bg(p.surface_2).add_modifier(Modifier::BOLD);
        }
        spans.push(Span::styled(span.content.to_string(), style));
    }
    if right > 0 {
        spans.push(Span::styled(" ".repeat(right), pad_style));
    }
    spans.push(Span::styled(" ", pad_style));
}
