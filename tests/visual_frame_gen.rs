//! Visual Frame Generator — produces a complete ANSI frame for xterm.js rendering.
//!
//! This generates a raw ANSI byte stream that represents one full frame of the
//! `hi` editor rendering a given file.  The output can be fed into xterm.js
//! for pixel-perfect visual regression testing.
//!
//! Run with:
//!   cargo test --test visual_frame_gen -- --nocapture

mod tui_harness;

use std::io::Write;
use crossterm::{
    execute,
    cursor,
    style::{Attribute, Color, SetForegroundColor, SetBackgroundColor, ResetColor, SetAttribute},
};
use unicode_width::UnicodeWidthChar;

use hi::buffer::Buffer;
use hi::config::Config;
use hi::editor::Editor;
use hi::syntax::highlight::{FileType, SyntectHighlighter, SyntectSpan};

/// Generate a complete ANSI frame for the given content and file type.
fn generate_frame(ext: &str, content: &str, cols: u16, rows: u16) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();

    // Setup editor state
    let config = Config::default();
    let mut editor = Editor::new(config, cols, rows);
    let mut buffer = Buffer::new();
    buffer.rope = ropey::Rope::from_str(content);
    editor.buffer = buffer;

    let ft = FileType::from_ext(ext);
    let mut syntect_hl = SyntectHighlighter::with_default_theme(ft);

    let edit_h = (rows as usize).saturating_sub(2); // 2 rows for status bar
    let gutter_width = 4usize; // "NNN "
    let text_w = (cols as usize).saturating_sub(gutter_width);

    // Clear screen and move to top
    execute!(buf, cursor::Hide, cursor::MoveTo(0, 0)).unwrap();

    // Reset syntect state
    syntect_hl.reset_state();

    // Render each visible line
    for screen_row in 0..edit_h {
        let buf_line = editor.scroll_line + screen_row;
        execute!(buf, cursor::MoveTo(0, screen_row as u16)).unwrap();

        if buf_line < editor.buffer.line_count() {
            // Gutter (line number)
            let lnum = format!("{:>3} ", buf_line + 1);
            execute!(buf, SetForegroundColor(Color::DarkGrey)).unwrap();
            write!(buf, "{}", lnum).unwrap();
            execute!(buf, ResetColor).unwrap();

            // Get line text and highlight
            let line = editor.buffer.line_str(buf_line);
            let spans = syntect_hl.highlight_line(&line);

            // Render line with spans
            render_line_to_buf(&mut buf, &line, &spans, text_w);
        } else {
            // Empty line past EOF
            execute!(buf, SetForegroundColor(Color::DarkGrey)).unwrap();
            write!(buf, "~").unwrap();
            execute!(buf, ResetColor).unwrap();
            let pad = (cols as usize).saturating_sub(1);
            write!(buf, "{:width$}", "", width = pad).unwrap();
        }
    }

    // Status bar
    let status_row = edit_h as u16;
    execute!(buf, cursor::MoveTo(0, status_row)).unwrap();
    execute!(buf, SetBackgroundColor(Color::DarkBlue), SetForegroundColor(Color::White)).unwrap();
    let filename = format!(" hi v0.1.2-fix1 | {} | {} lines ", ext, editor.buffer.line_count());
    let status = format!("{:<width$}", filename, width = cols as usize);
    write!(buf, "{}", status).unwrap();
    execute!(buf, ResetColor).unwrap();

    // Mode line
    let mode_row = status_row + 1;
    execute!(buf, cursor::MoveTo(0, mode_row)).unwrap();
    execute!(buf, SetForegroundColor(Color::Blue)).unwrap();
    write!(buf, " NORMAL").unwrap();
    execute!(buf, ResetColor).unwrap();
    let pad = (cols as usize).saturating_sub(7);
    write!(buf, "{:width$}", "", width = pad).unwrap();

    // Show cursor at top-left of editing area
    execute!(buf, cursor::MoveTo(gutter_width as u16, 0), cursor::Show).unwrap();

    buf
}

/// Render one line with syntax highlighting spans into a buffer.
fn render_line_to_buf(buf: &mut Vec<u8>, line: &str, spans: &[SyntectSpan], max_width: usize) {
    let chars: Vec<char> = line.chars().collect();

    let mut limit = 0;
    let mut used_width = 0;
    for ch in &chars {
        let w = UnicodeWidthChar::width(*ch).unwrap_or(0);
        if used_width + w > max_width { break; }
        used_width += w;
        limit += 1;
    }

    if spans.is_empty() {
        let display: String = chars[..limit].iter().collect();
        write!(buf, "{}", display).unwrap();
        let pad = max_width.saturating_sub(used_width);
        if pad > 0 {
            write!(buf, "{:width$}", "", width = pad).unwrap();
        }
        return;
    }

    let line_len = line.len();
    let mut byte_span: Vec<Option<usize>> = vec![None; line_len + 1];
    for (idx, sp) in spans.iter().enumerate() {
        let s = sp.start.min(line_len);
        let e = sp.end.min(line_len);
        for b in s..e {
            byte_span[b] = Some(idx);
        }
    }

    let mut col = 0usize;
    let mut byte_pos = 0usize;
    let mut last_idx: Option<usize> = None;

    for ch in chars.iter().take(limit) {
        let ch_len = ch.len_utf8();
        let cur_idx = byte_span[byte_pos];

        if cur_idx != last_idx {
            execute!(buf, ResetColor, SetAttribute(Attribute::Reset)).unwrap();
            if let Some(idx) = cur_idx {
                let sp = &spans[idx];
                if let Some(ov) = sp.overlay {
                    execute!(buf, SetBackgroundColor(ov.bg_color())).unwrap();
                    if let Some(fg) = ov.fg_color() {
                        execute!(buf, SetForegroundColor(fg)).unwrap();
                    } else {
                        execute!(buf, SetForegroundColor(sp.fg)).unwrap();
                    }
                } else {
                    execute!(buf, SetForegroundColor(sp.fg)).unwrap();
                }
                if sp.bold { execute!(buf, SetAttribute(Attribute::Bold)).unwrap(); }
                if sp.italic { execute!(buf, SetAttribute(Attribute::Italic)).unwrap(); }
            }
            last_idx = cur_idx;
        }

        write!(buf, "{}", ch).unwrap();
        byte_pos += ch_len;
        col += UnicodeWidthChar::width(*ch).unwrap_or(0);
    }

    execute!(buf, ResetColor, SetAttribute(Attribute::Reset)).unwrap();
    let pad = max_width.saturating_sub(col);
    if pad > 0 {
        write!(buf, "{:width$}", "", width = pad).unwrap();
    }
}

#[test]
fn generate_markdown_frame() {
    let md_content = r#"# Hello World

This is a **bold** paragraph with `inline code`.

## Second Heading

- List item one
- List item two

```rust
fn main() {
    println!("Hello, world!");
}
```

> A blockquote here

Normal text after blockquote.

### Third Level Heading

| Column A | Column B |
|----------|----------|
| Cell 1   | Cell 2   |
"#;

    let frame = generate_frame("md", md_content, 80, 30);

    // Write to file for xterm.js consumption
    let out_path = std::path::Path::new("tests/fixtures/md_frame.ans");
    std::fs::create_dir_all(out_path.parent().unwrap()).unwrap();
    std::fs::write(out_path, &frame).unwrap();

    println!("Generated: {} ({} bytes)", out_path.display(), frame.len());
    assert!(frame.len() > 100, "Frame should have substantial content");
}

#[test]
fn generate_rust_frame() {
    let rs_content = r#"use std::io::{self, Write};
use std::collections::HashMap;

/// A simple key-value store.
pub struct Store {
    data: HashMap<String, String>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Insert a key-value pair.
    pub fn insert(&mut self, key: &str, value: &str) {
        self.data.insert(key.to_string(), value.to_string());
    }

    /// Get a value by key.
    pub fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }
}

fn main() -> io::Result<()> {
    let mut store = Store::new();
    store.insert("hello", "world");

    if let Some(val) = store.get("hello") {
        println!("Found: {}", val);
    } else {
        eprintln!("Not found!");
    }

    Ok(())
}
"#;

    let frame = generate_frame("rs", rs_content, 80, 30);

    let out_path = std::path::Path::new("tests/fixtures/rs_frame.ans");
    std::fs::create_dir_all(out_path.parent().unwrap()).unwrap();
    std::fs::write(out_path, &frame).unwrap();

    println!("Generated: {} ({} bytes)", out_path.display(), frame.len());
    assert!(frame.len() > 100, "Frame should have substantial content");
}
