//! Debug: check if syntect can find and highlight TOML
//! Run with: cargo test --test syntect_toml_debug -- --nocapture

#[test]
fn syntect_toml_lookup() {
    use syntect::parsing::SyntaxSet;
    
    let ss = SyntaxSet::load_defaults_newlines();
    
    // Try various ways to find TOML
    let by_ext = ss.find_syntax_by_extension("toml");
    let by_token = ss.find_syntax_by_token("toml");
    let by_name = ss.find_syntax_by_name("TOML");
    
    println!("TOML by extension: {:?}", by_ext.map(|s| &s.name));
    println!("TOML by token: {:?}", by_token.map(|s| &s.name));
    println!("TOML by name: {:?}", by_name.map(|s| &s.name));
    
    // List all syntaxes
    println!("\nAll syntaxes:");
    for s in ss.syntaxes() {
        println!("  {} (ext: {:?})", s.name, s.file_extensions);
    }
}
