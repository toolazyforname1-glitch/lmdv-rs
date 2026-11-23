//! Application state and input: tabs, smooth scrolling, find, the command
//! palette, the file browser and the theme cycle.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

use crate::editor::{Editor, Format};
use crate::editor_ui::FORMAT_BUTTONS;
use crate::md::{self, Doc};
use crate::prefs::{self, Prefs, Setting, Spacing};
use crate::render::{self, section_at};
use crate::session::{self, Session, SessionTab};
use crate::style::{LineKind, VLine};
use crate::theme::{Palette, ThemeKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Reader,
    Editor,
}

pub struct Tab {
    pub path: PathBuf,
    pub name: String,
    pub doc: Doc,
    pub lines: Vec<VLine>,
    width: usize,
    pub scroll: f32,
    pub target: f32,
    pub max_scroll: f32,
    pub section: usize,
    pub viewport: usize,
    pub flash: Option<(usize, Instant)>,
    spacing: Spacing,
    pub mode: Mode,
    pub editor: Option<Editor>,
    pub preview_lines: Vec<VLine>,
    pub preview_width: usize,
    pub preview_rev: u64,
    pub heading_lines: Vec<usize>,
    pub start: bool,
    pub rev: u64,
}

impl Tab {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let source = std::fs::read_to_string(path)?;
        let doc = md::parse(&source);
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled.md".to_owned());
        Ok(Self {
            path: path.to_path_buf(),
            name,
            doc,
            lines: Vec::new(),
            width: 0,
            scroll: 0.0,
            target: 0.0,
            max_scroll: 0.0,
            section: 0,
            viewport: 1,
            flash: None,
            spacing: Spacing::Normal,
            mode: Mode::Reader,
            editor: None,
            preview_lines: Vec::new(),
            preview_width: 0,
            preview_rev: u64::MAX,
            heading_lines: Vec::new(),
            start: false,
            rev: 0,
        })
    }

    pub fn start() -> Self {
        Self {
            path: PathBuf::new(),
            name: "New tab".to_owned(),
            doc: Doc::default(),
            lines: Vec::new(),
            width: 0,
            scroll: 0.0,
            target: 0.0,
            max_scroll: 0.0,
            section: 0,
            viewport: 1,
            flash: None,
            spacing: Spacing::Normal,
            mode: Mode::Reader,
            editor: None,
            preview_lines: Vec::new(),
            preview_width: 0,
            preview_rev: u64::MAX,
            heading_lines: Vec::new(),
            start: true,
            rev: 0,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.editor.as_ref().is_some_and(|editor| editor.dirty)
    }

    pub fn reload(&mut self) -> std::io::Result<()> {
        let source = std::fs::read_to_string(&self.path)?;
        self.doc = md::parse(&source);
        self.width = 0;
        self.flash = None;
        Ok(())
    }

    pub fn relayout(&mut self, width: usize, p: &Palette, spacing: Spacing) {
        if self.start {
            return;
        }
        if width == self.width && spacing == self.spacing && !self.lines.is_empty() {
            return;
        }
        self.width = width.max(16);
        self.spacing = spacing;
        self.lines = render::render(&self.doc, self.width, p, spacing);
        self.rev = self.rev.wrapping_add(1);
        if self.lines.is_empty() {
            self.lines.push(VLine::new(
                vec![ratatui::text::Span::raw("This document is empty.")],
                LineKind::Text,
                0,
            ));
        }
        self.heading_lines.clear();
        for (index, line) in self.lines.iter().enumerate() {
            if let Some(heading) = line.heading {
                if self.heading_lines.len() <= heading {
                    self.heading_lines.resize(heading + 1, 0);
                }
                self.heading_lines[heading] = index;
            }
        }
    }

    pub fn goto_heading(&mut self, index: usize) {
        if let Some(line) = self.heading_lines.get(index).copied() {
            self.target = line as f32;
            self.flash = Some((line, Instant::now()));
        }
    }

    pub fn title(&self) -> &str {
        self.doc.title().filter(|title| !title.is_empty()).unwrap_or(&self.name)
    }
}

pub struct FindState {
    pub open: bool,
    pub query: String,
    pub matches: Vec<usize>,
    pub current: usize,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub cache: Option<(usize, u64, String, bool, bool)>,
}

impl Default for FindState {
    fn default() -> Self {
        Self {
            open: false,
            query: String::new(),
            matches: Vec::new(),
            current: 0,
            case_sensitive: false,
            whole_word: false,
            cache: None,
        }
    }
}

pub struct Command {
    pub id: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
}

pub const COMMANDS: &[Command] = &[
    Command { id: "theme.black", label: "Theme: black", hint: "t" },
    Command { id: "theme.dark", label: "Theme: dark", hint: "t" },
    Command { id: "theme.light", label: "Theme: light", hint: "t" },
    Command { id: "theme.sepia", label: "Theme: sepia", hint: "t" },
    Command { id: "view.outline", label: "Toggle outline", hint: "tab" },
    Command { id: "file.open", label: "Open markdown file…", hint: "o" },
    Command { id: "file.open-explorer", label: "Open with Windows Explorer…", hint: "ctrl+o" },
    Command { id: "edit.find", label: "Find in document", hint: "/" },
    Command { id: "edit.toggle", label: "Edit source", hint: "ctrl+e" },
    Command { id: "edit.strike", label: "Format: Strikethrough", hint: "" },
    Command { id: "edit.heading-3", label: "Format: Heading 3", hint: "" },
    Command { id: "edit.paragraph", label: "Format: Paragraph", hint: "" },
    Command { id: "edit.list-ordered", label: "Format: Numbered list", hint: "" },
    Command { id: "edit.insert-table", label: "Insert: Table", hint: "" },
    Command { id: "edit.insert-math", label: "Insert: Math block", hint: "" },
    Command { id: "edit.insert-divider", label: "Insert: Divider", hint: "" },
    Command { id: "edit.insert-note", label: "Insert: Note callout", hint: "" },
    Command { id: "edit.insert-tip", label: "Insert: Tip callout", hint: "" },
    Command { id: "edit.insert-warning", label: "Insert: Warning callout", hint: "" },
    Command { id: "edit.move-line-up", label: "Edit: Move line up", hint: "alt+↑" },
    Command { id: "edit.move-line-down", label: "Edit: Move line down", hint: "alt+↓" },
    Command { id: "edit.duplicate-line", label: "Edit: Duplicate line", hint: "ctrl+d" },
    Command { id: "edit.delete-line", label: "Edit: Delete line", hint: "ctrl+shift+k" },
    Command { id: "share.export-html", label: "Share: Export HTML", hint: "" },
    Command { id: "share.copy-path", label: "Share: Copy file path", hint: "" },
    Command { id: "share.copy-link", label: "Share: Copy Markdown link", hint: "" },
    Command { id: "share.copy-text", label: "Share: Copy document text", hint: "" },
    Command { id: "app.share", label: "Share menu…", hint: "" },
    Command { id: "file.reload", label: "Reload from disk", hint: "r" },
    Command { id: "tab.next", label: "Next tab", hint: "]" },
    Command { id: "tab.prev", label: "Previous tab", hint: "[" },
    Command { id: "tab.close", label: "Close tab", hint: "x" },
    Command { id: "tab.new", label: "New tab", hint: "ctrl+t" },
    Command { id: "tab.reopen", label: "Reopen closed tab", hint: "ctrl+shift+t" },
    Command { id: "scroll.top", label: "Go to top", hint: "g" },
    Command { id: "scroll.bottom", label: "Go to bottom", hint: "G" },
    Command { id: "view.motion", label: "Toggle motion", hint: "m" },
    Command { id: "app.settings", label: "Settings…", hint: "ctrl+," },
    Command { id: "app.settings-reset", label: "Reset all settings", hint: "" },
    Command { id: "app.default-handler", label: "Make lmdv-tui the default for Markdown", hint: "" },
    Command { id: "app.help", label: "Keyboard help", hint: "?" },
    Command { id: "app.quit", label: "Quit lmdv", hint: "q" },
];

pub struct PaletteState {
    pub query: String,
    pub selected: usize,
    filtered: Vec<usize>,
}

impl Default for PaletteState {
    fn default() -> Self {
        Self { query: String::new(), selected: 0, filtered: (0..COMMANDS.len()).collect() }
    }
}

impl PaletteState {
    pub fn refresh(&mut self) {
        let needle = self.query.to_lowercase();
        self.filtered = COMMANDS
            .iter()
            .enumerate()
            .filter(|(_, command)| fuzzy(&needle, command.label))
            .map(|(index, _)| index)
            .collect();
        if self.filtered.is_empty() {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(self.filtered.len() - 1);
        }
    }

    pub fn visible(&self) -> Vec<&'static Command> {
        self.filtered.iter().map(|index| &COMMANDS[*index]).collect()
    }

    pub fn chosen(&self) -> Option<&'static Command> {
        self.filtered.get(self.selected).map(|index| &COMMANDS[*index])
    }

    pub fn command_at(&self, index: usize) -> Option<&'static Command> {
        self.filtered.get(index).map(|index| &COMMANDS[*index])
    }
}

#[derive(Clone)]
pub struct Node {
    pub path: PathBuf,
    pub name: String,
    pub dir: bool,
    pub size: String,
    pub children: Vec<Node>,
}

#[derive(Clone)]
pub struct Row {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub dir: bool,
    pub open: bool,
    pub size: String,
    pub parent: String,
    pub search: bool,
}

pub struct BrowserState {
    pub open: bool,
    pub root: PathBuf,
    pub collapsed: std::collections::BTreeSet<PathBuf>,
    pub selected: usize,
    pub filter: String,
    pub show_hidden: bool,
    pub files: usize,
    pub folders: usize,
    pub rows: Vec<Row>,
    tree: Vec<Node>,
}

const TREE_DEPTH: usize = 4;
const TREE_FILE_LIMIT: usize = 2000;

impl BrowserState {
    pub fn new(root: PathBuf) -> Self {
        let mut state = Self {
            open: false,
            root,
            collapsed: std::collections::BTreeSet::new(),
            selected: 0,
            filter: String::new(),
            show_hidden: false,
            files: 0,
            folders: 0,
            rows: Vec::new(),
            tree: Vec::new(),
        };
        state.reload();
        state
    }

    pub fn reload(&mut self) {
        self.files = 0;
        self.folders = 0;
        self.tree = scan_tree(&self.root, 0, self.show_hidden, &mut self.files, &mut self.folders);
        self.rebuild();
    }

    pub fn set_root(&mut self, root: PathBuf) {
        self.root = root;
        self.collapsed.clear();
        self.filter.clear();
        self.selected = 0;
        self.reload();
    }

    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.selected = 0;
        self.rebuild();
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.reload();
    }

    pub fn set_selected(&mut self, selected: usize) {
        if self.rows.is_empty() {
            self.selected = 0;
        } else {
            self.selected = selected.min(self.rows.len() - 1);
        }
    }

    pub fn selected_row(&self) -> Option<&Row> {
        self.rows.get(self.selected)
    }

    pub fn toggle_dir(&mut self, path: &Path) {
        if !self.collapsed.remove(path) {
            self.collapsed.insert(path.to_path_buf());
        }
        self.rebuild();
    }

    pub fn expand(&mut self, path: &Path) {
        self.collapsed.remove(path);
        self.rebuild();
    }

    pub fn collapse(&mut self, path: &Path) {
        self.collapsed.insert(path.to_path_buf());
        self.rebuild();
    }

    pub fn reveal(&mut self, path: &Path) {
        for ancestor in path.ancestors().skip(1) {
            if ancestor.starts_with(&self.root) {
                self.collapsed.remove(ancestor);
            }
        }
        self.rebuild();
        if let Some(index) = self.rows.iter().position(|row| row.path == path) {
            self.selected = index;
        }
    }

    pub fn go_parent(&mut self) {
        if let Some(parent) = self.root.parent().map(Path::to_path_buf) {
            self.set_root(parent);
        }
    }

    fn rebuild(&mut self) {
        self.rows.clear();
        if self.filter.trim().is_empty() {
            flatten_tree(&self.tree, 0, &self.collapsed, &mut self.rows);
        } else {
            let needle = self.filter.trim().to_lowercase();
            collect_search(&self.tree, "", &needle, &mut self.rows);
        }
        if self.rows.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.rows.len() {
            self.selected = self.rows.len() - 1;
        }
    }
}

fn scan_tree(
    dir: &Path,
    depth: usize,
    show_hidden: bool,
    files: &mut usize,
    folders: &mut usize,
) -> Vec<Node> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<Node> = Vec::new();
    let mut docs: Vec<Node> = Vec::new();
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        let Ok(kind) = entry.file_type() else { continue };
        let path = entry.path();
        if kind.is_dir() {
            if skip_dir(&name) || depth >= TREE_DEPTH {
                continue;
            }
            let children = scan_tree(&path, depth + 1, show_hidden, files, folders);
            if children.is_empty() {
                continue;
            }
            *folders += 1;
            dirs.push(Node { path, name, dir: true, size: String::new(), children });
        } else if kind.is_file() && is_markdown(&path) {
            if *files >= TREE_FILE_LIMIT {
                continue;
            }
            *files += 1;
            let size = entry.metadata().ok().map(|meta| human_size(meta.len())).unwrap_or_default();
            docs.push(Node { path, name, dir: false, size, children: Vec::new() });
        }
    }
    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    docs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    dirs.extend(docs);
    dirs
}

fn flatten_tree(nodes: &[Node], depth: usize, collapsed: &std::collections::BTreeSet<PathBuf>, out: &mut Vec<Row>) {
    for node in nodes {
        let open = node.dir && !collapsed.contains(&node.path);
        out.push(Row {
            path: node.path.clone(),
            name: node.name.clone(),
            depth,
            dir: node.dir,
            open,
            size: node.size.clone(),
            parent: String::new(),
            search: false,
        });
        if open {
            flatten_tree(&node.children, depth + 1, collapsed, out);
        }
    }
}

fn collect_search(nodes: &[Node], base: &str, needle: &str, out: &mut Vec<Row>) {
    for node in nodes {
        let hay = format!("{base}/{}", node.name).to_lowercase();
        if hay.contains(needle) {
            out.push(Row {
                path: node.path.clone(),
                name: node.name.clone(),
                depth: 0,
                dir: node.dir,
                open: false,
                size: node.size.clone(),
                parent: base.to_owned(),
                search: true,
            });
            if out.len() >= 400 {
                return;
            }
        }
        if !node.children.is_empty() {
            let next = if base.is_empty() { node.name.clone() } else { format!("{base}/{}", node.name) };
            collect_search(&node.children, &next, needle, out);
            if out.len() >= 400 {
                return;
            }
        }
    }
}

fn skip_dir(name: &str) -> bool {
    matches!(
        name,
        "target"
            | "node_modules"
            | "dist"
            | "build"
            | ".git"
            | ".svn"
            | ".hg"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".next"
            | ".cache"
            | ".idea"
            | ".vscode"
    )
}

pub struct Toast {
    pub text: String,
    pub born: Instant,
}

/// A clickable region published by the frame and consumed by the mouse handler.
#[derive(Clone, Debug)]
pub enum Hit {
    Tab(usize),
    TabClose(usize),
    Theme,
    HeaderPath,
    HeaderOpen,
    HeaderSettings,
    HeaderEdit,
    HeaderShare,
    EditorSave,
    ProgressBar,
    Outline(usize),
    OutlinePanel,
    WelcomeFile(PathBuf),
    WelcomeOpen,
    WelcomeExplorer,
    WelcomeHelp,
    WelcomeQuit,
    PaletteRow(usize),
    BrowserRow(usize),
    BrowserPanel,
    BrowserUp,
    BrowserRefresh,
    BrowserSplitter,
    OutlineSplitter,
    EditorDivider,
    BrowserCrumb(PathBuf),
    BrowserHidden,
    SettingRow(usize),
    SettingLeft(usize),
    SettingRight(usize),
    SettingReset,
    SettingDefault,
    SettingsClose,
    ShareRow(usize),
    ShareClose,
    NewTab,
    CopyCode(String),
    EditorArea,
    EditorFormat(usize),
    EditorPreview,
    ToggleTask(usize),
    ConfirmChoice(usize),
    FindPrev,
    FindNext,
    FindClose,
    FindCase,
    FindWord,
    StatusHelp,
    StatusQuit,
    Panel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    None,
    Find,
    Palette,
    Browser,
    Help,
    Settings,
    Share,
    Confirm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmAction {
    Quit,
    CloseTab(usize),
}

#[derive(Clone, Debug)]
pub struct ConfirmState {
    pub action: ConfirmAction,
    pub selected: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct EditorGeom {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drag {
    Browser,
    Outline,
    Editor,
}

pub struct App {
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub theme: ThemeKind,
    pub outline_open: bool,
    pub outline_focus: bool,
    pub outline_sel: usize,
    pub browser_focus: bool,
    pub overlay: Overlay,
    pub find: FindState,
    pub palette: PaletteState,
    pub browser: BrowserState,
    pub quit: bool,
    pub toast: Option<Toast>,
    pub motion: bool,
    pub enter: f32,
    pub welcome_files: Vec<(String, String)>,
    pub hits: Vec<(Rect, Hit)>,
    pub pick_files: bool,
    pub prefs: Prefs,
    pub settings_sel: usize,
    pub share_sel: usize,
    pub positions: std::collections::HashMap<String, usize>,
    pub closed: Vec<PathBuf>,
    pub confirm: Option<ConfirmState>,
    pub editor_geom: Option<EditorGeom>,
    pub editor_width: usize,
    pub editor_height: usize,
    pub drag: Option<Drag>,
    pub hover: Option<(u16, u16)>,
    pub body_rect: Rect,
    pub editor_inner: Option<Rect>,
    pub default_registered: bool,
    last: Instant,
}

impl App {
    pub fn new(paths: Vec<PathBuf>, prefs: Prefs, restore: bool) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let initial_browser = if prefs.browser_root.is_empty() {
            cwd.clone()
        } else {
            let saved = PathBuf::from(&prefs.browser_root);
            if saved.is_dir() {
                saved
            } else {
                cwd.clone()
            }
        };
        let welcome_files = scan_markdown(&cwd);
        let saved = session::load();
        let theme = prefs.resolved_theme();
        let outline_open = !prefs.outline_hidden;
        let motion = prefs.motion;
        let mut app = Self {
            tabs: Vec::new(),
            active: 0,
            theme,
            outline_open,
            outline_focus: false,
            outline_sel: 0,
            browser_focus: false,
            overlay: Overlay::None,
            find: FindState::default(),
            palette: PaletteState::default(),
            browser: BrowserState::new(initial_browser),
            quit: false,
            toast: None,
            motion,
            enter: if motion { 0.0 } else { 1.0 },
            welcome_files,
            hits: Vec::new(),
            pick_files: false,
            settings_sel: 0,
            share_sel: 0,
            positions: saved.positions.clone(),
            closed: Vec::new(),
            confirm: None,
            editor_geom: None,
            editor_width: 80,
            editor_height: 20,
            drag: None,
            hover: None,
            body_rect: Rect::new(0, 0, 80, 24),
            editor_inner: None,
            default_registered: crate::assoc::is_registered(),
            prefs,
            last: Instant::now(),
        };

        let mut initial = paths;
        if initial.is_empty() && restore && app.prefs.restore_session {
            initial = saved.tabs.iter().map(|tab| PathBuf::from(&tab.path)).collect();
        }
        let mut was_first = true;
        for path in initial {
            if let Err(error) = app.open_path_internal(&path, was_first) {
                app.notify(error);
            }
            was_first = false;
        }
        for (index, tab) in app.tabs.iter_mut().enumerate() {
            if let Some(saved_tab) = saved.tabs.get(index) {
                let position = saved_tab.scroll as f32;
                if position > 0.0 {
                    tab.scroll = position;
                    tab.target = position;
                }
            }
        }
        app
    }

    pub fn palette(&self) -> Palette {
        Palette::of(self.theme)
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    pub fn open_path(&mut self, path: &Path) {
        match self.open_path_internal(path, true) {
            Ok(()) => {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                self.notify(format!("Opened {name}"));
            }
            Err(error) => self.notify(error),
        }
    }

    fn open_path_internal(&mut self, path: &Path, _first: bool) -> Result<(), String> {
        let path = absolute(path);
        if let Some(index) = self.tabs.iter().position(|tab| tab.path == path) {
            self.remember_position();
            self.active = index;
            self.remember_recent(&path);
            return Ok(());
        }
        self.remember_position();
        let mut tab = Tab::open(&path).map_err(|error| format!("Could not open {}: {error}", path.display()))?;
        if let Some(position) = self.positions.get(&path.to_string_lossy().to_string()) {
            tab.scroll = *position as f32;
            tab.target = *position as f32;
        }
        if self.tabs.get(self.active).is_some_and(|current| current.start) {
            self.tabs[self.active] = tab;
        } else {
            self.tabs.push(tab);
            self.active = self.tabs.len() - 1;
        }
        self.outline_sel = 0;
        self.find.matches.clear();
        self.find.current = 0;
        let key = path.to_string_lossy().to_string();
        self.prefs.recent.retain(|entry| entry != &key);
        self.prefs.recent.insert(0, key);
        self.prefs.recent.truncate(8);
        if let Some(parent) = path.parent() {
            self.prefs.browser_root = parent.to_string_lossy().to_string();
        }
        prefs::save(&self.prefs);
        Ok(())
    }

    fn remember_recent(&mut self, path: &Path) {
        let key = path.to_string_lossy().to_string();
        self.prefs.recent.retain(|entry| entry != &key);
        self.prefs.recent.insert(0, key);
        self.prefs.recent.truncate(8);
        if let Some(parent) = path.parent() {
            self.prefs.browser_root = parent.to_string_lossy().to_string();
        }
        prefs::save(&self.prefs);
    }

    fn remember_browser_root(&mut self) {
        self.prefs.browser_root = self.browser.root.to_string_lossy().to_string();
        prefs::save(&self.prefs);
    }

    pub fn new_tab(&mut self) {
        self.remember_position();
        self.tabs.push(Tab::start());
        self.active = self.tabs.len() - 1;
        self.outline_sel = 0;
        self.find.matches.clear();
        self.find.current = 0;
        self.browser_focus = false;
        self.overlay = Overlay::None;
    }

    pub fn remember_position(&mut self) {
        if let Some(tab) = self.tabs.get(self.active) {
            if tab.start {
                return;
            }
            let key = tab.path.to_string_lossy().to_string();
            let position = tab.scroll.round().max(0.0) as usize;
            self.positions.insert(key, position);
        }
    }

    pub fn session(&mut self) -> Session {
        self.remember_position();
        let mut tabs = Vec::new();
        let mut active = 0usize;
        for (index, tab) in self.tabs.iter().enumerate() {
            if tab.start {
                continue;
            }
            if index == self.active {
                active = tabs.len();
            }
            tabs.push(SessionTab {
                path: tab.path.to_string_lossy().to_string(),
                scroll: tab.scroll.round().max(0.0) as usize,
            });
        }
        if active >= tabs.len() {
            active = tabs.len().saturating_sub(1);
        }
        Session { tabs, active, positions: self.positions.clone() }
    }

    pub fn close_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        if self.tabs.get(self.active).is_some_and(Tab::is_dirty) {
            self.confirm = Some(ConfirmState { action: ConfirmAction::CloseTab(self.active), selected: 0 });
            self.overlay = Overlay::Confirm;
            return;
        }
        self.close_tab_now(self.active);
    }

    fn close_tab_now(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        let path = self.tabs[index].path.clone();
        let start = self.tabs[index].start;
        if !start {
            self.closed.push(path);
        }
        self.tabs.remove(index);
        if self.active > index {
            self.active -= 1;
        } else if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }
        self.find.matches.clear();
        self.find.current = 0;
    }

    pub fn close_tab_at(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        if self.tabs[index].is_dirty() {
            self.confirm = Some(ConfirmState { action: ConfirmAction::CloseTab(index), selected: 0 });
            self.overlay = Overlay::Confirm;
            return;
        }
        self.close_tab_now(index);
    }

    pub fn notify(&mut self, text: impl Into<String>) {
        self.toast = Some(Toast { text: text.into(), born: Instant::now() });
    }

    pub fn set_theme(&mut self, theme: ThemeKind) {
        if self.theme == theme {
            return;
        }
        self.theme = theme;
        self.prefs.theme = theme.name().to_owned();
        prefs::save(&self.prefs);
        self.invalidate();
        let name = theme.name();
        self.notify(format!("Theme: {name}"));
    }

    fn toggle_outline(&mut self) {
        self.outline_open = !self.outline_open;
        self.prefs.outline_hidden = !self.outline_open;
        prefs::save(&self.prefs);
        if !self.outline_open {
            self.outline_focus = false;
        }
    }

    fn invalidate(&mut self) {
        for tab in &mut self.tabs {
            tab.width = 0;
        }
        self.find.matches.clear();
    }

    fn open_settings(&mut self) {
        self.settings_sel = 0;
        self.overlay = Overlay::Settings;
    }

    fn settings_key(&mut self, key: KeyEvent) {
        let count = Setting::ALL.len();
        let setting = Setting::ALL[self.settings_sel];
        match key.code {
            KeyCode::Esc => self.overlay = Overlay::None,
            KeyCode::Up => self.settings_sel = (self.settings_sel + count - 1) % count,
            KeyCode::Down => self.settings_sel = (self.settings_sel + 1) % count,
            KeyCode::Left if setting != Setting::Reset && setting != Setting::DefaultHandler => {
                self.change_setting(-1)
            }
            KeyCode::Right if setting != Setting::Reset && setting != Setting::DefaultHandler => {
                self.change_setting(1)
            }
            KeyCode::Enter if setting == Setting::Reset => self.reset_settings(),
            KeyCode::Enter if setting == Setting::DefaultHandler => self.make_default(),
            KeyCode::Enter => self.change_setting(1),
            _ => {}
        }
    }

    fn reset_settings(&mut self) {
        self.prefs = Prefs::default();
        self.theme = self.prefs.resolved_theme();
        self.motion = self.prefs.motion;
        self.outline_open = !self.prefs.outline_hidden;
        self.invalidate();
        prefs::save(&self.prefs);
        self.notify("Settings reset");
    }

    fn make_default(&mut self) {
        match crate::assoc::register() {
            Ok(()) => {
                self.default_registered = true;
                let _ = crate::assoc::open_default_apps();
                self.notify("Registered — confirm lmdv-tui in Default Apps");
            }
            Err(error) => self.notify(format!("Registration failed: {error}")),
        }
    }

    pub fn export_html(&mut self) {
        let Some(index) = self.tabs.get(self.active).map(|_| self.active) else { return };
        let path = self.tabs[index].path.with_extension("html");
        let title = self.tabs[index]
            .doc
            .title()
            .map(str::to_owned)
            .unwrap_or_else(|| self.tabs[index].name.clone());
        let source = if let Some(editor) = &self.tabs[index].editor {
            editor.text()
        } else {
            match std::fs::read_to_string(&self.tabs[index].path) {
                Ok(source) => source,
                Err(error) => {
                    self.notify(format!("Export failed: {error}"));
                    return;
                }
            }
        };
        let body = md::to_html(&source);
        let page = format!(
            "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{title}</title><style>{EXPORT_CSS}</style></head><body><main>{body}</main></body></html>\n"
        );
        match std::fs::write(&path, page) {
            Ok(()) => {
                let name = path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
                self.notify(format!("Exported {name}"));
            }
            Err(error) => self.notify(format!("Export failed: {error}")),
        }
    }

    pub fn copy_path(&mut self) {
        let Some(text) = self.active_tab().map(|tab| tab.path.display().to_string()) else { return };
        match copy_to_clipboard(&text) {
            Ok(()) => self.notify("Copied file path"),
            Err(error) => self.notify(format!("Copy failed: {error}")),
        }
    }

    pub fn copy_text(&mut self) {
        let Some(source) = self.active_source() else {
            self.notify("Copy failed: cannot read the file");
            return;
        };
        match copy_to_clipboard(&source) {
            Ok(()) => self.notify("Copied document text"),
            Err(error) => self.notify(format!("Copy failed: {error}")),
        }
    }

    pub fn copy_code(&mut self, code: &str) {
        match copy_to_clipboard(code) {
            Ok(()) => self.notify("Copied code"),
            Err(error) => self.notify(format!("Copy failed: {error}")),
        }
    }

    fn active_source(&self) -> Option<String> {
        let tab = self.active_tab()?;
        if let Some(editor) = &tab.editor {
            return Some(editor.text());
        }
        std::fs::read_to_string(&tab.path).ok()
    }

    fn open_share(&mut self) {
        self.share_sel = 0;
        self.overlay = Overlay::Share;
    }

    fn apply_editor_format(&mut self, format: Format) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            if tab.mode != Mode::Editor {
                return;
            }
            if let Some(editor) = tab.editor.as_mut() {
                editor.apply_format(format);
            }
        }
    }

    fn editor_line_op(&mut self, op: usize) {
        let Some(tab) = self.tabs.get_mut(self.active) else { return };
        if tab.mode != Mode::Editor {
            return;
        }
        if let Some(editor) = tab.editor.as_mut() {
            match op {
                0 => editor.move_line(true),
                1 => editor.move_line(false),
                2 => editor.duplicate_line(),
                _ => editor.delete_line(),
            }
        }
    }

    fn share_key(&mut self, key: KeyEvent) {
        let count = SHARE_ITEMS.len();
        match key.code {
            KeyCode::Esc => self.overlay = Overlay::None,
            KeyCode::Up => self.share_sel = (self.share_sel + count - 1) % count,
            KeyCode::Down => self.share_sel = (self.share_sel + 1) % count,
            KeyCode::Enter => {
                let command = SHARE_ITEMS[self.share_sel].2;
                if let Some(command) = command {
                    self.overlay = Overlay::None;
                    self.run_command(command);
                }
            }
            _ => {}
        }
    }

    pub fn copy_link(&mut self) {
        let Some((name, target)) = self.active_tab().map(|tab| {
            let name = tab.name.clone();
            let target = tab
                .path
                .file_name()
                .map(|file| file.to_string_lossy().to_string())
                .unwrap_or_else(|| name.clone());
            (name, target)
        }) else {
            return;
        };
        match copy_to_clipboard(&format!("[{name}]({target})")) {
            Ok(()) => self.notify("Copied Markdown link"),
            Err(error) => self.notify(format!("Copy failed: {error}")),
        }
    }

    pub fn change_setting(&mut self, direction: isize) {
        let setting = Setting::ALL[self.settings_sel];
        if setting == Setting::DefaultHandler {
            return;
        }
        if setting == Setting::Reset {
            self.prefs = Prefs::default();
        } else {
            self.prefs.cycle(setting.key(), direction);
            if setting == Setting::SideSize {
                self.prefs.outline_cols = match self.prefs.side_size.as_str() {
                    "small" => 22,
                    "large" => 34,
                    _ => 28,
                };
            }
        }
        self.theme = self.prefs.resolved_theme();
        self.motion = self.prefs.motion;
        self.outline_open = !self.prefs.outline_hidden;
        self.invalidate();
        prefs::save(&self.prefs);
    }

    pub fn toggle_editor(&mut self) {
        let Some(tab) = self.tabs.get(self.active) else { return };
        if tab.start {
            return;
        }
        if tab.mode == Mode::Editor {
            self.tabs[self.active].mode = Mode::Reader;
            return;
        }
        if self.tabs[self.active].editor.is_none() {
            let path = self.tabs[self.active].path.clone();
            match Editor::open(&path) {
                Ok(editor) => self.tabs[self.active].editor = Some(editor),
                Err(error) => {
                    self.notify(format!("Editor failed: {error}"));
                    return;
                }
            }
        }
        self.tabs[self.active].mode = Mode::Editor;
        self.tabs[self.active].preview_rev = u64::MAX;
        if let Some(editor) = self.tabs[self.active].editor.as_mut() {
            editor.preview = true;
        }
        self.overlay = Overlay::None;
    }

    fn save_tab(&mut self, index: usize) -> Result<(), String> {
        let Some(tab) = self.tabs.get_mut(index) else { return Ok(()) };
        let Some(editor) = tab.editor.as_mut() else { return Ok(()) };
        let path = tab.path.clone();
        editor.save(&path).map_err(|error| format!("Save failed: {error}"))?;
        let text = editor.text();
        tab.doc = md::parse(&text);
        tab.width = 0;
        tab.preview_rev = u64::MAX;
        Ok(())
    }

    fn save_all(&mut self) -> Result<(), String> {
        for index in 0..self.tabs.len() {
            if self.tabs[index].is_dirty() {
                self.save_tab(index)?;
            }
        }
        Ok(())
    }

    pub fn save_active(&mut self) {
        match self.save_tab(self.active) {
            Ok(()) => self.notify("Saved"),
            Err(error) => self.notify(error),
        }
    }

    fn toggle_preview(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            if let Some(editor) = tab.editor.as_mut() {
                editor.preview = !editor.preview;
            }
        }
    }

    fn editor_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => {
                self.toggle_editor();
                return;
            }
            KeyCode::Char('e') if ctrl => {
                self.toggle_editor();
                return;
            }
            KeyCode::Char('s') if ctrl => {
                self.save_active();
                return;
            }
            KeyCode::Char('p') if ctrl => {
                self.open_palette();
                return;
            }
            KeyCode::Char('o') if ctrl => {
                self.pick_files = true;
                return;
            }
            KeyCode::Char(',') if ctrl => {
                self.open_settings();
                return;
            }
            _ => {}
        }
        let width = self.editor_width.max(8);
        let height = self.editor_height.max(1);
        let Some(tab) = self.tabs.get_mut(self.active) else { return };
        let Some(editor) = tab.editor.as_mut() else { return };
        let rows = editor.visual_rows(width);
        match key.code {
            KeyCode::Char('z') if ctrl => editor.undo(),
            KeyCode::Char('y') if ctrl => editor.redo(),
            KeyCode::Char('b') if ctrl => editor.apply_format(Format::Bold),
            KeyCode::Char('i') if ctrl => editor.apply_format(Format::Italic),
            KeyCode::Char('k') if ctrl && key.modifiers.contains(KeyModifiers::SHIFT) => editor.delete_line(),
            KeyCode::Char('k') if ctrl => editor.apply_format(Format::Link),
            KeyCode::Char('d') if ctrl => editor.duplicate_line(),
            KeyCode::Char('3') if ctrl => editor.apply_format(Format::H3),
            KeyCode::Char('0') if ctrl => editor.apply_format(Format::Paragraph),
            KeyCode::Enter => editor.newline(),
            KeyCode::Tab => editor.indent(),
            KeyCode::Backspace => editor.backspace(),
            KeyCode::Delete => editor.delete(),
            KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => editor.move_line(true),
            KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => editor.move_line(false),
            KeyCode::Left if ctrl => editor.move_word(-1),
            KeyCode::Right if ctrl => editor.move_word(1),
            KeyCode::Left => editor.move_left(),
            KeyCode::Right => editor.move_right(),
            KeyCode::Up => editor.move_vertical(&rows, -1),
            KeyCode::Down => editor.move_vertical(&rows, 1),
            KeyCode::PageUp => editor.page(&rows, -1, height),
            KeyCode::PageDown => editor.page(&rows, 1, height),
            KeyCode::Home if ctrl => editor.move_doc(false),
            KeyCode::End if ctrl => editor.move_doc(true),
            KeyCode::Home => editor.move_home(),
            KeyCode::End => editor.move_end(),
            KeyCode::F(2) => editor.preview = !editor.preview,
            KeyCode::Char(ch) if !ctrl => editor.insert_char(ch),
            _ => {}
        }
        let rows = editor.visual_rows(width);
        editor.ensure_visible(&rows, height);
    }

    fn editor_click(&mut self, x: u16, y: u16) {        let Some(geom) = self.editor_geom else { return };
        if x < geom.x
            || x >= geom.x.saturating_add(geom.width)
            || y < geom.y
            || y >= geom.y.saturating_add(geom.height)
        {
            return;
        }
        let width = self.editor_width.max(8);
        let height = geom.height as usize;
        let Some(tab) = self.tabs.get_mut(self.active) else { return };
        let Some(editor) = tab.editor.as_mut() else { return };
        let rows = editor.visual_rows(width);
        let row_index = editor.scroll + (y - geom.y) as usize;
        editor.click(&rows, row_index, (x - geom.x) as usize);
        let rows = editor.visual_rows(width);
        editor.ensure_visible(&rows, height);
    }

    fn toggle_task(&mut self, line: usize) {
        let index = self.active;
        let Some(tab) = self.tabs.get(index) else { return };
        if tab.editor.as_ref().is_some_and(|editor| editor.dirty) {
            self.notify("Save or discard editor changes first");
            return;
        }
        let path = tab.path.clone();
        let Ok(text) = std::fs::read_to_string(&path) else {
            self.notify("Task update failed: cannot read the file");
            return;
        };
        let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_owned).collect();
        if line >= lines.len() {
            return;
        }
        let target = &mut lines[line];
        let (Some(open), Some(close)) = (target.find('['), target.find(']')) else { return };
        if close != open + 2 {
            return;
        }
        let mark = target.as_bytes()[open + 1];
        let new_mark = if mark == b'x' || mark == b'X' { ' ' } else { 'x' };
        target.replace_range(open + 1..open + 2, &new_mark.to_string());
        let updated = lines.concat();
        match std::fs::write(&path, updated) {
            Ok(()) => {
                if let Some(tab) = self.tabs.get_mut(index) {
                    tab.width = 0;
                    if tab.editor.is_some() {
                        tab.editor = crate::editor::Editor::open(&tab.path).ok();
                    }
                    let _ = tab.reload();
                }
                self.notify(if new_mark == 'x' { "Task checked" } else { "Task unchecked" });
            }
            Err(error) => self.notify(format!("Task update failed: {error}")),
        }
    }

    fn request_quit(&mut self) {
        if self.tabs.iter().any(|tab| tab.is_dirty()) {
            self.confirm = Some(ConfirmState { action: ConfirmAction::Quit, selected: 0 });
            self.overlay = Overlay::Confirm;
        } else {
            self.quit = true;
        }
    }

    fn confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.overlay = Overlay::None;
                self.confirm = None;
            }
            KeyCode::Left => {
                if let Some(confirm) = self.confirm.as_mut() {
                    confirm.selected = (confirm.selected + 2) % 3;
                }
            }
            KeyCode::Right => {
                if let Some(confirm) = self.confirm.as_mut() {
                    confirm.selected = (confirm.selected + 1) % 3;
                }
            }
            KeyCode::Char('s') => self.confirm_choose(0),
            KeyCode::Char('d') => self.confirm_choose(1),
            KeyCode::Enter => {
                let choice = self.confirm.as_ref().map_or(0, |confirm| confirm.selected);
                self.confirm_choose(choice);
            }
            _ => {}
        }
    }

    fn confirm_choose(&mut self, choice: usize) {
        let Some(state) = self.confirm.take() else {
            self.overlay = Overlay::None;
            return;
        };
        if choice == 2 {
            self.overlay = Overlay::None;
            return;
        }
        if choice == 0 {
            let result = match state.action {
                ConfirmAction::Quit => self.save_all(),
                ConfirmAction::CloseTab(index) => self.save_tab(index),
            };
            if let Err(error) = result {
                self.notify(error);
                self.confirm = Some(state);
                return;
            }
        }
        self.overlay = Overlay::None;
        match state.action {
            ConfirmAction::Quit => self.quit = true,
            ConfirmAction::CloseTab(index) => self.close_tab_now(index),
        }
    }

    /// Relayout and clamp before a frame is drawn.
    pub fn prepare(&mut self, width: usize, height: usize, p: &Palette) {
        let spacing = self.prefs.spacing();
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        tab.viewport = height.max(1);
        tab.relayout(width, p, spacing);
        tab.max_scroll = tab.lines.len().saturating_sub(tab.viewport) as f32;
        tab.target = tab.target.clamp(0.0, tab.max_scroll);
        tab.scroll = tab.scroll.clamp(0.0, tab.max_scroll);
        tab.section = section_at(&tab.lines, tab.scroll.round() as usize);

        if !self.find.query.is_empty() {
            let key = (
                self.active,
                tab.rev,
                self.find.query.clone(),
                self.find.case_sensitive,
                self.find.whole_word,
            );
            if self.find.cache.as_ref() != Some(&key) {
                let query = self.find.query.clone();
                let case = self.find.case_sensitive;
                let word = self.find.whole_word;
                let matches: Vec<usize> = tab
                    .lines
                    .iter()
                    .enumerate()
                    .filter(|(_, line)| find_matches(&line.plain, &query, case, word))
                    .map(|(index, _)| index)
                    .collect();
                self.find.matches = matches;
                self.find.current = self.find.current.min(self.find.matches.len().saturating_sub(1));
                self.find.cache = Some(key);
            }
        }
    }

    /// Advance the animations; returns true while a frame is still moving.
    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;
        let mut moving = false;

        if self.motion && self.enter < 1.0 {
            self.enter = (self.enter + dt / 0.26).min(1.0);
            moving |= self.enter < 1.0;
        } else {
            self.enter = 1.0;
        }

        if let Some(tab) = self.tabs.get_mut(self.active) {
            if self.motion {
                let mut delta = tab.target - tab.scroll;
                if delta.abs() > 2.5 {
                    tab.scroll = tab.target - 2.5 * delta.signum();
                    delta = tab.target - tab.scroll;
                }
                if delta.abs() > 0.05 {
                    let ease = 1.0 - (-dt * 55.0).exp();
                    tab.scroll += delta * ease;
                    moving = true;
                } else {
                    tab.scroll = tab.target;
                }
            } else {
                tab.scroll = tab.target;
            }
            if let Some((_, born)) = tab.flash {
                if born.elapsed() > Duration::from_millis(520) {
                    tab.flash = None;
                } else {
                    moving = true;
                }
            }
        }
        if let Some(toast) = &self.toast {
            if toast.born.elapsed() > Duration::from_secs(3) {
                self.toast = None;
            } else {
                moving = true;
            }
        }
        moving
    }

    pub fn scroll_by(&mut self, delta: f32) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.target = (tab.target + delta).clamp(0.0, tab.max_scroll);
        }
    }

    pub fn wheel_burst(&mut self, x: u16, y: u16, notches: i32) {
        if notches == 0 {
            return;
        }
        if notches.abs() == 1 {
            self.wheel(x, y, notches.signum() as isize);
            return;
        }
        if self.active_tab().is_some_and(|tab| tab.mode == Mode::Editor) {
            let width = self.editor_width.max(8);
            let height = self.editor_height.max(1);
            if let Some(tab) = self.tabs.get_mut(self.active) {
                if let Some(editor) = tab.editor.as_mut() {
                    let rows = editor.visual_rows(width);
                    editor.scroll_by(&rows, notches as isize * 2, height);
                }
            }
            return;
        }
        match self.hit_at(x, y) {
            Some((_, Hit::Outline(_))) | Some((_, Hit::OutlinePanel)) => {
                self.outline_focus = true;
                self.outline_move(notches as isize * 2);
                return;
            }
            Some((_, Hit::BrowserPanel)) | Some((_, Hit::BrowserRow(_))) => {
                self.browser_focus = true;
                let count = self.browser.rows.len();
                if count > 0 {
                    let next = (self.browser.selected as isize + notches as isize)
                        .clamp(0, count as isize - 1);
                    self.browser.set_selected(next as usize);
                }
                return;
            }
            _ => {}
        }
        if let Some(tab) = self.tabs.get_mut(self.active) {
            let delta = notches as f32 * 2.0;
            tab.scroll = (tab.scroll + delta).clamp(0.0, tab.max_scroll);
            tab.target = tab.scroll;
        }
    }

    pub fn wheel_by(&mut self, delta: f32) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            let ahead = 4.0;
            let bounded = if delta > 0.0 {
                (tab.target + delta).min(tab.scroll + ahead)
            } else {
                (tab.target + delta).max(tab.scroll - ahead)
            };
            tab.target = bounded.clamp(0.0, tab.max_scroll);
        }
    }

    pub fn scroll_to(&mut self, line: f32) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.target = line.clamp(0.0, tab.max_scroll);
        }
    }

    pub fn half_page(&self) -> f32 {
        self.active_tab().map_or(10.0, |tab| (tab.viewport as f32 / 2.0).max(1.0))
    }

    pub fn page(&self) -> f32 {
        self.active_tab().map_or(20.0, |tab| (tab.viewport as f32 - 2.0).max(1.0))
    }

    pub fn progress(&self) -> f32 {
        self.active_tab().map_or(0.0, |tab| {
            if tab.start {
                0.0
            } else if tab.mode == Mode::Editor {
                tab.editor.as_ref().map_or(0.0, |editor| {
                    if editor.lines.is_empty() {
                        0.0
                    } else {
                        (editor.cursor.0 as f32 / (editor.lines.len() - 1).max(1) as f32).clamp(0.0, 1.0)
                    }
                })
            } else if tab.max_scroll <= 0.0 {
                1.0
            } else {
                (tab.scroll / tab.max_scroll).clamp(0.0, 1.0)
            }
        })
    }

    pub fn current_section(&self) -> Option<(usize, usize, String)> {
        let tab = self.active_tab()?;
        let total = tab.doc.outline.len();
        let entry = tab.doc.outline.get(tab.section)?;
        Some((tab.section, total, entry.title.clone()))
    }

    pub fn jump_match(&mut self, next: bool) {
        if self.find.matches.is_empty() {
            return;
        }
        if next {
            self.find.current = (self.find.current + 1) % self.find.matches.len();
        } else {
            self.find.current = if self.find.current == 0 {
                self.find.matches.len() - 1
            } else {
                self.find.current - 1
            };
        }
        let line = self.find.matches[self.find.current];
        self.scroll_to((line.saturating_sub(2)) as f32);
    }

    pub fn start_find(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.overlay = Overlay::Find;
        self.find.open = true;
        self.find.matches.clear();
        self.find.current = 0;
    }

    pub fn close_find(&mut self) {
        self.overlay = Overlay::None;
        self.find.open = false;
    }

    pub fn open_browser(&mut self) {
        let saved = if self.prefs.browser_root.is_empty() {
            None
        } else {
            let path = PathBuf::from(&self.prefs.browser_root);
            path.is_dir().then_some(path)
        };
        let active_parent = self
            .active_tab()
            .and_then(|tab| tab.path.parent().map(Path::to_path_buf));
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if let Some(root) = saved.or(active_parent).or(Some(cwd)) {
            self.browser.root = root;
        }
        self.browser.filter.clear();
        self.browser.reload();
        if let Some(path) = self.active_tab().map(|tab| tab.path.clone()) {
            if path.starts_with(&self.browser.root) {
                self.browser.reveal(&path);
            }
        }
        self.browser.open = true;
        self.browser_focus = true;
        self.overlay = Overlay::None;
    }

    pub fn open_palette(&mut self) {
        self.palette.query.clear();
        self.palette.selected = 0;
        self.palette.refresh();
        self.overlay = Overlay::Palette;
    }

    pub fn run_command(&mut self, id: &str) {
        match id {
            "theme.black" => self.set_theme(ThemeKind::Black),
            "theme.dark" => self.set_theme(ThemeKind::Dark),
            "theme.light" => self.set_theme(ThemeKind::Light),
            "theme.sepia" => self.set_theme(ThemeKind::Sepia),
            "view.outline" => self.toggle_outline(),
            "edit.toggle" => self.toggle_editor(),
            "file.open" => self.open_browser(),
            "file.open-explorer" => self.pick_files = true,
            "edit.find" => self.start_find(),
            "share.export-html" => self.export_html(),
            "share.copy-path" => self.copy_path(),
            "share.copy-link" => self.copy_link(),
            "share.copy-text" => self.copy_text(),
            "edit.strike" => self.apply_editor_format(Format::Strike),
            "edit.heading-3" => self.apply_editor_format(Format::H3),
            "edit.paragraph" => self.apply_editor_format(Format::Paragraph),
            "edit.list-ordered" => self.apply_editor_format(Format::OrderedList),
            "edit.insert-table" => self.apply_editor_format(Format::Table),
            "edit.insert-math" => self.apply_editor_format(Format::Math),
            "edit.insert-divider" => self.apply_editor_format(Format::Divider),
            "edit.insert-note" => self.apply_editor_format(Format::Note),
            "edit.insert-tip" => self.apply_editor_format(Format::Tip),
            "edit.insert-warning" => self.apply_editor_format(Format::Warning),
            "edit.move-line-up" => self.editor_line_op(0),
            "edit.move-line-down" => self.editor_line_op(1),
            "edit.duplicate-line" => self.editor_line_op(2),
            "edit.delete-line" => self.editor_line_op(3),
            "app.share" => self.open_share(),
            "share.toggle-break-h1" => {
                self.prefs.print_break_h1 = !self.prefs.print_break_h1;
                prefs::save(&self.prefs);
                let state = if self.prefs.print_break_h1 { "on" } else { "off" };
                self.notify(format!("New page at H1: {state}"));
            }
            "file.reload" => {
                let result = self.tabs.get_mut(self.active).map(|tab| tab.reload());
                match result {
                    Some(Ok(())) => self.notify("Reloaded from disk"),
                    Some(Err(error)) => self.notify(format!("Reload failed: {error}")),
                    None => {}
                }
            }
            "tab.next" => {
                if !self.tabs.is_empty() {
                    self.remember_position();
                    self.active = (self.active + 1) % self.tabs.len();
                }
            }
            "tab.prev" => {
                if !self.tabs.is_empty() {
                    self.remember_position();
                    self.active = if self.active == 0 { self.tabs.len() - 1 } else { self.active - 1 };
                }
            }
            "tab.close" => self.close_tab(),
            "tab.new" => self.new_tab(),
            "tab.reopen" => {
                if let Some(path) = self.closed.pop() {
                    self.open_path(&path);
                }
            }
            "scroll.top" => self.scroll_to(0.0),
            "scroll.bottom" => {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.target = tab.max_scroll;
                }
            }
            "view.motion" => {
                self.motion = !self.motion;
                self.prefs.motion = self.motion;
                prefs::save(&self.prefs);
                let state = if self.motion { "on" } else { "off" };
                self.notify(format!("Motion: {state}"));
            }
            "app.settings" => self.open_settings(),
            "app.settings-reset" => self.reset_settings(),
            "app.default-handler" => self.make_default(),
            "app.help" => self.overlay = Overlay::Help,
            "app.quit" => self.request_quit(),
            _ => {}
        }
        if id != "file.open" && id != "edit.find" && id != "app.help" && id != "app.settings" && id != "app.share" {
            self.overlay = Overlay::None;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
        {
            self.request_quit();
            return;
        }
        match self.overlay {
            Overlay::Help => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::F(1)) {
                    self.overlay = Overlay::None;
                }
            }
            Overlay::Palette => self.palette_key(key),
            Overlay::Browser => self.browser_key(key),
            Overlay::Find => self.find_key(key),
            Overlay::Settings => self.settings_key(key),
            Overlay::Share => self.share_key(key),
            Overlay::Confirm => self.confirm_key(key),
            Overlay::None => {
                if self.browser.open && self.browser_focus {
                    self.browser_key(key);
                } else if self.active_tab().is_some_and(|tab| tab.mode == Mode::Editor) {
                    self.editor_key(key);
                } else {
                    self.normal_key(key);
                }
            }
        }
    }

    fn normal_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') if !ctrl => self.request_quit(),
            KeyCode::Esc => {
                if self.outline_focus {
                    self.outline_focus = false;
                }
            }
            KeyCode::Char('j') | KeyCode::Down if !ctrl => {
                if self.outline_focus {
                    self.outline_move(1);
                } else {
                    self.scroll_by(1.0);
                }
            }
            KeyCode::Char('k') | KeyCode::Up if !ctrl => {
                if self.outline_focus {
                    self.outline_move(-1);
                } else {
                    self.scroll_by(-1.0);
                }
            }
            KeyCode::Char('d') if !ctrl => self.scroll_by(self.half_page()),
            KeyCode::Char('u') if !ctrl => self.scroll_by(-self.half_page()),
            KeyCode::PageDown | KeyCode::Char(' ') => self.scroll_by(self.page()),
            KeyCode::PageUp => self.scroll_by(-self.page()),
            KeyCode::Home | KeyCode::Char('g') => self.scroll_to(0.0),
            KeyCode::End | KeyCode::Char('G') => {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.target = tab.max_scroll;
                }
            }
            KeyCode::Char('t') if ctrl && key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.run_command("tab.reopen")
            }
            KeyCode::Char('t') if ctrl => self.new_tab(),
            KeyCode::Char('t') if !ctrl => {
                let next = self.theme.next();
                self.set_theme(next);
            }
            KeyCode::Char('1') if !ctrl => self.set_theme(ThemeKind::Black),
            KeyCode::Char('2') if !ctrl => self.set_theme(ThemeKind::Dark),
            KeyCode::Char('3') if !ctrl => self.set_theme(ThemeKind::Light),
            KeyCode::Char('4') if !ctrl => self.set_theme(ThemeKind::Sepia),
            KeyCode::Char('e') if ctrl => self.toggle_editor(),
            KeyCode::Tab | KeyCode::Char('b') => self.toggle_outline(),
            KeyCode::Char('o') if ctrl => self.pick_files = true,
            KeyCode::Char('o') if !ctrl => {
                if self.browser.open {
                    self.browser.open = false;
                    self.browser_focus = false;
                } else {
                    self.open_browser();
                }
            }
            KeyCode::Char('/') if !ctrl => self.start_find(),
            KeyCode::Char('f') if ctrl => self.start_find(),
            KeyCode::Char(',') if ctrl => self.open_settings(),
            KeyCode::Char('p') if ctrl => self.open_palette(),
            KeyCode::Char(':') => self.open_palette(),
            KeyCode::Char('?') | KeyCode::F(1) => self.overlay = Overlay::Help,
            KeyCode::Char('[') => self.run_command("tab.prev"),
            KeyCode::Char(']') => self.run_command("tab.next"),
            KeyCode::Char('x') if !ctrl => self.close_tab(),
            KeyCode::Char('r') if !ctrl => self.run_command("file.reload"),
            KeyCode::Char('m') if !ctrl => self.run_command("view.motion"),
            KeyCode::Char('n') if !ctrl => self.jump_match(true),
            KeyCode::Char('N') => self.jump_match(false),
            KeyCode::Enter => {
                if self.outline_focus {
                    let index = self.outline_index();
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        tab.goto_heading(index);
                    }
                } else {
                    self.outline_focus = true;
                    self.follow_section();
                }
            }
            KeyCode::Left => self.outline_focus = true,
            KeyCode::Right => self.outline_focus = false,
            _ => {}
        }
    }

    fn outline_index(&self) -> usize {
        let Some(tab) = self.active_tab() else { return 0 };
        self.outline_sel.min(tab.doc.outline.len().saturating_sub(1))
    }

    fn outline_move(&mut self, delta: isize) {
        let Some(tab) = self.active_tab() else { return };
        let count = tab.doc.outline.len();
        if count == 0 {
            return;
        }
        let next = self.outline_sel as isize + delta;
        self.outline_sel = next.clamp(0, count as isize - 1) as usize;
    }

    pub fn follow_section(&mut self) {
        let section = self
            .active_tab()
            .map(|tab| tab.section.min(tab.doc.outline.len().saturating_sub(1)));
        if let Some(section) = section {
            self.outline_sel = section;
        }
    }

    fn find_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.close_find(),
            KeyCode::Enter => self.jump_match(true),
            KeyCode::Backspace => {
                self.find.query.pop();
                self.find.matches.clear();
                self.find.current = 0;
            }
            KeyCode::Up => self.jump_match(false),
            KeyCode::Down => self.jump_match(true),
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.find.query.push(ch);
                self.find.matches.clear();
                self.find.current = 0;
            }
            _ => {}
        }
    }

    fn palette_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.overlay = Overlay::None,
            KeyCode::Enter => {
                if let Some(command) = self.palette.chosen() {
                    let id = command.id;
                    self.overlay = Overlay::None;
                    self.run_command(id);
                }
            }
            KeyCode::Up => {
                self.palette.selected = self.palette.selected.saturating_sub(1);
            }
            KeyCode::Down => {
                let count = self.palette.filtered_len();
                if count > 0 {
                    self.palette.selected = (self.palette.selected + 1).min(count - 1);
                }
            }
            KeyCode::Backspace => {
                self.palette.query.pop();
                self.palette.refresh();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.palette.query.push(ch);
                self.palette.selected = 0;
                self.palette.refresh();
            }
            _ => {}
        }
    }

    fn browser_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => {
                self.browser.open = false;
                self.browser_focus = false;
                self.overlay = Overlay::None;
            }
            KeyCode::Up => self.browser.set_selected(self.browser.selected.saturating_sub(1)),
            KeyCode::Down => self.browser.set_selected(self.browser.selected + 1),
            KeyCode::PageUp => self.browser.set_selected(self.browser.selected.saturating_sub(10)),
            KeyCode::PageDown => self.browser.set_selected(self.browser.selected + 10),
            KeyCode::Home => self.browser.set_selected(0),
            KeyCode::End => {
                let last = self.browser.rows.len().saturating_sub(1);
                self.browser.set_selected(last);
            }
            KeyCode::Right => {
                if let Some(row) = self.browser.selected_row().cloned() {
                    if row.dir {
                        self.browser.expand(&row.path);
                    }
                }
            }
            KeyCode::Left => {
                if let Some(row) = self.browser.selected_row().cloned() {
                    if row.dir && row.open {
                        self.browser.collapse(&row.path);
                    } else if let Some(parent) = row.path.parent().map(Path::to_path_buf) {
                        if let Some(index) = self.browser.rows.iter().position(|item| item.path == parent) {
                            self.browser.set_selected(index);
                        }
                    }
                }
            }
            KeyCode::Enter => self.browser_activate(),
            KeyCode::Char('h') if ctrl => self.browser.toggle_hidden(),
            KeyCode::Backspace if self.browser.filter.is_empty() => {
                self.browser.go_parent();
                self.remember_browser_root();
            }
            KeyCode::Backspace => {
                let mut filter = self.browser.filter.clone();
                filter.pop();
                self.browser.set_filter(filter);
            }
            KeyCode::Char(ch) if !ctrl => {
                let mut filter = self.browser.filter.clone();
                filter.push(ch);
                self.browser.set_filter(filter);
            }
            _ => {}
        }
    }

    fn browser_activate(&mut self) {
        let Some(row) = self.browser.selected_row().cloned() else { return };
        if row.dir {
            self.browser.toggle_dir(&row.path);
            return;
        }
        self.browser.open = false;
        self.overlay = Overlay::None;
        self.open_path(&row.path);
    }

    fn hit_at(&self, x: u16, y: u16) -> Option<(Rect, Hit)> {
        self.hits
            .iter()
            .rev()
            .find(|(rect, _)| {
                rect.width > 0
                    && rect.height > 0
                    && x >= rect.x
                    && x < rect.x.saturating_add(rect.width)
                    && y >= rect.y
                    && y < rect.y.saturating_add(rect.height)
            })
            .cloned()
    }

    pub fn set_hover(&mut self, x: u16, y: u16) -> bool {
        let next = Some((x, y));
        if self.hover == next {
            false
        } else {
            self.hover = next;
            true
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => self.click(mouse.column, mouse.row),
            MouseEventKind::Drag(MouseButton::Left) => self.drag_to(mouse.column, mouse.row),
            MouseEventKind::Up(MouseButton::Left) => {
                if self.drag.take().is_some() {
                    prefs::save(&self.prefs);
                }
            }
            MouseEventKind::ScrollDown => self.wheel(mouse.column, mouse.row, 1),
            MouseEventKind::ScrollUp => self.wheel(mouse.column, mouse.row, -1),
            _ => {}
        }
    }

    fn drag_to(&mut self, x: u16, _y: u16) {
        match self.drag {
            Some(Drag::Browser) => {
                let body = self.body_rect;
                let max = body.width.saturating_sub(50).max(18).min(48);
                self.prefs.browser_width = x.saturating_sub(body.x).clamp(18, max);
            }
            Some(Drag::Outline) => {
                let body = self.body_rect;
                let max = body.width.saturating_sub(50).max(18).min(48);
                let right = body.x.saturating_add(body.width);
                self.prefs.outline_cols = right.saturating_sub(x).clamp(18, max);
            }
            Some(Drag::Editor) => {
                if let Some(inner) = self.editor_inner {
                    if inner.width > 0 {
                        let ratio =
                            ((x.saturating_sub(inner.x)) as f32 / inner.width as f32 * 100.0).round() as u16;
                        self.prefs.editor_split = ratio.clamp(30, 75);
                    }
                }
            }
            None => {}
        }
    }

    fn click(&mut self, x: u16, y: u16) {
        if self.overlay == Overlay::Help {
            self.overlay = Overlay::None;
            return;
        }
        let Some((rect, hit)) = self.hit_at(x, y) else {
            self.browser_focus = false;
            if matches!(self.overlay, Overlay::Find | Overlay::Palette | Overlay::Browser) {
                self.overlay = Overlay::None;
                self.find.open = false;
                self.browser.open = false;
            }
            return;
        };
        if !matches!(
            hit,
            Hit::BrowserRow(_)
                | Hit::BrowserPanel
                | Hit::BrowserUp
                | Hit::BrowserRefresh
                | Hit::BrowserCrumb(_)
                | Hit::BrowserHidden
                | Hit::BrowserSplitter
                | Hit::OutlineSplitter
                | Hit::EditorDivider
        ) {
            self.browser_focus = false;
        }
        match hit {
            Hit::Tab(index) => {
                if index < self.tabs.len() {
                    self.remember_position();
                    self.active = index;
                    self.outline_focus = false;
                }
            }
            Hit::TabClose(index) => self.close_tab_at(index),
            Hit::Theme => {
                let next = self.theme.next();
                self.set_theme(next);
            }
            Hit::HeaderSettings => self.open_settings(),
            Hit::HeaderOpen => self.open_browser(),
            Hit::HeaderEdit => self.toggle_editor(),
            Hit::HeaderShare => self.open_share(),
            Hit::EditorSave => self.save_active(),
            Hit::ShareRow(index) => {
                self.share_sel = index.min(SHARE_ITEMS.len().saturating_sub(1));
                if let Some(command) = SHARE_ITEMS.get(self.share_sel).and_then(|item| item.2) {
                    self.overlay = Overlay::None;
                    self.run_command(command);
                }
            }
            Hit::ShareClose => self.overlay = Overlay::None,
            Hit::NewTab => self.new_tab(),
            Hit::CopyCode(code) => self.copy_code(&code),
            Hit::HeaderPath => self.open_browser(),
            Hit::ProgressBar => {
                let fraction = f32::from(x.saturating_sub(rect.x)) / f32::from(rect.width.max(1));
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.target = tab.max_scroll * fraction.clamp(0.0, 1.0);
                }
            }
            Hit::Outline(index) => {
                self.outline_focus = true;
                self.outline_sel = index;
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.goto_heading(index);
                }
            }
            Hit::OutlinePanel => self.outline_focus = true,
            Hit::WelcomeFile(path) => self.open_path(&path),
            Hit::WelcomeOpen => self.open_browser(),
            Hit::WelcomeExplorer => self.pick_files = true,
            Hit::WelcomeHelp => self.overlay = Overlay::Help,
            Hit::WelcomeQuit => self.quit = true,
            Hit::PaletteRow(index) => {
                if let Some(command) = self.palette.command_at(index) {
                    let id = command.id;
                    self.overlay = Overlay::None;
                    self.run_command(id);
                }
            }
            Hit::BrowserRow(position) => {
                self.browser_focus = true;
                self.browser.selected = position;
                if let Some(row) = self.browser.selected_row().cloned() {
                    if row.dir {
                        self.browser.toggle_dir(&row.path);
                    } else {
                        self.browser_focus = false;
                        self.open_path(&row.path);
                    }
                }
            }
            Hit::BrowserPanel => self.browser_focus = true,
            Hit::BrowserUp => {
                self.browser.go_parent();
                self.remember_browser_root();
            }
            Hit::BrowserRefresh => self.browser.reload(),
            Hit::BrowserSplitter => self.drag = Some(Drag::Browser),
            Hit::OutlineSplitter => self.drag = Some(Drag::Outline),
            Hit::EditorDivider => self.drag = Some(Drag::Editor),
            Hit::BrowserCrumb(path) => {
                self.browser.set_root(path);
                self.remember_browser_root();
            }
            Hit::BrowserHidden => self.browser.toggle_hidden(),
            Hit::SettingRow(index) => self.settings_sel = index,
            Hit::SettingLeft(index) => {
                self.settings_sel = index;
                self.change_setting(-1);
            }
            Hit::SettingRight(index) => {
                self.settings_sel = index;
                self.change_setting(1);
            }
            Hit::SettingReset => self.reset_settings(),
            Hit::SettingDefault => self.make_default(),
            Hit::SettingsClose => self.overlay = Overlay::None,
            Hit::EditorArea => self.editor_click(x, y),
            Hit::EditorFormat(index) => {
                if let Some((_, format)) = FORMAT_BUTTONS.get(index) {
                    let format = *format;
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if let Some(editor) = tab.editor.as_mut() {
                            editor.apply_format(format);
                        }
                    }
                }
            }
            Hit::EditorPreview => self.toggle_preview(),
            Hit::ToggleTask(line) => self.toggle_task(line),
            Hit::ConfirmChoice(choice) => self.confirm_choose(choice),
            Hit::FindPrev => self.jump_match(false),
            Hit::FindNext => self.jump_match(true),
            Hit::FindClose => self.close_find(),
            Hit::FindCase => {
                self.find.case_sensitive = !self.find.case_sensitive;
                self.find.matches.clear();
                self.find.current = 0;
            }
            Hit::FindWord => {
                self.find.whole_word = !self.find.whole_word;
                self.find.matches.clear();
                self.find.current = 0;
            }
            Hit::StatusHelp => self.overlay = Overlay::Help,
            Hit::StatusQuit => self.request_quit(),
            Hit::Panel => {}
        }
    }

    pub fn wheel(&mut self, x: u16, y: u16, direction: isize) {
        if self.active_tab().is_some_and(|tab| tab.mode == Mode::Editor) {
            let width = self.editor_width.max(8);
            let height = self.editor_height.max(1);
            if let Some(tab) = self.tabs.get_mut(self.active) {
                if let Some(editor) = tab.editor.as_mut() {
                    let rows = editor.visual_rows(width);
                    editor.scroll_by(&rows, direction * 2, height);
                }
            }
            return;
        }
        match self.overlay {
            Overlay::Browser => {
                let count = self.browser.rows.len();
                let next = step(self.browser.selected, direction, count);
                self.browser.set_selected(next);
                return;
            }
            Overlay::Palette => {
                let count = self.palette.filtered_len();
                self.palette.selected = step(self.palette.selected, direction, count);
                return;
            }
            _ => {}
        }
        if matches!(self.hit_at(x, y), Some((_, Hit::Outline(_))) | Some((_, Hit::OutlinePanel))) {
            self.outline_focus = true;
            self.outline_move(direction);
            return;
        }
        if self.browser.open {
            if matches!(
                self.hit_at(x, y),
                Some((_, Hit::BrowserPanel)) | Some((_, Hit::BrowserRow(_)))
            ) {
                self.browser_focus = true;
                let count = self.browser.rows.len();
                let next = step(self.browser.selected, direction, count);
                self.browser.set_selected(next);
                return;
            }
        }
        self.wheel_by(direction as f32 * 2.0);
    }

    /// A bracketed-paste payload: dropped file paths, or text into an overlay input.
    pub fn handle_paste(&mut self, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        match self.overlay {
            Overlay::Find => {
                self.find.query.push_str(trimmed);
                self.find.matches.clear();
                self.find.current = 0;
                return;
            }
            Overlay::Palette => {
                self.palette.query.push_str(trimmed);
                self.palette.refresh();
                return;
            }
            Overlay::Browser => {
                let mut filter = self.browser.filter.clone();
                filter.push_str(trimmed);
                self.browser.set_filter(filter);
                return;
            }
            _ => {}
        }
        let mut opened = 0usize;
        for raw in split_paths(text) {
            let path = absolute(Path::new(&raw));
            if path.is_dir() {
                self.browser.set_root(path);
                self.browser.open = true;
                self.overlay = Overlay::Browser;
                return;
            }
            if path.is_file() {
                if is_markdown(&path) {
                    match self.open_path_internal(&path, false) {
                        Ok(()) => opened += 1,
                        Err(error) => self.notify(error),
                    }
                } else {
                    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    self.notify(format!("Not a markdown file: {name}"));
                }
            }
        }
        if opened > 0 {
            let plural = if opened == 1 { "file" } else { "files" };
            self.notify(format!("Opened {opened} {plural}"));
        }
    }
}

impl PaletteState {
    pub fn filtered_len(&self) -> usize {
        self.filtered.len()
    }
}

fn is_markdown(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_ascii_lowercase()).as_deref(),
        Some("md" | "markdown" | "mdown" | "mkd" | "mdx" | "txt")
    )
}

fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} kB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
    }
}

fn step(current: usize, direction: isize, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    if direction >= 0 {
        (current + 1).min(count - 1)
    } else {
        current.saturating_sub(1)
    }
}

/// Split a paste payload into paths, honouring Windows Terminal's quoted drops.
fn split_paths(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for ch in input.chars() {
        match ch {
            '"' => quoted = !quoted,
            '\r' => {}
            '\n' if !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn scan_markdown(dir: &Path) -> Vec<(String, String)> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<(String, String)> = read
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() || !is_markdown(&path) {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let size = entry.metadata().ok().map(|meta| human_size(meta.len())).unwrap_or_default();
            Some((name, size))
        })
        .collect();
    files.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    files.truncate(7);
    files
}

/// Subsequence match, the same feel as lmdv's fuzzy palette.
fn fuzzy(needle: &str, haystack: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let hay = haystack.to_lowercase();
    let mut chars = hay.chars();
    'outer: for want in needle.chars() {
        for got in chars.by_ref() {
            if got == want {
                continue 'outer;
            }
        }
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::split_paths;

    #[test]
    fn splits_quoted_drops() {
        let text = "\"C:\\Users\\me\\one file.md\" \"C:\\docs\\two.md\"\r\n";
        assert_eq!(
            split_paths(text),
            vec!["C:\\Users\\me\\one file.md".to_owned(), "C:\\docs\\two.md".to_owned()]
        );
    }

    #[test]
    fn splits_unquoted_tokens() {
        assert_eq!(split_paths("a.md b.md"), vec!["a.md".to_owned(), "b.md".to_owned()]);
    }
}

fn find_matches(text: &str, query: &str, case_sensitive: bool, whole_word: bool) -> bool {
    if query.is_empty() {
        return false;
    }
    let (hay, needle) = if case_sensitive {
        (text.to_owned(), query.to_owned())
    } else {
        (text.to_lowercase(), query.to_lowercase())
    };
    let mut from = 0usize;
    while let Some(found) = hay[from..].find(&needle) {
        let start = from + found;
        let end = start + needle.len();
        let left_ok = start == 0
            || !hay[..start]
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_alphanumeric() || ch == '_');
        let right_ok = end >= hay.len()
            || !hay[end..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_alphanumeric() || ch == '_');
        if !whole_word || (left_ok && right_ok) {
            return true;
        }
        from = end.max(start + 1);
    }
    false
}

fn copy_to_clipboard(text: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::io::Write;
        let mut child = std::process::Command::new("clip")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        let status = child.wait()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("clip exited with an error"))
        }
    }
    #[cfg(not(windows))]
    {
        let _ = text;
        Err(std::io::Error::other("no clipboard backend"))
    }
}

const EXPORT_CSS: &str = r#"
:root{color-scheme:dark light}
body{margin:0;background:#0e1117;color:#e6edf3;font:16px/1.6 -apple-system,"Segoe UI",sans-serif}
main{max-width:760px;margin:0 auto;padding:48px 24px}
a{color:#d97757}
code{background:#1f2933;padding:.15em .35em;border-radius:4px}
pre{background:#0a0e14;padding:16px;overflow:auto;border-radius:8px}
pre code{background:none;padding:0}
table{border-collapse:collapse}
th,td{border:1px solid #2a3441;padding:6px 10px}
blockquote{border-left:3px solid #d97757;margin:0;padding-left:14px;color:#98a2b3}
img{max-width:100%}
hr{border:none;border-top:1px solid #2a3441}
"#;

pub const SHARE_ITEMS: &[(&str, &str, Option<&str>)] = &[
    ("Copy document text", "plain text to the clipboard", Some("share.copy-text")),
    ("Print…", "desktop app only", None),
    ("Save as PDF", "desktop app only", None),
    ("New page at every H1", "print and PDF", Some("share.toggle-break-h1")),
    ("Export as HTML", "self-contained page next to the document", Some("share.export-html")),
    ("Export as DOCX", "desktop app only", None),
    ("Copy file path", "absolute path", Some("share.copy-path")),
    ("Copy Markdown link", "relative to the document folder", Some("share.copy-link")),
];
