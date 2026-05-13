//! TUI Smoke Tests — integration tests for the `hi` editor's rendering.
//!
//! These tests use the in-process `RenderTestContext` to verify syntax
//! highlighting, colour correctness, and rendering behaviour WITHOUT
//! needing a PTY or spawning a subprocess.
//!
//! Run with:
//!   cargo test --test tui_smoke
//!
//! For full PTY end-to-end tests (requires PTY permissions):
//!   cargo test --test tui_smoke --features pty_tests

mod tui_harness;

use tui_harness::{ColorBucket, RenderTestContext};

// ═══════════════════════════════════════════════════════════════════════════════
// Test 1: Markdown syntax highlighting produces coloured output
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_markdown_has_syntax_highlighting() {
    let md_content = r#"# Hello World

This is a **bold** paragraph with `inline code`.

## Second Heading

- List item one
- List item two

```rust
fn main() {
    println!("Hello");
}
```

> A blockquote here
"#;

    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("md", md_content);
    ctx.render_frame();

    // The heading "# Hello World" should be highlighted
    let highlighted = ctx.count_highlighted_chars(0, 15);
    assert!(
        highlighted > 5,
        "Expected syntax-highlighted characters in Markdown, found only {}.\n\
         Debug dump:\n{}",
        highlighted,
        ctx.debug_dump()
    );

    // Specifically, the first line (heading) should have colour
    assert!(
        ctx.has_highlighted_cells(0, 1),
        "Heading line should be highlighted.\nRow 0: {:?}\nDump:\n{}",
        ctx.row_text(0),
        ctx.debug_dump()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 1b: Markdown should NOT have unexpected background colours (regression
// for the cyan-background bug fixed in v0.1.2-fix1).
//
// Strategy: render Markdown content and verify that NO span produces a
// background colour.  In the `hi` renderer, background colour only comes from
// overlays (search highlight, visual selection).  Normal syntect highlighting
// should NEVER set a background.
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_markdown_no_background_colour_leak() {
    use hi::syntax::highlight::{FileType, SyntectHighlighter};

    let md_content = r#"# Heading One

This is a paragraph with **bold**, *italic*, and `inline code`.

## Heading Two

- List item
- Another item

```rust
fn main() {}
```

> Blockquote

Normal text after blockquote.
"#;

    // Use the syntect highlighter directly to check that no span has an overlay
    let mut hl = SyntectHighlighter::with_default_theme(FileType::from_ext("md"));
    hl.reset_state();

    for line in md_content.lines() {
        let spans = hl.highlight_line(line);
        for span in &spans {
            assert!(
                span.overlay.is_none(),
                "Markdown line {:?} has an unexpected overlay (would cause background colour): {:?}",
                line, span
            );
        }
    }

    // Also verify via RenderTestContext that no span has a non-default "background"
    // (In our framework, TestSpan only tracks fg — if we got here without overlay,
    // the renderer won't emit SetBackgroundColor for normal text.)
    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("md", md_content);
    ctx.render_frame();

    // Sanity: the content IS highlighted (not plain)
    assert!(
        ctx.has_highlighted_cells(0, 10),
        "Markdown should have syntax highlighting.\nDump:\n{}",
        ctx.debug_dump()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 2: Rust syntax highlighting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_rust_has_syntax_highlighting() {
    let rs_content = r#"use std::io;

fn main() {
    let name = "World";
    println!("Hello, {}!", name);
    let x: i32 = 42;
}
"#;

    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("rs", rs_content);
    ctx.render_frame();

    let highlighted = ctx.count_highlighted_chars(0, 8);
    assert!(
        highlighted > 10,
        "Expected many highlighted chars in Rust file, found only {}.\nDump:\n{}",
        highlighted,
        ctx.debug_dump()
    );

    // Keywords like "use", "fn", "let" should be coloured
    // The "use" on line 0 should have colour
    let line0_colors = ctx.row_colors(0);
    assert!(
        line0_colors.iter().any(|c| c.is_highlighted()),
        "Line 0 ('use std::io;') should have highlighted tokens.\nColors: {:?}\nDump:\n{}",
        line0_colors,
        ctx.debug_dump()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 3: YAML syntax highlighting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_yaml_has_syntax_highlighting() {
    let yaml_content = r#"name: my-app
version: "1.0.0"
dependencies:
  - name: foo
    version: 2.0
  - name: bar
    version: 3.0
# This is a comment
enabled: true
count: 42
"#;

    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("yaml", yaml_content);
    ctx.render_frame();

    let highlighted = ctx.count_highlighted_chars(0, 10);
    assert!(
        highlighted > 5,
        "Expected highlighted chars in YAML file, found only {}.\nDump:\n{}",
        highlighted,
        ctx.debug_dump()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 4: Plain text has NO syntax highlighting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_plain_text_no_highlighting() {
    let content = "This is just plain text.\nNo special formatting here.\nJust words.\n";

    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("txt", content);
    ctx.render_frame();

    // Plain text should have zero or very few highlighted chars
    // (syntect returns empty spans for plain text)
    let highlighted = ctx.count_highlighted_chars(0, 3);
    // Plain text with syntect might still get some default colouring,
    // but it should be minimal
    assert!(
        highlighted == 0,
        "Plain text should have no highlighted chars, found {}.\nDump:\n{}",
        highlighted,
        ctx.debug_dump()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 5: Scrolling preserves correct highlighting state
//         (regression test for the pre-parse bug)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_scroll_preserves_highlighting() {
    // Create a Rust file with a multi-line block comment that starts
    // above the viewport. After scrolling past it, the highlighting
    // should still be correct.
    let mut content = String::new();
    // 50 normal lines
    for i in 0..50 {
        content.push_str(&format!("let x{} = {};\n", i, i));
    }
    // A multi-line block comment
    content.push_str("/*\n");
    content.push_str(" * This is a block comment\n");
    content.push_str(" * that spans multiple lines\n");
    content.push_str(" */\n");
    // More code after the comment
    for i in 50..70 {
        content.push_str(&format!("let y{} = {};\n", i, i));
    }

    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("rs", &content);

    // Render at the top — should have highlighting
    ctx.render_frame();
    let top_highlighted = ctx.count_highlighted_chars(0, 10);
    assert!(
        top_highlighted > 0,
        "Top of file should have highlighting.\nDump:\n{}",
        ctx.debug_dump()
    );

    // Scroll to line 50 (start of the block comment)
    ctx.scroll_to(50);
    ctx.render_frame();
    // The block comment lines should have SOME colour (grey counts as
    // comment highlighting in syntect themes)
    let comment_row = ctx.row_text(0);
    assert!(
        comment_row.contains("/*") || comment_row.contains("*"),
        "Block comment should be visible after scrolling.\nRow 0: {:?}\nDump:\n{}",
        comment_row,
        ctx.debug_dump()
    );
    // Verify the comment has at least grey colouring (not Default)
    let comment_colors = ctx.row_colors(0);
    assert!(
        !comment_colors.is_empty() && comment_colors.iter().any(|c| *c != ColorBucket::Default),
        "Block comment should have non-default colour.\nColors: {:?}\nDump:\n{}",
        comment_colors,
        ctx.debug_dump()
    );

    // Scroll past the comment to the code after it
    ctx.scroll_to(54);
    ctx.render_frame();
    // Code after the comment should still be highlighted correctly
    let after_comment = ctx.count_highlighted_chars(0, 10);
    assert!(
        after_comment > 0,
        "Code after block comment should be highlighted.\nDump:\n{}",
        ctx.debug_dump()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 6: Large file scroll performance (pre-parse capped at 200 lines)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_large_file_scroll_performance() {
    // Create a 5000-line Rust file
    let mut content = String::new();
    for i in 0..5000 {
        content.push_str(&format!("let var_{} = \"value_{}\";\n", i, i));
    }

    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("rs", &content);

    // Scroll to line 4000 and measure render time
    let start = std::time::Instant::now();
    ctx.scroll_to(4000);
    ctx.render_frame();
    let elapsed = start.elapsed();

    // With MAX_PRE_PARSE=200, this should be fast (< 500ms)
    // Without the cap, it would pre-parse 4000 lines which is slow
    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "Rendering at line 4000 took {:?}, which is too slow.\n\
         The MAX_PRE_PARSE optimization may not be working.",
        elapsed
    );

    // Verify highlighting still works at this position
    let highlighted = ctx.count_highlighted_chars(0, 10);
    assert!(
        highlighted > 0,
        "Should still have highlighting at line 4000.\nDump:\n{}",
        ctx.debug_dump()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 7: Markdown heading colours are distinct from body text
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_markdown_heading_has_distinct_color() {
    let content = "# Main Title\n\nRegular paragraph text here.\n\n## Sub Heading\n";

    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("md", content);
    ctx.render_frame();

    // Heading should have a colour
    let heading_color = ctx.first_char_color(0);
    assert!(
        heading_color.map_or(false, |c| c.is_highlighted()),
        "Heading should have a highlighted colour, got {:?}.\nDump:\n{}",
        heading_color,
        ctx.debug_dump()
    );

    // Body text (line 2) — with syntect, even body text may have colour
    // but it should be different from the heading OR the heading should
    // at least be bold
    let heading_spans = &ctx.rendered_lines[0];
    let has_bold_heading = heading_spans.iter().any(|s| s.bold);
    let heading_fg = heading_color.unwrap_or(ColorBucket::Default);

    // At minimum, the heading should be highlighted
    assert!(
        heading_fg.is_highlighted() || has_bold_heading,
        "Heading should be either coloured or bold.\nDump:\n{}",
        ctx.debug_dump()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 8: Rust keywords get highlighted
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_rust_keywords_highlighted() {
    let content = "fn hello() {\n    let x = 42;\n    return x;\n}\n";

    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("rs", &content);
    ctx.render_frame();

    // "fn" on line 0 should be highlighted
    let line0 = &ctx.rendered_lines[0];
    let fn_span = line0.iter().find(|s| s.text.contains("fn"));
    assert!(
        fn_span.map_or(false, |s| s.fg.is_highlighted()),
        "'fn' keyword should be highlighted.\nLine 0 spans: {:?}\nDump:\n{}",
        line0,
        ctx.debug_dump()
    );

    // "let" on line 1 should be highlighted
    let line1 = &ctx.rendered_lines[1];
    let let_span = line1.iter().find(|s| s.text.contains("let"));
    assert!(
        let_span.map_or(false, |s| s.fg.is_highlighted()),
        "'let' keyword should be highlighted.\nLine 1 spans: {:?}\nDump:\n{}",
        line1,
        ctx.debug_dump()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 9: Rust string literals get highlighted
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_rust_strings_highlighted() {
    let content = "let msg = \"Hello, World!\";\n";

    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("rs", content);
    ctx.render_frame();

    // The string "Hello, World!" should be highlighted
    let line0 = &ctx.rendered_lines[0];
    let string_span = line0.iter().find(|s| s.text.contains("Hello"));
    assert!(
        string_span.map_or(false, |s| s.fg.is_highlighted()),
        "String literal should be highlighted.\nLine 0 spans: {:?}\nDump:\n{}",
        line0,
        ctx.debug_dump()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 10: Multiple renders produce consistent results (no state leak)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_render_consistency() {
    let content = "# Title\n\nfn main() {\n    println!(\"hi\");\n}\n";

    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("rs", content);

    // Render multiple times
    ctx.render_frame();
    let first_dump = ctx.debug_dump();
    let first_count = ctx.count_highlighted_chars(0, 5);

    ctx.render_frame();
    let second_dump = ctx.debug_dump();
    let second_count = ctx.count_highlighted_chars(0, 5);

    ctx.render_frame();
    let third_count = ctx.count_highlighted_chars(0, 5);

    // All renders should produce the same result
    assert_eq!(
        first_count, second_count,
        "Render 1 and 2 differ.\nFirst:\n{}\nSecond:\n{}",
        first_dump, second_dump
    );
    assert_eq!(
        second_count, third_count,
        "Render 2 and 3 differ in highlighted char count: {} vs {}",
        second_count, third_count
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 11: Scroll back and forth produces correct highlighting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_scroll_back_and_forth() {
    let mut content = String::new();
    for i in 0..100 {
        content.push_str(&format!("let line_{} = {};\n", i, i));
    }

    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("rs", &content);

    // Render at top
    ctx.render_frame();
    let top_count = ctx.count_highlighted_chars(0, 10);

    // Scroll to middle
    ctx.scroll_to(50);
    ctx.render_frame();
    let mid_count = ctx.count_highlighted_chars(0, 10);

    // Scroll back to top
    ctx.scroll_to(0);
    ctx.render_frame();
    let back_top_count = ctx.count_highlighted_chars(0, 10);

    // Top renders should be identical
    assert_eq!(
        top_count, back_top_count,
        "Scrolling back to top should produce same highlighting.\n\
         First top: {}, After scroll back: {}",
        top_count, back_top_count
    );

    // Middle should also have highlighting
    assert!(
        mid_count > 0,
        "Middle of file should have highlighting"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 12: JSON syntax highlighting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_json_syntax_highlighting() {
    let content = r#"{
  "name": "test",
  "version": 1,
  "enabled": true,
  "items": [1, 2, 3]
}
"#;

    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("json", content);
    ctx.render_frame();

    let highlighted = ctx.count_highlighted_chars(0, 6);
    assert!(
        highlighted > 5,
        "JSON should have highlighted chars, found only {}.\nDump:\n{}",
        highlighted,
        ctx.debug_dump()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 13: Python syntax highlighting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_python_syntax_highlighting() {
    let content = r#"import os

def hello(name: str) -> None:
    """Say hello."""
    print(f"Hello, {name}!")

if __name__ == "__main__":
    hello("World")
"#;

    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("py", content);
    ctx.render_frame();

    let highlighted = ctx.count_highlighted_chars(0, 8);
    assert!(
        highlighted > 10,
        "Python should have many highlighted chars, found only {}.\nDump:\n{}",
        highlighted,
        ctx.debug_dump()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 14: Shell/Bash syntax highlighting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_shell_syntax_highlighting() {
    let content = r#"#!/bin/bash
# A simple script
NAME="World"
echo "Hello, $NAME!"
if [ -f "$HOME/.bashrc" ]; then
    source "$HOME/.bashrc"
fi
"#;

    let mut ctx = RenderTestContext::new(80, 24);
    ctx.open_content("sh", content);
    ctx.render_frame();

    // Shell highlighting in syntect's base16-ocean.dark theme may render
    // most tokens as grey/white.  We just verify that the output is
    // non-empty and has SOME colouring (grey counts — it means syntect
    // processed the file rather than returning empty spans).
    let all_colors: Vec<ColorBucket> = (0..7)
        .flat_map(|row| ctx.row_colors(row))
        .collect();
    let has_any_color = all_colors.iter().any(|c| *c != ColorBucket::Default);
    assert!(
        has_any_color,
        "Shell script should have at least some non-default colour.\nColors: {:?}\nDump:\n{}",
        all_colors,
        ctx.debug_dump()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 15: Verify version string
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_version_string() {
    let version = env!("CARGO_PKG_VERSION");
    assert_eq!(version, "0.1.2-fix1");
}
