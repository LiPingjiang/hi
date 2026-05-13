#[test]
fn dump_json_scopes() {
    use syntect::parsing::{SyntaxSet, ParseState, ScopeStack};
    use syntect::highlighting::ThemeSet;
    use syntect::easy::HighlightLines;

    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let theme = &ts.themes["base16-ocean.dark"];
    
    let syntax = ss.find_syntax_by_token("json").unwrap();
    let mut h = HighlightLines::new(syntax, theme);
    
    let json_line = r#"  "name": "hello","#;
    
    println!("\n=== Syntect highlight colors for JSON ===");
    let ranges = h.highlight_line(json_line, &ss).unwrap();
    for (style, text) in &ranges {
        let fg = style.foreground;
        println!("text={:?}  fg=#{:02X}{:02X}{:02X}", text, fg.r, fg.g, fg.b);
    }
    
    println!("\n=== Syntect scopes for JSON ===");
    let mut state = ParseState::new(syntax);
    let ops = state.parse_line(json_line, &ss).unwrap();
    
    let mut scope_stack = ScopeStack::new();
    let mut pos = 0;
    for (offset, op) in &ops {
        if *offset > pos {
            let text = &json_line[pos..*offset];
            let scopes: Vec<String> = scope_stack.as_slice().iter().map(|s| format!("{}", s)).collect();
            println!("text={:?}  scopes={}", text, scopes.join(" > "));
        }
        scope_stack.apply(op).unwrap();
        pos = *offset;
    }
    if pos < json_line.len() {
        let text = &json_line[pos..];
        let scopes: Vec<String> = scope_stack.as_slice().iter().map(|s| format!("{}", s)).collect();
        println!("text={:?}  scopes={}", text, scopes.join(" > "));
    }
    
    // Also dump a multi-line JSON to see full structure
    println!("\n=== Multi-line JSON scopes ===");
    let json_lines = vec![
        r#"{"#,
        r#"  "key": "value","#,
        r#"  "number": 42,"#,
        r#"  "bool": true"#,
        r#"}"#,
    ];
    
    let mut state2 = ParseState::new(syntax);
    for line in &json_lines {
        let ops = state2.parse_line(line, &ss).unwrap();
        let mut scope_stack2 = ScopeStack::new();
        let mut pos = 0;
        println!("--- line: {:?} ---", line);
        for (offset, op) in &ops {
            if *offset > pos {
                let text = &line[pos..*offset];
                let scopes: Vec<String> = scope_stack2.as_slice().iter().map(|s| format!("{}", s)).collect();
                println!("  text={:?}  scopes={}", text, scopes.join(" > "));
            }
            scope_stack2.apply(op).unwrap();
            pos = *offset;
        }
        if pos < line.len() {
            let text = &line[pos..];
            let scopes: Vec<String> = scope_stack2.as_slice().iter().map(|s| format!("{}", s)).collect();
            println!("  text={:?}  scopes={}", text, scopes.join(" > "));
        }
    }
}
