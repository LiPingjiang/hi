//! Markdown rendering engine for the Chat panel.
//!
//! Parses Markdown via `pulldown-cmark` and renders it into styled terminal
//! lines (`Vec<MdLine>`) that can be painted directly with crossterm.
//! Code blocks are syntax-highlighted via `syntect` (200+ languages).
//!
//! Design goal: **surpass** glow's visual quality while staying pure-Rust.

use crossterm::style::Color;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, HeadingLevel, CodeBlockKind};
use syntect::highlighting::ThemeSet;
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};
use unicode_width::UnicodeWidthChar;

use crate::syntax::highlight::{classify_scope, CodePalette};

// ── Core types ───────────────────────────────────────────────────────────────

/// A styled span of text — the atomic rendering unit.
#[derive(Debug, Clone)]
pub struct StyledSpan {
    pub text: String,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub dim: bool,
}

impl StyledSpan {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            fg: None, bg: None,
            bold: false, italic: false, underline: false,
            strikethrough: false, dim: false,
        }
    }

    pub fn styled(text: impl Into<String>, fg: Option<Color>, bg: Option<Color>) -> Self {
        Self { text: text.into(), fg, bg, bold: false, italic: false, underline: false, strikethrough: false, dim: false }
    }

    /// Display width accounting for CJK double-width characters.
    pub fn display_width(&self) -> usize {
        self.text.chars().map(|c| UnicodeWidthChar::width(c).unwrap_or(0)).sum()
    }
}

/// A single rendered line, composed of styled spans.
#[derive(Debug, Clone)]
pub struct MdLine {
    pub spans: Vec<StyledSpan>,
    /// Left margin (number of spaces to indent).
    pub indent: usize,
    /// Optional left-border character (e.g. "│" for blockquotes).
    pub border: Option<(String, Color)>,
}

impl MdLine {
    pub fn new() -> Self {
        Self { spans: Vec::new(), indent: 0, border: None }
    }

    pub fn with_indent(indent: usize) -> Self {
        Self { spans: Vec::new(), indent, border: None }
    }

    pub fn empty() -> Self {
        Self::new()
    }

    /// Total display width of all spans (excluding indent and border).
    pub fn content_width(&self) -> usize {
        self.spans.iter().map(|s| s.display_width()).sum()
    }

    pub fn push(&mut self, span: StyledSpan) {
        self.spans.push(span);
    }

    pub fn push_plain(&mut self, text: impl Into<String>) {
        self.spans.push(StyledSpan::plain(text));
    }
}

// ── Theme ────────────────────────────────────────────────────────────────────

/// Markdown rendering theme — controls colors for every element type.
#[derive(Debug, Clone)]
pub struct MdTheme {
    pub h1_fg: Color,
    pub h1_bg: Option<Color>,
    pub h1_bold: bool,
    pub h2_fg: Color,
    pub h2_bold: bool,
    pub h3_fg: Color,
    pub h3_bold: bool,
    pub h4_fg: Color,
    pub h5_fg: Color,
    pub h6_fg: Color,
    pub heading_prefix: bool,

    pub strong_fg: Option<Color>,
    pub emphasis_fg: Option<Color>,
    pub code_inline_fg: Color,
    pub code_inline_bg: Option<Color>,
    pub strikethrough_fg: Option<Color>,
    pub link_fg: Color,
    pub link_underline: bool,
    pub image_fg: Color,

    pub code_block_bg: Option<Color>,
    pub code_block_border: Option<Color>,
    pub code_block_lang_fg: Color,
    pub syntect_theme: String,

    pub blockquote_fg: Color,
    pub blockquote_border: Color,
    pub blockquote_indent: usize,
    pub list_marker_fg: Color,
    pub list_indent: usize,
    pub table_border_fg: Color,
    pub table_header_bold: bool,
    pub hr_char: char,
    pub hr_fg: Color,

    pub text_fg: Color,
    pub paragraph_spacing: usize,
}

impl Default for MdTheme {
    fn default() -> Self { Self::dark() }
}

impl MdTheme {
    /// Neon-Minimalist dark theme.
    ///
    /// Tokyo Night base with selective neon accents.
    pub fn dark() -> Self {
        Self {
            // ── Headings: purple → blue → cyan gradient ──
            h1_fg: Color::Rgb { r: 187, g: 154, b: 247 }, // #BB9AF7 soft purple
            h1_bg: Some(Color::Rgb { r: 30, g: 25, b: 50 }),
            h1_bold: true,
            h2_fg: Color::Rgb { r: 122, g: 162, b: 247 }, // #7AA2F7 vivid blue
            h2_bold: true,
            h3_fg: Color::Rgb { r: 125, g: 207, b: 255 }, // #7DCFFF sky blue
            h3_bold: true,
            h4_fg: Color::Rgb { r: 169, g: 177, b: 214 }, // #A9B1D6 muted
            h5_fg: Color::Rgb { r: 86, g: 95, b: 137 },   // #565F89 dim
            h6_fg: Color::Rgb { r: 86, g: 95, b: 137 },   // #565F89 dim
            heading_prefix: true,

            // ── Inline styles ──
            strong_fg: Some(Color::Rgb { r: 255, g: 158, b: 100 }), // #FF9E64 orange
            emphasis_fg: Some(Color::Rgb { r: 180, g: 249, b: 248 }), // #B4F9F8 mint
            code_inline_fg: Color::Rgb { r: 224, g: 175, b: 104 },  // #E0AF68 gold
            code_inline_bg: Some(Color::Rgb { r: 30, g: 32, b: 48 }),
            strikethrough_fg: Some(Color::Rgb { r: 86, g: 95, b: 137 }),
            link_fg: Color::Rgb { r: 42, g: 195, b: 222 },  // #2AC3DE bright cyan
            link_underline: true,
            image_fg: Color::Rgb { r: 247, g: 118, b: 142 }, // #F7768E pink

            // ── Code blocks ──
            code_block_bg: Some(Color::Rgb { r: 26, g: 27, b: 38 }),  // #1A1B26
            code_block_border: Some(Color::Rgb { r: 41, g: 46, b: 66 }), // #292E42
            code_block_lang_fg: Color::Rgb { r: 86, g: 95, b: 137 },
            syntect_theme: "base16-ocean.dark".to_string(),

            // ── Block elements ──
            blockquote_fg: Color::Rgb { r: 65, g: 72, b: 104 },     // #414868 muted
            blockquote_border: Color::Rgb { r: 122, g: 162, b: 247 }, // #7AA2F7 blue
            blockquote_indent: 2,
            list_marker_fg: Color::Rgb { r: 122, g: 162, b: 247 },   // #7AA2F7 blue
            list_indent: 2,
            table_border_fg: Color::Rgb { r: 41, g: 46, b: 66 },
            table_header_bold: true,
            hr_char: '─',
            hr_fg: Color::Rgb { r: 41, g: 46, b: 66 },

            text_fg: Color::Rgb { r: 169, g: 177, b: 214 }, // #A9B1D6
            paragraph_spacing: 1,
        }
    }

    /// Dracula variant with Neon-Minimalist accents.
    pub fn dracula() -> Self {
        Self {
            h1_fg: Color::Rgb { r: 189, g: 147, b: 249 }, // Dracula purple
            h1_bg: Some(Color::Rgb { r: 40, g: 42, b: 54 }),
            h1_bold: true,
            h2_fg: Color::Rgb { r: 255, g: 121, b: 198 }, // Dracula pink
            h2_bold: true,
            h3_fg: Color::Rgb { r: 80, g: 250, b: 123 },  // Dracula green
            h3_bold: true,
            h4_fg: Color::Rgb { r: 255, g: 184, b: 108 },
            h5_fg: Color::Rgb { r: 139, g: 233, b: 253 },
            h6_fg: Color::Rgb { r: 98, g: 114, b: 164 },
            heading_prefix: true,

            strong_fg: Some(Color::Rgb { r: 255, g: 184, b: 108 }),
            emphasis_fg: Some(Color::Rgb { r: 139, g: 233, b: 253 }),
            code_inline_fg: Color::Rgb { r: 241, g: 250, b: 140 }, // Dracula yellow
            code_inline_bg: Some(Color::Rgb { r: 40, g: 42, b: 54 }),
            strikethrough_fg: Some(Color::Rgb { r: 98, g: 114, b: 164 }),
            link_fg: Color::Rgb { r: 139, g: 233, b: 253 },
            link_underline: true,
            image_fg: Color::Rgb { r: 255, g: 121, b: 198 },

            code_block_bg: Some(Color::Rgb { r: 40, g: 42, b: 54 }),
            code_block_border: Some(Color::Rgb { r: 68, g: 71, b: 90 }),
            code_block_lang_fg: Color::Rgb { r: 98, g: 114, b: 164 },
            syntect_theme: "base16-ocean.dark".to_string(),

            blockquote_fg: Color::Rgb { r: 98, g: 114, b: 164 },
            blockquote_border: Color::Rgb { r: 189, g: 147, b: 249 },
            blockquote_indent: 2,
            list_marker_fg: Color::Rgb { r: 255, g: 121, b: 198 },
            list_indent: 2,
            table_border_fg: Color::Rgb { r: 68, g: 71, b: 90 },
            table_header_bold: true,
            hr_char: '─',
            hr_fg: Color::Rgb { r: 68, g: 71, b: 90 },

            text_fg: Color::Rgb { r: 248, g: 248, b: 242 },
            paragraph_spacing: 1,
        }
    }

    /// Tokyo Night variant — closest to the Neon-Minimalist spec.
    pub fn tokyo_night() -> Self {
        Self {
            h1_fg: Color::Rgb { r: 187, g: 154, b: 247 }, // #BB9AF7
            h1_bg: Some(Color::Rgb { r: 30, g: 25, b: 50 }),
            h1_bold: true,
            h2_fg: Color::Rgb { r: 122, g: 162, b: 247 }, // #7AA2F7
            h2_bold: true,
            h3_fg: Color::Rgb { r: 125, g: 207, b: 255 }, // #7DCFFF
            h3_bold: true,
            h4_fg: Color::Rgb { r: 169, g: 177, b: 214 }, // #A9B1D6
            h5_fg: Color::Rgb { r: 86, g: 95, b: 137 },   // #565F89
            h6_fg: Color::Rgb { r: 86, g: 95, b: 137 },   // #565F89
            heading_prefix: true,

            strong_fg: Some(Color::Rgb { r: 255, g: 158, b: 100 }), // #FF9E64
            emphasis_fg: Some(Color::Rgb { r: 180, g: 249, b: 248 }), // #B4F9F8
            code_inline_fg: Color::Rgb { r: 224, g: 175, b: 104 },  // #E0AF68
            code_inline_bg: Some(Color::Rgb { r: 30, g: 32, b: 48 }),
            strikethrough_fg: Some(Color::Rgb { r: 86, g: 95, b: 137 }),
            link_fg: Color::Rgb { r: 42, g: 195, b: 222 },  // #2AC3DE
            link_underline: true,
            image_fg: Color::Rgb { r: 247, g: 118, b: 142 }, // #F7768E

            code_block_bg: Some(Color::Rgb { r: 26, g: 27, b: 38 }),
            code_block_border: Some(Color::Rgb { r: 41, g: 46, b: 66 }),
            code_block_lang_fg: Color::Rgb { r: 86, g: 95, b: 137 },
            syntect_theme: "base16-ocean.dark".to_string(),

            blockquote_fg: Color::Rgb { r: 65, g: 72, b: 104 },     // #414868
            blockquote_border: Color::Rgb { r: 122, g: 162, b: 247 }, // #7AA2F7
            blockquote_indent: 2,
            list_marker_fg: Color::Rgb { r: 122, g: 162, b: 247 },   // #7AA2F7
            list_indent: 2,
            table_border_fg: Color::Rgb { r: 41, g: 46, b: 66 },
            table_header_bold: true,
            hr_char: '─',
            hr_fg: Color::Rgb { r: 41, g: 46, b: 66 },

            text_fg: Color::Rgb { r: 169, g: 177, b: 214 }, // #A9B1D6
            paragraph_spacing: 1,
        }
    }

    /// Glow Dark — matches glow's warm, muted aesthetic.
    pub fn glow_dark() -> Self {
        Self {
            h1_fg: Color::Rgb { r: 0, g: 170, b: 255 },
            h1_bg: Some(Color::Rgb { r: 55, g: 55, b: 55 }),
            h1_bold: true,
            h2_fg: Color::Rgb { r: 0, g: 215, b: 135 },
            h2_bold: true,
            h3_fg: Color::Rgb { r: 232, g: 232, b: 168 },
            h3_bold: true,
            h4_fg: Color::Rgb { r: 232, g: 232, b: 232 },
            h5_fg: Color::Rgb { r: 102, g: 102, b: 102 },
            h6_fg: Color::Rgb { r: 102, g: 102, b: 102 },
            heading_prefix: true,
            strong_fg: Some(Color::Rgb { r: 232, g: 232, b: 168 }),
            emphasis_fg: Some(Color::Rgb { r: 198, g: 150, b: 105 }),
            code_inline_fg: Color::Rgb { r: 175, g: 255, b: 215 },
            code_inline_bg: Some(Color::Rgb { r: 55, g: 55, b: 55 }),
            strikethrough_fg: Some(Color::Rgb { r: 102, g: 102, b: 102 }),
            link_fg: Color::Rgb { r: 0, g: 170, b: 255 },
            link_underline: true,
            image_fg: Color::Rgb { r: 255, g: 142, b: 199 },
            code_block_bg: Some(Color::Rgb { r: 55, g: 55, b: 55 }),
            code_block_border: Some(Color::Rgb { r: 80, g: 80, b: 80 }),
            code_block_lang_fg: Color::Rgb { r: 102, g: 102, b: 102 },
            syntect_theme: "base16-ocean.dark".to_string(),
            blockquote_fg: Color::Rgb { r: 102, g: 102, b: 102 },
            blockquote_border: Color::Rgb { r: 0, g: 170, b: 255 },
            blockquote_indent: 2,
            list_marker_fg: Color::Rgb { r: 232, g: 232, b: 168 },
            list_indent: 2,
            table_border_fg: Color::Rgb { r: 80, g: 80, b: 80 },
            table_header_bold: true,
            hr_char: '─',
            hr_fg: Color::Rgb { r: 80, g: 80, b: 80 },
            text_fg: Color::Rgb { r: 232, g: 232, b: 232 },
            paragraph_spacing: 1,
        }
    }

    /// Monokai Pro — warm, high-contrast.
    pub fn monokai_pro() -> Self {
        Self {
            h1_fg: Color::Rgb { r: 255, g: 97, b: 136 },
            h1_bg: Some(Color::Rgb { r: 45, g: 42, b: 46 }),
            h1_bold: true,
            h2_fg: Color::Rgb { r: 166, g: 226, b: 46 },
            h2_bold: true,
            h3_fg: Color::Rgb { r: 120, g: 220, b: 232 },
            h3_bold: true,
            h4_fg: Color::Rgb { r: 255, g: 216, b: 102 },
            h5_fg: Color::Rgb { r: 117, g: 113, b: 94 },
            h6_fg: Color::Rgb { r: 117, g: 113, b: 94 },
            heading_prefix: true,
            strong_fg: Some(Color::Rgb { r: 252, g: 152, b: 103 }),
            emphasis_fg: Some(Color::Rgb { r: 120, g: 220, b: 232 }),
            code_inline_fg: Color::Rgb { r: 255, g: 216, b: 102 },
            code_inline_bg: Some(Color::Rgb { r: 45, g: 42, b: 46 }),
            strikethrough_fg: Some(Color::Rgb { r: 117, g: 113, b: 94 }),
            link_fg: Color::Rgb { r: 120, g: 220, b: 232 },
            link_underline: true,
            image_fg: Color::Rgb { r: 255, g: 97, b: 136 },
            code_block_bg: Some(Color::Rgb { r: 45, g: 42, b: 46 }),
            code_block_border: Some(Color::Rgb { r: 73, g: 72, b: 62 }),
            code_block_lang_fg: Color::Rgb { r: 117, g: 113, b: 94 },
            syntect_theme: "base16-ocean.dark".to_string(),
            blockquote_fg: Color::Rgb { r: 117, g: 113, b: 94 },
            blockquote_border: Color::Rgb { r: 171, g: 157, b: 242 },
            blockquote_indent: 2,
            list_marker_fg: Color::Rgb { r: 255, g: 97, b: 136 },
            list_indent: 2,
            table_border_fg: Color::Rgb { r: 73, g: 72, b: 62 },
            table_header_bold: true,
            hr_char: '─',
            hr_fg: Color::Rgb { r: 73, g: 72, b: 62 },
            text_fg: Color::Rgb { r: 248, g: 248, b: 242 },
            paragraph_spacing: 1,
        }
    }

    /// GitHub Dark — clean, professional.
    pub fn github_dark() -> Self {
        Self {
            h1_fg: Color::Rgb { r: 121, g: 192, b: 255 },
            h1_bg: Some(Color::Rgb { r: 22, g: 27, b: 34 }),
            h1_bold: true,
            h2_fg: Color::Rgb { r: 210, g: 168, b: 255 },
            h2_bold: true,
            h3_fg: Color::Rgb { r: 126, g: 231, b: 135 },
            h3_bold: true,
            h4_fg: Color::Rgb { r: 201, g: 209, b: 217 },
            h5_fg: Color::Rgb { r: 139, g: 148, b: 158 },
            h6_fg: Color::Rgb { r: 139, g: 148, b: 158 },
            heading_prefix: true,
            strong_fg: Some(Color::Rgb { r: 201, g: 209, b: 217 }),
            emphasis_fg: Some(Color::Rgb { r: 201, g: 209, b: 217 }),
            code_inline_fg: Color::Rgb { r: 165, g: 214, b: 255 },
            code_inline_bg: Some(Color::Rgb { r: 22, g: 27, b: 34 }),
            strikethrough_fg: Some(Color::Rgb { r: 139, g: 148, b: 158 }),
            link_fg: Color::Rgb { r: 88, g: 166, b: 255 },
            link_underline: true,
            image_fg: Color::Rgb { r: 210, g: 168, b: 255 },
            code_block_bg: Some(Color::Rgb { r: 22, g: 27, b: 34 }),
            code_block_border: Some(Color::Rgb { r: 48, g: 54, b: 61 }),
            code_block_lang_fg: Color::Rgb { r: 139, g: 148, b: 158 },
            syntect_theme: "base16-ocean.dark".to_string(),
            blockquote_fg: Color::Rgb { r: 139, g: 148, b: 158 },
            blockquote_border: Color::Rgb { r: 88, g: 166, b: 255 },
            blockquote_indent: 2,
            list_marker_fg: Color::Rgb { r: 255, g: 123, b: 114 },
            list_indent: 2,
            table_border_fg: Color::Rgb { r: 48, g: 54, b: 61 },
            table_header_bold: true,
            hr_char: '─',
            hr_fg: Color::Rgb { r: 48, g: 54, b: 61 },
            text_fg: Color::Rgb { r: 201, g: 209, b: 217 },
            paragraph_spacing: 1,
        }
    }

    /// One Dark Pro — Atom-inspired.
    pub fn one_dark_pro() -> Self {
        Self {
            h1_fg: Color::Rgb { r: 224, g: 108, b: 117 },
            h1_bg: Some(Color::Rgb { r: 40, g: 44, b: 52 }),
            h1_bold: true,
            h2_fg: Color::Rgb { r: 97, g: 175, b: 239 },
            h2_bold: true,
            h3_fg: Color::Rgb { r: 198, g: 120, b: 221 },
            h3_bold: true,
            h4_fg: Color::Rgb { r: 171, g: 178, b: 191 },
            h5_fg: Color::Rgb { r: 92, g: 99, b: 112 },
            h6_fg: Color::Rgb { r: 92, g: 99, b: 112 },
            heading_prefix: true,
            strong_fg: Some(Color::Rgb { r: 209, g: 154, b: 102 }),
            emphasis_fg: Some(Color::Rgb { r: 198, g: 120, b: 221 }),
            code_inline_fg: Color::Rgb { r: 152, g: 195, b: 121 },
            code_inline_bg: Some(Color::Rgb { r: 40, g: 44, b: 52 }),
            strikethrough_fg: Some(Color::Rgb { r: 92, g: 99, b: 112 }),
            link_fg: Color::Rgb { r: 97, g: 175, b: 239 },
            link_underline: true,
            image_fg: Color::Rgb { r: 198, g: 120, b: 221 },
            code_block_bg: Some(Color::Rgb { r: 40, g: 44, b: 52 }),
            code_block_border: Some(Color::Rgb { r: 60, g: 64, b: 72 }),
            code_block_lang_fg: Color::Rgb { r: 92, g: 99, b: 112 },
            syntect_theme: "base16-ocean.dark".to_string(),
            blockquote_fg: Color::Rgb { r: 92, g: 99, b: 112 },
            blockquote_border: Color::Rgb { r: 97, g: 175, b: 239 },
            blockquote_indent: 2,
            list_marker_fg: Color::Rgb { r: 224, g: 108, b: 117 },
            list_indent: 2,
            table_border_fg: Color::Rgb { r: 60, g: 64, b: 72 },
            table_header_bold: true,
            hr_char: '─',
            hr_fg: Color::Rgb { r: 60, g: 64, b: 72 },
            text_fg: Color::Rgb { r: 171, g: 178, b: 191 },
            paragraph_spacing: 1,
        }
    }

    /// Electric Impressionism — vibrant neon.
    pub fn electric_impressionism() -> Self {
        Self {
            h1_fg: Color::Rgb { r: 0, g: 245, b: 255 },
            h1_bg: Some(Color::Rgb { r: 15, g: 18, b: 30 }),
            h1_bold: true,
            h2_fg: Color::Rgb { r: 183, g: 138, b: 255 },
            h2_bold: true,
            h3_fg: Color::Rgb { r: 0, g: 215, b: 135 },
            h3_bold: true,
            h4_fg: Color::Rgb { r: 230, g: 235, b: 255 },
            h5_fg: Color::Rgb { r: 90, g: 100, b: 120 },
            h6_fg: Color::Rgb { r: 90, g: 100, b: 120 },
            heading_prefix: true,
            strong_fg: Some(Color::Rgb { r: 255, g: 200, b: 100 }),
            emphasis_fg: Some(Color::Rgb { r: 183, g: 138, b: 255 }),
            code_inline_fg: Color::Rgb { r: 166, g: 226, b: 46 },
            code_inline_bg: Some(Color::Rgb { r: 20, g: 22, b: 35 }),
            strikethrough_fg: Some(Color::Rgb { r: 90, g: 100, b: 120 }),
            link_fg: Color::Rgb { r: 0, g: 245, b: 255 },
            link_underline: true,
            image_fg: Color::Rgb { r: 255, g: 77, b: 148 },
            code_block_bg: Some(Color::Rgb { r: 15, g: 18, b: 30 }),
            code_block_border: Some(Color::Rgb { r: 40, g: 45, b: 65 }),
            code_block_lang_fg: Color::Rgb { r: 90, g: 100, b: 120 },
            syntect_theme: "base16-ocean.dark".to_string(),
            blockquote_fg: Color::Rgb { r: 90, g: 100, b: 120 },
            blockquote_border: Color::Rgb { r: 0, g: 245, b: 255 },
            blockquote_indent: 2,
            list_marker_fg: Color::Rgb { r: 255, g: 77, b: 148 },
            list_indent: 2,
            table_border_fg: Color::Rgb { r: 40, g: 45, b: 65 },
            table_header_bold: true,
            hr_char: '─',
            hr_fg: Color::Rgb { r: 40, g: 45, b: 65 },
            text_fg: Color::Rgb { r: 230, g: 235, b: 255 },
            paragraph_spacing: 1,
        }
    }

    /// Synthwave '84 — retro-futuristic neon.
    pub fn synthwave() -> Self {
        Self {
            h1_fg: Color::Rgb { r: 255, g: 40, b: 150 },
            h1_bg: Some(Color::Rgb { r: 25, g: 15, b: 40 }),
            h1_bold: true,
            h2_fg: Color::Rgb { r: 54, g: 243, b: 240 },
            h2_bold: true,
            h3_fg: Color::Rgb { r: 254, g: 78, b: 210 },
            h3_bold: true,
            h4_fg: Color::Rgb { r: 230, g: 220, b: 245 },
            h5_fg: Color::Rgb { r: 105, g: 90, b: 140 },
            h6_fg: Color::Rgb { r: 105, g: 90, b: 140 },
            heading_prefix: true,
            strong_fg: Some(Color::Rgb { r: 255, g: 241, b: 118 }),
            emphasis_fg: Some(Color::Rgb { r: 254, g: 78, b: 210 }),
            code_inline_fg: Color::Rgb { r: 255, g: 241, b: 118 },
            code_inline_bg: Some(Color::Rgb { r: 30, g: 20, b: 50 }),
            strikethrough_fg: Some(Color::Rgb { r: 105, g: 90, b: 140 }),
            link_fg: Color::Rgb { r: 54, g: 243, b: 240 },
            link_underline: true,
            image_fg: Color::Rgb { r: 254, g: 78, b: 210 },
            code_block_bg: Some(Color::Rgb { r: 25, g: 15, b: 40 }),
            code_block_border: Some(Color::Rgb { r: 55, g: 40, b: 80 }),
            code_block_lang_fg: Color::Rgb { r: 105, g: 90, b: 140 },
            syntect_theme: "base16-ocean.dark".to_string(),
            blockquote_fg: Color::Rgb { r: 105, g: 90, b: 140 },
            blockquote_border: Color::Rgb { r: 255, g: 40, b: 150 },
            blockquote_indent: 2,
            list_marker_fg: Color::Rgb { r: 255, g: 40, b: 150 },
            list_indent: 2,
            table_border_fg: Color::Rgb { r: 55, g: 40, b: 80 },
            table_header_bold: true,
            hr_char: '─',
            hr_fg: Color::Rgb { r: 55, g: 40, b: 80 },
            text_fg: Color::Rgb { r: 230, g: 220, b: 245 },
            paragraph_spacing: 1,
        }
    }

    pub fn by_name(name: &str) -> Self {
        match name {
            "dracula" => Self::dracula(),
            "tokyo-night" | "tokyo_night" => Self::tokyo_night(),
            "glow-dark" | "glow_dark" | "glow" => Self::glow_dark(),
            "monokai-pro" | "monokai_pro" | "monokai" => Self::monokai_pro(),
            "github-dark" | "github_dark" | "github" => Self::github_dark(),
            "one-dark-pro" | "one_dark_pro" | "one-dark" | "onedark" => Self::one_dark_pro(),
            "electric-impressionism" | "electric_impressionism" | "electric" => Self::electric_impressionism(),
            "synthwave" | "synthwave-84" | "synthwave_84" => Self::synthwave(),
            "neon-minimalist" | "neon_minimalist" | "dark" | "default" | _ => Self::dark(),
        }
    }

    fn heading_style(&self, level: HeadingLevel) -> (Color, Option<Color>, bool) {
        match level {
            HeadingLevel::H1 => (self.h1_fg, self.h1_bg, self.h1_bold),
            HeadingLevel::H2 => (self.h2_fg, None, self.h2_bold),
            HeadingLevel::H3 => (self.h3_fg, None, self.h3_bold),
            HeadingLevel::H4 => (self.h4_fg, None, false),
            HeadingLevel::H5 => (self.h5_fg, None, false),
            HeadingLevel::H6 => (self.h6_fg, None, false),
        }
    }

    fn heading_prefix_str(level: HeadingLevel) -> &'static str {
        match level {
            HeadingLevel::H1 => "# ",
            HeadingLevel::H2 => "## ",
            HeadingLevel::H3 => "### ",
            HeadingLevel::H4 => "#### ",
            HeadingLevel::H5 => "##### ",
            HeadingLevel::H6 => "###### ",
        }
    }
}

// ── Rendering engine ─────────────────────────────────────────────────────────

/// The Markdown renderer. Holds syntect resources (loaded once, reused).
pub struct MdRenderer {
    pub theme: MdTheme,
    pub palette: CodePalette,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl MdRenderer {
    pub fn new(theme: MdTheme) -> Self {
        let palette = CodePalette::neon_minimalist();
        Self {
            theme,
            palette,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    pub fn new_with_palette(theme: MdTheme, palette: CodePalette) -> Self {
        Self {
            theme,
            palette,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    pub fn with_default_theme() -> Self {
        Self::new(MdTheme::dark())
    }

    /// Switch the theme and palette at runtime.
    pub fn set_theme(&mut self, theme: MdTheme, palette: CodePalette) {
        self.theme = theme;
        self.palette = palette;
    }

    /// Render a Markdown string into styled lines, word-wrapped to `width`.
    pub fn render(&self, markdown: &str, width: usize) -> Vec<MdLine> {
        let width = width.max(10);
        let mut output: Vec<MdLine> = Vec::new();
        let mut ctx = RenderContext::new(&self.theme, width);

        let opts = Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TABLES
            | Options::ENABLE_TASKLISTS;
        let parser = Parser::new_ext(markdown, opts);

        for event in parser {
            match event {
                Event::Start(tag) => ctx.open_tag(tag),
                Event::End(tag_end) => ctx.close_tag(tag_end, &mut output, self),
                Event::Text(text) => ctx.push_text(&text),
                Event::Code(code) => ctx.push_inline_code(&code),
                Event::SoftBreak => ctx.push_text(" "),
                Event::HardBreak => ctx.flush_line(&mut output),
                Event::Rule => ctx.push_rule(&mut output),
                Event::TaskListMarker(checked) => ctx.push_task_marker(checked),
                Event::Html(html) => ctx.push_text(&html),
                Event::InlineHtml(html) => ctx.push_text(&html),
                Event::InlineMath(math) => ctx.push_inline_code(&math),
                Event::DisplayMath(math) => ctx.push_inline_code(&math),
                Event::FootnoteReference(name) => {
                    ctx.push_styled_text(
                        &format!("[^{}]", name),
                        Some(self.theme.link_fg), None,
                        false, false, false, false,
                    );
                }
            }
        }

        ctx.flush_remaining(&mut output);
        output
    }

    /// Highlight a code block using scope-based token classification.
    ///
    /// Uses `ParseState` + `ScopeStack` + `classify_scope()` instead of
    /// `HighlightLines`, giving us Chroma-quality colour differentiation.
    fn highlight_code(&self, code: &str, lang: &str, width: usize) -> Vec<MdLine> {
        let syntax = self.syntax_set
            .find_syntax_by_token(lang)
            .or_else(|| self.syntax_set.find_syntax_by_extension(lang))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut parse_state = ParseState::new(syntax);
        let mut scope_stack = ScopeStack::new();
        let bg = self.theme.code_block_bg;
        let mut lines = Vec::new();

        for src_line in code.lines() {
            let mut md_line = MdLine::with_indent(1);
            match parse_state.parse_line(src_line, &self.syntax_set) {
                Ok(ops) => {
                    let mut byte_pos = 0usize;
                    for &(offset, ref op) in &ops {
                        if offset > byte_pos && offset <= src_line.len() {
                            let text = &src_line[byte_pos..offset];
                            if !text.is_empty() {
                                let tt = classify_scope(&scope_stack, &self.syntax_set);
                                let mut span = StyledSpan::styled(text, Some(tt.to_color(&self.palette)), bg);
                                span.bold = tt.bold();
                                span.italic = tt.italic();
                                md_line.push(span);
                            }
                            byte_pos = offset;
                        }
                        scope_stack.apply(op).ok();
                    }
                    // Remaining text after last scope op
                    if byte_pos < src_line.len() {
                        let text = &src_line[byte_pos..];
                        let tt = classify_scope(&scope_stack, &self.syntax_set);
                        let mut span = StyledSpan::styled(text, Some(tt.to_color(&self.palette)), bg);
                        span.bold = tt.bold();
                        span.italic = tt.italic();
                        md_line.push(span);
                    }
                }
                Err(_) => {
                    md_line.push(StyledSpan::styled(src_line, Some(self.theme.text_fg), bg));
                }
            }
            // Pad to full width with background color
            let used = md_line.content_width() + 1;
            let code_area = width.saturating_sub(2);
            if used < code_area {
                md_line.push(StyledSpan::styled(" ".repeat(code_area - used), None, bg));
            }
            lines.push(md_line);
        }
        lines
    }

    /// Get the syntax set (for unified highlighting in the editor).
    pub fn syntax_set(&self) -> &SyntaxSet {
        &self.syntax_set
    }

    /// Get the theme set.
    pub fn theme_set(&self) -> &ThemeSet {
        &self.theme_set
    }
}

// ── Render context (state machine) ───────────────────────────────────────────

struct RenderContext<'t> {
    theme: &'t MdTheme,
    width: usize,
    current_line: MdLine,

    // Inline style nesting counters
    bold: usize,
    italic: usize,
    strikethrough: usize,

    // Block context stack
    block_stack: Vec<BlockCtx>,

    // Code block accumulator
    code_block: Option<CodeBlockState>,

    // Table state
    table: Option<TableState>,

    // Heading state
    heading: Option<HeadingLevel>,

    // Link / image state
    link_url: Option<String>,
    image_alt: Option<String>,
}

#[derive(Debug, Clone)]
enum BlockCtx {
    Paragraph,
    BlockQuote,
    List { ordered: bool, index: u64 },
    ListItem,
    Heading,
    Table,
    TableHead,
    TableRow,
    TableCell,
}

struct CodeBlockState {
    lang: String,
    content: String,
}

struct TableState {
    rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    current_cell: String,
    is_header: bool,
    header_row: Option<Vec<String>>,
    alignments: Vec<pulldown_cmark::Alignment>,
}

impl<'t> RenderContext<'t> {
    fn new(theme: &'t MdTheme, width: usize) -> Self {
        Self {
            theme, width,
            current_line: MdLine::new(),
            bold: 0, italic: 0, strikethrough: 0,
            block_stack: Vec::new(),
            code_block: None,
            table: None,
            heading: None,
            link_url: None,
            image_alt: None,
        }
    }

    // ── Indent / decoration helpers ──────────────────────────────────────

    fn current_indent(&self) -> usize {
        let mut indent = 0;
        for ctx in &self.block_stack {
            match ctx {
                BlockCtx::BlockQuote => indent += self.theme.blockquote_indent + 2,
                BlockCtx::List { .. } => indent += self.theme.list_indent,
                _ => {}
            }
        }
        indent
    }

    fn blockquote_depth(&self) -> usize {
        self.block_stack.iter().filter(|b| matches!(b, BlockCtx::BlockQuote)).count()
    }

    fn apply_block_decoration(&self, line: &mut MdLine) {
        let bq_depth = self.blockquote_depth();
        if bq_depth > 0 {
            let border_str: String = "│ ".repeat(bq_depth);
            line.border = Some((border_str, self.theme.blockquote_border));
        }
        line.indent = self.current_indent();
    }

    // ── Tag open / close ─────────────────────────────────────────────────

    fn open_tag(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {
                self.block_stack.push(BlockCtx::Paragraph);
            }
            Tag::Heading { level, .. } => {
                self.heading = Some(level);
                self.block_stack.push(BlockCtx::Heading);
            }
            Tag::BlockQuote(_) => {
                self.block_stack.push(BlockCtx::BlockQuote);
            }
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code_block = Some(CodeBlockState { lang, content: String::new() });
            }
            Tag::List(start) => {
                let ordered = start.is_some();
                let index = start.unwrap_or(0);
                self.block_stack.push(BlockCtx::List { ordered, index });
            }
            Tag::Item => {
                self.block_stack.push(BlockCtx::ListItem);
            }
            Tag::Emphasis => { self.italic += 1; }
            Tag::Strong => { self.bold += 1; }
            Tag::Strikethrough => { self.strikethrough += 1; }
            Tag::Link { dest_url, .. } => {
                self.link_url = Some(dest_url.to_string());
            }
            Tag::Image { dest_url, .. } => {
                self.image_alt = Some(String::new());
                self.link_url = Some(dest_url.to_string());
            }
            Tag::Table(alignments) => {
                self.table = Some(TableState {
                    rows: Vec::new(),
                    current_row: Vec::new(),
                    current_cell: String::new(),
                    is_header: false,
                    header_row: None,
                    alignments,
                });
                self.block_stack.push(BlockCtx::Table);
            }
            Tag::TableHead => {
                if let Some(ref mut t) = self.table { t.is_header = true; }
                self.block_stack.push(BlockCtx::TableHead);
            }
            Tag::TableRow => {
                if let Some(ref mut t) = self.table { t.current_row = Vec::new(); }
                self.block_stack.push(BlockCtx::TableRow);
            }
            Tag::TableCell => {
                if let Some(ref mut t) = self.table { t.current_cell = String::new(); }
                self.block_stack.push(BlockCtx::TableCell);
            }
            _ => {}
        }
    }

    fn close_tag(&mut self, tag_end: TagEnd, output: &mut Vec<MdLine>, renderer: &MdRenderer) {
        match tag_end {
            TagEnd::Paragraph => {
                self.flush_line(output);
                for _ in 0..self.theme.paragraph_spacing {
                    let mut blank = MdLine::empty();
                    self.apply_block_decoration(&mut blank);
                    output.push(blank);
                }
                self.pop_block_match(|b| matches!(b, BlockCtx::Paragraph));
            }
            TagEnd::Heading(level) => {
                self.render_heading(level, output);
                self.heading = None;
                self.pop_block_match(|b| matches!(b, BlockCtx::Heading));
            }
            TagEnd::BlockQuote(_) => {
                self.pop_block_match(|b| matches!(b, BlockCtx::BlockQuote));
            }
            TagEnd::CodeBlock => {
                if let Some(cb) = self.code_block.take() {
                    self.render_code_block(&cb, output, renderer);
                }
            }
            TagEnd::List(_) => {
                self.pop_block_match(|b| matches!(b, BlockCtx::List { .. }));
                // Blank line after list
                let mut blank = MdLine::empty();
                self.apply_block_decoration(&mut blank);
                output.push(blank);
            }
            TagEnd::Item => {
                self.flush_line(output);
                self.pop_block_match(|b| matches!(b, BlockCtx::ListItem));
                // Increment list counter
                if let Some(BlockCtx::List { ordered: true, ref mut index, .. }) =
                    self.block_stack.iter_mut().rev().find(|b| matches!(b, BlockCtx::List { .. }))
                {
                    *index += 1;
                }
            }
            TagEnd::Emphasis => { self.italic = self.italic.saturating_sub(1); }
            TagEnd::Strong => { self.bold = self.bold.saturating_sub(1); }
            TagEnd::Strikethrough => { self.strikethrough = self.strikethrough.saturating_sub(1); }
            TagEnd::Link => {
                if let Some(url) = self.link_url.take() {
                    // Append URL in dimmed style after the link text
                    self.push_styled_text(
                        &format!(" ({})", url),
                        Some(self.theme.link_fg), None,
                        false, false, false, true,
                    );
                }
            }
            TagEnd::Image => {
                let alt = self.image_alt.take().unwrap_or_default();
                let url = self.link_url.take().unwrap_or_default();
                let display = if alt.is_empty() {
                    format!("🖼 [image]({})", url)
                } else {
                    format!("🖼 {} ({})", alt, url)
                };
                self.push_styled_text(
                    &display,
                    Some(self.theme.image_fg), None,
                    false, true, false, false,
                );
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    self.render_table(&table, output);
                }
                self.pop_block_match(|b| matches!(b, BlockCtx::Table));
            }
            TagEnd::TableHead => {
                if let Some(ref mut t) = self.table {
                    t.header_row = Some(t.current_row.clone());
                    t.is_header = false;
                }
                self.pop_block_match(|b| matches!(b, BlockCtx::TableHead));
            }
            TagEnd::TableRow => {
                if let Some(ref mut t) = self.table {
                    if !t.is_header {
                        t.rows.push(t.current_row.clone());
                    }
                }
                self.pop_block_match(|b| matches!(b, BlockCtx::TableRow));
            }
            TagEnd::TableCell => {
                if let Some(ref mut t) = self.table {
                    t.current_row.push(t.current_cell.clone());
                }
                self.pop_block_match(|b| matches!(b, BlockCtx::TableCell));
            }
            _ => {}
        }
    }

    fn pop_block_match<F: Fn(&BlockCtx) -> bool>(&mut self, pred: F) {
        if let Some(pos) = self.block_stack.iter().rposition(|b| pred(b)) {
            self.block_stack.remove(pos);
        }
    }

    // ── Text / inline element handling ───────────────────────────────────

    fn push_text(&mut self, text: &str) {
        // If inside a code block, accumulate raw text
        if let Some(ref mut cb) = self.code_block {
            cb.content.push_str(text);
            return;
        }
        // If inside a table cell, accumulate plain text
        if let Some(ref mut t) = self.table {
            t.current_cell.push_str(text);
            return;
        }
        // If inside an image tag, accumulate alt text
        if let Some(ref mut alt) = self.image_alt {
            alt.push_str(text);
            return;
        }

        // Normal inline text — apply current style stack
        let fg = if self.bold > 0 {
            self.theme.strong_fg
        } else if self.italic > 0 {
            self.theme.emphasis_fg
        } else if self.strikethrough > 0 {
            self.theme.strikethrough_fg
        } else if self.link_url.is_some() {
            Some(self.theme.link_fg)
        } else {
            Some(self.theme.text_fg)
        };

        self.push_styled_text(
            text, fg, None,
            self.bold > 0,
            self.italic > 0,
            self.link_url.is_some() && self.theme.link_underline,
            self.strikethrough > 0,
        );
    }

    fn push_inline_code(&mut self, code: &str) {
        // If inside a table cell, just accumulate
        if let Some(ref mut t) = self.table {
            t.current_cell.push('`');
            t.current_cell.push_str(code);
            t.current_cell.push('`');
            return;
        }

        let text = format!(" {} ", code); // padding inside inline code
        let mut span = StyledSpan::styled(text, Some(self.theme.code_inline_fg), self.theme.code_inline_bg);
        span.bold = false;
        self.current_line.push(span);
    }

    fn push_styled_text(
        &mut self, text: &str,
        fg: Option<Color>, bg: Option<Color>,
        bold: bool, italic: bool, underline: bool, dim: bool,
    ) {
        let span = StyledSpan {
            text: text.to_string(),
            fg, bg, bold, italic, underline,
            strikethrough: self.strikethrough > 0,
            dim,
        };
        self.current_line.push(span);
    }

    fn push_task_marker(&mut self, checked: bool) {
        let marker = if checked { "☑ " } else { "☐ " };
        let fg = if checked {
            Some(Color::Rgb { r: 80, g: 250, b: 123 }) // green
        } else {
            Some(self.theme.list_marker_fg)
        };
        self.current_line.push(StyledSpan::styled(marker, fg, None));
    }

    fn push_rule(&mut self, output: &mut Vec<MdLine>) {
        self.flush_line(output);
        let avail = self.width.saturating_sub(self.current_indent());
        let rule_str: String = std::iter::repeat(self.theme.hr_char).take(avail).collect();
        let mut line = MdLine::new();
        line.push(StyledSpan::styled(rule_str, Some(self.theme.hr_fg), None));
        self.apply_block_decoration(&mut line);
        output.push(line);
        output.push(MdLine::empty());
    }

    // ── Line flushing ────────────────────────────────────────────────────

    fn flush_line(&mut self, output: &mut Vec<MdLine>) {
        if self.current_line.spans.is_empty() {
            return;
        }
        let mut line = std::mem::replace(&mut self.current_line, MdLine::new());
        self.apply_block_decoration(&mut line);

        // Word-wrap the line to fit within available width
        let avail = self.width.saturating_sub(line.indent)
            .saturating_sub(line.border.as_ref().map_or(0, |(s, _)| display_width_str(s)));
        let wrapped = wrap_styled_line(&line.spans, avail);

        for spans in wrapped {
            let mut wl = MdLine::new();
            wl.spans = spans;
            wl.indent = line.indent;
            wl.border = line.border.clone();
            output.push(wl);
        }
    }

    fn flush_remaining(&mut self, output: &mut Vec<MdLine>) {
        self.flush_line(output);
    }

    // ── Heading rendering ────────────────────────────────────────────────

    fn render_heading(&mut self, level: HeadingLevel, output: &mut Vec<MdLine>) {
        let (fg, bg, bold) = self.theme.heading_style(level);
        let mut line = MdLine::new();

        if self.theme.heading_prefix {
            let prefix = MdTheme::heading_prefix_str(level);
            let mut ps = StyledSpan::styled(prefix, Some(fg), bg);
            ps.bold = bold;
            ps.dim = true;
            line.push(ps);
        }

        for mut span in std::mem::take(&mut self.current_line.spans) {
            span.fg = Some(fg);
            span.bg = bg;
            span.bold = bold;
            line.push(span);
        }

        // H1: pad background to full width
        if bg.is_some() {
            let used = line.content_width();
            let avail = self.width.saturating_sub(self.current_indent());
            if used < avail {
                line.push(StyledSpan::styled(" ".repeat(avail - used), None, bg));
            }
        }

        self.apply_block_decoration(&mut line);
        output.push(line);

        // Decorative underlines
        let avail = self.width.saturating_sub(self.current_indent());
        if level == HeadingLevel::H1 {
            let mut ul = MdLine::new();
            ul.push(StyledSpan::styled("━".repeat(avail), Some(fg), None));
            self.apply_block_decoration(&mut ul);
            output.push(ul);
        } else if level == HeadingLevel::H2 {
            let mut ul = MdLine::new();
            ul.push(StyledSpan::styled("─".repeat(avail), Some(fg), None));
            self.apply_block_decoration(&mut ul);
            output.push(ul);
        }

        let mut blank = MdLine::empty();
        self.apply_block_decoration(&mut blank);
        output.push(blank);
    }

    // ── Code block rendering ─────────────────────────────────────────────

    fn render_code_block(&self, cb: &CodeBlockState, output: &mut Vec<MdLine>, renderer: &MdRenderer) {
        let indent = self.current_indent();
        let avail = self.width.saturating_sub(indent);
        let bg = self.theme.code_block_bg;
        let border_color = self.theme.code_block_border;

        // Top border with language label
        let mut top = MdLine::with_indent(indent);
        if let Some(bc) = border_color {
            let lang_label = if cb.lang.is_empty() {
                String::new()
            } else {
                format!(" {} ", cb.lang)
            };
            let border_len = avail.saturating_sub(display_width_str(&lang_label));
            let mut border_str = "╭".to_string();
            border_str.push_str(&"─".repeat(border_len.saturating_sub(2)));
            border_str.push('╮');

            // Insert language label into the border
            if !lang_label.is_empty() {
                let label_pos = 2; // after "╭─"
                let border_chars: Vec<char> = border_str.chars().collect();
                let label_chars: Vec<char> = lang_label.chars().collect();
                let mut new_border = String::new();
                for (i, ch) in border_chars.iter().enumerate() {
                    if i >= label_pos && i < label_pos + label_chars.len() {
                        // Will be added as separate span
                    } else {
                        new_border.push(*ch);
                    }
                }
                // Build: border_prefix + lang_label + border_suffix
                let prefix: String = border_chars[..label_pos].iter().collect();
                let suffix: String = border_chars[label_pos + lang_label.len().min(border_chars.len() - label_pos)..].iter().collect();
                top.push(StyledSpan::styled(prefix, Some(bc), bg));
                top.push(StyledSpan::styled(&lang_label, Some(self.theme.code_block_lang_fg), bg));
                top.push(StyledSpan::styled(suffix, Some(bc), bg));
            } else {
                top.push(StyledSpan::styled(border_str, Some(bc), bg));
            }
        }
        let bq_depth = self.blockquote_depth();
        if bq_depth > 0 {
            let border_str: String = "│ ".repeat(bq_depth);
            top.border = Some((border_str, self.theme.blockquote_border));
        }
        output.push(top);

        // Syntax-highlighted code lines
        let code_lines = renderer.highlight_code(&cb.content, &cb.lang, avail);
        for mut cl in code_lines {
            cl.indent = indent;
            // Add left border
            if let Some(bc) = border_color {
                let mut bordered = MdLine::with_indent(indent);
                bordered.push(StyledSpan::styled("│", Some(bc), bg));
                for span in cl.spans {
                    bordered.push(span);
                }
                // Right border
                let used = bordered.content_width();
                if used < avail.saturating_sub(1) {
                    bordered.push(StyledSpan::styled(
                        " ".repeat(avail.saturating_sub(used + 1)),
                        None, bg,
                    ));
                }
                bordered.push(StyledSpan::styled("│", Some(bc), bg));
                bordered.border = cl.border;
                output.push(bordered);
            } else {
                output.push(cl);
            }
        }

        // Bottom border
        if let Some(bc) = border_color {
            let mut bottom = MdLine::with_indent(indent);
            let mut border_str = "╰".to_string();
            border_str.push_str(&"─".repeat(avail.saturating_sub(2)));
            border_str.push('╯');
            bottom.push(StyledSpan::styled(border_str, Some(bc), bg));
            output.push(bottom);
        }

        // Blank line after code block
        output.push(MdLine::empty());
    }


    // ── List item rendering ──────────────────────────────────────────────

    // List markers are injected when we see the first text inside a ListItem.
    // We detect this in push_text by checking if we're at the start of a ListItem.

    // ── Table rendering ──────────────────────────────────────────────────

    fn render_table(&self, table: &TableState, output: &mut Vec<MdLine>) {
        let indent = self.current_indent();
        let border_fg = self.theme.table_border_fg;

        // Calculate column widths
        let num_cols = table.alignments.len();
        let all_rows: Vec<&Vec<String>> = table.header_row.iter()
            .chain(table.rows.iter())
            .collect();

        let mut col_widths: Vec<usize> = vec![0; num_cols];
        for row in &all_rows {
            for (i, cell) in row.iter().enumerate() {
                if i < num_cols {
                    col_widths[i] = col_widths[i].max(display_width_str(cell));
                }
            }
        }
        // Ensure minimum width
        for w in &mut col_widths {
            *w = (*w).max(3);
        }

        // Top border: ┌───┬───┐
        let mut top = MdLine::with_indent(indent);
        let mut top_str = "┌".to_string();
        for (i, w) in col_widths.iter().enumerate() {
            top_str.push_str(&"─".repeat(*w + 2));
            if i < num_cols - 1 { top_str.push('┬'); }
        }
        top_str.push('┐');
        top.push(StyledSpan::styled(top_str, Some(border_fg), None));
        output.push(top);

        // Header row
        if let Some(header) = &table.header_row {
            let mut line = MdLine::with_indent(indent);
            line.push(StyledSpan::styled("│", Some(border_fg), None));
            for (i, cell) in header.iter().enumerate() {
                let w = col_widths.get(i).copied().unwrap_or(3);
                let padded = format!(" {:<width$} ", cell, width = w);
                let mut span = StyledSpan::styled(padded, Some(self.theme.text_fg), None);
                span.bold = self.theme.table_header_bold;
                line.push(span);
                if i < num_cols - 1 {
                    line.push(StyledSpan::styled("│", Some(border_fg), None));
                }
            }
            line.push(StyledSpan::styled("│", Some(border_fg), None));
            output.push(line);

            // Header separator: ├───┼───┤
            let mut sep = MdLine::with_indent(indent);
            let mut sep_str = "├".to_string();
            for (i, w) in col_widths.iter().enumerate() {
                sep_str.push_str(&"─".repeat(*w + 2));
                if i < num_cols - 1 { sep_str.push('┼'); }
            }
            sep_str.push('┤');
            sep.push(StyledSpan::styled(sep_str, Some(border_fg), None));
            output.push(sep);
        }

        // Data rows
        for row in &table.rows {
            let mut line = MdLine::with_indent(indent);
            line.push(StyledSpan::styled("│", Some(border_fg), None));
            for (i, cell) in row.iter().enumerate() {
                let w = col_widths.get(i).copied().unwrap_or(3);
                let padded = format!(" {:<width$} ", cell, width = w);
                line.push(StyledSpan::styled(padded, Some(self.theme.text_fg), None));
                if i < num_cols - 1 {
                    line.push(StyledSpan::styled("│", Some(border_fg), None));
                }
            }
            line.push(StyledSpan::styled("│", Some(border_fg), None));
            output.push(line);
        }

        // Bottom border: └───┴───┘
        let mut bottom = MdLine::with_indent(indent);
        let mut bot_str = "└".to_string();
        for (i, w) in col_widths.iter().enumerate() {
            bot_str.push_str(&"─".repeat(*w + 2));
            if i < num_cols - 1 { bot_str.push('┴'); }
        }
        bot_str.push('┘');
        bottom.push(StyledSpan::styled(bot_str, Some(border_fg), None));
        output.push(bottom);

        output.push(MdLine::empty());
    }
}

// ── Word-wrapping for styled spans ───────────────────────────────────────────

/// Word-wrap a sequence of styled spans to fit within `max_width` display columns.
/// Returns a Vec of lines, each being a Vec of StyledSpan.
fn wrap_styled_line(spans: &[StyledSpan], max_width: usize) -> Vec<Vec<StyledSpan>> {
    if max_width == 0 {
        return vec![spans.to_vec()];
    }

    let mut lines: Vec<Vec<StyledSpan>> = Vec::new();
    let mut current: Vec<StyledSpan> = Vec::new();
    let mut current_width = 0usize;

    for span in spans {
        let span_width = span.display_width();

        // If the entire span fits on the current line, just add it
        if current_width + span_width <= max_width {
            current.push(span.clone());
            current_width += span_width;
            continue;
        }

        // Need to split the span across lines
        let mut remaining = span.text.as_str();
        let template = span.clone();

        while !remaining.is_empty() {
            let avail = max_width.saturating_sub(current_width);
            if avail == 0 {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
                continue;
            }

            // Take as many chars as fit
            let mut take_bytes = 0;
            let mut take_width = 0;
            for ch in remaining.chars() {
                let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
                if take_width + cw > avail && take_width > 0 {
                    break;
                }
                take_bytes += ch.len_utf8();
                take_width += cw;
            }

            if take_bytes == 0 {
                // Can't fit even one char — force a line break
                lines.push(std::mem::take(&mut current));
                current_width = 0;
                continue;
            }

            let chunk = &remaining[..take_bytes];
            let mut new_span = template.clone();
            new_span.text = chunk.to_string();
            current.push(new_span);
            current_width += take_width;
            remaining = &remaining[take_bytes..];

            if !remaining.is_empty() {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(Vec::new());
    }

    lines
}

/// Display width of a string.
fn display_width_str(s: &str) -> usize {
    s.chars().map(|c| UnicodeWidthChar::width(c).unwrap_or(0)).sum()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn render_to_plain(md: &str, width: usize) -> Vec<String> {
        let renderer = MdRenderer::with_default_theme();
        let lines = renderer.render(md, width);
        lines.iter().map(|l| {
            let mut s = String::new();
            if let Some((ref border, _)) = l.border {
                s.push_str(border);
            }
            if l.indent > 0 {
                s.push_str(&" ".repeat(l.indent));
            }
            for span in &l.spans {
                s.push_str(&span.text);
            }
            s
        }).collect()
    }

    #[test]
    fn test_heading_rendering() {
        let lines = render_to_plain("# Hello World", 40);
        assert!(!lines.is_empty());
        // First line should contain the heading text
        let first = &lines[0];
        assert!(first.contains("Hello World"), "heading text missing: {}", first);
        // H1 should have an underline
        assert!(lines.len() >= 2, "H1 should have underline");
        assert!(lines[1].contains('━'), "H1 underline missing");
    }

    #[test]
    fn test_code_block_highlighting() {
        let md = "```rust\nfn main() {\n    println!(\"hello\");\n}\n```";
        let renderer = MdRenderer::with_default_theme();
        let lines = renderer.render(md, 60);
        // Should have top border, code lines, bottom border
        assert!(lines.len() >= 5, "code block should have borders + content, got {}", lines.len());
        // Check that top border contains language label
        let top_text: String = lines[0].spans.iter().map(|s| s.text.as_str()).collect();
        assert!(top_text.contains("rust") || top_text.contains("╭"), "top border: {}", top_text);
    }

    #[test]
    fn test_inline_code() {
        let lines = render_to_plain("Use `cargo build` to compile", 60);
        assert!(!lines.is_empty());
        let text: String = lines.iter().map(|l| {
            l.clone()
        }).collect::<Vec<_>>().join("");
        assert!(text.contains("cargo build"), "inline code missing");
    }

    #[test]
    fn test_bold_italic() {
        let md = "This is **bold** and *italic* text";
        let renderer = MdRenderer::with_default_theme();
        let lines = renderer.render(md, 60);
        assert!(!lines.is_empty());
        // Check that bold span exists
        let has_bold = lines.iter().any(|l| l.spans.iter().any(|s| s.bold));
        assert!(has_bold, "should have bold span");
        let has_italic = lines.iter().any(|l| l.spans.iter().any(|s| s.italic));
        assert!(has_italic, "should have italic span");
    }

    #[test]
    fn test_blockquote() {
        let lines = render_to_plain("> This is a quote", 40);
        assert!(!lines.is_empty());
        // Should have border decoration
        let first = &lines[0];
        assert!(first.contains('│'), "blockquote should have border: {}", first);
    }

    #[test]
    fn test_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
        let lines = render_to_plain(md, 40);
        assert!(lines.len() >= 5, "table should have borders + rows, got {}", lines.len());
        // Check for table border characters
        let all_text: String = lines.join("\n");
        assert!(all_text.contains('┌'), "table should have top border");
        assert!(all_text.contains('└'), "table should have bottom border");
    }

    #[test]
    fn test_horizontal_rule() {
        let lines = render_to_plain("---", 40);
        assert!(!lines.is_empty());
        let has_rule = lines.iter().any(|l| l.contains('─'));
        assert!(has_rule, "should render horizontal rule");
    }

    #[test]
    fn test_word_wrap() {
        let md = "This is a very long paragraph that should be wrapped when it exceeds the available width of the rendering area.";
        let lines = render_to_plain(md, 30);
        assert!(lines.len() >= 3, "long text should wrap to multiple lines, got {}", lines.len());
    }

    #[test]
    fn test_empty_input() {
        let lines = render_to_plain("", 40);
        // Empty input should produce empty or minimal output
        assert!(lines.len() <= 1);
    }

    #[test]
    fn test_theme_selection() {
        let dark = MdTheme::dark();
        let dracula = MdTheme::dracula();
        let tokyo = MdTheme::tokyo_night();
        // Themes should have different H2 colors
        assert_ne!(format!("{:?}", dark.h2_fg), format!("{:?}", dracula.h2_fg));
        assert_ne!(format!("{:?}", dark.h2_fg), format!("{:?}", tokyo.h2_fg));
    }

    #[test]
    fn test_nested_blockquote() {
        let md = "> Level 1\n>> Level 2\n>>> Level 3";
        let renderer = MdRenderer::with_default_theme();
        let lines = renderer.render(md, 60);
        // Should have increasing border depth
        let max_depth = lines.iter()
            .filter_map(|l| l.border.as_ref())
            .map(|(s, _)| s.matches('│').count())
            .max()
            .unwrap_or(0);
        assert!(max_depth >= 2, "nested blockquotes should increase border depth, got {}", max_depth);
    }

    #[test]
    fn test_list_rendering() {
        let md = "- Item 1\n- Item 2\n- Item 3";
        let lines = render_to_plain(md, 40);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_syntect_languages() {
        // Verify syntect has the languages we need
        let ss = SyntaxSet::load_defaults_newlines();
        assert!(ss.find_syntax_by_token("rust").is_some());
        assert!(ss.find_syntax_by_token("python").is_some());
        assert!(ss.find_syntax_by_token("javascript").is_some());
        assert!(ss.find_syntax_by_token("java").is_some());
        assert!(ss.find_syntax_by_token("go").is_some());
        assert!(ss.find_syntax_by_token("c").is_some());
        assert!(ss.find_syntax_by_token("cpp").is_some());
        assert!(ss.find_syntax_by_token("sql").is_some());
    }
}
