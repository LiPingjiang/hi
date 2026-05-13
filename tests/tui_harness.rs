//! TUI Test Harness — Multi-level integration testing for the `hi` editor.
//!
//! ## Architecture
//!
//! This harness provides THREE levels of testing:
//!
//! ### Level 1: In-Process Render Capture (no PTY needed)
//!
//! Constructs `Editor` + `SyntectHighlighter` in-process, renders to a
//! `Vec<u8>` buffer, then feeds that buffer into `vt100::Parser` to get a
//! virtual screen with per-cell character and colour information.
//!
//! This is the primary testing mode — works everywhere, including sandboxed
//! environments without PTY access.
//!
//! ### Level 2: Full PTY End-to-End (requires PTY permissions)
//!
//! Spawns the real `hi` binary in a PTY, sends keystrokes, captures screen.
//! Only runs in environments with PTY access (CI, local terminal).
//! Tests using this level are marked `#[ignore]` by default.
//!
//! ## Usage
//!
//! ```rust,ignore
//! // Level 1: In-process render test
//! let mut ctx = RenderTestContext::new(80, 24);
//! ctx.open_content("md", "# Hello\n\nWorld\n");
//! ctx.render_frame();
//! assert!(ctx.has_highlighted_cells(0, 10));
//! assert!(ctx.no_unexpected_bg(0, 20));
//!
//! // Level 2: PTY test (requires PTY access)
//! let mut h = PtyHarness::spawn_with_file("test.md", 80, 24)?;
//! h.wait_for_text("NORMAL", Duration::from_secs(3))?;
//! ```

use hi::buffer::Buffer;
use hi::config::Config;
use hi::editor::Editor;
use hi::syntax::highlight::{FileType, SyntectHighlighter, SyntectSpan};

// ═══════════════════════════════════════════════════════════════════════════════
// ANSI Colour Classification
// ═══════════════════════════════════════════════════════════════════════════════

/// ANSI colour categories for assertion purposes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorBucket {
    Default,
    Grey,
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Yellow,
    White,
    /// Any RGB colour that doesn't fit the above buckets.
    Other(u8, u8, u8),
}

impl ColorBucket {
    /// Classify a vt100 `Color` into a bucket.
    pub fn from_vt100(color: vt100::Color) -> Self {
        match color {
            vt100::Color::Default => ColorBucket::Default,
            vt100::Color::Idx(idx) => Self::from_ansi_index(idx),
            vt100::Color::Rgb(r, g, b) => Self::from_rgb(r, g, b),
        }
    }

    /// Classify a crossterm `Color` into a bucket.
    pub fn from_crossterm(color: crossterm::style::Color) -> Self {
        use crossterm::style::Color;
        match color {
            Color::Reset => ColorBucket::Default,
            Color::Black => ColorBucket::Default,
            Color::DarkGrey => ColorBucket::Grey,
            Color::Grey => ColorBucket::Grey,
            Color::White => ColorBucket::White,
            Color::Red | Color::DarkRed => ColorBucket::Red,
            Color::Green | Color::DarkGreen => ColorBucket::Green,
            Color::Blue | Color::DarkBlue => ColorBucket::Blue,
            Color::Cyan | Color::DarkCyan => ColorBucket::Cyan,
            Color::Magenta | Color::DarkMagenta => ColorBucket::Magenta,
            Color::Yellow | Color::DarkYellow => ColorBucket::Yellow,
            Color::Rgb { r, g, b } => Self::from_rgb(r, g, b),
            Color::AnsiValue(idx) => Self::from_ansi_index(idx),
        }
    }

    fn from_ansi_index(idx: u8) -> Self {
        match idx {
            0 => ColorBucket::Default,
            1 => ColorBucket::Red,
            2 => ColorBucket::Green,
            3 => ColorBucket::Yellow,
            4 => ColorBucket::Blue,
            5 => ColorBucket::Magenta,
            6 => ColorBucket::Cyan,
            7 => ColorBucket::White,
            8 => ColorBucket::Grey,
            9..=11 => ColorBucket::Red,
            12..=14 => ColorBucket::Green,
            _ => ColorBucket::Other(idx, 0, 0),
        }
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);

        if max < 60 {
            return ColorBucket::Default;
        }
        if max - min < 30 {
            return if max > 180 { ColorBucket::White } else { ColorBucket::Grey };
        }

        if r > g && r > b && r - g > 40 && r - b > 40 {
            if g > 100 && b < 80 { return ColorBucket::Yellow; }
            if b > 100 { return ColorBucket::Magenta; }
            return ColorBucket::Red;
        }
        if g > r && g > b && g - r > 40 && g - b > 40 {
            if b > 100 { return ColorBucket::Cyan; }
            return ColorBucket::Green;
        }
        if b > r && b > g && b - r > 40 && b - g > 40 {
            if r > 100 { return ColorBucket::Magenta; }
            if g > 100 { return ColorBucket::Cyan; }
            return ColorBucket::Blue;
        }
        if g > 100 && b > 100 && r < 100 { return ColorBucket::Cyan; }
        if r > 100 && g > 100 && b < 100 { return ColorBucket::Yellow; }
        if r > 100 && b > 100 && g < 100 { return ColorBucket::Magenta; }

        ColorBucket::Other(r, g, b)
    }

    /// Returns true if this colour is NOT the default/black/grey — i.e. it has
    /// actual syntax highlighting colour.
    pub fn is_highlighted(&self) -> bool {
        !matches!(self, ColorBucket::Default | ColorBucket::Grey | ColorBucket::White)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Level 1: In-Process Render Test Context
// ═══════════════════════════════════════════════════════════════════════════════

/// A span with colour information, representing one highlighted segment on a line.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TestSpan {
    pub text: String,
    pub fg: ColorBucket,
    pub bold: bool,
    pub italic: bool,
}

/// In-process render test context.
///
/// Constructs an `Editor` and `SyntectHighlighter`, renders the visible
/// portion of the buffer, and provides assertion APIs on the output.
pub struct RenderTestContext {
    pub editor: Editor,
    pub syntect_hl: SyntectHighlighter,
    /// Rendered lines: each line is a Vec of TestSpans.
    pub rendered_lines: Vec<Vec<TestSpan>>,
    /// Terminal dimensions.
    #[allow(dead_code)]
    pub cols: u16,
    pub rows: u16,
}

impl RenderTestContext {
    /// Create a new test context with the given terminal dimensions.
    pub fn new(cols: u16, rows: u16) -> Self {
        let config = Config::default();
        let editor = Editor::new(config, cols, rows);
        let syntect_hl = SyntectHighlighter::with_default_theme(FileType::Plain);
        Self {
            editor,
            syntect_hl,
            rendered_lines: Vec::new(),
            cols,
            rows,
        }
    }

    /// Load content into the editor buffer with the given file extension.
    pub fn open_content(&mut self, ext: &str, content: &str) {
        let ft = FileType::from_ext(ext);
        let mut buf = Buffer::new();
        buf.rope = ropey::Rope::from_str(content);
        self.editor.buffer = buf;
        self.syntect_hl.set_filetype(ft);
    }

    /// Render the currently visible portion of the buffer.
    ///
    /// This simulates what `Renderer::render()` does for the editing area:
    /// reset syntect state, pre-parse lines above viewport, then highlight
    /// each visible line.
    pub fn render_frame(&mut self) {
        self.rendered_lines.clear();

        // Reset and pre-parse (same logic as renderer.rs)
        const MAX_PRE_PARSE: usize = 200;
        self.syntect_hl.reset_state();
        let pre_start = self.editor.scroll_line.saturating_sub(MAX_PRE_PARSE);
        let pre_end = self.editor.scroll_line.min(self.editor.buffer.line_count());
        for pre_line in pre_start..pre_end {
            let text = self.editor.buffer.line_str(pre_line);
            let _ = self.syntect_hl.highlight_line(&text);
        }

        // Render visible lines
        let edit_h = (self.rows as usize).saturating_sub(2); // status bar takes 2 rows
        for screen_row in 0..edit_h {
            let buf_line = self.editor.scroll_line + screen_row;
            if buf_line >= self.editor.buffer.line_count() {
                break;
            }
            let line = self.editor.buffer.line_str(buf_line);
            let spans = self.syntect_hl.highlight_line(&line);
            let test_spans = self.convert_spans(&line, &spans);
            self.rendered_lines.push(test_spans);
        }
    }

    /// Scroll to a specific line and re-render.
    pub fn scroll_to(&mut self, line: usize) {
        self.editor.scroll_line = line;
        self.render_frame();
    }

    // ── Assertions ───────────────────────────────────────────────────────────

    /// Check if any rendered cell in the given row range has a non-default colour.
    pub fn has_highlighted_cells(&self, start_row: usize, end_row: usize) -> bool {
        for row in start_row..end_row.min(self.rendered_lines.len()) {
            for span in &self.rendered_lines[row] {
                if span.text.trim().is_empty() { continue; }
                if span.fg.is_highlighted() {
                    return true;
                }
            }
        }
        false
    }

    /// Count highlighted (coloured) characters in the given row range.
    pub fn count_highlighted_chars(&self, start_row: usize, end_row: usize) -> usize {
        let mut count = 0;
        for row in start_row..end_row.min(self.rendered_lines.len()) {
            for span in &self.rendered_lines[row] {
                if span.fg.is_highlighted() {
                    count += span.text.chars().filter(|c| !c.is_whitespace()).count();
                }
            }
        }
        count
    }

    /// Get the plain text of a rendered row.
    pub fn row_text(&self, row: usize) -> String {
        if row >= self.rendered_lines.len() {
            return String::new();
        }
        self.rendered_lines[row]
            .iter()
            .map(|s| s.text.as_str())
            .collect()
    }

    /// Get the colour of the first non-whitespace character on a row.
    pub fn first_char_color(&self, row: usize) -> Option<ColorBucket> {
        if row >= self.rendered_lines.len() {
            return None;
        }
        for span in &self.rendered_lines[row] {
            if !span.text.trim().is_empty() {
                return Some(span.fg);
            }
        }
        None
    }

    /// Get all unique colours used in a row.
    pub fn row_colors(&self, row: usize) -> Vec<ColorBucket> {
        if row >= self.rendered_lines.len() {
            return Vec::new();
        }
        let mut colors: Vec<ColorBucket> = self.rendered_lines[row]
            .iter()
            .filter(|s| !s.text.trim().is_empty())
            .map(|s| s.fg)
            .collect();
        colors.dedup();
        colors
    }

    /// Produce a debug dump of the rendered output with colour annotations.
    pub fn debug_dump(&self) -> String {
        let mut out = String::new();
        for (i, line) in self.rendered_lines.iter().enumerate() {
            out.push_str(&format!("L{:2}: ", i));
            for span in line {
                if span.text.is_empty() { continue; }
                let color_tag = match span.fg {
                    ColorBucket::Default => "",
                    ColorBucket::Grey => "[grey]",
                    ColorBucket::Red => "[RED]",
                    ColorBucket::Green => "[GRN]",
                    ColorBucket::Blue => "[BLU]",
                    ColorBucket::Cyan => "[CYN]",
                    ColorBucket::Magenta => "[MAG]",
                    ColorBucket::Yellow => "[YEL]",
                    ColorBucket::White => "[WHT]",
                    ColorBucket::Other(..) => "[RGB]"
                };
                out.push_str(&format!("{}{}", color_tag, span.text));
            }
            out.push('\n');
        }
        out
    }

    // ── Private ──────────────────────────────────────────────────────────────

    fn convert_spans(&self, line: &str, spans: &[SyntectSpan]) -> Vec<TestSpan> {
        if spans.is_empty() {
            return vec![TestSpan {
                text: line.to_string(),
                fg: ColorBucket::Default,
                bold: false,
                italic: false,
            }];
        }

        spans
            .iter()
            .filter(|s| s.start < s.end && s.end <= line.len())
            .map(|s| {
                TestSpan {
                    text: line[s.start..s.end].to_string(),
                    fg: ColorBucket::from_crossterm(s.fg),
                    bold: s.bold,
                    italic: s.italic,
                }
            })
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Level 1b: ANSI Escape Sequence Render Capture
//
// This captures what the Renderer would actually write to the terminal,
// by redirecting output to a Vec<u8> and parsing with vt100.
// ═══════════════════════════════════════════════════════════════════════════════

/// Captures ANSI output and provides a virtual screen via vt100.
#[allow(dead_code)]
pub struct AnsiCapture {
    pub parser: vt100::Parser,
    pub cols: u16,
    pub rows: u16,
}

#[allow(dead_code)]
impl AnsiCapture {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, 0),
            cols,
            rows,
        }
    }

    /// Feed raw ANSI bytes into the parser.
    pub fn feed(&mut self, data: &[u8]) {
        self.parser.process(data);
    }

    /// Get the full screen as plain text.
    pub fn screen_text(&self) -> String {
        let screen = self.parser.screen();
        let mut lines = Vec::new();
        for row in 0..self.rows {
            let mut line = String::new();
            for col in 0..self.cols {
                if let Some(cell) = screen.cell(row, col) {
                    let contents = cell.contents();
                    if contents.is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(&contents);
                    }
                }
            }
            lines.push(line.trim_end().to_string());
        }
        lines.join("\n")
    }

    /// Check if any cell in the given row range has a non-default foreground.
    pub fn has_highlighted_cells(&self, start_row: u16, end_row: u16) -> bool {
        let screen = self.parser.screen();
        for row in start_row..end_row.min(self.rows) {
            for col in 0..self.cols {
                if let Some(cell) = screen.cell(row, col) {
                    if cell.contents().trim().is_empty() { continue; }
                    let fg = ColorBucket::from_vt100(cell.fgcolor());
                    if fg.is_highlighted() {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Count cells with non-default foreground.
    pub fn count_highlighted_cells(&self, start_row: u16, end_row: u16) -> usize {
        let screen = self.parser.screen();
        let mut count = 0;
        for row in start_row..end_row.min(self.rows) {
            for col in 0..self.cols {
                if let Some(cell) = screen.cell(row, col) {
                    if cell.contents().trim().is_empty() { continue; }
                    let fg = ColorBucket::from_vt100(cell.fgcolor());
                    if fg.is_highlighted() {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// Find cells with non-default background (for detecting colour leaks).
    pub fn cells_with_bg(&self, start_row: u16, end_row: u16) -> Vec<(u16, u16, ColorBucket)> {
        let screen = self.parser.screen();
        let mut results = Vec::new();
        for row in start_row..end_row.min(self.rows) {
            for col in 0..self.cols {
                if let Some(cell) = screen.cell(row, col) {
                    let bg = ColorBucket::from_vt100(cell.bgcolor());
                    if bg != ColorBucket::Default {
                        results.push((row, col, bg));
                    }
                }
            }
        }
        results
    }

    /// Get foreground colour of a specific cell.
    pub fn fg_at(&self, row: u16, col: u16) -> ColorBucket {
        let screen = self.parser.screen();
        screen
            .cell(row, col)
            .map(|c| ColorBucket::from_vt100(c.fgcolor()))
            .unwrap_or(ColorBucket::Default)
    }

    /// Get background colour of a specific cell.
    pub fn bg_at(&self, row: u16, col: u16) -> ColorBucket {
        let screen = self.parser.screen();
        screen
            .cell(row, col)
            .map(|c| ColorBucket::from_vt100(c.bgcolor()))
            .unwrap_or(ColorBucket::Default)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Level 2: Full PTY Harness (requires PTY permissions)
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(feature = "pty_tests")]
pub mod pty_harness {
    //! PTY-based end-to-end test harness.
    //! Only available when the `pty_tests` feature is enabled and the
    //! environment supports PTY allocation.
    //!
    //! Run with: `cargo test --features pty_tests --test tui_smoke`

    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};
    use std::thread;
    use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
    use super::ColorBucket;

    pub struct PtyHarness {
        master_write: Box<dyn Write + Send>,
        master_read: Box<dyn Read + Send>,
        parser: vt100::Parser,
        child: Box<dyn portable_pty::Child + Send + Sync>,
        cols: u16,
        rows: u16,
    }

    impl PtyHarness {
        fn hi_binary() -> PathBuf {
            let mut path = std::env::current_exe().expect("cannot determine test binary path");
            path.pop();
            if path.ends_with("deps") { path.pop(); }
            path.push("hi");
            path
        }

        pub fn spawn_with_file(file: impl AsRef<Path>, cols: u16, rows: u16) -> anyhow::Result<Self> {
            let pty_system = NativePtySystem::default();
            let pair = pty_system.openpty(PtySize {
                rows, cols, pixel_width: 0, pixel_height: 0,
            })?;

            let mut cmd = CommandBuilder::new(Self::hi_binary());
            cmd.arg(file.as_ref());
            cmd.env("HOME", "/tmp/hi_test_home_nonexistent");

            let child = pair.slave.spawn_command(cmd)?;
            drop(pair.slave);

            Ok(Self {
                master_read: pair.master.try_clone_reader()?,
                master_write: pair.master.take_writer()?,
                parser: vt100::Parser::new(rows, cols, 0),
                child,
                cols,
                rows,
            })
        }

        pub fn send_keys(&mut self, keys: &str) -> anyhow::Result<()> {
            self.master_write.write_all(keys.as_bytes())?;
            self.master_write.flush()?;
            Ok(())
        }

        pub fn wait_for_text(&mut self, needle: &str, timeout: Duration) -> anyhow::Result<()> {
            let start = Instant::now();
            let mut buf = [0u8; 4096];
            loop {
                if start.elapsed() > timeout {
                    anyhow::bail!("Timed out waiting for {:?}", needle);
                }
                match self.master_read.read(&mut buf) {
                    Ok(n) if n > 0 => self.parser.process(&buf[..n]),
                    _ => {}
                }
                let screen = self.parser.screen();
                let mut text = String::new();
                for row in 0..self.rows {
                    for col in 0..self.cols {
                        if let Some(cell) = screen.cell(row, col) {
                            text.push_str(&cell.contents());
                        }
                    }
                }
                if text.contains(needle) { return Ok(()); }
                thread::sleep(Duration::from_millis(20));
            }
        }

        pub fn quit(&mut self) -> anyhow::Result<()> {
            self.send_keys("\x1b\x1b:q!\r")
        }

        pub fn wait_for_exit(&mut self, timeout: Duration) -> anyhow::Result<()> {
            let start = Instant::now();
            loop {
                if start.elapsed() > timeout {
                    let _ = self.child.kill();
                    anyhow::bail!("Child did not exit");
                }
                if let Ok(Some(_)) = self.child.try_wait() {
                    return Ok(());
                }
                thread::sleep(Duration::from_millis(20));
            }
        }
    }

    impl Drop for PtyHarness {
        fn drop(&mut self) {
            let _ = self.send_keys("\x1b:q!\r");
            thread::sleep(Duration::from_millis(50));
            let _ = self.child.kill();
        }
    }
}
