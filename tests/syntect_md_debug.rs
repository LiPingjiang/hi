//! Debug test: print syntect highlighting output for Markdown
//! Run with: cargo test syntect_md_debug -- --nocapture

#[test]
fn syntect_md_debug() {
    use syntect::easy::HighlightLines;
    use syntect::highlighting::ThemeSet;
    use syntect::parsing::SyntaxSet;

    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    println!("\n=== Available themes ===");
    for name in ts.themes.keys() {
        println!("  - {}", name);
    }

    let theme = &ts.themes["base16-ocean.dark"];
    println!("\nTheme default bg: {:?}", theme.settings.background);
    println!("Theme default fg: {:?}", theme.settings.foreground);

    // Find markdown syntax
    let syntax = ss.find_syntax_by_extension("md")
        .or_else(|| ss.find_syntax_by_token("md"));
    println!("\nSyntax by ext 'md': {:?}", syntax.map(|s| &s.name));

    let syntax2 = ss.find_syntax_by_token("md");
    println!("Syntax by token 'md': {:?}", syntax2.map(|s| &s.name));

    let syntax3 = ss.find_syntax_by_extension("markdown");
    println!("Syntax by ext 'markdown': {:?}", syntax3.map(|s| &s.name));

    // List all syntaxes containing "mark" in name
    println!("\nSyntaxes containing 'mark':");
    for s in ss.syntaxes() {
        if s.name.to_lowercase().contains("mark") {
            println!("  - {} (ext: {:?})", s.name, s.file_extensions);
        }
    }

    let syntax = syntax.unwrap_or_else(|| ss.find_syntax_plain_text());
    println!("\nUsing syntax: {}", syntax.name);

    let mut hl = HighlightLines::new(syntax, theme);

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

    for line in &test_lines {
        let ranges = hl.highlight_line(line, &ss).unwrap();
        println!("\nLine: {:?}", line);
        for (style, text) in &ranges {
            let fg = style.foreground;
            let bg = style.background;
            println!("  [{:?}] fg=({},{},{},{}) bg=({},{},{},{})",
                text, fg.r, fg.g, fg.b, fg.a, bg.r, bg.g, bg.b, bg.a);
        }
    }
}
