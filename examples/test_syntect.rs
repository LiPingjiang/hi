use syntect::parsing::{SyntaxSet, ParseState, ScopeStack};

fn main() {
    let ss = SyntaxSet::load_defaults_newlines();

    // Check if sh and md syntaxes exist
    for token in &["sh", "md", "json", "rs", "py"] {
        let found = ss
            .find_syntax_by_token(token)
            .or_else(|| ss.find_syntax_by_extension(token));
        match found {
            Some(s) => println!("OK {}: found syntax '{}'", token, s.name),
            None => println!("MISSING {}: NOT FOUND", token),
        }
    }

    // Test highlighting a shell line
    println!("\n--- Shell highlighting test ---");
    if let Some(syntax) = ss
        .find_syntax_by_token("sh")
        .or_else(|| ss.find_syntax_by_extension("sh"))
    {
        let mut ps = ParseState::new(syntax);
        let mut scope_stack = ScopeStack::new();
        let lines = vec!["#!/bin/bash", "echo \"hello world\"", "if [ -f foo ]; then"];
        for src_line in &lines {
            match ps.parse_line(src_line, &ss) {
                Ok(ops) => {
                    println!("  line: {:?}", src_line);
                    println!("  ops count: {}", ops.len());
                    let mut byte_pos = 0usize;
                    for &(offset, ref op) in &ops {
                        if offset > byte_pos && offset <= src_line.len() {
                            let text = &src_line[byte_pos..offset];
                            let scopes: Vec<String> = scope_stack
                                .as_slice()
                                .iter()
                                .map(|s| s.build_string())
                                .collect();
                            println!("    text={:?} scopes={:?}", text, scopes);
                        }
                        byte_pos = offset;
                        scope_stack.apply(op).ok();
                    }
                    if byte_pos < src_line.len() {
                        let text = &src_line[byte_pos..];
                        let scopes: Vec<String> = scope_stack
                            .as_slice()
                            .iter()
                            .map(|s| s.build_string())
                            .collect();
                        println!("    text={:?} scopes={:?}", text, scopes);
                    }
                }
                Err(e) => println!("  ERROR: {:?}", e),
            }
        }
    }

    // Test highlighting markdown
    println!("\n--- Markdown highlighting test ---");
    if let Some(syntax) = ss
        .find_syntax_by_token("md")
        .or_else(|| ss.find_syntax_by_extension("md"))
    {
        let mut ps = ParseState::new(syntax);
        let mut scope_stack = ScopeStack::new();
        let lines = vec![
            "# Hello",
            "Some **bold** text",
            "```rust",
            "let x = 42;",
            "```",
        ];
        for src_line in &lines {
            match ps.parse_line(src_line, &ss) {
                Ok(ops) => {
                    println!("  line: {:?}", src_line);
                    let mut byte_pos = 0usize;
                    for &(offset, ref op) in &ops {
                        if offset > byte_pos && offset <= src_line.len() {
                            let text = &src_line[byte_pos..offset];
                            let scopes: Vec<String> = scope_stack
                                .as_slice()
                                .iter()
                                .map(|s| s.build_string())
                                .collect();
                            println!("    text={:?} scopes={:?}", text, scopes);
                        }
                        byte_pos = offset;
                        scope_stack.apply(op).ok();
                    }
                    if byte_pos < src_line.len() {
                        let text = &src_line[byte_pos..];
                        let scopes: Vec<String> = scope_stack
                            .as_slice()
                            .iter()
                            .map(|s| s.build_string())
                            .collect();
                        println!("    text={:?} scopes={:?}", text, scopes);
                    }
                }
                Err(e) => println!("  ERROR: {:?}", e),
            }
        }
    }
}
