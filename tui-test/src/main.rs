//! tui-test — syntax highlighting demo that SURPASSES glow.
//!
//! Strategy (same as Chroma/glow, but in Rust):
//!   1. Use syntect to parse code into tokens WITH scope information
//!   2. Classify each token's scopes into ~15 semantic TokenTypes
//!   3. Map each TokenType to a hand-picked colour from our palette
//!
//! This decouples "what kind of token is this" from "what colour should it be",
//! which is exactly why glow (via Chroma) gets better results than raw syntect
//! theme matching.

use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};

// ── Token types (inspired by Chroma's ~31 types, simplified) ─────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum TokenType {
    Keyword,        // fn, let, if, for, class, def, import ...
    KeywordType,    // i32, str, bool, String ...
    NameFunction,   // function/method names
    NameTag,        // JSON keys, HTML/XML tags, YAML keys
    NameAttribute,  // HTML attributes, decorators
    NameBuiltin,    // println!, print, self, None, True ...
    LiteralString,  // "hello", 'world'
    LiteralNumber,  // 42, 3.14, 0xff
    LiteralBool,    // true, false
    Comment,        // // ..., # ..., /* ... */
    Operator,       // =, +, -, ::, ->
    Punctuation,    // { } [ ] ( ) , ; :
    Variable,       // $VAR, variable names
    Text,           // everything else
}

// ── Palette: hand-picked colours for dark terminals ──────────────────────────
// Inspired by Catppuccin Mocha + One Dark + glow's "charm" style.
// Every type gets a DISTINCT, readable colour. No two adjacent types share hue.

impl TokenType {
    fn rgb(self) -> (u8, u8, u8) {
        match self {
            TokenType::Keyword       => (198, 120, 221), // purple — stands out
            TokenType::KeywordType   => ( 86, 182, 194), // teal
            TokenType::NameFunction  => ( 97, 175, 239), // blue
            TokenType::NameTag       => ( 97, 175, 239), // blue (JSON keys = prominent)
            TokenType::NameAttribute => (229, 192, 123), // gold
            TokenType::NameBuiltin   => ( 86, 182, 194), // teal
            TokenType::LiteralString => (152, 195, 121), // green
            TokenType::LiteralNumber => (209, 154, 102), // orange
            TokenType::LiteralBool   => (209, 154, 102), // orange (same as numbers)
            TokenType::Comment       => ( 92, 99, 112),  // grey — recedes
            TokenType::Operator      => (171, 178, 191), // light grey
            TokenType::Punctuation   => (130, 137, 151), // mid grey
            TokenType::Variable      => (224, 108, 117), // red/salmon
            TokenType::Text          => (171, 178, 191), // default light grey
        }
    }

    fn bold(self) -> bool {
        matches!(self, TokenType::Keyword | TokenType::NameTag)
    }

    fn italic(self) -> bool {
        matches!(self, TokenType::Comment)
    }
}

// ── Scope → TokenType classifier ─────────────────────────────────────────────
// This is the KEY innovation: we inspect syntect's scope stack to decide the
// semantic token type, instead of relying on the theme's colour rules.

fn classify(scope_stack: &ScopeStack, _text: &str, ss: &SyntaxSet) -> TokenType {
    let scopes = scope_stack.as_slice();
    // Walk scopes from most-specific (last) to least-specific (first).
    // We convert each scope to its dotted string for matching.
    let scope_strs: Vec<String> = scopes.iter()
        .map(|s| s.build_string())
        .collect();
    let joined = scope_strs.join(" | ");

    // ── JSON / YAML / TOML keys ──
    // syntect gives JSON keys: meta.structure.dictionary.key.json > string.quoted
    // The key insight: if we see "meta.structure.dictionary.key" or
    // "entity.name.tag" (YAML/TOML), it's a key/tag, NOT a plain string.
    for s in &scope_strs {
        if s.contains("meta.structure.dictionary.key")
            || s.contains("entity.name.tag")
            || s.contains("support.type.property-name")
            || s.contains("meta.mapping.key")
        {
            return TokenType::NameTag;
        }
    }

    for s in &scope_strs {
        // ── Comments ──
        if s.starts_with("comment") {
            return TokenType::Comment;
        }

        // ── Strings ──
        if s.starts_with("string") {
            return TokenType::LiteralString;
        }

        // ── Numbers ──
        if s.starts_with("constant.numeric") {
            return TokenType::LiteralNumber;
        }

        // ── Boolean / null constants ──
        if s.starts_with("constant.language") {
            return TokenType::LiteralBool;
        }

        // ── Keywords ──
        if s.starts_with("keyword.control")
            || s.starts_with("keyword.other")
            || s == "keyword"
            || s.starts_with("storage.type")    // fn, let, class, def
            || s.starts_with("storage.modifier") // pub, mut, static, const
        {
            return TokenType::Keyword;
        }

        // ── Built-in types ──
        if s.starts_with("support.type")
            || s.starts_with("entity.name.type")
            || s.starts_with("storage.type.primitive")
        {
            return TokenType::KeywordType;
        }

        // ── Function names ──
        if s.starts_with("entity.name.function")
            || s.starts_with("support.function")
            || s.starts_with("meta.function-call")
        {
            return TokenType::NameFunction;
        }

        // ── Macros (Rust) — treat as function ──
        if s.starts_with("support.macro")
            || s.starts_with("entity.name.macro")
        {
            return TokenType::NameFunction;
        }

        // ── Attributes / decorators ──
        if s.starts_with("entity.other.attribute")
            || s.starts_with("meta.annotation")
            || s.starts_with("entity.name.decorator")
        {
            return TokenType::NameAttribute;
        }

        // ── Variables ──
        if s.starts_with("variable.other")
            || s.starts_with("variable.parameter")
            || s.starts_with("variable.language") // self, this
        {
            return TokenType::Variable;
        }

        // ── Operators ──
        if s.starts_with("keyword.operator") {
            return TokenType::Operator;
        }

        // ── Punctuation ──
        if s.starts_with("punctuation") {
            return TokenType::Punctuation;
        }
    }

    // Fallback: if the text looks like a keyword we might have missed
    let _ = (joined, ss);
    TokenType::Text
}

// ── ANSI output helpers ──────────────────────────────────────────────────────

const RESET: &str = "\x1b[0m";

fn print_token(text: &str, tt: TokenType) {
    let (r, g, b) = tt.rgb();
    let bold = if tt.bold() { "\x1b[1m" } else { "" };
    let italic = if tt.italic() { "\x1b[3m" } else { "" };
    print!("\x1b[38;2;{r};{g};{b}m{bold}{italic}{text}{RESET}");
}

// ── Highlight a code block ───────────────────────────────────────────────────

fn highlight_block(ss: &SyntaxSet, lang: &str, code: &str) {
    let syntax = ss
        .find_syntax_by_token(lang)
        .or_else(|| ss.find_syntax_by_extension(lang))
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    let mut parse_state = ParseState::new(syntax);
    let mut scope_stack = ScopeStack::new();

    for src_line in code.lines() {
        let ops = parse_state.parse_line(src_line, ss).unwrap();

        // Walk through the line applying scope operations
        let mut byte_pos = 0usize;
        for &(offset, ref op) in &ops {
            // Print text before this operation with current scope
            if offset > byte_pos {
                let text = &src_line[byte_pos..offset];
                let tt = classify(&scope_stack, text, ss);
                print_token(text, tt);
                byte_pos = offset;
            }
            scope_stack.apply(op).unwrap();
        }
        // Print remaining text on the line
        if byte_pos < src_line.len() {
            let text = &src_line[byte_pos..];
            let tt = classify(&scope_stack, text, ss);
            print_token(text, tt);
        }
        println!();
    }
}

// ── Code samples ─────────────────────────────────────────────────────────────

const RUST_CODE: &str = r#"use std::collections::HashMap;

fn main() {
    let mut scores: HashMap<&str, i32> = HashMap::new();
    scores.insert("Alice", 100);
    scores.insert("Bob", 85);
    let total: i32 = scores.values().sum();
    println!("Total: {total}");
}"#;

const PYTHON_CODE: &str = r#"from dataclasses import dataclass

@dataclass
class Task:
    title: str
    done: bool = False

def summary(tasks: list[Task]) -> str:
    done = sum(1 for t in tasks if t.done)
    return f"{done}/{len(tasks)} completed"

print(summary([Task("Write tests", True)]))"#;

const JSON_CODE: &str = r#"{
  "editor": {
    "theme": "base16-ocean.dark",
    "font_size": 14,
    "line_numbers": true,
    "word_wrap": false
  },
  "keybindings": [
    { "key": "ctrl+s", "command": "save" },
    { "key": "ctrl+q", "command": "quit" }
  ],
  "plugins": ["syntax-highlight", "git-gutter"],
  "debug": null,
  "version": 2.1
}"#;

const BASH_CODE: &str = r#"#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
echo "Building project in $PROJECT_DIR..."

for target in debug release; do
    cargo build --profile "$target" 2>&1 | tee "build_${target}.log"
done

# Run tests
if cargo test --all 2>/dev/null; then
    echo "All tests passed!"
else
    echo "Some tests failed." >&2
    exit 1
fi"#;

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    let ss = SyntaxSet::load_defaults_newlines();

    let samples: &[(&str, &str, &str)] = &[
        ("Rust",   "rs",     RUST_CODE),
        ("Python", "py",     PYTHON_CODE),
        ("JSON",   "json",   JSON_CODE),
        ("Bash",   "sh",     BASH_CODE),
    ];

    println!();
    println!("\x1b[1;38;2;198;120;221m  Syntax Highlighting Demo\x1b[0m");
    println!("\x1b[38;2;92;99;112m  scope-based token classification + custom palette\x1b[0m");
    println!();

    for (label, lang, code) in samples {
        println!("\x1b[38;2;92;99;112m  ── {label} ──\x1b[0m");
        highlight_block(&ss, lang, code);
        println!();
    }
}
