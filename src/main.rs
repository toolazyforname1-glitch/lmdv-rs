//! lmdv-tui — the lmdv Markdown reader, in the terminal.
//!
//! Same document surface as the native app: reading card, outline, find,
//! command palette, file browser, four themes and a reading-progress hairline.

mod app;
mod assoc;
mod editor;
mod editor_ui;
mod highlight;
mod md;
mod prefs;
mod render;
mod session;
mod style;
mod table;
mod theme;
mod ui;

use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::SetTitle;
use ratatui::Terminal;

use crate::app::App;
use crate::theme::ThemeKind;

const USAGE: &str = "\
lmdv-tui — a Markdown reader for the terminal

USAGE:
    lmdv-tui [FILES…] [OPTIONS]

OPTIONS:
    --theme <black|dark|light|sepia>   Start in this theme (default: black)
    --no-motion                        Disable entrance and scroll animation
    --snapshot                         Render one frame headlessly and print it
    --width <COLS>                     Snapshot width (default: 120)
    --height <ROWS>                    Snapshot height (default: 36)
    --scroll <LINE>                    Snapshot from this line
    -h, --help                         Show this help

KEYS:
    j/k scroll · d/u half page · space page · g/G top/bottom
    tab outline · o open · / find · t theme · [ ] tabs · x close
    ctrl+p palette · ? help · q quit
";

struct Options {
    files: Vec<PathBuf>,
    theme: Option<ThemeKind>,
    motion: bool,
    snapshot: bool,
    width: u16,
    height: u16,
    scroll: Option<usize>,
    clicks: Vec<(u16, u16)>,
    drags: Vec<(u16, u16)>,
    hits: bool,
    settings: bool,
    share: bool,
    browser: bool,
    newtab: bool,
    editor: bool,
    help: bool,
}

fn main() {
    if std::env::args().any(|argument| argument == "--fps") {
        println!("{}", refresh_rate());
        return;
    }
    let options = match parse_args() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("lmdv-tui: {error}\n\n{USAGE}");
            std::process::exit(2);
        }
    };
    if options.help {
        print!("{USAGE}");
        return;
    }
    let prefs = load_prefs(&options);
    let result = if options.snapshot {
        snapshot(options, prefs)
    } else {
        run(options, prefs)
    };
    if let Err(error) = result {
        eprintln!("lmdv-tui: {error}");
        std::process::exit(1);
    }
}

fn parse_args() -> Result<Options, String> {
    let mut options = Options {
        files: Vec::new(),
        theme: None,
        motion: std::env::var_os("NO_ANIMATIONS").is_none(),
        snapshot: false,
        width: 120,
        height: 36,
        scroll: None,
        clicks: Vec::new(),
        drags: Vec::new(),
        hits: false,
        settings: false,
        share: false,
        browser: false,
        newtab: false,
        editor: false,
        help: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => options.help = true,
            "--no-motion" => options.motion = false,
            "--snapshot" => options.snapshot = true,
            "--theme" => {
                let value = args.next().ok_or("--theme needs a value")?;
                options.theme = Some(
                    ThemeKind::parse(&value)
                        .ok_or_else(|| format!("unknown theme '{value}' (black, dark, light, sepia)"))?,
                );
            }
            "--width" => {
                options.width = args.next().ok_or("--width needs a value")?.parse().map_err(|_| "invalid --width")?;
            }
            "--height" => {
                options.height = args.next().ok_or("--height needs a value")?.parse().map_err(|_| "invalid --height")?;
            }
            "--scroll" => {
                options.scroll = Some(args.next().ok_or("--scroll needs a value")?.parse().map_err(|_| "invalid --scroll")?);
            }
            "--settings" => options.settings = true,
            "--share" => options.share = true,
            "--browser" => options.browser = true,
            "--newtab" => options.newtab = true,
            "--editor" => options.editor = true,
            "--hits" => options.hits = true,
            "--click" => {
                let value = args.next().ok_or("--click needs X,Y")?;
                options.clicks.push(parse_click(&value)?);
            }
            "--drag" => {
                let value = args.next().ok_or("--drag needs X,Y")?;
                options.drags.push(parse_click(&value)?);
            }
            other if other.starts_with("--theme=") => {
                let value = &other["--theme=".len()..];
                options.theme = Some(
                    ThemeKind::parse(value)
                        .ok_or_else(|| format!("unknown theme '{value}' (black, dark, light, sepia)"))?,
                );
            }
            other if other.starts_with("--width=") => {
                options.width = other["--width=".len()..].parse().map_err(|_| "invalid --width")?;
            }
            other if other.starts_with("--height=") => {
                options.height = other["--height=".len()..].parse().map_err(|_| "invalid --height")?;
            }
            other if other.starts_with("--click=") => {
                options.clicks.push(parse_click(&other["--click=".len()..])?);
            }
            other if other.starts_with("--drag=") => {
                options.drags.push(parse_click(&other["--drag=".len()..])?);
            }
            "--" => {
                options.files.extend(args.map(PathBuf::from));
                break;
            }
            other if other.starts_with('-') => return Err(format!("unknown option '{other}'")),
            other => options.files.push(PathBuf::from(other)),
        }
    }
    Ok(options)
}

fn load_prefs(options: &Options) -> prefs::Prefs {
    let mut prefs = prefs::load();
    if let Some(theme) = options.theme {
        prefs.theme = theme.name().to_owned();
    }
    if !options.motion {
        prefs.motion = false;
    }
    prefs
}

fn snapshot(options: Options, prefs: prefs::Prefs) -> io::Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(options.width, options.height))
        .expect("the test backend is infallible");
    let mut app = App::new(options.files, prefs, false);
    app.enter = 1.0;
    if let Some(line) = options.scroll {
        app.scroll_to(line as f32);
        if let Some(tab) = app.tabs.get_mut(app.active) {
            tab.scroll = line as f32;
        }
    }
    if options.settings {
        app.run_command("app.settings");
    }
    if options.share {
        app.run_command("app.share");
    }
    if options.browser {
        app.open_browser();
    }
    if options.newtab {
        app.new_tab();
    }
    if options.editor {
        app.toggle_editor();
    }
    if options.editor {
        if let Some(line) = options.scroll {
            if let Some(tab) = app.tabs.get_mut(app.active) {
                if let Some(editor) = tab.editor.as_mut() {
                    editor.scroll = line;
                }
            }
        }
    }
    terminal.draw(|frame| ui::draw(frame, &mut app)).expect("the test backend is infallible");
    if options.hits {
        for (rect, hit) in &app.hits {
            eprintln!("hit {}x{}+{},{} {:?}", rect.width, rect.height, rect.x, rect.y, hit);
        }
    }
    for (column, row) in &options.clicks {
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: *column,
            row: *row,
            modifiers: KeyModifiers::NONE,
        });
        terminal.draw(|frame| ui::draw(frame, &mut app)).expect("the test backend is infallible");
    }
    for (column, row) in &options.drags {
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: *column,
            row: *row,
            modifiers: KeyModifiers::NONE,
        });
        terminal.draw(|frame| ui::draw(frame, &mut app)).expect("the test backend is infallible");
    }
    if let Some((column, row)) = options.drags.last().copied() {
        app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
        terminal.draw(|frame| ui::draw(frame, &mut app)).expect("the test backend is infallible");
    }
    terminal.draw(|frame| ui::draw(frame, &mut app)).expect("the test backend is infallible");
    let buffer = terminal.backend().buffer();
    let mut out = io::stdout().lock();
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        writeln!(out, "{}", line.trim_end())?;
    }
    Ok(())
}

fn run(options: Options, prefs: prefs::Prefs) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut stdout = io::stdout();
    let _ = execute!(stdout, EnableMouseCapture, EnableBracketedPaste);
    let _ = write!(io::stdout(), "\x1b[?1003h");
    let _ = io::stdout().flush();

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(io::stdout(), DisableBracketedPaste, DisableMouseCapture);
        let _ = write!(io::stdout(), "\x1b[?1003l");
        ratatui::restore();
        previous(info);
    }));

    let mut app = App::new(options.files, prefs, true);
    if let Some(line) = options.scroll {
        if let Some(tab) = app.tabs.get_mut(app.active) {
            tab.scroll = line as f32;
            tab.target = line as f32;
        }
    }
    let mut title = window_title(&app);
    let _ = execute!(io::stdout(), SetTitle(title.clone()));
    if options.browser {
        app.open_browser();
    }
    if options.newtab {
        app.new_tab();
    }
    if options.editor {
        app.toggle_editor();
    }
    if options.editor {
        if let Some(line) = options.scroll {
            if let Some(tab) = app.tabs.get_mut(app.active) {
                if let Some(editor) = tab.editor.as_mut() {
                    editor.scroll = line;
                }
            }
        }
    }
    let hz = refresh_rate().clamp(30, 360);
    let frame = Duration::from_micros(1_000_000 / hz as u64);
    let result = event_loop(&mut terminal, &mut app, &mut title, frame);

    let saved = app.session();
    session::save(&saved);
    prefs::save(&app.prefs);

    let _ = execute!(stdout, DisableBracketedPaste, DisableMouseCapture);
    let _ = write!(stdout, "\x1b[?1003l");
    ratatui::restore();
    result
}

fn window_title(app: &App) -> String {
    app.active_tab()
        .map(|tab| format!("lmdv - {}", tab.name))
        .unwrap_or_else(|| "lmdv".to_owned())
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    title: &mut String,
    frame: Duration,
) -> io::Result<()> {
    let mut last_draw = Instant::now() - Duration::from_secs(1);
    let mut dirty = true;
    let mut draw_interval = Duration::from_millis(16);
    loop {
        let moving = app.tick();
        let wanted = window_title(app);
        if wanted != *title {
            *title = wanted.clone();
            let _ = execute!(io::stdout(), SetTitle(wanted));
        }
        let due = last_draw.elapsed() >= Duration::from_millis(200);
        if (dirty || moving || due) && last_draw.elapsed() >= draw_interval {
            let started = Instant::now();
            terminal.draw(|frame| ui::draw(frame, app))?;
            let cost = started.elapsed();
            draw_interval = if cost > Duration::from_millis(4) {
                (cost * 2).clamp(Duration::from_millis(16), Duration::from_millis(100))
            } else {
                Duration::from_millis(16)
            };
            last_draw = Instant::now();
            dirty = false;
        }
        if event::poll(frame)? {
            let mut scroll_notches = 0i32;
            let mut scroll_position = None;
            loop {
                match event::read()? {
                    Event::Key(key) => {
                        app.handle_key(key);
                        dirty = true;
                    }
                    Event::Mouse(mouse) => match mouse.kind {
                        MouseEventKind::ScrollDown => {
                            scroll_notches = scroll_notches.saturating_add(1);
                            scroll_position = Some((mouse.column, mouse.row));
                        }
                        MouseEventKind::ScrollUp => {
                            scroll_notches = scroll_notches.saturating_sub(1);
                            scroll_position = Some((mouse.column, mouse.row));
                        }
                        MouseEventKind::Moved => {
                            if app.set_hover(mouse.column, mouse.row) {
                                dirty = true;
                            }
                        }
                        _ => {
                            app.handle_mouse(mouse);
                            dirty = true;
                        }
                    },
                    Event::Paste(text) => {
                        app.handle_paste(&text);
                        dirty = true;
                    }
                    Event::Resize(_, _) => {
                        dirty = true;
                    }
                    _ => {
                        dirty = true;
                    }
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
            if scroll_notches != 0 {
                if let Some((column, row)) = scroll_position {
                    app.wheel_burst(column, row, scroll_notches);
                }
                dirty = true;
            }
        }
        if std::mem::take(&mut app.pick_files) {
            let saved = if app.prefs.browser_root.is_empty() {
                None
            } else {
                let path = std::path::PathBuf::from(&app.prefs.browser_root);
                path.is_dir().then_some(path)
            };
            let start = saved
                .or_else(|| {
                    app.active_tab()
                        .and_then(|tab| tab.path.parent().map(std::path::Path::to_path_buf))
                })
                .or_else(|| std::env::current_dir().ok());
            match pick_with_explorer(terminal, start) {
                Ok(files) => {
                    for path in files {
                        app.open_path(&path);
                    }
                }
                Err(error) => app.notify(format!("Explorer picker failed: {error}")),
            }
            dirty = true;
        }
        if app.quit {
            return Ok(());
        }
    }
}

#[cfg(windows)]
fn refresh_rate() -> u32 {
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};
    unsafe {
        let mut device: DEVMODEW = std::mem::zeroed();
        device.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        let mode = ENUM_CURRENT_SETTINGS as u32;
        if EnumDisplaySettingsW(std::ptr::null(), mode, &mut device) != 0 && device.dmDisplayFrequency > 1 {
            return device.dmDisplayFrequency;
        }
    }
    60
}

#[cfg(not(windows))]
fn refresh_rate() -> u32 {
    60
}

fn pick_with_explorer(
    terminal: &mut ratatui::DefaultTerminal,
    start: Option<PathBuf>,
) -> io::Result<Vec<PathBuf>> {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, DisableBracketedPaste, DisableMouseCapture);
    let _ = write!(stdout, "\x1b[?1003l");
    ratatui::restore();
    let mut dialog = rfd::FileDialog::new()
        .set_title("Open markdown — lmdv")
        .add_filter("Markdown", &["md", "markdown", "mdown", "mkd", "mdx", "txt"])
        .add_filter("All files", &["*"]);
    if let Some(start) = start {
        dialog = dialog.set_directory(start);
    }
    let files = dialog.pick_files().unwrap_or_default();
    *terminal = ratatui::init();
    let _ = execute!(stdout, EnableMouseCapture, EnableBracketedPaste);
    let _ = write!(io::stdout(), "\x1b[?1003h");
    let _ = io::stdout().flush();
    terminal.clear()?;
    Ok(files)
}

fn parse_click(value: &str) -> Result<(u16, u16), String> {
    let (x, y) = value.split_once(',').ok_or("--click expects X,Y")?;
    let x = x.trim().parse().map_err(|_| "invalid --click X")?;
    let y = y.trim().parse().map_err(|_| "invalid --click Y")?;
    Ok((x, y))
}
