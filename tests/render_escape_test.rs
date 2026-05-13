//! Test: capture crossterm escape sequences to verify no unexpected background colors
//! Run with: cargo test render_escape_test -- --nocapture

use std::io::{Write, Cursor};
use crossterm::{execute, style::{Color, SetForegroundColor, SetBackgroundColor, ResetColor, SetAttribute, Attribute}};

use hi::syntax::highlight::{SyntectHighlighter, SyntectSpan, FileType};

/// Simulate render_line_with_spans logic, capturing output to a buffer
fn simulate_render(line: &str, spans: &[SyntectSpan], max_width: usize) -> Vec<u8> {
    use unicode_width::UnicodeWidthChar;
    
    let mut buf: Vec<u8> = Vec::new();
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
        return buf;
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
    }
    
    execute!(buf, ResetColor, SetAttribute(Attribute::Reset)).unwrap();
    buf
}

#[test]
fn test_no_unexpected_background_in_markdown_render() {
    let mut hl = SyntectHighlighter::new(FileType::Markdown, "base16-ocean.dark");
    
    let test_lines = vec![
        "# Hello World",
        "This is a **bold** and *italic* text.",
        "```rust",
        "fn main() {}",
        "```",
        "- list item 1",
    ];
    
    for line in &test_lines {
        let spans = hl.highlight_line(line);
        let output = simulate_render(line, &spans, 120);
        let output_str = String::from_utf8_lossy(&output);
        
        // Check for SetBackgroundColor escape sequences (ESC[48;...)
        // Normal syntect spans should NEVER set background color
        let has_bg = output_str.contains("\x1b[48;");
        println!("Line: {:?}", line);
        println!("  Output: {:?}", output_str);
        println!("  Has background color: {}", has_bg);
        assert!(!has_bg, "Line {:?} should not have background color set, but found ESC[48;... in output", line);
    }
}

#[test]
fn test_no_unexpected_background_in_toml_render() {
    let mut hl = SyntectHighlighter::new(FileType::Toml, "base16-ocean.dark");
    
    let test_lines = vec![
        "[package]",
        "name = \"hi\"",
        "version = \"0.1.2\"",
        "# comment",
    ];
    
    for line in &test_lines {
        let spans = hl.highlight_line(line);
        let output = simulate_render(line, &spans, 120);
        let output_str = String::from_utf8_lossy(&output);
        
        let has_bg = output_str.contains("\x1b[48;");
        println!("Line: {:?}", line);
        println!("  Output: {:?}", output_str);
        println!("  Has background color: {}", has_bg);
        assert!(!has_bg, "Line {:?} should not have background color set", line);
    }
}

#[test]
fn test_no_unexpected_background_in_rust_render() {
    let mut hl = SyntectHighlighter::new(FileType::Rust, "base16-ocean.dark");
    
    let test_lines = vec![
        "fn main() {",
        "    let x = 42;",
        "    println!(\"hello\");",
        "}",
        "// comment",
        "use std::io;",
    ];
    
    for line in &test_lines {
        let spans = hl.highlight_line(line);
        let output = simulate_render(line, &spans, 120);
        let output_str = String::from_utf8_lossy(&output);
        
        let has_bg = output_str.contains("\x1b[48;");
        println!("Line: {:?}", line);
        println!("  Output: {:?}", output_str);
        println!("  Has background color: {}", has_bg);
        assert!(!has_bg, "Line {:?} should not have background color set", line);
    }
}
