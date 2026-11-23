//! The source editor: a character buffer with soft wrap, undo, explicit save
//! and Markdown shortcuts. Line endings and BOM follow the file that was read.

use std::path::Path;

use unicode_width::UnicodeWidthChar;

#[derive(Clone)]
struct Snapshot {
    lines: Vec<Vec<char>>,
    cursor: (usize, usize),
}

#[derive(Clone, Copy, Debug)]
pub struct VRow {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Bold,
    Italic,
    Strike,
    Code,
    Link,
    H1,
    H2,
    H3,
    Paragraph,
    Quote,
    List,
    OrderedList,
    Task,
    Table,
    Divider,
    Math,
    Note,
    Tip,
    Warning,
}

pub struct Editor {
    pub lines: Vec<Vec<char>>,
    pub cursor: (usize, usize),
    pub desired: usize,
    pub scroll: usize,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    pub dirty: bool,    pub eol: String,
    pub bom: bool,
    pub revision: u64,
    pub bytes: usize,
    pub preview: bool,
}

impl Editor {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        let bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
        let text = String::from_utf8_lossy(if bom { &bytes[3..] } else { &bytes }).into_owned();
        let eol = if text.contains("\r\n") { "\r\n" } else { "\n" }.to_owned();
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut lines: Vec<Vec<char>> = normalized.split('\n').map(|line| line.chars().collect()).collect();
        if lines.is_empty() {
            lines.push(Vec::new());
        }
        let bytes = bytes.len();
        Ok(Self {
            lines,
            cursor: (0, 0),
            desired: 0,
            scroll: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
            eol,
            bom,
            revision: 0,
            bytes,
            preview: false,
        })
    }

    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|line| line.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join(&self.eol)
    }

    pub fn save(&mut self, path: &Path) -> std::io::Result<()> {
        let body = self.text();
        let mut bytes = Vec::with_capacity(body.len() + 3);
        if self.bom {
            bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        bytes.extend_from_slice(body.as_bytes());
        std::fs::write(path, &bytes)?;
        self.dirty = false;
        self.bytes = bytes.len();
        Ok(())
    }

    fn snapshot(&mut self) {
        self.undo.push(Snapshot { lines: self.lines.clone(), cursor: self.cursor });
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self) {
        if let Some(snapshot) = self.undo.pop() {
            self.redo.push(Snapshot { lines: self.lines.clone(), cursor: self.cursor });
            self.lines = snapshot.lines;
            self.cursor = snapshot.cursor;
            self.dirty = true;
            self.revision += 1;
        }
    }

    pub fn redo(&mut self) {
        if let Some(snapshot) = self.redo.pop() {
            self.undo.push(Snapshot { lines: self.lines.clone(), cursor: self.cursor });
            self.lines = snapshot.lines;
            self.cursor = snapshot.cursor;
            self.dirty = true;
            self.revision += 1;
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        self.snapshot();
        let (line, col) = self.cursor;
        self.lines[line].insert(col, ch);
        self.cursor = (line, col + 1);
        self.dirty = true;
        self.revision += 1;
    }

    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.snapshot();
        let (line, col) = self.cursor;
        let chars: Vec<char> = text.chars().collect();
        if !chars.contains(&'\n') {
            for (offset, ch) in chars.iter().enumerate() {
                self.lines[line].insert(col + offset, *ch);
            }
            self.cursor = (line, col + chars.len());
        } else {
            let tail: Vec<char> = self.lines[line].split_off(col);
            let mut parts = text.split('\n').collect::<Vec<_>>();
            let first = parts.remove(0);
            self.lines[line].extend(first.chars());
            for (index, part) in parts.iter().enumerate() {
                self.lines.insert(line + 1 + index, part.chars().collect());
            }
            let last = line + parts.len();
            if !parts.is_empty() {
                self.lines[last].extend(tail);
            } else {
                self.lines[line].extend(tail);
            }
            self.cursor = (last, parts.last().map_or(0, |part| part.chars().count()));
        }
        self.dirty = true;
        self.revision += 1;
    }

    pub fn newline(&mut self) {
        self.snapshot();
        let (line, col) = self.cursor;
        let tail = self.lines[line].split_off(col);
        self.lines.insert(line + 1, tail);
        self.cursor = (line + 1, 0);
        self.dirty = true;
        self.revision += 1;
    }

    pub fn backspace(&mut self) {
        let (line, col) = self.cursor;
        if col > 0 {
            self.snapshot();
            self.lines[line].remove(col - 1);
            self.cursor = (line, col - 1);
        } else if line > 0 {
            self.snapshot();
            let current = self.lines.remove(line);
            let prev_len = self.lines[line - 1].len();
            self.lines[line - 1].extend(current);
            self.cursor = (line - 1, prev_len);
        } else {
            return;
        }
        self.dirty = true;
        self.revision += 1;
    }

    pub fn delete(&mut self) {
        let (line, col) = self.cursor;
        if col < self.lines[line].len() {
            self.snapshot();
            self.lines[line].remove(col);
        } else if line + 1 < self.lines.len() {
            self.snapshot();
            let next = self.lines.remove(line + 1);
            self.lines[line].extend(next);
        } else {
            return;
        }
        self.dirty = true;
        self.revision += 1;
    }

    pub fn indent(&mut self) {
        self.insert_text("  ");
    }

    pub fn move_left(&mut self) {
        let (line, col) = self.cursor;
        if col > 0 {
            self.cursor = (line, col - 1);
        } else if line > 0 {
            self.cursor = (line - 1, self.lines[line - 1].len());
        }
        self.desired = self.cursor.1;
    }

    pub fn move_right(&mut self) {
        let (line, col) = self.cursor;
        if col < self.lines[line].len() {
            self.cursor = (line, col + 1);
        } else if line + 1 < self.lines.len() {
            self.cursor = (line + 1, 0);
        }
        self.desired = self.cursor.1;
    }

    pub fn move_word(&mut self, direction: isize) {
        let (mut line, mut col) = self.cursor;
        let forward = direction > 0;
        let line_len = self.lines[line].len();
        if forward {
            while line < self.lines.len() {
                let len = self.lines[line].len();
                let mut index = col;
                while index < len {
                    if self.lines[line][index].is_alphanumeric() {
                        break;
                    }
                    index += 1;
                }
                while index < len && self.lines[line][index].is_alphanumeric() {
                    index += 1;
                }
                if index < len || index != col {
                    self.cursor = (line, index);
                    self.desired = index;
                    return;
                }
                if line + 1 < self.lines.len() {
                    line += 1;
                    col = 0;
                } else {
                    self.cursor = (line, len);
                    return;
                }
            }
        } else {
            if col == 0 && line > 0 {
                line -= 1;
                col = self.lines[line].len();
            }
            let len = self.lines[line].len();
            let mut index = col.min(len);
            while index > 0 && !self.lines[line][index - 1].is_alphanumeric() {
                index -= 1;
            }
            while index > 0 && self.lines[line][index - 1].is_alphanumeric() {
                index -= 1;
            }
            self.cursor = (line, index);
            self.desired = index;
            let _ = line_len;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor = (self.cursor.0, 0);
        self.desired = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = (self.cursor.0, self.lines[self.cursor.0].len());
        self.desired = self.cursor.1;
    }

    pub fn move_doc(&mut self, end: bool) {
        if end {
            let line = self.lines.len() - 1;
            self.cursor = (line, self.lines[line].len());
        } else {
            self.cursor = (0, 0);
        }
        self.desired = self.cursor.1;
    }

    pub fn visual_rows(&self, width: usize) -> Vec<VRow> {
        let width = width.max(4);
        let mut rows = Vec::new();
        for (line_index, line) in self.lines.iter().enumerate() {
            if line.is_empty() {
                rows.push(VRow { line: line_index, start: 0, end: 0 });
                continue;
            }
            let mut start = 0usize;
            while start < line.len() {
                let mut used = 0usize;
                let mut end = start;
                let mut last_space = None;
                while end < line.len() {
                    let ch = line[end];
                    let cell = if ch == '\t' { 4 } else { UnicodeWidthChar::width(ch).unwrap_or(1) };
                    if used + cell > width {
                        break;
                    }
                    used += cell;
                    if ch == ' ' {
                        last_space = Some(end + 1);
                    }
                    end += 1;
                }
                if end == start {
                    end += 1;
                }
                let row_end = if end < line.len() {
                    last_space.unwrap_or(end).max(start + 1).min(line.len())
                } else {
                    end
                };
                rows.push(VRow { line: line_index, start, end: row_end });
                start = row_end;
            }
        }
        rows
    }

    pub fn cursor_row(&self, rows: &[VRow]) -> usize {
        let (line, col) = self.cursor;
        let mut best = 0;
        for (index, row) in rows.iter().enumerate() {
            if row.line == line && row.start <= col {
                best = index;
                if col <= row.end {
                    return index;
                }
            }
        }
        best
    }

    pub fn ensure_visible(&mut self, rows: &[VRow], height: usize) {
        if height == 0 {
            return;
        }
        let index = self.cursor_row(rows);
        if index < self.scroll {
            self.scroll = index;
        } else if index >= self.scroll + height {
            self.scroll = index + 1 - height;
        }
        let max = rows.len().saturating_sub(1);
        self.scroll = self.scroll.min(max);
    }

    pub fn move_vertical(&mut self, rows: &[VRow], delta: isize) {
        if rows.is_empty() {
            return;
        }
        let index = self.cursor_row(rows) as isize;
        let target = (index + delta).clamp(0, rows.len() as isize - 1) as usize;
        let row = rows[target];
        let span = row.end.saturating_sub(row.start);
        let col = row.start + self.desired.min(span);
        self.cursor = (row.line, col);
    }

    pub fn page(&mut self, rows: &[VRow], delta: isize, viewport: usize) {
        let step = viewport.max(2) as isize * delta;
        self.move_vertical(rows, step);
    }

    pub fn scroll_by(&mut self, rows: &[VRow], delta: isize, height: usize) {
        let max = rows.len().saturating_sub(height.max(1));
        self.scroll = (self.scroll as isize + delta).clamp(0, max as isize) as usize;
    }

    pub fn click(&mut self, rows: &[VRow], row_index: usize, dx: usize) {
        let Some(row) = rows.get(row_index) else { return };
        let line = &self.lines[row.line];
        let mut used = 0usize;
        let mut col = row.start;
        while col < row.end {
            let ch = line[col];
            let cell = if ch == '\t' { 4 } else { UnicodeWidthChar::width(ch).unwrap_or(1) };
            if used + cell / 2 >= dx {
                break;
            }
            used += cell;
            col += 1;
        }
        self.cursor = (row.line, col);
        self.desired = col - row.start;
    }

    pub fn apply_format(&mut self, format: Format) {
        match format {
            Format::Bold => self.wrap_inline("**"),
            Format::Italic => self.wrap_inline("*"),
            Format::Strike => self.wrap_inline("~~"),
            Format::Code => self.wrap_inline("`"),
            Format::Link => {
                self.insert_text("[](url)");
                let (line, col) = self.cursor;
                self.cursor = (line, col.saturating_sub(6));
            }
            Format::H1 => self.prefix_line("# "),
            Format::H2 => self.prefix_line("## "),
            Format::H3 => self.prefix_line("### "),
            Format::Paragraph => self.strip_prefix(),
            Format::Quote => self.prefix_line("> "),
            Format::List => self.prefix_line("- "),
            Format::OrderedList => self.prefix_line("1. "),
            Format::Task => self.prefix_line("- [ ] "),
            Format::Table => self.insert_text("\n| Column | Column |\n| --- | --- |\n| Cell | Cell |\n"),
            Format::Divider => self.insert_text("\n---\n"),
            Format::Math => self.insert_text("$$\n\n$$"),
            Format::Note => self.insert_text("> [!NOTE]\n> "),
            Format::Tip => self.insert_text("> [!TIP]\n> "),
            Format::Warning => self.insert_text("> [!WARNING]\n> "),
        }
    }

    fn strip_prefix(&mut self) {
        let line = self.cursor.0;
        let text: String = self.lines[line].iter().collect();
        for prefix in ["### ", "## ", "# ", "> ", "- [ ] ", "- ", "1. "] {
            if let Some(stripped) = text.strip_prefix(prefix) {
                let new_line: Vec<char> = stripped.chars().collect();
                let delta = text.chars().count() - new_line.len();
                self.snapshot();
                self.lines[line] = new_line;
                self.cursor = (line, self.cursor.1.saturating_sub(delta));
                self.dirty = true;
                self.revision += 1;
                return;
            }
        }
    }

    fn wrap_inline(&mut self, marker: &str) {
        self.insert_text(marker);
        self.insert_text(marker);
        let (line, col) = self.cursor;
        self.cursor = (line, col - marker.chars().count());
    }

    fn prefix_line(&mut self, prefix: &str) {
        let line = self.cursor.0;
        self.snapshot();
        if !self.lines[line].starts_with(&prefix.chars().collect::<Vec<_>>()) {
            for (offset, ch) in prefix.chars().enumerate() {
                self.lines[line].insert(offset, ch);
            }
            self.cursor = (line, self.cursor.1 + prefix.chars().count());
            self.dirty = true;
            self.revision += 1;
        }
    }

    pub fn row_text(&self, row: VRow, cursor: Option<usize>) -> Vec<(String, bool)> {
        let line = &self.lines[row.line];
        let mut out: Vec<(String, bool)> = Vec::new();
        let mut chunk = String::new();
        for index in row.start..row.end {
            if Some(index) == cursor {
                if !chunk.is_empty() {
                    out.push((std::mem::take(&mut chunk), false));
                }
                out.push((line[index].to_string(), true));
            } else if line[index] == '\t' {
                chunk.push_str("    ");
            } else {
                chunk.push(line[index]);
            }
        }
        if !chunk.is_empty() {
            out.push((chunk, false));
        }
        if cursor == Some(row.end) {
            out.push((" ".to_owned(), true));
        }
        out
    }
}

impl Editor {
    pub fn move_line(&mut self, up: bool) {
        let line = self.cursor.0;
        let target = if up {
            line.checked_sub(1)
        } else if line + 1 < self.lines.len() {
            Some(line + 1)
        } else {
            None
        };
        let Some(target) = target else { return };
        self.snapshot();
        self.lines.swap(line, target);
        self.cursor = (target, self.cursor.1);
        self.dirty = true;
        self.revision += 1;
    }

    pub fn duplicate_line(&mut self) {
        let line = self.cursor.0;
        let copy = self.lines[line].clone();
        self.snapshot();
        self.lines.insert(line + 1, copy);
        self.cursor = (line + 1, self.cursor.1);
        self.dirty = true;
        self.revision += 1;
    }

    pub fn delete_line(&mut self) {
        let line = self.cursor.0;
        self.snapshot();
        if self.lines.len() == 1 {
            self.lines[0].clear();
        } else {
            self.lines.remove(line);
        }
        let target = line.min(self.lines.len() - 1);
        self.cursor = (target, 0);
        self.dirty = true;
        self.revision += 1;
    }
}
