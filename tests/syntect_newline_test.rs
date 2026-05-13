//! Test: verify syntect behavior with and without trailing newlines
//! Run with: cargo test --test syntect_newline_test -- --nocapture

#[test]
fn syntect_newline_comparison() {
    use syntect::easy::HighlightLines;
    use syntect::highlighting::ThemeSet;
    use syntect::parsing::SyntaxSet;

    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let theme = &ts.themes["base16-ocean.dark"];
    let syntax = ss.find_syntax_by_extension("md").unwrap();

    // Test WITH newlines (correct for load_defaults_newlines)
    println!("=== WITH trailing newlines ===");
    let mut hl = HighlightLines::new(syntax, theme);
    let lines_with_nl = vec![
        "# Hello World\n",
        "\n",
        "This is **bold** text.\n",
        "\n",
        "```rust\n",
        "fn main() {}\n",
        "```\n",
    ];
    for line in &lines_with_nl {
        let ranges = hl.highlight_line(line, &ss).unwrap();
        let colors: Vec<String> = ranges.iter()
            .map(|(s, t)| format!("{:?}=({},{},{})", t.trim_end(), s.foreground.r, s.foreground.g, s.foreground.b))
            .collect();
        println!("  {:?} -> {}", line.trim_end(), colors.join(", "));
    }

    // Test WITHOUT newlines (what our code does)
    println!("\n=== WITHOUT trailing newlines ===");
    let mut hl2 = HighlightLines::new(syntax, theme);
    let lines_without_nl = vec![
        "# Hello World",
        "",
        "This is **bold** text.",
        "",
        "```rust",
        "fn main() {}",
        "```",
    ];
    for line in &lines_without_nl {
        let ranges = hl2.highlight_line(line, &ss).unwrap();
        let colors: Vec<String> = ranges.iter()
            .map(|(s, t)| format!("{:?}=({},{},{})", t, s.foreground.r, s.foreground.g, s.foreground.b))
            .collect();
        println!("  {:?} -> {}", line, colors.join(", "));
    }

    // Test: does the state diverge after several lines?
    println!("\n=== State divergence test (20 lines of markdown) ===");
    let long_md: Vec<String> = (0..20).map(|i| {
        match i % 5 {
            0 => format!("## Heading {}", i),
            1 => format!("Normal paragraph text line {}", i),
            2 => format!("- list item {}", i),
            3 => format!("**bold line {}**", i),
            4 => String::new(),
            _ => unreachable!(),
        }
    }).collect();

    let mut hl_with = HighlightLines::new(syntax, theme);
    let mut hl_without = HighlightLines::new(syntax, theme);

    println!("Line | With NL spans | Without NL spans | Match?");
    for (i, line) in long_md.iter().enumerate() {
        let with_nl = format!("{}\n", line);
        let r_with = hl_with.highlight_line(&with_nl, &ss).unwrap();
        let r_without = hl_without.highlight_line(line, &ss).unwrap();

        let count_with = r_with.len();
        let count_without = r_without.len();
        let matches = count_with == count_without;
        
        if !matches || i < 5 {
            println!("  {:2} {:?}: with={} without={} match={}", 
                i, line, count_with, count_without, matches);
        }
    }
}
