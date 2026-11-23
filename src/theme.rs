//! The lmdv colour system, ported from `crates/lmdv-app/src/appearance.rs`.
//!
//! Four schemes (black, dark, light, sepia) each fill the same semantic roles,
//! so every widget references a role and a theme swap repaints the whole app.
//! The black and sepia values are the original app's CSS tokens; dark and light
//! are the neutral GitHub-style kits lmdv uses for `Dark` and `Light`.

use ratatui::style::Color;

/// Which of the four lmdv schemes is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeKind {
    Black,
    Dark,
    Light,
    Sepia,
}

impl ThemeKind {
    /// Cycle order matches the toolbar theme menu: black, dark, light, sepia.
    pub fn next(self) -> Self {
        match self {
            Self::Black => Self::Dark,
            Self::Dark => Self::Light,
            Self::Light => Self::Sepia,
            Self::Sepia => Self::Black,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Black => "black",
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Sepia => "sepia",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "black" => Some(Self::Black),
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            "sepia" => Some(Self::Sepia),
            _ => None,
        }
    }
}

/// Every role the UI and the Markdown renderer draw from.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub kind: ThemeKind,
    /// The window behind the reading card.
    pub page: Color,
    /// The reading card and toolbar.
    pub surface: Color,
    /// Chips, table headers and selected rows.
    pub surface_2: Color,
    /// A third elevation for overlays.
    pub surface_3: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub border: Color,
    pub border_strong: Color,
    /// Fills, rules and the current-section marker.
    pub accent: Color,
    pub link: Color,
    pub code_bg: Color,
    pub chip_bg: Color,
    pub chip_border: Color,
    pub danger: Color,
    pub success: Color,
    pub warning: Color,
    pub info: Color,
    pub on_accent: Color,
    // Syntax inks.
    pub syn_kw: Color,
    pub syn_str: Color,
    pub syn_num: Color,
    pub syn_com: Color,
    pub syn_fun: Color,
    pub syn_typ: Color,
    pub syn_attr: Color,
    pub syn_punct: Color,
}

impl Palette {
    pub fn of(kind: ThemeKind) -> Self {
        match kind {
            ThemeKind::Black => Self {
                kind,
                page: rgb(0x000000),
                surface: rgb(0x121212),
                surface_2: rgb(0x1a1a1a),
                surface_3: rgb(0x232323),
                text: rgb(0xe6e6e3),
                muted: rgb(0x9c9a93),
                faint: rgb(0x6a6862),
                border: rgb(0x262626),
                border_strong: rgb(0x3a3a38),
                accent: rgb(0xd97757),
                link: rgb(0xdf8768),
                code_bg: rgb(0x0c0c0c),
                chip_bg: rgb(0x1f1f1f),
                chip_border: rgb(0x333333),
                danger: rgb(0xff8f80),
                success: rgb(0x7ccf98),
                warning: rgb(0xe3b341),
                info: rgb(0x6cb6ff),
                on_accent: rgb(0x1a1915),
                syn_kw: rgb(0xff9d8a),
                syn_str: rgb(0xa7d8a0),
                syn_num: rgb(0xe8c07d),
                syn_com: rgb(0x74726a),
                syn_fun: rgb(0xc9a6ff),
                syn_typ: rgb(0xf3b46b),
                syn_attr: rgb(0x7ec8e3),
                syn_punct: rgb(0xb8b6ae),
            },
            ThemeKind::Dark => Self {
                kind,
                page: rgb(0x0e1117),
                surface: rgb(0x151b23),
                surface_2: rgb(0x1c2530),
                surface_3: rgb(0x232e3a),
                text: rgb(0xe6edf3),
                muted: rgb(0x98a2b3),
                faint: rgb(0x6b7686),
                border: rgb(0x2a3441),
                border_strong: rgb(0x3d4a5c),
                accent: rgb(0xd97757),
                link: rgb(0x6fb3ff),
                code_bg: rgb(0x0a0e14),
                chip_bg: rgb(0x1f2933),
                chip_border: rgb(0x35414f),
                danger: rgb(0xff7b72),
                success: rgb(0x7ee2a8),
                warning: rgb(0xe3b341),
                info: rgb(0x6cb6ff),
                on_accent: rgb(0x10131a),
                syn_kw: rgb(0xff8f80),
                syn_str: rgb(0x9fd8a8),
                syn_num: rgb(0xe8c07d),
                syn_com: rgb(0x7c8a99),
                syn_fun: rgb(0xc9a6ff),
                syn_typ: rgb(0xf3b46b),
                syn_attr: rgb(0x7ec8e3),
                syn_punct: rgb(0xb0bcc9),
            },
            ThemeKind::Light => Self {
                kind,
                page: rgb(0xeef0f3),
                surface: rgb(0xffffff),
                surface_2: rgb(0xf2f4f7),
                surface_3: rgb(0xe8ebef),
                text: rgb(0x1f2328),
                muted: rgb(0x656d76),
                faint: rgb(0x8c959f),
                border: rgb(0xd8dee4),
                border_strong: rgb(0xb8c0c8),
                accent: rgb(0xb2552f),
                link: rgb(0x0969da),
                code_bg: rgb(0xf6f8fa),
                chip_bg: rgb(0xeff2f5),
                chip_border: rgb(0xd0d7de),
                danger: rgb(0xcf222e),
                success: rgb(0x1a7f37),
                warning: rgb(0x9a6700),
                info: rgb(0x0969da),
                on_accent: rgb(0xffffff),
                syn_kw: rgb(0xcf222e),
                syn_str: rgb(0x0a6b2d),
                syn_num: rgb(0x0550ae),
                syn_com: rgb(0x8a949e),
                syn_fun: rgb(0x8250df),
                syn_typ: rgb(0x953800),
                syn_attr: rgb(0x0a5c8a),
                syn_punct: rgb(0x57606a),
            },
            ThemeKind::Sepia => Self {
                kind,
                page: rgb(0xf3e9d6),
                surface: rgb(0xfaf3e4),
                surface_2: rgb(0xebdfc8),
                surface_3: rgb(0xe2d4b8),
                text: rgb(0x3b2f22),
                muted: rgb(0x6f5d49),
                faint: rgb(0x93816a),
                border: rgb(0xdccdb2),
                border_strong: rgb(0xc4b294),
                accent: rgb(0xb2552f),
                link: rgb(0x9a4322),
                code_bg: rgb(0xeee3cd),
                chip_bg: rgb(0xe6d9bf),
                chip_border: rgb(0xcfbf9f),
                danger: rgb(0xa2271c),
                success: rgb(0x2a6e40),
                warning: rgb(0x865a00),
                info: rgb(0x085fc5),
                on_accent: rgb(0xffffff),
                syn_kw: rgb(0xa2271c),
                syn_str: rgb(0x2a6e40),
                syn_num: rgb(0x8a5a00),
                syn_com: rgb(0x93836a),
                syn_fun: rgb(0x7447c8),
                syn_typ: rgb(0x865a00),
                syn_attr: rgb(0x085fc5),
                syn_punct: rgb(0x6f5d49),
            },
        }
    }
}

/// The original tokens are `#rrggbb`; ratatui takes 8-bit triples.
const fn rgb(hex: u32) -> Color {
    Color::Rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

/// Linear blend from `a` to `b`; non-RGB values fall back to `b`.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) = (a, b) else {
        return b;
    };
    let t = t.clamp(0.0, 1.0);
    let blend = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color::Rgb(blend(ar, br), blend(ag, bg), blend(ab, bb))
}
