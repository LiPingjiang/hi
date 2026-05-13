use syntect::parsing::{SyntaxSet, ParseState, ScopeStack};

fn main() {
    let ss = SyntaxSet::load_defaults_newlines();
    let syntax = ss.find_syntax_by_token("sh")
        .or_else(|| ss.find_syntax_by_extension("sh"))
        .expect("sh syntax not found");

    println!("Syntax name: {}", syntax.name);

    let mut ps = ParseState::new(syntax);
    let mut scope_stack = ScopeStack::new();

    // Typical shell script lines
    let lines = vec![
        "#!/bin/bash",
        "",
        "# This is a comment",
        "export PATH=\"/usr/local/bin:$PATH\"",
        "MY_VAR=\"hello world\"",
        "echo \"Hello $USER\"",
        "if [ -f /etc/passwd ]; then",
        "    cat /etc/passwd | grep root",
        "fi",
        "for i in 1 2 3; do",
        "    echo $i",
        "done",
        "function my_func() {",
        "    return 0",
        "}",
    ];

    for src_line in &lines {
        println!("\n>>> {:?}", src_line);
        match ps.parse_line(&format!("{}\n", src_line), &ss) {
            Ok(ops) => {
                let full_line = format!("{}\n", src_line);
                let mut byte_pos = 0usize;
                for &(offset, ref op) in &ops {
                    if offset > byte_pos && offset <= full_line.len() {
                        let text = &full_line[byte_pos..offset];
                        if !text.trim().is_empty() {
                            let scopes: Vec<String> = scope_stack
                                .as_slice()
                                .iter()
                                .map(|s| s.build_string())
                                .collect();
                            println!("  text={:20?} scopes={:?}", text, scopes);
                        }
                    }
                    byte_pos = offset;
                    scope_stack.apply(op).ok();
                }
                if byte_pos < full_line.len() {
                    let text = &full_line[byte_pos..];
                    if !text.trim().is_empty() {
                        let scopes: Vec<String> = scope_stack
                            .as_slice()
                            .iter()
                            .map(|s| s.build_string())
                            .collect();
                        println!("  text={:20?} scopes={:?}", text, scopes);
                    }
                }
            }
            Err(e) => println!("  ERROR: {:?}", e),
        }
    }
}
