//! Editor frame: format bar, line-number gutter, soft-wrapped source, cursor
//! and the live preview pane.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Drag, Hit};
use crate::editor::Format;
use crate::md;
use crate::render;
use crate::style::width_of;
use crate::theme::Palette;

pub const FORMAT_BUTTONS: [(&str, Format); 9] = [
    (" B ", Format::Bold),
    (" I ", Format::Italic),
    (" `code` ", Format::Code),
    (" link ", Format::Link),
    (" H1 ", Format::H1),
    (" H2 ", Format::H2),
    (" quote ", Format::Quote),
    (" list ", Format::List),
    (" task ", Format::Task),
];

pub fn draw(frame: &mut Frame, app: &mut App, p: &Palette, body: Rect, hits: &mut Vec<(Rect, Hit)>) {
    let rect = Rect::new(body.x + 2, body.y, body.width.saturating_sub(4), body.height);
    if rect.width < 24 || rect.height < 6 {
        return;
    }
    let name = app
        .active_tab()
        .map(|tab| tab.name.clone())
        .unwrap_or_default();
    let dirty = app.active_tab().is_some_and(|tab| tab.is_dirty());
    let title = format!(" Editor — {name}{} ", if dirty { " ●" } else { "" });
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(p.accent))
            .style(Style::new().bg(p.surface))
            .title(Line::from(Span::styled(title, Style::new().fg(p.muted)))),
        rect,
    );
    let inner = Rect::new(rect.x + 1, rect.y + 1, rect.width.saturating_sub(2), rect.height.saturating_sub(2));
    if inner.height < 4 {
        return;
    }

    let Some(tab) = app.active_tab() else { return };
    let Some(_) = tab.editor.as_ref() else { return };
    let format_bar = app.prefs.format_bar && inner.height > 6;
    let (preview_enabled, editor_bytes) = tab
        .editor
        .as_ref()
        .map(|editor| (editor.preview, editor.bytes))
        .unwrap_or((false, 0));
    let block_mode = app.prefs.large_files_block(editor_bytes);
    let preview_on = preview_enabled;
    let split = preview_on && inner.width >= 100;

    let mut top = inner.y;
    if format_bar {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut x = inner.x;
        for (index, (label, _)) in FORMAT_BUTTONS.iter().enumerate() {
            let width = label.width() as u16;
            hits.push((Rect::new(x, top, width, 1), Hit::EditorFormat(index)));
            spans.push(Span::styled(
                *label,
                Style::new().bg(p.surface_2).fg(if index == 0 { p.accent } else { p.text }),
            ));
            x += width + 1;
            if x < inner.x + inner.width {
                spans.push(Span::styled(" ", Style::new()));
            }
        }
        let save_label = " save ";
        let preview_label = " preview ";
        let save_width = save_label.width() as u16;
        let preview_width = preview_label.width() as u16;
        let right_total = save_width + preview_width + 1;
        let save_x = inner.x + inner.width.saturating_sub(right_total);
        let preview_x = save_x + save_width + 1;
        hits.push((Rect::new(save_x, top, save_width, 1), Hit::EditorSave));
        hits.push((Rect::new(preview_x, top, preview_width, 1), Hit::EditorPreview));
        if x < save_x {
            spans.push(Span::styled(" ".repeat((save_x - x) as usize), Style::new()));
        }
        spans.push(Span::styled(
            save_label,
            Style::new().bg(p.surface_2).fg(p.text),
        ));
        spans.push(Span::styled(" ", Style::new()));
        spans.push(Span::styled(
            preview_label,
            if preview_on {
                Style::new().bg(p.accent).fg(p.on_accent).add_modifier(Modifier::BOLD)
            } else {
                Style::new().bg(p.surface_2).fg(p.muted)
            },
        ));
        frame.render_widget(Paragraph::new(Line::from(spans)), Rect::new(inner.x, top, inner.width, 1));
        top += 1;
    }

    let footer_y = inner.y + inner.height - 1;
    let content_height = footer_y.saturating_sub(top);
    if content_height == 0 {
        return;
    }
    let source_width = if split {
        let percent = app.prefs.editor_split.clamp(30, 75) as usize;
        ((inner.width as usize * percent) / 100)
            .clamp(30, inner.width.saturating_sub(20) as usize) as u16
    } else {
        inner.width
    };
    app.editor_inner = Some(inner);
    let preview_width = if split {
        let divider_x = inner.x + source_width;
        let preview_x = divider_x + 1;
        inner.x + inner.width - preview_x
    } else {
        0
    };
    if split && preview_width > 6 {
        sync_preview(app, p, preview_width.saturating_sub(1) as usize);
    }
    let Some(tab) = app.tabs.get(app.active) else { return };
    let Some(editor) = tab.editor.as_ref() else { return };
    let line_count = editor.lines.len().max(1);
    let digits = line_count.to_string().len();
    let gutter = (digits + 2) as u16;
    if source_width <= gutter + 4 {
        return;
    }
    let text_width = source_width - gutter;
    let text_height = content_height as usize;
    app.editor_width = text_width as usize;
    app.editor_height = text_height;
    app.editor_geom = Some(crate::app::EditorGeom {
        x: inner.x + gutter,
        y: top,
        width: text_width,
        height: content_height,
    });
    hits.push((
        Rect::new(inner.x + gutter, top, text_width, content_height),
        Hit::EditorArea,
    ));

    let rows = editor.visual_rows(text_width as usize);
    let cursor_row = editor.cursor_row(&rows);
    let start = editor.scroll.min(rows.len().saturating_sub(text_height));
    for (row_offset, row) in rows.iter().enumerate().skip(start).take(text_height) {
        let y = top + (row_offset - start) as u16;
        let current = row.line == editor.cursor.0;
        let number = if row.start == 0 {
            format!("{:>width$} ", row.line + 1, width = digits)
        } else {
            " ".repeat(digits + 1)
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                number,
                if current {
                    Style::new().fg(p.accent)
                } else {
                    Style::new().fg(p.faint)
                },
            ))),
            Rect::new(inner.x, y, gutter, 1),
        );
        let caret = if row_offset == cursor_row { Some(editor.cursor.1) } else { None };
        let chunks = editor.row_text(*row, caret);
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (text, is_cursor) in chunks {
            if is_cursor {
                spans.push(Span::styled(
                    if text.trim().is_empty() { " ".to_owned() } else { text },
                    Style::new().bg(p.accent).fg(p.on_accent),
                ));
            } else {
                spans.push(Span::styled(text, Style::new().fg(p.text)));
            }
        }
        if current {
            let used = width_of(&spans);
            if used < text_width as usize {
                spans.push(Span::styled(
                    " ".repeat(text_width as usize - used),
                    Style::new().bg(p.surface_2),
                ));
            }
        }
        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect::new(inner.x + gutter, y, text_width, 1),
        );
    }

    if split {
        let divider_x = inner.x + source_width;
        let divider_height = (inner.y + inner.height).saturating_sub(top);
        let hovered = app.hover.is_some_and(|(hx, hy)| {
            hy >= top
                && hy < top + divider_height
                && (hx == divider_x || hx + 1 == divider_x)
        });
        let active = hovered || app.drag == Some(Drag::Editor);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                if active { "┃" } else { "│" },
                Style::new().fg(if active { p.accent } else { p.border_strong }),
            ))),
            Rect::new(divider_x, top, 1, divider_height),
        );
        hits.push((
            Rect::new(divider_x.saturating_sub(1), top, 2, divider_height),
            Hit::EditorDivider,
        ));
        let preview_x = divider_x + 1;
        if preview_width > 6 {
            let source_rows = rows.len().max(1);
            let fraction = editor.scroll as f32 / source_rows as f32;
            let preview_start = ((fraction * tab.preview_lines.len() as f32).round() as usize)
                .min(tab.preview_lines.len().saturating_sub(1));
            let lines: Vec<Line> = tab
                .preview_lines
                .iter()
                .skip(preview_start)
                .take(content_height.saturating_sub(1) as usize)
                .map(|line| {
                    Line::from(
                        line.spans
                            .iter()
                            .map(|span| Span::styled(span.content.to_string(), span.style))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect();
            frame.render_widget(
                Paragraph::new(lines),
                Rect::new(preview_x, top + 1, preview_width.saturating_sub(1), content_height.saturating_sub(1)),
            );
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    if block_mode { "PREVIEW · BLOCK" } else { "PREVIEW" },
                    Style::new().fg(p.faint),
                ))),
                Rect::new(preview_x, top, preview_width.saturating_sub(1), 1),
            );
        }
    }

    let line = editor.cursor.0 + 1;
    let column = editor.cursor.1 + 1;
    let hint = format!(
        "{}:{}   ctrl+s save   ctrl+z undo   f2 preview   esc reader",
        line, column
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            truncate(&hint, inner.width as usize),
            Style::new().fg(p.faint),
        ))),
        Rect::new(inner.x, footer_y, inner.width, 1),
    );
}

fn sync_preview(app: &mut App, p: &Palette, width: usize) {
    let spacing = app.prefs.spacing();
    let Some(tab) = app.tabs.get_mut(app.active) else { return };
    let Some(editor) = tab.editor.as_ref() else { return };
    if tab.preview_width == width && tab.preview_rev == editor.revision {
        return;
    }
    let text = editor.text();
    let doc = md::parse(&text);
    tab.preview_lines = render::render(&doc, width.max(20), p, spacing);
    tab.preview_width = width;
    tab.preview_rev = editor.revision;
}

fn truncate(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let cell = ch.to_string().width();
        if used + cell > width.saturating_sub(1) {
            break;
        }
        out.push(ch);
        used += cell;
    }
    out.push('…');
    out
}
