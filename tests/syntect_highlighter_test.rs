//! Integration test for SyntectHighlighter with Markdown
//! Run with: cargo test syntect_highlighter_test -- --nocapture

use hi::syntax::highlight::{SyntectHighlighter, FileType};

#[test]
fn test_syntect_highlighter_markdown() {
    let mut hl = SyntectHighlighter::new(FileType::Markdown, "base16-ocean.dark");

    let test_lines = vec![
        "# Hello World",
        "",
        "This is a **bold** and *italic* text.",
        "",
        "```rust",
        "fn main() {}",
        "```",
        "",
        "- list item 1",
        "- list item 2",
    ];

    println!("\n=== SyntectHighlighter Markdown output ===");
    for line in &test_lines {
        let spans = hl.highlight_line(line);
        println!("\nLine: {:?} -> {} spans", line, spans.len());
        for sp in &spans {
            let text = &line[sp.start..sp.end];
            println!("  [{:?}] fg={:?} bold={} italic={} overlay={:?}",
                text, sp.fg, sp.bold, sp.italic, sp.overlay);
        }
    }

    // Now test reset + re-highlight (simulating frame redraw)
    println!("\n=== After reset_state() ===");
    hl.reset_state();
    for line in &test_lines {
        let spans = hl.highlight_line(line);
        println!("\nLine: {:?} -> {} spans", line, spans.len());
        for sp in &spans {
            let text = &line[sp.start..sp.end];
            println!("  [{:?}] fg={:?} bold={} italic={} overlay={:?}",
                text, sp.fg, sp.bold, sp.italic, sp.overlay);
        }
    }

    // Test: reset + pre-parse some lines, then highlight remaining
    // (simulating scroll_line > 0)
    println!("\n=== Simulating scroll_line=5 (pre-parse 0..5, render 5..10) ===");
    hl.reset_state();
    // Pre-parse lines 0..5
    for line in &test_lines[..5] {
        let _ = hl.highlight_line(line);
    }
    // Now render lines 5..10
    for line in &test_lines[5..] {
        let spans = hl.highlight_line(line);
        println!("\nLine: {:?} -> {} spans", line, spans.len());
        for sp in &spans {
            let text = &line[sp.start..sp.end];
            println!("  [{:?}] fg={:?} bold={} italic={} overlay={:?}",
                text, sp.fg, sp.bold, sp.italic, sp.overlay);
        }
    }
}

#[test]
fn test_syntect_highlighter_markdown_spans_not_empty() {
    let mut hl = SyntectHighlighter::new(FileType::Markdown, "base16-ocean.dark");
    
    let heading = "# Hello World";
    let spans = hl.highlight_line(heading);
    
    // Markdown heading should produce non-empty spans
    assert!(!spans.is_empty(), "Markdown heading should produce spans");
    
    // All overlays should be None for normal syntect output
    for sp in &spans {
        assert!(sp.overlay.is_none(), "Normal syntect spans should have overlay=None");
    }
}

#[test]
fn test_syntect_highlighter_reset_produces_consistent_output() {
    let mut hl = SyntectHighlighter::new(FileType::Markdown, "base16-ocean.dark");
    
    let lines = vec!["# Hello", "Some text", "**bold**"];
    
    // First pass
    let mut first_pass = Vec::new();
    for line in &lines {
        first_pass.push(hl.highlight_line(line));
    }
    
    // Reset and second pass
    hl.reset_state();
    let mut second_pass = Vec::new();
    for line in &lines {
        second_pass.push(hl.highlight_line(line));
    }
    
    // Both passes should produce the same number of spans per line
    for (i, (a, b)) in first_pass.iter().zip(second_pass.iter()).enumerate() {
        assert_eq!(a.len(), b.len(), 
            "Line {} span count differs after reset: {} vs {}", i, a.len(), b.len());
    }
}
