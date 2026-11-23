//! Persisted preferences and session data, mirroring `lmdv-core`'s prefs.
//!
//! Every setting the native app exposes is here. The terminal cannot change a
//! font face or run a spellchecker, so those rows are stored and labelled as
//! system-controlled; everything else has a visible effect.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::theme::ThemeKind;

pub const THEME_OPTIONS: [&str; 5] = ["system", "light", "sepia", "dark", "black"];
pub const SIZE_STEPS: [u16; 8] = [13, 14, 15, 16, 17, 18, 20, 22];
pub const LINE_HEIGHTS: [&str; 3] = ["compact", "normal", "relaxed"];
pub const COLUMN_WIDTHS: [&str; 4] = ["narrow", "normal", "wide", "full"];
pub const SIDE_SIZES: [&str; 3] = ["small", "normal", "large"];
pub const LARGE_FILE_MODES: [&str; 2] = ["auto", "block"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spacing {
    Compact,
    Normal,
    Relaxed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub theme: String,
    pub size: u16,
    pub font: String,
    pub line_height: String,
    pub column_width: String,
    pub side_size: String,
    pub outline_hidden: bool,
    pub restore_session: bool,
    pub motion: bool,
    pub print_break_h1: bool,
    pub format_bar: bool,
    pub spellcheck: bool,
    pub large_file_mode: String,
    pub locale: String,
    pub recent: Vec<String>,
    pub browser_width: u16,
    pub outline_cols: u16,
    pub editor_split: u16,
    pub browser_root: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: "system".to_owned(),
            size: 16,
            font: "sans".to_owned(),
            line_height: "normal".to_owned(),
            column_width: "normal".to_owned(),
            side_size: "normal".to_owned(),
            outline_hidden: false,
            restore_session: true,
            motion: true,
            print_break_h1: false,
            format_bar: true,
            spellcheck: true,
            large_file_mode: "auto".to_owned(),
            locale: "auto".to_owned(),
            recent: Vec::new(),
            browser_width: 28,
            outline_cols: 28,
            editor_split: 58,
            browser_root: String::new(),
        }
    }
}

impl Prefs {
    pub fn resolved_theme(&self) -> ThemeKind {
        match self.theme.as_str() {
            "light" => ThemeKind::Light,
            "sepia" => ThemeKind::Sepia,
            "dark" => ThemeKind::Dark,
            "black" => ThemeKind::Black,
            _ => {
                if system_dark() {
                    ThemeKind::Dark
                } else {
                    ThemeKind::Light
                }
            }
        }
    }

    pub fn spacing(&self) -> Spacing {
        match self.line_height.as_str() {
            "compact" => Spacing::Compact,
            "relaxed" => Spacing::Relaxed,
            _ => Spacing::Normal,
        }
    }

    pub fn size_index(&self) -> usize {
        SIZE_STEPS.iter().position(|step| *step == self.size).unwrap_or(3)
    }

    pub fn outline_width(&self) -> u16 {
        self.outline_cols.clamp(18, 48)
    }

    /// The reading column width, derived from the text-size and width rows.
    pub fn card_cap(&self, available: u16) -> u16 {
        let base: u16 = [104, 100, 96, 92, 88, 84, 78, 72][self.size_index()];
        let width = match self.column_width.as_str() {
            "narrow" => (f32::from(base) * 0.72).round() as u16,
            "wide" => (f32::from(base) * 1.25).round() as u16,
            "full" => available.saturating_sub(4),
            _ => base,
        };
        width.min(available.saturating_sub(4)).max(30)
    }

    pub fn large_files_block(&self, bytes: usize) -> bool {
        self.large_file_mode == "block" || (self.large_file_mode == "auto" && bytes > 400 * 1024)
    }

    pub fn theme_label(&self) -> &'static str {
        match self.theme.as_str() {
            "light" => "Light",
            "sepia" => "Sepia",
            "dark" => "Dark",
            "black" => "Black",
            _ => "System",
        }
    }

    pub fn cycle(&mut self, key: &str, direction: isize) {
        match key {
            "theme" => self.theme = cycle_str(&THEME_OPTIONS, &self.theme, direction).to_owned(),
            "size" => {
                let index = self.size_index() as isize + direction;
                let index = index.rem_euclid(SIZE_STEPS.len() as isize) as usize;
                self.size = SIZE_STEPS[index];
            }
            "font" => {
                self.font = if self.font == "sans" { "serif".to_owned() } else { "sans".to_owned() };
            }
            "line_height" => {
                self.line_height = cycle_str(&LINE_HEIGHTS, &self.line_height, direction).to_owned();
            }
            "column_width" => {
                self.column_width = cycle_str(&COLUMN_WIDTHS, &self.column_width, direction).to_owned();
            }
            "side_size" => {
                self.side_size = cycle_str(&SIDE_SIZES, &self.side_size, direction).to_owned();
            }
            "outline_hidden" => self.outline_hidden = !self.outline_hidden,
            "restore_session" => self.restore_session = !self.restore_session,
            "motion" => self.motion = !self.motion,
            "print_break_h1" => self.print_break_h1 = !self.print_break_h1,
            "format_bar" => self.format_bar = !self.format_bar,
            "spellcheck" => self.spellcheck = !self.spellcheck,
            "large_file_mode" => {
                self.large_file_mode = cycle_str(&LARGE_FILE_MODES, &self.large_file_mode, direction).to_owned();
            }
            "locale" => {}
            _ => {}
        }
    }
}

fn cycle_str<'a>(options: &'a [&'a str], current: &str, direction: isize) -> &'a str {
    let index = options.iter().position(|option| *option == current).unwrap_or(0) as isize;
    let index = (index + direction).rem_euclid(options.len() as isize) as usize;
    options[index]
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("lmdv-tui")
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn load() -> Prefs {
    std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|text| serde_json::from_str::<Prefs>(&text).ok())
        .unwrap_or_default()
}

pub fn save(prefs: &Prefs) {
    let _ = std::fs::create_dir_all(data_dir());
    if let Ok(text) = serde_json::to_string_pretty(prefs) {
        let _ = std::fs::write(settings_path(), text);
    }
}

fn system_dark() -> bool {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
        return match key {
            Ok(key) => key.get_value::<u32, _>("AppsUseLightTheme").map(|light| light == 0).unwrap_or(true),
            Err(_) => true,
        };
    }
    #[cfg(not(windows))]
    {
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Theme,
    Size,
    Font,
    LineHeight,
    ColumnWidth,
    SideSize,
    Outline,
    Restore,
    Motion,
    PrintH1,
    DefaultHandler,
    FormatBar,
    LargeFile,
    Spellcheck,
    Locale,
    Reset,
}

impl Setting {
    pub const ALL: [Setting; 16] = [
        Setting::Theme,
        Setting::Size,
        Setting::Font,
        Setting::LineHeight,
        Setting::ColumnWidth,
        Setting::SideSize,
        Setting::Outline,
        Setting::Restore,
        Setting::Motion,
        Setting::PrintH1,
        Setting::DefaultHandler,
        Setting::FormatBar,
        Setting::LargeFile,
        Setting::Spellcheck,
        Setting::Locale,
        Setting::Reset,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Theme => "Theme",
            Self::Size => "Text size",
            Self::Font => "Font",
            Self::LineHeight => "Spacing",
            Self::ColumnWidth => "Width",
            Self::SideSize => "Side panels",
            Self::Outline => "Outline",
            Self::Restore => "Restore session",
            Self::Motion => "Motion",
            Self::PrintH1 => "Print page break at H1",
            Self::DefaultHandler => "Default Markdown app",
            Self::FormatBar => "Format bar",
            Self::LargeFile => "Large files",
            Self::Spellcheck => "Spellcheck",
            Self::Locale => "Language",
            Self::Reset => "Reset to defaults",
        }
    }

    pub fn section(self) -> Option<&'static str> {
        match self {
            Self::Theme => Some("APPEARANCE"),
            Self::Outline => Some("PANELS"),
            Self::Restore => Some("GENERAL"),
            Self::FormatBar => Some("EDITOR"),
            Self::Locale => Some("LANGUAGE"),
            Self::Reset => Some(""),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Size => "size",
            Self::Font => "font",
            Self::LineHeight => "line_height",
            Self::ColumnWidth => "column_width",
            Self::SideSize => "side_size",
            Self::Outline => "outline_hidden",
            Self::Restore => "restore_session",
            Self::Motion => "motion",
            Self::PrintH1 => "print_break_h1",
            Self::DefaultHandler => "default_handler",
            Self::FormatBar => "format_bar",
            Self::LargeFile => "large_file_mode",
            Self::Spellcheck => "spellcheck",
            Self::Locale => "locale",
            Self::Reset => "reset",
        }
    }

    pub fn value(self, p: &Prefs) -> String {
        match self {
            Self::Theme => p.theme_label().to_owned(),
            Self::Size => format!("{} px", p.size),
            Self::Font => format!("{} (terminal)", title(&p.font)),
            Self::LineHeight => title(&p.line_height),
            Self::ColumnWidth => title(&p.column_width),
            Self::SideSize => title(&p.side_size),
            Self::Outline => if p.outline_hidden { "Hidden" } else { "Shown" }.to_owned(),
            Self::Restore => on_off(p.restore_session),
            Self::Motion => on_off(p.motion),
            Self::PrintH1 => if p.print_break_h1 { "Print at H1" } else { "Continuous" }.to_owned(),
            Self::DefaultHandler => "Make default…".to_owned(),
            Self::FormatBar => if p.format_bar { "Shown" } else { "Hidden" }.to_owned(),
            Self::LargeFile => title(&p.large_file_mode),
            Self::Spellcheck => format!("{} (system)", on_off(p.spellcheck)),
            Self::Locale => "English (system)".to_owned(),
            Self::Reset => String::new(),
        }
    }
}

fn title(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn on_off(value: bool) -> String {
    if value { "On".to_owned() } else { "Off".to_owned() }
}
