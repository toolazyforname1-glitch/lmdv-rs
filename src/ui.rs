//! The frame: header, progress hairline, tab band, reading card, outline,
//! status bar and the modal overlays.
//!
//! Colour comes only from [`Palette`]; spacing is one rhythm; the card is the
//! focal point and everything else is deliberately quiet.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, ConfirmAction, Hit, Mode, Overlay, SHARE_ITEMS};
use crate::editor_ui;
use crate::prefs::Setting;
use crate::style::{highlight_spans, width_of};
use crate::theme::Palette;

const CARD_MAX: usize = 104;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let p = app.palette();
    let mut hits = std::mem::take(&mut app.hits);
    hits.clear();
    frame.render_widget(Block::default().style(Style::new().bg(p.page)), area);
    if area.width < 32 || area.height < 8 {
        draw_tiny(frame, app, &p, area);
        app.hits = hits;
        return;
    }

    let tabs_h = 1u16;
    let header = Rect::new(area.x, area.y, area.width, 1);
    let hairline = Rect::new(area.x, area.y + 1, area.width, 1);
    let mut top = area.y + 2;
    draw_header(frame, app, &p, header, &mut hits);
    draw_hairline(frame, app, &p, hairline, &mut hits);
    if tabs_h == 1 {
        draw_tabs(frame, app, &p, Rect::new(area.x, top, area.width, 1), &mut hits);
        top += 1;
    }
    let status = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
    let body = Rect::new(area.x, top, area.width, status.y.saturating_sub(top));
    draw_body(frame, app, &p, body, &mut hits);
    draw_status(frame, app, &p, status, &mut hits);

    if matches!(app.overlay, Overlay::Palette | Overlay::Browser | Overlay::Help | Overlay::Settings | Overlay::Share | Overlay::Confirm) {
        frame.render_widget(Clear, body);
        frame.render_widget(
            Block::default().style(Style::new().bg(mix(p.page, Color::Rgb(0, 0, 0), 0.55))),
            body,
        );
    }

    match app.overlay {
        Overlay::Find => draw_find(frame, app, &p, body, &mut hits),
        Overlay::Palette => draw_palette(frame, app, &p, body, &mut hits),
        Overlay::Browser => draw_browser(frame, app, &p, body, &mut hits),
        Overlay::Help => draw_help(frame, &p, body),
        Overlay::Settings => draw_settings(frame, app, &p, body, &mut hits),
        Overlay::Share => draw_share(frame, app, &p, body, &mut hits),
        Overlay::Confirm => draw_confirm(frame, app, &p, body, &mut hits),
        Overlay::None => {}
    }
    draw_toast(frame, app, &p, body);
    app.hits = hits;
}

fn draw_tiny(frame: &mut Frame, app: &App, p: &Palette, area: Rect) {
    let Some(tab) = app.active_tab() else {
        return;
    };
    let start = tab.scroll.round() as usize;
    let lines: Vec<Line> = tab
        .lines
        .iter()
        .skip(start)
        .take(area.height as usize)
        .map(|line| {
            Line::from(
                line.spans
                    .iter()
                    .map(|span| Span::styled(span.content.to_string(), span.style))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(Style::new().bg(p.page)), area);
}

fn draw_header(frame: &mut Frame, app: &App, p: &Palette, rect: Rect, hits: &mut Vec<(Rect, Hit)>) {
    let editing = app
        .active_tab()
        .is_some_and(|tab| tab.mode == crate::app::Mode::Editor);
    let mut right = vec![
        Span::styled(" open ", Style::new().bg(p.surface_2).fg(p.muted)),
        Span::styled(" ", Style::new()),
        Span::styled(" settings ", Style::new().bg(p.surface_2).fg(p.muted)),
        Span::styled(" ", Style::new()),
        Span::styled(
            if editing { " read " } else { " edit " },
            Style::new()
                .bg(p.surface_2)
                .fg(if editing { p.accent } else { p.text }),
        ),
        Span::styled(" ", Style::new()),
        Span::styled(" share ", Style::new().bg(p.surface_2).fg(p.muted)),
        Span::styled(" ", Style::new()),
        Span::styled(format!(" {} ", p.kind.name()), Style::new().bg(p.surface_2).fg(p.muted)),
    ];
    right.push(Span::styled(" ", Style::new()));
    right.push(Span::styled(
        format!("{:>3}%", (app.progress() * 100.0).round() as u32),
        Style::new().fg(p.accent).add_modifier(Modifier::BOLD),
    ));
    if app.tabs.len() > 1 {
        right.push(Span::styled("  ", Style::new()));
        right.push(Span::styled(
            format!("{}/{}", app.active + 1, app.tabs.len()),
            Style::new().fg(p.faint),
        ));
    }
    right.push(Span::styled(" ", Style::new()));
    let right_width = width_of(&right);
    let base_x = rect.x + rect.width.saturating_sub(right_width as u16);
    let settings_width = " settings ".width() as u16;
    let edit_width = " edit ".width() as u16;
    let share_width = " share ".width() as u16;
    let open_width = " open ".width() as u16;
    let theme_width = app.theme.name().width() as u16 + 2;
    hits.push((Rect::new(base_x, rect.y, open_width, 1), Hit::HeaderOpen));
    hits.push((Rect::new(base_x + open_width + 1, rect.y, settings_width, 1), Hit::HeaderSettings));
    hits.push((
        Rect::new(base_x + open_width + settings_width + 2, rect.y, edit_width, 1),
        Hit::HeaderEdit,
    ));
    hits.push((
        Rect::new(base_x + open_width + settings_width + edit_width + 3, rect.y, share_width, 1),
        Hit::HeaderShare,
    ));
    hits.push((
        Rect::new(
            base_x + open_width + settings_width + edit_width + share_width + 4,
            rect.y,
            theme_width,
            1,
        ),
        Hit::Theme,
    ));

    let mut left = vec![
        Span::styled(
            " lmdv ",
            Style::new().bg(p.accent).fg(p.on_accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::new()),
    ];
    if let Some(tab) = app.active_tab() {
        left.push(Span::styled(
            truncate(tab.title(), rect.width.saturating_sub(30) as usize),
            Style::new().fg(p.text).add_modifier(Modifier::BOLD),
        ));
        let parent = tab
            .path
            .parent()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let room = (rect.width as usize).saturating_sub(width_of(&left) + right_width + 4);
        if room > 10 {
            let gap_start = width_of(&left);
            left.push(Span::styled("   ", Style::new()));
            let path_text = truncate(&parent, room);
            let path_width = path_text.width();
            left.push(Span::styled(path_text, Style::new().fg(p.faint)));
            hits.push((
                Rect::new(rect.x + (gap_start + 3) as u16, rect.y, path_width as u16, 1),
                Hit::HeaderPath,
            ));
        }
    } else {
        left.push(Span::styled("markdown, in the terminal", Style::new().fg(p.muted)));
    }
    row(frame, rect, left, right, p.page);
}

fn draw_hairline(frame: &mut Frame, app: &App, p: &Palette, rect: Rect, hits: &mut Vec<(Rect, Hit)>) {
    let width = rect.width as usize;
    let filled = ((app.progress() * width as f32).round() as usize).min(width);
    let spans = vec![
        Span::styled("━".repeat(filled), Style::new().fg(p.accent)),
        Span::styled("─".repeat(width - filled), Style::new().fg(p.border)),
    ];
    hits.push((rect, Hit::ProgressBar));
    frame.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(p.page)), rect);
}

fn draw_tabs(frame: &mut Frame, app: &App, p: &Palette, rect: Rect, hits: &mut Vec<(Rect, Hit)>) {
    let mut spans = vec![Span::styled(" ", Style::new())];
    let mut x = rect.x + 1;
    if app.tabs.is_empty() {
        let label = " lmdv ";
        spans.push(Span::styled(
            label,
            Style::new().bg(p.surface_2).fg(p.muted).add_modifier(Modifier::BOLD),
        ));
        x += label.width() as u16 + 1;
    }
    for (index, tab) in app.tabs.iter().enumerate() {
        let active = index == app.active;
        let label = if tab.start {
            " ▤ New tab × ".to_owned()
        } else if active {
            format!(
                " ▤ {}{} · {} min read × ",
                truncate(&tab.name, 20),
                if tab.is_dirty() { " ●" } else { "" },
                tab.doc.reading_minutes()
            )
        } else {
            format!(
                " ▤ {}{} × ",
                truncate(&tab.name, 20),
                if tab.is_dirty() { " ●" } else { "" }
            )
        };
        let label_width = label.width() as u16;
        hits.push((Rect::new(x, rect.y, label_width.saturating_sub(2), 1), Hit::Tab(index)));
        hits.push((Rect::new(x + label_width.saturating_sub(2), rect.y, 2, 1), Hit::TabClose(index)));
        x += label_width + 1;
        if active {
            spans.push(Span::styled(
                label,
                Style::new().bg(p.surface_2).fg(p.text).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(label, Style::new().fg(p.muted)));
        }
        spans.push(Span::styled(" ", Style::new()));
    }
    if x + 3 < rect.x + rect.width {
        hits.push((Rect::new(x, rect.y, 3, 1), Hit::NewTab));
        spans.push(Span::styled(" + ", Style::new().bg(p.surface_2).fg(p.accent)));
    }
    let used = width_of(&spans);
    let hint = "o open   ? keys";
    let mut right = Vec::new();
    if used + hint.width() + 4 < rect.width as usize {
        right.push(Span::styled(hint, Style::new().fg(p.faint)));
        right.push(Span::styled("  ", Style::new()));
    }
    row(frame, rect, spans, right, p.page);
}

fn draw_body(frame: &mut Frame, app: &mut App, p: &Palette, body: Rect, hits: &mut Vec<(Rect, Hit)>) {
    if body.height == 0 {
        return;
    }
    if app.active_tab().is_some_and(|tab| tab.mode == Mode::Editor) {
        editor_ui::draw(frame, app, p, body, hits);
        return;
    }
    let browser_width = if app.browser.open && body.width >= 72 {
        app.prefs
            .browser_width
            .clamp(18, 48)
            .min(body.width.saturating_sub(50).max(18))
    } else {
        0
    };
    app.body_rect = body;
    let remaining = body.width.saturating_sub(browser_width);
    let outline_width = if app.outline_open && app.active_tab().is_some_and(|tab| !tab.start) {
        let candidate = app.prefs.outline_width();
        let room = remaining.saturating_sub(40);
        if room >= 18 {
            candidate.min(room)
        } else {
            0
        }
    } else {
        0
    };
    let card_zone = Rect::new(
        body.x + browser_width,
        body.y,
        remaining.saturating_sub(outline_width),
        body.height,
    );
    if browser_width > 0 {
        draw_browser_pane(
            frame,
            app,
            p,
            Rect::new(body.x, body.y, browser_width, body.height),
            hits,
        );
        hits.push((
            Rect::new(body.x + browser_width.saturating_sub(1), body.y, 2, body.height),
            Hit::BrowserSplitter,
        ));
    }
    if outline_width > 0 {
        let outline_x = body.x + body.width - outline_width;
        draw_outline(
            frame,
            app,
            p,
            Rect::new(outline_x, body.y, outline_width, body.height),
            hits,
        );
        hits.push((
            Rect::new(outline_x.saturating_sub(1), body.y, 2, body.height),
            Hit::OutlineSplitter,
        ));
    }
    if app.tabs.is_empty() || app.active_tab().is_some_and(|tab| tab.start) {
        draw_welcome(frame, app, p, card_zone, hits);
        return;
    }
    let card_width = card_zone
        .width
        .saturating_sub(4)
        .min(app.prefs.card_cap(card_zone.width))
        .max(28)
        .min(card_zone.width);
    let card = Rect::new(
        card_zone.x + (card_zone.width.saturating_sub(card_width)) / 2,
        card_zone.y,
        card_width,
        card_zone.height,
    );
    draw_card(frame, app, p, card, hits);
}

fn draw_card(frame: &mut Frame, app: &mut App, p: &Palette, card: Rect, hits: &mut Vec<(Rect, Hit)>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(p.border))
        .style(Style::new().bg(p.surface));
    frame.render_widget(block, card);

    let pad = if card.width >= 64 { 2 } else { 1 };
    let inner = Rect::new(
        card.x + 1 + pad,
        card.y + 1,
        card.width.saturating_sub(2 + pad * 2),
        card.height.saturating_sub(2),
    );
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let sticky = app.active_tab().and_then(|tab| {
        let start = tab.scroll.round() as usize;
        if start == 0 {
            return None;
        }
        let (index, total, title) = app.current_section()?;
        let heading_line = *tab.heading_lines.get(index)?;
        (heading_line < start).then_some((title, index + 1, total))
    });

    let mut content_y = inner.y;
    if let Some((title, number, total)) = &sticky {
        let mut spans = vec![
            Span::styled("▍ ", Style::new().fg(p.accent)),
            Span::styled(
                truncate(title, inner.width.saturating_sub(12) as usize),
                Style::new().fg(p.text).add_modifier(Modifier::BOLD),
            ),
        ];
        let used = width_of(&spans);
        let right = format!("§ {number}/{total} ");
        if (used + right.width()) < inner.width as usize {
            spans.push(Span::styled(" ".repeat(inner.width as usize - used - right.width()), Style::new()));
        }
        spans.push(Span::styled(right, Style::new().fg(p.faint)));
        let band = Style::new().bg(p.surface_2);
        for span in &mut spans {
            if span.style.bg.is_none() {
                span.style = span.style.bg(p.surface_2);
            }
        }
        frame.render_widget(Paragraph::new(Line::from(spans)).style(band), Rect::new(inner.x, inner.y, inner.width, 1));
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("─".repeat(inner.width as usize), Style::new().fg(p.chip_border)))),
            Rect::new(inner.x, inner.y + 1, inner.width, 1),
        );
        content_y += 2;
    }

    let content_height = (inner.y + inner.height).saturating_sub(content_y);
    if content_height == 0 {
        return;
    }
    app.prepare(inner.width as usize, content_height as usize, p);
    let Some(tab) = app.active_tab() else { return };

    let offset = ((1.0 - app.enter) * 3.0).round() as u16;
    let visible = content_height.saturating_sub(offset) as usize;
    let start = tab.scroll.round() as usize;
    let query = app.find.query.clone();
    let hit = Style::new().bg(p.accent).fg(p.on_accent).add_modifier(Modifier::BOLD);
    let current_match = app.find.matches.get(app.find.current).copied();
    let flash = tab.flash.map(|(line, _)| line);

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(visible);
    let end = start.min(tab.lines.len()).saturating_add(visible).min(tab.lines.len());
    let slice_start = start.min(tab.lines.len());
    for (index, line) in tab.lines[slice_start..end].iter().enumerate() {
        let absolute = start + index;
        let mut spans = if query.is_empty() {
            line.spans.clone()
        } else {
            highlight_spans(&line.spans, &query, hit, app.find.case_sensitive)
        };
        if Some(absolute) == current_match {
            let tint = Style::new().bg(mix(p.surface, p.accent, 0.22));
            for span in &mut spans {
                span.style = span.style.patch(tint);
            }
        }
        if Some(absolute) == flash {
            let tint = Style::new().bg(mix(p.surface, p.accent, 0.34));
            for span in &mut spans {
                span.style = span.style.patch(tint);
            }
        }
        if let Some((source_line, _)) = line.task {
            hits.push((
                Rect::new(inner.x, content_y + offset + index as u16, inner.width, 1),
                Hit::ToggleTask(source_line),
            ));
        }
        if let Some(code) = &line.copy {
            let chip_width = " copy ".width() as u16;
            hits.push((
                Rect::new(
                    inner.x + inner.width.saturating_sub(chip_width),
                    content_y + offset + index as u16,
                    chip_width,
                    1,
                ),
                Hit::CopyCode(code.clone()),
            ));
        }
        lines.push(Line::from(spans));
    }
    let rect = Rect::new(inner.x, content_y + offset, inner.width, visible as u16);
    frame.render_widget(Paragraph::new(lines), rect);
}

fn draw_outline(frame: &mut Frame, app: &mut App, p: &Palette, rect: Rect, hits: &mut Vec<(Rect, Hit)>) {
    hits.push((rect, Hit::OutlinePanel));
    let sep = Rect::new(rect.x, rect.y, 1, rect.height);
    let hovered = app
        .hover
        .is_some_and(|(hx, hy)| hy >= rect.y && hy < rect.y + rect.height && (hx == rect.x || hx + 1 == rect.x));
    frame.render_widget(
        Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                if hovered { "┃" } else { "│" },
                Style::new().fg(if hovered { p.accent } else { p.border_strong }),
            ));
            rect.height as usize
        ])),
        sep,
    );
    let inner = Rect::new(rect.x + 2, rect.y, rect.width.saturating_sub(2), rect.height);
    let count = app.active_tab().map_or(0, |tab| tab.doc.outline.len());
    let mut head = vec![Span::styled(
        " Outline",
        Style::new().fg(p.text).add_modifier(Modifier::BOLD),
    )];
    let tail = format!("{count} ");
    let used = width_of(&head);
    if used + tail.len() < inner.width as usize {
        head.push(Span::styled(" ".repeat(inner.width as usize - used - tail.len()), Style::new()));
    }
    head.push(Span::styled(tail, Style::new().fg(p.faint)));
    frame.render_widget(
        Paragraph::new(Line::from(head)),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    let Some(tab) = app.active_tab() else {
        return;
    };
    let height = inner.height.saturating_sub(2) as usize;
    if height == 0 {
        return;
    }
    let count = tab.doc.outline.len();
    let selected = app.outline_sel.min(count.saturating_sub(1));
    let anchor = if app.outline_focus { selected } else { tab.section.min(count.saturating_sub(1)) };
    let start = anchor.saturating_sub(height.saturating_sub(1) / 2).min(count.saturating_sub(height));

    for (row, index) in (start..count).take(height).enumerate() {
        let entry = &tab.doc.outline[index];
        let is_current = index == tab.section;
        let is_selected = app.outline_focus && index == selected;
        let indent = (entry.level.saturating_sub(1) as usize) * 2;
        let mut spans = vec![Span::raw(" ".repeat(indent))];
        spans.push(Span::styled(
            if is_current || is_selected { "▌ " } else { "  " },
            Style::new().fg(p.accent),
        ));
        let style = if is_selected {
            Style::new().bg(p.surface_3).fg(p.text).add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::new().fg(p.text).add_modifier(Modifier::BOLD)
        } else if entry.level == 1 {
            Style::new().fg(p.muted)
        } else {
            Style::new().fg(p.faint)
        };
        let label = truncate(&entry.title, inner.width.saturating_sub(indent as u16 + 3) as usize);
        spans.push(Span::styled(label, style));
        let line = Line::from(spans);
        let row_rect = Rect::new(inner.x, inner.y + 2 + row as u16, inner.width, 1);
        hits.push((row_rect, Hit::Outline(index)));
        frame.render_widget(Paragraph::new(line).style(Style::new().bg(p.page)), row_rect);
    }
}

fn draw_status(frame: &mut Frame, app: &App, p: &Palette, rect: Rect, hits: &mut Vec<(Rect, Hit)>) {
    if let Some(tab) = app.active_tab() {
        if tab.start {
            let left = vec![
                Span::styled(
                    " NEW TAB ",
                    Style::new().bg(p.accent).fg(p.on_accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" open a file to begin", Style::new().fg(p.muted)),
            ];
            let right = vec![Span::styled(
                " ctrl+o picker   ctrl+t new tab   ? keys ",
                Style::new().fg(p.faint),
            )];
            row(frame, rect, left, right, p.surface_2);
            return;
        }
    }
    if let Some(tab) = app.active_tab() {
        if tab.mode == Mode::Editor {
            if let Some(editor) = &tab.editor {
                let mut left = vec![
                    Span::styled(
                        " EDIT ",
                        Style::new().bg(p.accent).fg(p.on_accent).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!(" {}", tab.name), Style::new().fg(p.text)),
                ];
                if tab.is_dirty() {
                    left.push(Span::styled(" ● unsaved", Style::new().fg(p.warning)));
                }
                left.push(Span::styled(
                    format!("  {}:{}", editor.cursor.0 + 1, editor.cursor.1 + 1),
                    Style::new().fg(p.muted),
                ));
                let right = vec![Span::styled(
                    " ctrl+s save   esc reader   ? keys ",
                    Style::new().fg(p.faint),
                )];
                row(frame, rect, left, right, p.surface_2);
                return;
            }
        }
    }
    let width = rect.width as usize;
    let mut left = vec![Span::styled(" ", Style::new())];
    let mut section_spans = Vec::new();
    if let Some(tab) = app.active_tab() {
        left.push(Span::styled(tab.name.clone(), Style::new().fg(p.text)));
        left.push(Span::styled(" · ", Style::new().fg(p.faint)));
        left.push(Span::styled(format!("{} words", tab.doc.words), Style::new().fg(p.muted)));
        left.push(Span::styled(" · ", Style::new().fg(p.faint)));
        left.push(Span::styled(format!("{} min", tab.doc.reading_minutes()), Style::new().fg(p.muted)));
        if let Some((index, total, title)) = app.current_section() {
            section_spans.push(Span::styled(" · ", Style::new().fg(p.faint)));
            section_spans.push(Span::styled(format!("§{}", index + 1), Style::new().fg(p.accent)));
            section_spans.push(Span::styled(
                format!("/{total}  {}", truncate(&title, 26)),
                Style::new().fg(p.muted),
            ));
        }
    } else {
        left.push(Span::styled("no document", Style::new().fg(p.muted)));
    }

    let mut right = Vec::new();
    let bar_width = 10;
    let filled = ((app.progress() * bar_width as f32).round() as usize).min(bar_width);
    right.push(Span::styled("━".repeat(filled), Style::new().fg(p.accent)));
    right.push(Span::styled("─".repeat(bar_width - filled), Style::new().fg(p.border)));
    right.push(Span::styled(
        format!(" {:>3}% ", (app.progress() * 100.0).round() as u32),
        Style::new().fg(p.muted),
    ));

    let right_width = width_of(&right);
    let hint = if app.outline_focus {
        "↑↓ select  enter jump  esc back "
    } else {
        "? keys   q quit "
    };
    let base = width_of(&left);
    let section_width = width_of(&section_spans);
    let hint_parts: Vec<(&'static str, Option<Hit>)> = if app.outline_focus {
        vec![(hint, None)]
    } else {
        vec![
            ("? keys  ", Some(Hit::StatusHelp)),
            ("q quit ", Some(Hit::StatusQuit)),
        ]
    };
    if base + section_width + right_width + hint.width() + 2 <= width {
        left.extend(section_spans);
        let mut x = rect.x + rect.width - hint.width() as u16;
        for (label, hit) in hint_parts {
            let w = label.width() as u16;
            if let Some(hit) = hit {
                hits.push((Rect::new(x, rect.y, w, 1), hit));
            }
            right.push(Span::styled(label, Style::new().fg(p.faint)));
            x += w;
        }
    } else if base + section_width + right_width + 2 <= width {
        left.extend(section_spans);
    } else if base + right_width + hint.width() + 2 <= width {
        let mut x = rect.x + rect.width - hint.width() as u16;
        for (label, hit) in hint_parts {
            let w = label.width() as u16;
            if let Some(hit) = hit {
                hits.push((Rect::new(x, rect.y, w, 1), hit));
            }
            right.push(Span::styled(label, Style::new().fg(p.faint)));
            x += w;
        }
    }
    row(frame, rect, left, right, p.surface_2);
}

fn draw_welcome(frame: &mut Frame, app: &App, p: &Palette, zone: Rect, hits: &mut Vec<(Rect, Hit)>) {
    let width = zone.width.saturating_sub(8).min(64).max(24);
    let mut lines = vec![
        Line::from(Span::styled("lmdv", Style::new().fg(p.text).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(
            "A quiet Markdown viewer, in the terminal.",
            Style::new().fg(p.muted),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Open a file… ",
            Style::new().bg(p.accent).fg(p.on_accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    let button_index = 3;
    let button_label = " Open a file… ";
    let mut file_rows: Vec<(usize, PathBuf)> = Vec::new();
    let recents: Vec<PathBuf> = app
        .prefs
        .recent
        .iter()
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .take(5)
        .collect();
    if !recents.is_empty() {
        lines.push(Line::from(Span::styled("Recent files", Style::new().fg(p.faint))));
        for path in recents {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            let parent = path
                .parent()
                .map(|parent| parent.display().to_string())
                .unwrap_or_default();
            let age = age_label(&path);
            file_rows.push((lines.len(), path));
            lines.push(Line::from(vec![
                Span::styled(format!("  {name}"), Style::new().fg(p.text)),
                Span::styled(format!("   {}", truncate(&parent, 22)), Style::new().fg(p.faint)),
                Span::styled(format!("  {age}"), Style::new().fg(p.faint)),
            ]));
        }
        lines.push(Line::from(""));
    }
    if app.welcome_files.is_empty() {
        lines.push(Line::from(Span::styled("This folder has no markdown files.", Style::new().fg(p.faint))));
    } else {
        lines.push(Line::from(Span::styled("This folder", Style::new().fg(p.faint))));
        for (name, size) in &app.welcome_files {
            file_rows.push((lines.len(), PathBuf::from(name)));
            lines.push(Line::from(vec![
                Span::styled(format!("  {name}"), Style::new().fg(p.text)),
                Span::styled(format!("  {size}"), Style::new().fg(p.faint)),
            ]));
        }
    }
    lines.push(Line::from(""));
    let segments: Vec<(Vec<Span<'static>>, Hit)> = vec![
        (
            vec![
                Span::styled(" o ", Style::new().bg(p.surface_2).fg(p.accent)),
                Span::styled(" files     ", Style::new().fg(p.muted)),
            ],
            Hit::WelcomeOpen,
        ),
        (
            vec![
                Span::styled(" ? ", Style::new().bg(p.surface_2).fg(p.accent)),
                Span::styled(" keys     ", Style::new().fg(p.muted)),
            ],
            Hit::WelcomeHelp,
        ),
        (
            vec![
                Span::styled(" q ", Style::new().bg(p.surface_2).fg(p.accent)),
                Span::styled(" quit", Style::new().fg(p.muted)),
            ],
            Hit::WelcomeQuit,
        ),
    ];
    let footer_index = lines.len();
    let footer_spans: Vec<Span<'static>> = segments.iter().flat_map(|(spans, _)| spans.clone()).collect();
    let footer_width = width_of(&footer_spans);
    lines.push(Line::from(footer_spans));
    lines.push(Line::from(Span::styled(
        "Version 0.6.0-tui",
        Style::new().fg(p.faint),
    )));

    let height = (lines.len() as u16).min(zone.height.saturating_sub(2)).max(1);
    let rect = Rect::new(
        zone.x + (zone.width.saturating_sub(width)) / 2,
        zone.y + (zone.height.saturating_sub(height)) / 2,
        width,
        height,
    );
    frame.render_widget(
        Paragraph::new(Text::from(lines)).alignment(ratatui::layout::Alignment::Center),
        rect,
    );

    for (index, name) in file_rows {
        if (index as u16) < rect.height {
            hits.push((
                Rect::new(rect.x, rect.y + index as u16, rect.width, 1),
                Hit::WelcomeFile(name),
            ));
        }
    }
    if (button_index as u16) < rect.height {
        let button_width = button_label.width() as u16;
        let x = rect.x + rect.width.saturating_sub(button_width) / 2;
        hits.push((
            Rect::new(x, rect.y + button_index as u16, button_width, 1),
            Hit::WelcomeExplorer,
        ));
    }
    if (footer_index as u16) < rect.height {
        let pad = (rect.width as usize).saturating_sub(footer_width) / 2;
        let mut x = rect.x + pad as u16;
        let y = rect.y + footer_index as u16;
        for (spans, hit) in segments {
            let w = width_of(&spans) as u16;
            hits.push((Rect::new(x, y, w, 1), hit));
            x += w;
        }
    }
}

fn draw_find(frame: &mut Frame, app: &App, p: &Palette, body: Rect, hits: &mut Vec<(Rect, Hit)>) {
    let card_width = body
        .width
        .saturating_sub(if app.outline_open && body.width >= 92 { 28 } else { 0 })
        .saturating_sub(4)
        .min(CARD_MAX as u16);
    let width = (card_width.saturating_sub(2)).max(30);
    let height = 3;
    if body.height <= height + 2 {
        return;
    }
    let rect = Rect::new(
        body.x + (body.width.saturating_sub(width)) / 2,
        body.y + body.height - height - 2,
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    hits.push((rect, Hit::Panel));
    let title = format!(
        " Find — {}/{} ",
        if app.find.matches.is_empty() { 0 } else { app.find.current + 1 },
        app.find.matches.len()
    );
    let border = if app.find.matches.is_empty() && !app.find.query.is_empty() {
        p.warning
    } else {
        p.accent
    };
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(border))
            .style(Style::new().bg(p.surface))
            .title(Line::from(Span::styled(title, Style::new().fg(p.muted)))),
        rect,
    );
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width.saturating_sub(4), 1);
    let buttons: [(&'static str, Hit); 5] = [
        (" Aa ", Hit::FindCase),
        (" W ", Hit::FindWord),
        (" prev ", Hit::FindPrev),
        (" next ", Hit::FindNext),
        (" close ", Hit::FindClose),
    ];
    let buttons_total: usize = buttons.iter().map(|(label, _)| label.width()).sum::<usize>() + 4;
    let input_width = inner.width.saturating_sub(buttons_total as u16 + 1);
    let spans = vec![
        Span::styled("❯ ", Style::new().fg(p.accent)),
        Span::styled(app.find.query.clone(), Style::new().fg(p.text)),
        Span::styled("▌", Style::new().fg(p.accent)),
        Span::styled(
            if app.find.query.is_empty() {
                "   type to search".to_owned()
            } else {
                String::new()
            },
            Style::new().fg(p.faint),
        ),
    ];
    frame.render_widget(Paragraph::new(Line::from(spans)), Rect::new(inner.x, inner.y, input_width, 1));
    let mut x = rect.x + rect.width - 1 - buttons_total as u16;
    for (label, hit) in buttons {
        let w = label.width() as u16;
        let toggle_on = match &hit {
            Hit::FindCase => app.find.case_sensitive,
            Hit::FindWord => app.find.whole_word,
            _ => false,
        };
        hits.push((Rect::new(x, inner.y, w, 1), hit));
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                label,
                if toggle_on {
                    Style::new().bg(p.accent).fg(p.on_accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::new().bg(p.surface_3).fg(p.text)
                },
            ))),
            Rect::new(x, inner.y, w, 1),
        );
        x += w + 1;
    }
}

fn draw_palette(frame: &mut Frame, app: &App, p: &Palette, body: Rect, hits: &mut Vec<(Rect, Hit)>) {
    let width = body.width.saturating_sub(8).min(74).max(40);
    let height = body.height.saturating_sub(4).min(16).max(8);
    let rect = Rect::new(
        body.x + (body.width.saturating_sub(width)) / 2,
        body.y + 1,
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    hits.push((rect, Hit::Panel));
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(p.accent))
            .style(Style::new().bg(p.surface))
            .title(Line::from(Span::styled(" Commands ", Style::new().fg(p.muted)))),
        rect,
    );
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width.saturating_sub(4), rect.height.saturating_sub(2));
    let input = vec![
        Span::styled("❯ ", Style::new().fg(p.accent)),
        Span::styled(app.palette.query.clone(), Style::new().fg(p.text)),
        Span::styled("▌", Style::new().fg(p.accent)),
    ];
    frame.render_widget(Paragraph::new(Line::from(input)), Rect::new(inner.x, inner.y, inner.width, 1));

    let visible = app.palette.visible();
    let rows = inner.height.saturating_sub(3) as usize;
    let selected = app.palette.selected;
    let start = selected.saturating_sub(rows.saturating_sub(1));
    for (row, command) in visible.iter().skip(start).take(rows).enumerate() {
        let index = start + row;
        let active = index == selected;
        let mut spans = vec![Span::styled(if active { "▍ " } else { "  " }, Style::new().fg(p.accent))];
        spans.push(Span::styled(
            command.label,
            if active {
                Style::new().bg(p.surface_3).fg(p.text).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(p.muted)
            },
        ));
        let used = width_of(&spans);
        let hint = format!("{}  ", command.hint);
        if used + hint.width() < inner.width as usize {
            spans.push(Span::styled(" ".repeat(inner.width as usize - used - hint.width()), Style::new()));
        }
        spans.push(Span::styled(hint, Style::new().fg(p.faint)));
        let row_rect = Rect::new(inner.x, inner.y + 2 + row as u16, inner.width, 1);
        hits.push((row_rect, Hit::PaletteRow(index)));
        frame.render_widget(Paragraph::new(Line::from(spans)), row_rect);
    }
    if visible.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("no matching command", Style::new().fg(p.faint)))),
            Rect::new(inner.x, inner.y + 2, inner.width, 1),
        );
    }
    let footer = "↑↓ select   enter run   esc close";
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(footer, Style::new().fg(p.faint)))),
        Rect::new(inner.x, inner.y + inner.height.saturating_sub(1), inner.width, 1),
    );
}

fn draw_browser(frame: &mut Frame, app: &App, p: &Palette, body: Rect, hits: &mut Vec<(Rect, Hit)>) {
    let width = body.width.saturating_sub(6).min(96).max(44);
    let height = body.height.saturating_sub(4).min(32).max(12);
    let rect = Rect::new(
        body.x + (body.width.saturating_sub(width)) / 2,
        body.y + (body.height.saturating_sub(height)) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    hits.push((rect, Hit::Panel));
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(p.accent))
            .style(Style::new().bg(p.surface))
            .title(Line::from(Span::styled(" Files ", Style::new().fg(p.muted)))),
        rect,
    );
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width.saturating_sub(4), rect.height.saturating_sub(2));
    if inner.height < 6 || inner.width < 20 {
        return;
    }

    let mut crumbs: Vec<(String, PathBuf)> = Vec::new();
    let mut acc = PathBuf::new();
    for comp in app.browser.root.components() {
        match comp {
            std::path::Component::Prefix(prefix) => {
                acc.push(prefix.as_os_str());
                crumbs.push((prefix.as_os_str().to_string_lossy().to_string(), acc.clone()));
            }
            std::path::Component::RootDir => {
                acc.push(std::path::MAIN_SEPARATOR_STR);
                if let Some((label, path)) = crumbs.last_mut() {
                    label.push(std::path::MAIN_SEPARATOR);
                    *path = acc.clone();
                } else {
                    crumbs.push((std::path::MAIN_SEPARATOR_STR.to_owned(), acc.clone()));
                }
            }
            std::path::Component::Normal(name) => {
                acc.push(name);
                crumbs.push((name.to_string_lossy().to_string(), acc.clone()));
            }
            _ => {}
        }
    }
    let hidden_label = " hidden ";
    let hidden_width = hidden_label.width() as u16;
    let hidden_rect = Rect::new(inner.x + inner.width.saturating_sub(hidden_width), inner.y, hidden_width, 1);
    hits.push((hidden_rect, Hit::BrowserHidden));
    let crumb_budget = inner.width.saturating_sub(hidden_width + 1) as usize;
    let crumb_width = |start: usize| -> usize {
        let mut width = if start > 0 { 2 } else { 0 };
        for (index, (name, _)) in crumbs.iter().enumerate().skip(start) {
            width += name.width();
            if index + 1 < crumbs.len() {
                width += 3;
            }
        }
        width
    };
    let mut crumb_start = 0;
    while crumb_start + 1 < crumbs.len() && crumb_width(crumb_start) > crumb_budget {
        crumb_start += 1;
    }
    let mut crumb_spans = Vec::new();
    let mut crumb_x = inner.x;
    if crumb_start > 0 {
        crumb_spans.push(Span::styled("… ", Style::new().fg(p.faint)));
        crumb_x += 2;
    }
    for (index, (name, path)) in crumbs.iter().enumerate().skip(crumb_start) {
        let label_width = name.width() as u16;
        hits.push((Rect::new(crumb_x, inner.y, label_width, 1), Hit::BrowserCrumb(path.clone())));
        if index + 1 == crumbs.len() {
            crumb_spans.push(Span::styled(name.clone(), Style::new().fg(p.text).add_modifier(Modifier::BOLD)));
        } else {
            crumb_spans.push(Span::styled(name.clone(), Style::new().fg(p.muted)));
        }
        crumb_x += label_width;
        if index + 1 < crumbs.len() {
            crumb_spans.push(Span::styled(" › ", Style::new().fg(p.faint)));
            crumb_x += 3;
        }
    }
    let used = width_of(&crumb_spans);
    if used < crumb_budget {
        crumb_spans.push(Span::styled(" ".repeat(crumb_budget - used), Style::new()));
    }
    frame.render_widget(
        Paragraph::new(Line::from(crumb_spans)),
        Rect::new(inner.x, inner.y, crumb_budget as u16, 1),
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            hidden_label,
            if app.browser.show_hidden {
                Style::new().bg(p.surface_3).fg(p.text)
            } else {
                Style::new().fg(p.faint)
            },
        ))),
        hidden_rect,
    );

    let mut search_spans = vec![
        Span::styled("❯ ", Style::new().fg(p.accent)),
        Span::styled(app.browser.filter.clone(), Style::new().fg(p.text)),
        Span::styled("▌", Style::new().fg(p.accent)),
    ];
    if app.browser.filter.is_empty() {
        search_spans.push(Span::styled("   filter by name", Style::new().fg(p.faint)));
    }
    let counter = if app.browser.filter.trim().is_empty() {
        format!(
            "{} {} · {} {} ",
            app.browser.files,
            if app.browser.files == 1 { "file" } else { "files" },
            app.browser.folders,
            if app.browser.folders == 1 { "folder" } else { "folders" }
        )
    } else {
        let matches = app.browser.rows.len();
        format!("{matches} {} ", if matches == 1 { "match" } else { "matches" })
    };
    let used = width_of(&search_spans);
    let counter_width = counter.width();
    if used + counter_width < inner.width as usize {
        search_spans.push(Span::styled(" ".repeat(inner.width as usize - used - counter_width), Style::new()));
    }
    search_spans.push(Span::styled(counter, Style::new().fg(p.faint)));
    frame.render_widget(Paragraph::new(Line::from(search_spans)), Rect::new(inner.x, inner.y + 1, inner.width, 1));

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "─".repeat(inner.width as usize),
            Style::new().fg(p.border),
        ))),
        Rect::new(inner.x, inner.y + 2, inner.width, 1),
    );

    let list_y = inner.y + 3;
    let list_height = inner.height.saturating_sub(4) as usize;
    let selected = app.browser.selected;
    let start = selected
        .saturating_sub(list_height.saturating_sub(1) / 2)
        .min(app.browser.rows.len().saturating_sub(list_height));
    for (row, item) in app.browser.rows.iter().skip(start).take(list_height).enumerate() {
        let index = start + row;
        let active = index == selected;
        let indent = if item.search { 0 } else { item.depth * 2 };
        let mut spans = vec![
            Span::raw(" ".repeat(indent)),
            Span::styled(if active { "▌ " } else { "  " }, Style::new().fg(p.accent)),
        ];
        if item.dir {
            spans.push(Span::styled(
                if item.open { "▾ " } else { "▸ " },
                Style::new().fg(p.info),
            ));
        } else {
            spans.push(Span::styled("· ", Style::new().fg(p.faint)));
        }
        let name_style = if app.active_tab().is_some_and(|tab| tab.path == item.path) {
            Style::new().fg(p.accent).add_modifier(Modifier::BOLD)
        } else if item.dir {
            Style::new().fg(p.text).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(p.text)
        };
        let detail = if item.size.is_empty() { String::new() } else { format!("{}  ", item.size) };
        let detail_width = detail.width();
        let name_limit = (inner.width as usize)
            .saturating_sub(detail_width + indent + 8)
            .max(8);
        let name_text = truncate(&item.name, name_limit);
        if app.browser.filter.trim().is_empty() {
            spans.push(Span::styled(name_text, name_style));
        } else {
            spans.extend(highlight_spans(
                &[Span::styled(name_text, name_style)],
                app.browser.filter.trim(),
                Style::new()
                    .bg(mix(p.surface, p.accent, 0.35))
                    .fg(p.text)
                    .add_modifier(Modifier::BOLD),
                false,
            ));
        }
        let limit = (inner.width as usize).saturating_sub(detail_width + 2);
        let mut used = width_of(&spans);
        if item.search && !item.parent.is_empty() {
            let remaining = limit.saturating_sub(used + 2);
            if remaining > 4 {
                spans.push(Span::styled("  ", Style::new()));
                used += 2;
                let parent = truncate(&item.parent, remaining);
                used += parent.width();
                spans.push(Span::styled(parent, Style::new().fg(p.faint)));
            }
        }
        if used + detail_width <= inner.width as usize {
            spans.push(Span::styled(" ".repeat(inner.width as usize - used - detail_width), Style::new()));
            spans.push(Span::styled(detail, Style::new().fg(p.faint)));
        }
        if active {
            let tint = Style::new().bg(p.surface_3);
            for span in &mut spans {
                span.style = span.style.patch(tint);
            }
        }
        let row_rect = Rect::new(inner.x, list_y + row as u16, inner.width, 1);
        hits.push((row_rect, Hit::BrowserRow(index)));
        frame.render_widget(Paragraph::new(Line::from(spans)), row_rect);
    }

    if app.browser.rows.is_empty() {
        let message = if app.browser.filter.trim().is_empty() {
            "No markdown files in this folder"
        } else {
            "No matches"
        };
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from(""),
                Line::from(Span::styled(message, Style::new().fg(p.muted))),
                Line::from(Span::styled(
                    if app.browser.filter.trim().is_empty() { "put a .md file here or go up" } else { "backspace edits the filter" },
                    Style::new().fg(p.faint),
                )),
            ]))
            .alignment(ratatui::layout::Alignment::Center),
            Rect::new(inner.x, list_y, inner.width, list_height as u16),
        );
    }

    let footer = "↑↓ move   → expand   ← collapse   enter open   backspace up   ctrl+h hidden   esc close";
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            truncate(footer, inner.width as usize),
            Style::new().fg(p.faint),
        ))),
        Rect::new(inner.x, inner.y + inner.height.saturating_sub(1), inner.width, 1),
    );
}

const KEYS: &[(&str, &str)] = &[
    ("j / k", "scroll one line"),
    ("d / u", "half page"),
    ("space", "page down"),
    ("g / G", "top / bottom"),
    ("tab", "outline; enter or esc"),
    ("/ or ctrl+f", "find in document"),
    ("n / N", "next / previous match"),
    ("o", "open markdown file"),
    ("ctrl+o", "windows explorer picker"),
    ("ctrl+,", "settings"),
    ("ctrl+e", "edit source"),
    ("ctrl+s", "save (editor)"),
    ("ctrl+t", "new tab"),
    ("f2", "editor preview"),
    ("[ / ]", "previous / next tab"),
    ("x", "close tab (or click ×)"),
    ("drag", "resize panes"),
    ("r", "reload from disk"),
    ("t", "cycle theme"),
    ("1 – 4", "black / dark / light / sepia"),
    ("m", "toggle motion"),
    ("ctrl+p", "command palette"),
    ("? / F1", "this help"),
    ("q", "quit"),
];

fn draw_help(frame: &mut Frame, p: &Palette, body: Rect) {
    let width = body.width.saturating_sub(8).min(78).max(40);
    let height = body.height.saturating_sub(4).min(KEYS.len() as u16 + 4).max(10);
    let rect = Rect::new(
        body.x + (body.width.saturating_sub(width)) / 2,
        body.y + (body.height.saturating_sub(height)) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(p.accent))
            .style(Style::new().bg(p.surface))
            .title(Line::from(Span::styled(" Keys ", Style::new().fg(p.muted)))),
        rect,
    );
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width.saturating_sub(4), rect.height.saturating_sub(2));
    let columns = if inner.width >= 62 { 2 } else { 1 };
    let per_column = KEYS.len().div_ceil(columns);
    let column_width = inner.width as usize / columns;
    for (index, (key, action)) in KEYS.iter().enumerate() {
        let column = index / per_column;
        let row = index % per_column;
        if row >= inner.height as usize {
            continue;
        }
        let x = inner.x + (column * column_width) as u16;
        let mut spans = vec![Span::styled(
            format!(" {key:<12} "),
            Style::new().bg(p.surface_2).fg(p.accent),
        )];
        spans.push(Span::styled("  ", Style::new()));
        spans.push(Span::styled(*action, Style::new().fg(p.muted)));
        frame.render_widget(Paragraph::new(Line::from(spans)), Rect::new(x, inner.y + row as u16, column_width as u16, 1));
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("esc / click closes", Style::new().fg(p.faint))))
            .alignment(ratatui::layout::Alignment::Right),
        Rect::new(inner.x, inner.y + inner.height.saturating_sub(1), inner.width, 1),
    );
}

fn draw_confirm(frame: &mut Frame, app: &App, p: &Palette, body: Rect, hits: &mut Vec<(Rect, Hit)>) {
    let Some(state) = &app.confirm else { return };
    let message = match state.action {
        ConfirmAction::Quit => {
            let dirty = app.tabs.iter().filter(|tab| tab.is_dirty()).count();
            format!("{dirty} tab(s) have unsaved changes. Save before quitting?")
        }
        ConfirmAction::CloseTab(index) => {
            let name = app.tabs.get(index).map(|tab| tab.name.clone()).unwrap_or_default();
            format!("Save changes to {name}?")
        }
    };
    let width = body.width.saturating_sub(10).min(66).max(40);
    let height = 7;
    let rect = Rect::new(
        body.x + (body.width.saturating_sub(width)) / 2,
        body.y + (body.height.saturating_sub(height)) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    hits.push((rect, Hit::Panel));
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(p.warning))
            .style(Style::new().bg(p.surface))
            .title(Line::from(Span::styled(" Unsaved changes ", Style::new().fg(p.warning)))),
        rect,
    );
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width.saturating_sub(4), rect.height.saturating_sub(2));
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            truncate(&message, inner.width as usize),
            Style::new().fg(p.text),
        ))),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Save writes the file. Discard drops the edits.",
            Style::new().fg(p.faint),
        ))),
        Rect::new(inner.x, inner.y + 1, inner.width, 1),
    );

    let buttons: [(&str, usize); 3] = [(" save ", 0), (" discard ", 1), (" cancel ", 2)];
    let total: usize = buttons.iter().map(|(label, _)| label.width()).sum::<usize>() + 4;
    let mut x = rect.x + (rect.width as usize).saturating_sub(total) as u16 / 2;
    let y = inner.y + inner.height.saturating_sub(1);
    for (label, choice) in buttons {
        let label_width = label.width() as u16;
        let selected = state.selected == choice;
        hits.push((Rect::new(x, y, label_width, 1), Hit::ConfirmChoice(choice)));
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                label,
                if selected {
                    Style::new().bg(p.accent).fg(p.on_accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::new().bg(p.surface_3).fg(p.text)
                },
            ))),
            Rect::new(x, y, label_width, 1),
        );
        x += label_width + 2;
    }
}

fn draw_settings(frame: &mut Frame, app: &App, p: &Palette, body: Rect, hits: &mut Vec<(Rect, Hit)>) {
    let width = body.width.saturating_sub(8).min(84).max(46);
    let height = body.height.saturating_sub(4).min(27).max(10);
    let rect = Rect::new(
        body.x + (body.width.saturating_sub(width)) / 2,
        body.y + (body.height.saturating_sub(height)) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    hits.push((rect, Hit::Panel));
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(p.accent))
            .style(Style::new().bg(p.surface))
            .title(Line::from(Span::styled(" Settings ", Style::new().fg(p.muted)))),
        rect,
    );
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width.saturating_sub(4), rect.height.saturating_sub(2));
    if inner.height < 5 {
        return;
    }

    let mut items: Vec<(bool, usize, &'static str)> = Vec::new();
    for (index, setting) in Setting::ALL.iter().enumerate() {
        if let Some(section) = setting.section() {
            if !section.is_empty() {
                items.push((true, 0, section));
            }
        }
        items.push((false, index, setting.label()));
    }
    let list_height = inner.height.saturating_sub(1) as usize;
    let selected_line = items
        .iter()
        .position(|(section, index, _)| !*section && *index == app.settings_sel)
        .unwrap_or(0);
    let start = selected_line
        .saturating_sub(list_height.saturating_sub(1))
        .min(items.len().saturating_sub(list_height));

    for (row, (section, index, label)) in items.iter().skip(start).take(list_height).enumerate() {
        let y = inner.y + row as u16;
        if *section {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(*label, Style::new().fg(p.faint)))),
                Rect::new(inner.x, y, inner.width, 1),
            );
            continue;
        }
        let setting = Setting::ALL[*index];
        let active = *index == app.settings_sel;
        let value = if setting == Setting::Reset {
            "reset".to_owned()
        } else {
            setting.value(&app.prefs)
        };
        let mut spans = vec![
            Span::styled(if active { "▌ " } else { "  " }, Style::new().fg(p.accent)),
            Span::styled(
                *label,
                if active {
                    Style::new().fg(p.text).add_modifier(Modifier::BOLD)
                } else {
                    Style::new().fg(p.muted)
                },
            ),
        ];
        let used = width_of(&spans);
        hits.push((Rect::new(inner.x, y, inner.width, 1), Hit::SettingRow(*index)));
        if matches!(setting, Setting::Reset | Setting::DefaultHandler) {
            let (chip, action) = if setting == Setting::Reset {
                (" reset ".to_owned(), Hit::SettingReset)
            } else if app.default_registered {
                (" registered ".to_owned(), Hit::SettingDefault)
            } else {
                (" make default ".to_owned(), Hit::SettingDefault)
            };
            let chip_width = chip.width();
            if used + chip_width < inner.width as usize {
                spans.push(Span::styled(" ".repeat(inner.width as usize - used - chip_width), Style::new()));
            }
            spans.push(Span::styled(
                chip,
                if active {
                    Style::new().bg(p.accent).fg(p.on_accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::new().bg(p.surface_3).fg(p.text)
                },
            ));
            let x = inner.x + (inner.width as usize).saturating_sub(chip_width) as u16;
            hits.push((Rect::new(x, y, chip_width as u16, 1), action));
        } else if active {
            let right = format!("< {value} >");
            let right_width = right.width();
            if used + right_width < inner.width as usize {
                spans.push(Span::styled(" ".repeat(inner.width as usize - used - right_width), Style::new()));
            }
            spans.push(Span::styled("< ", Style::new().fg(p.accent).add_modifier(Modifier::BOLD)));
            spans.push(Span::styled(value, Style::new().fg(p.accent).add_modifier(Modifier::BOLD)));
            spans.push(Span::styled(" >", Style::new().fg(p.accent).add_modifier(Modifier::BOLD)));
            let x = inner.x + (inner.width as usize).saturating_sub(right_width) as u16;
            hits.push((Rect::new(x, y, 1, 1), Hit::SettingLeft(*index)));
            hits.push((Rect::new(x + right_width as u16 - 1, y, 1, 1), Hit::SettingRight(*index)));
        } else {
            let value_width = value.width();
            if used + value_width < inner.width as usize {
                spans.push(Span::styled(" ".repeat(inner.width as usize - used - value_width), Style::new()));
            }
            spans.push(Span::styled(value, Style::new().fg(p.text)));
        }
        if active {
            let tint = Style::new().bg(p.surface_3);
            for span in &mut spans {
                span.style = span.style.patch(tint);
            }
        }
        let row_rect = Rect::new(inner.x, y, inner.width, 1);
        frame.render_widget(Paragraph::new(Line::from(spans)), row_rect);
    }

    let footer_y = inner.y + inner.height - 1;
    let done = " done ";
    let done_width = done.width() as u16;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "↑↓ move   ←→ change   enter cycle   esc close",
            Style::new().fg(p.faint),
        ))),
        Rect::new(inner.x, footer_y, inner.width.saturating_sub(done_width + 1), 1),
    );
    let done_rect = Rect::new(inner.x + inner.width - done_width, footer_y, done_width, 1);
    hits.push((done_rect, Hit::SettingsClose));
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(done, Style::new().bg(p.surface_3).fg(p.text)))),
        done_rect,
    );
}

fn draw_toast(frame: &mut Frame, app: &App, p: &Palette, body: Rect) {    let Some(toast) = &app.toast else { return };
    let age = toast.born.elapsed().as_secs_f32();
    if age > 3.0 {
        return;
    }
    let width = toast.text.width() as u16 + 6;
    let height = 3;
    if body.width <= width + 2 || body.height <= height + 2 {
        return;
    }
    let rect = Rect::new(body.x + body.width - width - 2, body.y + body.height - height - 1, width, height);
    frame.render_widget(Clear, rect);
    let fade = ((3.0 - age) / 0.6).clamp(0.0, 1.0);
    let border = mix(p.surface, p.success, fade * 0.7 + 0.3);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(border))
            .style(Style::new().bg(p.surface_2)),
        rect,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("✓ ", Style::new().fg(p.success)),
            Span::styled(toast.text.clone(), Style::new().fg(p.text)),
        ])),
        Rect::new(rect.x + 2, rect.y + 1, rect.width.saturating_sub(4), 1),
    );
}

fn row(frame: &mut Frame, rect: Rect, left: Vec<Span<'static>>, right: Vec<Span<'static>>, bg: Color) {
    let left_width = width_of(&left);
    let right_width = width_of(&right);
    let mut spans = left;
    let gap = (rect.width as usize).saturating_sub(left_width + right_width);
    if gap > 0 {
        spans.push(Span::styled(" ".repeat(gap), Style::new().bg(bg)));
    }
    spans.extend(right);
    let mut spans = clip(spans, rect.width as usize);
    for span in &mut spans {
        if span.style.bg.is_none() {
            span.style = span.style.bg(bg);
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(bg)), rect);
}

fn clip(spans: Vec<Span<'static>>, width: usize) -> Vec<Span<'static>> {
    let mut used = 0usize;
    let mut out = Vec::new();
    for span in spans {
        if used >= width {
            break;
        }
        let text = span.content.as_ref();
        let span_width = text.width();
        if used + span_width <= width {
            used += span_width;
            out.push(span);
        } else {
            let mut kept = String::new();
            let mut budget = width - used;
            for ch in text.chars() {
                let cw = ch.to_string().width();
                if cw > budget {
                    break;
                }
                kept.push(ch);
                budget -= cw;
            }
            if !kept.is_empty() {
                out.push(Span::styled(kept, span.style));
            }
            break;
        }
    }
    out
}

fn truncate(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let cw = ch.to_string().width();
        if used + cw > width.saturating_sub(1) {
            break;
        }
        out.push(ch);
        used += cw;
    }
    out.push('…');
    out
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    crate::theme::mix(a, b, t)
}

fn draw_share(frame: &mut Frame, app: &App, p: &Palette, body: Rect, hits: &mut Vec<(Rect, Hit)>) {
    let width = body.width.saturating_sub(10).min(66).max(42);
    let height = (SHARE_ITEMS.len() as u16 + 4).min(body.height.saturating_sub(2)).max(8);
    let rect = Rect::new(
        body.x + (body.width.saturating_sub(width)) / 2,
        body.y + (body.height.saturating_sub(height)) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    hits.push((rect, Hit::Panel));
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(p.accent))
            .style(Style::new().bg(p.surface))
            .title(Line::from(Span::styled(" Share ", Style::new().fg(p.muted)))),
        rect,
    );
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width.saturating_sub(4), rect.height.saturating_sub(2));
    let rows = (inner.height.saturating_sub(2) as usize).min(SHARE_ITEMS.len());
    for row in 0..rows {
        let (label, hint, command) = SHARE_ITEMS[row];
        let active = row == app.share_sel.min(SHARE_ITEMS.len().saturating_sub(1));
        let enabled = command.is_some();
        let mut spans = vec![
            Span::styled(if active { "▌ " } else { "  " }, Style::new().fg(p.accent)),
            Span::styled(
                label,
                if !enabled {
                    Style::new().fg(p.faint)
                } else if active {
                    Style::new().fg(p.text).add_modifier(Modifier::BOLD)
                } else {
                    Style::new().fg(p.text)
                },
            ),
        ];
        let used = width_of(&spans);
        let hint_width = hint.width();
        if used + hint_width < inner.width as usize {
            spans.push(Span::styled(" ".repeat(inner.width as usize - used - hint_width), Style::new()));
        }
        spans.push(Span::styled(hint, Style::new().fg(p.faint)));
        if active {
            let tint = Style::new().bg(p.surface_3);
            for span in &mut spans {
                span.style = span.style.patch(tint);
            }
        }
        let row_rect = Rect::new(inner.x, inner.y + row as u16, inner.width, 1);
        hits.push((row_rect, Hit::ShareRow(row)));
        frame.render_widget(Paragraph::new(Line::from(spans)), row_rect);
    }
    let footer_y = inner.y + inner.height - 1;
    let done = " done ";
    let done_width = done.width() as u16;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "↑↓ select   enter run   esc close",
            Style::new().fg(p.faint),
        ))),
        Rect::new(inner.x, footer_y, inner.width.saturating_sub(done_width + 1), 1),
    );
    let done_rect = Rect::new(inner.x + inner.width - done_width, footer_y, done_width, 1);
    hits.push((done_rect, Hit::ShareClose));
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(done, Style::new().bg(p.surface_3).fg(p.text)))),
        done_rect,
    );
}

fn draw_browser_pane(frame: &mut Frame, app: &mut App, p: &Palette, rect: Rect, hits: &mut Vec<(Rect, Hit)>) {
    hits.push((rect, Hit::BrowserPanel));
    frame.render_widget(Block::default().style(Style::new().bg(p.page)), rect);
    let separator = Rect::new(rect.x + rect.width - 1, rect.y, 1, rect.height);
    let sep_x = separator.x;
    let hovered = app
        .hover
        .is_some_and(|(hx, hy)| hy >= rect.y && hy < rect.y + rect.height && (hx == sep_x || hx + 1 == sep_x));
    frame.render_widget(
        Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                if hovered { "┃" } else { "│" },
                Style::new().fg(if hovered { p.accent } else { p.border_strong }),
            ));
            rect.height as usize
        ])),
        separator,
    );
    let inner = Rect::new(rect.x + 1, rect.y, rect.width.saturating_sub(2), rect.height);
    if inner.width < 10 || inner.height < 5 {
        return;
    }
    let root = app
        .browser
        .root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| app.browser.root.display().to_string());
    let up = " ↑ ";
    let refresh = " R ";
    let chip_total = (up.width() + refresh.width() + 1) as u16;
    let mut header = vec![Span::styled(
        format!(" {}", truncate(&root, inner.width.saturating_sub(chip_total + 2) as usize)),
        Style::new().fg(p.text).add_modifier(Modifier::BOLD),
    )];
    let used = width_of(&header);
    if used + chip_total as usize <= inner.width as usize {
        header.push(Span::styled(
            " ".repeat(inner.width as usize - used - chip_total as usize),
            Style::new(),
        ));
    }
    let mut x = inner.x + inner.width.saturating_sub(chip_total);
    header.push(Span::styled(up, Style::new().bg(p.surface_2).fg(p.text)));
    hits.push((Rect::new(x, inner.y, up.width() as u16, 1), Hit::BrowserUp));
    x += up.width() as u16 + 1;
    header.push(Span::styled(refresh, Style::new().bg(p.surface_2).fg(p.text)));
    hits.push((Rect::new(x, inner.y, refresh.width() as u16, 1), Hit::BrowserRefresh));
    frame.render_widget(Paragraph::new(Line::from(header)), Rect::new(inner.x, inner.y, inner.width, 1));

    let mut search = vec![
        Span::styled(app.browser.filter.clone(), Style::new().fg(p.text)),
    ];
    if app.browser.filter.is_empty() {
        search.push(Span::styled("Search this folder", Style::new().fg(p.faint)));
    } else {
        search.push(Span::styled("▌", Style::new().fg(p.accent)));
    }
    frame.render_widget(
        Paragraph::new(Line::from(search)),
        Rect::new(inner.x, inner.y + 1, inner.width, 1),
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "─".repeat(inner.width as usize),
            Style::new().fg(p.border),
        ))),
        Rect::new(inner.x, inner.y + 2, inner.width, 1),
    );

    let list_y = inner.y + 3;
    let list_height = inner.height.saturating_sub(4) as usize;
    let selected = app.browser.selected;
    let start = selected
        .saturating_sub(list_height.saturating_sub(1) / 2)
        .min(app.browser.rows.len().saturating_sub(list_height));
    for (row, item) in app.browser.rows.iter().skip(start).take(list_height).enumerate() {
        let index = start + row;
        let active = index == selected;
        let indent = if item.search { 0 } else { item.depth * 2 };
        let mut spans = vec![
            Span::raw(" ".repeat(indent)),
            Span::styled(if active { "▌ " } else { "  " }, Style::new().fg(p.accent)),
        ];
        if item.dir {
            spans.push(Span::styled(
                if item.open { "▾ " } else { "▸ " },
                Style::new().fg(p.info),
            ));
        } else {
            spans.push(Span::styled("· ", Style::new().fg(p.faint)));
        }
        let name_style = if app.active_tab().is_some_and(|tab| tab.path == item.path) {
            Style::new().fg(p.accent).add_modifier(Modifier::BOLD)
        } else if item.dir {
            Style::new().fg(p.text).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(p.text)
        };
        let name_limit = (inner.width as usize).saturating_sub(indent + 6).max(4);
        spans.push(Span::styled(truncate(&item.name, name_limit), name_style));
        if active {
            let tint = Style::new().bg(p.surface_3);
            for span in &mut spans {
                span.style = span.style.patch(tint);
            }
            let used = width_of(&spans);
            if used < inner.width as usize {
                spans.push(Span::styled(
                    " ".repeat(inner.width as usize - used),
                    Style::new().bg(p.surface_3),
                ));
            }
        }
        let row_rect = Rect::new(inner.x, list_y + row as u16, inner.width, 1);
        hits.push((row_rect, Hit::BrowserRow(index)));
        frame.render_widget(Paragraph::new(Line::from(spans)), row_rect);
    }
    if app.browser.rows.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "no markdown files",
                Style::new().fg(p.faint),
            ))),
            Rect::new(inner.x, list_y, inner.width, 1),
        );
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " o close   esc reader",
            Style::new().fg(p.faint),
        ))),
        Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1),
    );
}

fn age_label(path: &std::path::Path) -> String {
    let Ok(modified) = std::fs::metadata(path).and_then(|meta| meta.modified()) else {
        return String::new();
    };
    let Ok(elapsed) = modified.elapsed() else {
        return String::new();
    };
    let seconds = elapsed.as_secs();
    if seconds < 60 {
        "now".to_owned()
    } else if seconds < 3600 {
        format!("{} min", seconds / 60)
    } else if seconds < 86_400 {
        format!("{} h", seconds / 3600)
    } else {
        format!("{} d", seconds / 86_400)
    }
}
