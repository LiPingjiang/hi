//! Syntax highlighting — two tiers:
//!
//! * **`Highlighter`** — legacy rule-based engine kept for search-match /
//!   visual-block overlays and as a fallback for plain text.
//! * **`SyntectHighlighter`** — syntect-backed engine that reuses the
//!   `SyntaxSet` / `Theme` already loaded by `MdRenderer`, giving the editor
//!   the same 200+ language, Sublime-Text-quality highlighting as the Chat
//!   panel code blocks.
use std::path::Path;
use crossterm::style::Color;
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Normal,
    Keyword,
    String,
    Number,
    Comment,
    Error,    // ERROR log level
    Warning,  // WARN log level
    Info,     // INFO log level
    Debug,    // DEBUG log level
    Tag,      // XML/HTML tag name
    Attribute,// XML/HTML attribute
    Key,      // YAML/TOML key
    Section,  // TOML section header
    Operator,
    Heading,       // Markdown headings
    Link,          // Markdown links / URLs
    CodeInline,    // Markdown inline code `...`
    CodeBlock,     // Markdown fenced code block markers
    Emphasis,      // Markdown *italic* / _italic_
    Strong,        // Markdown **bold** / __bold__
    ListMarker,    // Markdown list markers (-, *, +, 1.)
    Blockquote,    // Markdown > blockquote
    HorizontalRule,// Markdown --- / *** / ___
    Type,          // Rust/Java type names
    Macro,         // Rust macros
    Lifetime,      // Rust lifetimes
    SearchMatch,
    SearchMatchCurrent,
}

impl TokenKind {
    pub fn fg_color(&self) -> Option<Color> {
        match self {
            TokenKind::Normal           => None,
            TokenKind::Keyword          => Some(Color::Blue),
            TokenKind::String           => Some(Color::Green),
            TokenKind::Number           => Some(Color::Cyan),
            TokenKind::Comment          => Some(Color::DarkGrey),
            TokenKind::Error            => Some(Color::Red),
            TokenKind::Warning          => Some(Color::Yellow),
            TokenKind::Info             => Some(Color::Green),
            TokenKind::Debug            => Some(Color::DarkGrey),
            TokenKind::Tag              => Some(Color::Blue),
            TokenKind::Attribute        => Some(Color::Magenta),
            TokenKind::Key              => Some(Color::Magenta),
            TokenKind::Section          => Some(Color::Cyan),
            TokenKind::Operator         => Some(Color::White),
            TokenKind::Heading           => Some(Color::Magenta),
            TokenKind::Link              => Some(Color::Blue),
            TokenKind::CodeInline        => Some(Color::Yellow),
            TokenKind::CodeBlock         => Some(Color::Yellow),
            TokenKind::Emphasis          => Some(Color::Cyan),
            TokenKind::Strong            => Some(Color::Cyan),
            TokenKind::ListMarker        => Some(Color::Blue),
            TokenKind::Blockquote        => Some(Color::DarkGrey),
            TokenKind::HorizontalRule    => Some(Color::DarkGrey),
            TokenKind::Type              => Some(Color::Cyan),
            TokenKind::Macro             => Some(Color::Yellow),
            TokenKind::Lifetime          => Some(Color::Magenta),
            TokenKind::SearchMatch        => None,
            TokenKind::SearchMatchCurrent => None,
        }
    }

    pub fn bg_color(&self) -> Option<Color> {
        match self {
            TokenKind::SearchMatch        => Some(Color::DarkGrey),
            TokenKind::SearchMatchCurrent => Some(Color::Yellow),
            _ => None,
        }
    }

    pub fn bold(&self) -> bool {
        matches!(self, TokenKind::Keyword | TokenKind::Tag | TokenKind::Section | TokenKind::Heading | TokenKind::Strong)
    }

    pub fn italic(&self) -> bool {
        matches!(self, TokenKind::Comment | TokenKind::Emphasis)
    }
}

/// A span on a single rendered line.
#[derive(Debug, Clone)]
pub struct Span {
    pub start: usize, // byte offset in the line string
    pub end:   usize,
    pub kind:  TokenKind,
}

pub struct Highlighter {
    filetype: FileType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileType {
    Plain,
    Yaml,
    Json,
    Toml,
    Properties,
    Xml,
    Html,
    Shell,
    Log,
    Java,
    Python,
    Markdown,
    Rust,
    Go,
    JavaScript,
    TypeScript,
}

impl FileType {
    /// Create from a bare extension string (without the leading dot).
    pub fn from_ext(ext: &str) -> Self {
        match ext.to_ascii_lowercase().as_str() {
            "yml" | "yaml" => FileType::Yaml,
            "json"          => FileType::Json,
            "toml"          => FileType::Toml,
            "properties" | "env" => FileType::Properties,
            "xml" | "pom"   => FileType::Xml,
            "html" | "htm"  => FileType::Html,
            "sh" | "bash" | "zsh" | "fish" => FileType::Shell,
            "log"           => FileType::Log,
            "java"          => FileType::Java,
            "py"            => FileType::Python,
            "md" | "markdown" | "mkd" => FileType::Markdown,
            "rs"            => FileType::Rust,
            "go"            => FileType::Go,
            "js" | "mjs" | "cjs" => FileType::JavaScript,
            "ts" | "mts" | "cts" => FileType::TypeScript,
            _               => FileType::Plain,
        }
    }

    pub fn from_path(path: &Path) -> Self {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
        match ext.as_str() {
            "yml" | "yaml" => FileType::Yaml,
            "json"          => FileType::Json,
            "toml"          => FileType::Toml,
            "properties" | "env" | "env.example" => FileType::Properties,
            "xml" | "pom" | "xsd" | "wsdl" | "xslt" => FileType::Xml,
            "html" | "htm"  => FileType::Html,
            "sh" | "bash" | "zsh" | "fish" => FileType::Shell,
            "log"           => FileType::Log,
            "java"          => FileType::Java,
            "py"            => FileType::Python,
            "md" | "markdown" | "mkd" => FileType::Markdown,
            "rs"            => FileType::Rust,
            "go"            => FileType::Go,
            "js" | "mjs" | "cjs" => FileType::JavaScript,
            "ts" | "mts" | "cts" => FileType::TypeScript,
            _ => match name.as_str() {
                "makefile" | "dockerfile" | "vagrantfile" => FileType::Shell,
                _ => FileType::Plain,
            }
        }
    }

    pub fn from_content(first_line: &str) -> Option<Self> {
        if first_line.starts_with("#!/bin/bash") || first_line.starts_with("#!/bin/sh")
            || first_line.starts_with("#!/usr/bin/env bash") {
            return Some(FileType::Shell);
        }
        if first_line.starts_with("<?xml") {
            return Some(FileType::Xml);
        }
        None
    }

    pub fn name(&self) -> &'static str {
        match self {
            FileType::Plain      => "TEXT",
            FileType::Yaml       => "YAML",
            FileType::Json       => "JSON",
            FileType::Toml       => "TOML",
            FileType::Properties => "PROPS",
            FileType::Xml        => "XML",
            FileType::Html       => "HTML",
            FileType::Shell      => "SHELL",
            FileType::Log        => "LOG",
            FileType::Java       => "JAVA",
            FileType::Python     => "PYTHON",
            FileType::Markdown   => "MARKDOWN",
            FileType::Rust       => "RUST",
            FileType::Go         => "GO",
            FileType::JavaScript => "JS",
            FileType::TypeScript => "TS",
        }
    }
}

impl Highlighter {
    pub fn new(filetype: FileType) -> Self {
        Self { filetype }
    }

    pub fn for_path(path: &Path) -> Self {
        Self::new(FileType::from_path(path))
    }

    pub fn filetype(&self) -> FileType { self.filetype }

    /// Return spans for a single line of text.
    pub fn highlight_line(&self, line: &str) -> Vec<Span> {
        match self.filetype {
            FileType::Yaml       => highlight_yaml(line),
            FileType::Json       => highlight_json(line),
            FileType::Toml       => highlight_toml(line),
            FileType::Properties => highlight_properties(line),
            FileType::Xml        => highlight_xml(line),
            FileType::Html       => highlight_xml(line), // re-use XML rules
            FileType::Shell      => highlight_shell(line),
            FileType::Log        => highlight_log(line),
            FileType::Java       => highlight_java(line),
            FileType::Python     => highlight_python(line),
            FileType::Markdown   => highlight_markdown(line),
            FileType::Rust       => highlight_rust(line),
            // Tree-sitter handles these; legacy engine returns empty spans
            FileType::Go | FileType::JavaScript | FileType::TypeScript => vec![],
            FileType::Plain      => vec![],
        }
    }
}

// ── Individual language highlighters ──────────────────────────────────────────

fn span(start: usize, end: usize, kind: TokenKind) -> Span { Span { start, end, kind } }

fn highlight_yaml(line: &str) -> Vec<Span> {
    let mut spans = vec![];
    let trimmed = line.trim_start();

    // Comment
    if trimmed.starts_with('#') {
        spans.push(span(0, line.len(), TokenKind::Comment));
        return spans;
    }

    // key: value
    if let Some(colon) = line.find(':') {
        let key_part = &line[..colon];
        if !key_part.trim().is_empty() {
            spans.push(span(0, colon, TokenKind::Key));
            let rest = &line[colon+1..];
            // String value
            let value_start = colon + 1 + rest.len() - rest.trim_start().len();
            let value = rest.trim();
            if (value.starts_with('"') && value.ends_with('"')) ||
               (value.starts_with('\'') && value.ends_with('\'')) {
                spans.push(span(value_start, line.len(), TokenKind::String));
            } else if value.parse::<f64>().is_ok() || value == "true" || value == "false" || value == "null" {
                spans.push(span(value_start, line.len(), TokenKind::Number));
            }
        }
    } else if trimmed.starts_with("- ") {
        // list item — highlight the value part
        let prefix_len = line.len() - trimmed.len() + 2;
        let value = trimmed[2..].trim();
        if (value.starts_with('"') && value.ends_with('"')) ||
           (value.starts_with('\'') && value.ends_with('\'')) {
            spans.push(span(prefix_len, line.len(), TokenKind::String));
        }
    }
    spans
}

fn highlight_json(line: &str) -> Vec<Span> {
    let mut spans = vec![];
    let mut i = 0usize;
    let bytes = line.as_bytes();
    while i < bytes.len() {
        // String
        if bytes[i] == b'"' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' { i += 1; }
                else if bytes[i] == b'"' { i += 1; break; }
                i += 1;
            }
            // Decide if it's a key (followed by :) or value
            let after = line[i..].trim_start();
            if after.starts_with(':') {
                spans.push(span(start, i, TokenKind::Key));
            } else {
                spans.push(span(start, i, TokenKind::String));
            }
            continue;
        }
        // true / false / null
        for kw in &["true", "false", "null"] {
            if line[i..].starts_with(kw) {
                spans.push(span(i, i + kw.len(), TokenKind::Keyword));
                i += kw.len();
            }
        }
        // Number
        if bytes[i].is_ascii_digit() || (bytes[i] == b'-' && i+1 < bytes.len() && bytes[i+1].is_ascii_digit()) {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b'e' || bytes[i] == b'-') {
                i += 1;
            }
            spans.push(span(start, i, TokenKind::Number));
            continue;
        }
        i += 1;
    }
    spans
}

fn highlight_toml(line: &str) -> Vec<Span> {
    let mut spans = vec![];
    let trimmed = line.trim_start();
    // Comment
    if trimmed.starts_with('#') {
        spans.push(span(0, line.len(), TokenKind::Comment));
        return spans;
    }
    // Section header [...]
    if trimmed.starts_with('[') {
        spans.push(span(0, line.len(), TokenKind::Section));
        return spans;
    }
    // key = value
    if let Some(eq) = line.find('=') {
        spans.push(span(0, eq, TokenKind::Key));
        let val_start = eq + 1 + line[eq+1..].len() - line[eq+1..].trim_start().len();
        let val = line[eq+1..].trim();
        if (val.starts_with('"') && val.ends_with('"')) ||
           (val.starts_with('\'') && val.ends_with('\'')) {
            spans.push(span(val_start, line.len(), TokenKind::String));
        } else if val.parse::<f64>().is_ok() || val == "true" || val == "false" {
            spans.push(span(val_start, line.len(), TokenKind::Number));
        }
    }
    spans
}

fn highlight_properties(line: &str) -> Vec<Span> {
    let mut spans = vec![];
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || trimmed.starts_with('!') {
        spans.push(span(0, line.len(), TokenKind::Comment));
        return spans;
    }
    if let Some(eq) = line.find('=').or_else(|| line.find(':')) {
        spans.push(span(0, eq, TokenKind::Key));
        let val_start = eq + 1;
        if val_start < line.len() {
            spans.push(span(val_start, line.len(), TokenKind::String));
        }
    }
    spans
}

fn highlight_xml(line: &str) -> Vec<Span> {
    let mut spans = vec![];
    let mut i = 0usize;
    let bytes = line.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let tag_start = i;
            i += 1;
            // comment <!--
            if line[i..].starts_with("!--") {
                while i < bytes.len() { i += 1; }
                spans.push(span(tag_start, line.len(), TokenKind::Comment));
                break;
            }
            // closing tag </
            let is_close = i < bytes.len() && bytes[i] == b'/';
            if is_close { i += 1; }
            let name_start = i;
            while i < bytes.len() && bytes[i] != b' ' && bytes[i] != b'>' && bytes[i] != b'/' {
                i += 1;
            }
            spans.push(span(tag_start, i, TokenKind::Tag));
            // attributes
            while i < bytes.len() && bytes[i] != b'>' {
                if bytes[i] == b'"' {
                    let s = i;
                    i += 1;
                    while i < bytes.len() && bytes[i] != b'"' { i += 1; }
                    i += 1;
                    spans.push(span(s, i, TokenKind::String));
                } else if bytes[i].is_ascii_alphabetic() {
                    let s = i;
                    while i < bytes.len() && bytes[i] != b'=' && bytes[i] != b' ' && bytes[i] != b'>' { i += 1; }
                    spans.push(span(s, i, TokenKind::Attribute));
                } else {
                    i += 1;
                }
            }
            let _ = name_start;
        } else {
            i += 1;
        }
    }
    spans
}

fn highlight_shell(line: &str) -> Vec<Span> {
    let mut spans = vec![];
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        spans.push(span(0, line.len(), TokenKind::Comment));
        return spans;
    }
    const KEYWORDS: &[&str] = &[
        "if", "then", "else", "elif", "fi", "for", "while", "do", "done",
        "case", "in", "esac", "function", "return", "local", "export", "echo",
        "exit", "source", ".", "set", "unset", "readonly",
    ];
    let mut i = 0usize;
    let chars: Vec<char> = line.chars().collect();
    while i < chars.len() {
        // String
        if chars[i] == '"' || chars[i] == '\'' {
            let quote = chars[i];
            let start_byte = chars[..i].iter().collect::<String>().len();
            let mut j = i + 1;
            while j < chars.len() && chars[j] != quote { j += 1; }
            j += 1;
            let end_byte = chars[..j].iter().collect::<String>().len();
            spans.push(span(start_byte, end_byte, TokenKind::String));
            i = j;
            continue;
        }
        // Variable $VAR
        if chars[i] == '$' {
            let start_byte = chars[..i].iter().collect::<String>().len();
            let mut j = i + 1;
            if j < chars.len() && chars[j] == '{' {
                while j < chars.len() && chars[j] != '}' { j += 1; }
                j += 1;
            } else {
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') { j += 1; }
            }
            let end_byte = chars[..j].iter().collect::<String>().len();
            spans.push(span(start_byte, end_byte, TokenKind::Keyword));
            i = j;
            continue;
        }
        // Keywords
        for kw in KEYWORDS {
            let rest: String = chars[i..].iter().collect();
            if rest.starts_with(kw) {
                let after = rest[kw.len()..].chars().next();
                if after.map_or(true, |c| !c.is_alphanumeric() && c != '_') {
                    let start_byte = chars[..i].iter().collect::<String>().len();
                    let end_byte = start_byte + kw.len();
                    spans.push(span(start_byte, end_byte, TokenKind::Keyword));
                    i += kw.chars().count();
                    break;
                }
            }
        }
        i += 1;
    }
    spans
}

fn highlight_log(line: &str) -> Vec<Span> {
    let upper = line.to_uppercase();
    if upper.contains("ERROR") || upper.contains("EXCEPTION") || upper.contains("FATAL") {
        return vec![span(0, line.len(), TokenKind::Error)];
    }
    if upper.contains("WARN") {
        return vec![span(0, line.len(), TokenKind::Warning)];
    }
    if upper.contains("INFO") {
        return vec![span(0, line.len(), TokenKind::Info)];
    }
    if upper.contains("DEBUG") || upper.contains("TRACE") {
        return vec![span(0, line.len(), TokenKind::Debug)];
    }
    vec![]
}

fn highlight_java(line: &str) -> Vec<Span> {
    let mut spans = vec![];
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("/*") {
        spans.push(span(0, line.len(), TokenKind::Comment));
        return spans;
    }
    const KEYWORDS: &[&str] = &[
        "public", "private", "protected", "static", "final", "abstract",
        "class", "interface", "enum", "extends", "implements", "import",
        "package", "return", "new", "void", "int", "long", "boolean",
        "String", "if", "else", "for", "while", "try", "catch", "throw",
        "throws", "super", "this", "null", "true", "false",
    ];
    add_keyword_spans(&mut spans, line, KEYWORDS, TokenKind::Keyword);
    add_string_spans(&mut spans, line);
    spans
}

fn highlight_python(line: &str) -> Vec<Span> {
    let mut spans = vec![];
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        spans.push(span(0, line.len(), TokenKind::Comment));
        return spans;
    }
    // Decorator
    if trimmed.starts_with('@') {
        let prefix_len = line.len() - trimmed.len();
        let end = prefix_len + trimmed.split_whitespace().next().unwrap_or("@").len();
        spans.push(span(prefix_len, end, TokenKind::Attribute));
    }
    const KEYWORDS: &[&str] = &[
        "def", "class", "import", "from", "as", "return", "if", "elif",
        "else", "for", "while", "with", "try", "except", "finally",
        "raise", "pass", "break", "continue", "lambda", "yield",
        "True", "False", "None", "and", "or", "not", "in", "is",
        "async", "await",
    ];
    add_keyword_spans(&mut spans, line, KEYWORDS, TokenKind::Keyword);
    add_string_spans(&mut spans, line);
    spans
}

fn add_keyword_spans(spans: &mut Vec<Span>, line: &str, keywords: &[&str], kind: TokenKind) {
    for kw in keywords {
        let mut start = 0;
        while let Some(pos) = line[start..].find(kw) {
            let abs = start + pos;
            let before_ok = abs == 0 || !line.as_bytes()[abs-1].is_ascii_alphanumeric();
            let after_ok = abs + kw.len() >= line.len() || !line.as_bytes()[abs+kw.len()].is_ascii_alphanumeric();
            if before_ok && after_ok {
                spans.push(span(abs, abs + kw.len(), kind.clone()));
            }
            start = abs + kw.len().max(1);
            if start >= line.len() { break; }
        }
    }
}

fn add_string_spans(spans: &mut Vec<Span>, line: &str) {
    let mut i = 0usize;
    let bytes = line.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let q = bytes[i];
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' { i += 2; continue; }
                if bytes[i] == q { i += 1; break; }
                i += 1;
            }
            spans.push(span(start, i, TokenKind::String));
        } else {
            i += 1;
        }
    }
}

// ── Markdown highlighter ──────────────────────────────────────────────────────

fn highlight_markdown(line: &str) -> Vec<Span> {
    let mut spans = vec![];
    let trimmed = line.trim_start();

    // Heading: # ## ### etc.
    if trimmed.starts_with('#') {
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if hashes <= 6 && trimmed.chars().nth(hashes).map_or(true, |c| c == ' ') {
            spans.push(span(0, line.len(), TokenKind::Heading));
            return spans;
        }
    }

    // Horizontal rule: --- / *** / ___ (3+ of same char, optionally with spaces)
    {
        let hr_trimmed = trimmed.replace(' ', "");
        if hr_trimmed.len() >= 3
            && (hr_trimmed.chars().all(|c| c == '-')
                || hr_trimmed.chars().all(|c| c == '*')
                || hr_trimmed.chars().all(|c| c == '_'))
        {
            spans.push(span(0, line.len(), TokenKind::HorizontalRule));
            return spans;
        }
    }

    // Fenced code block markers: ``` or ~~~
    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        spans.push(span(0, line.len(), TokenKind::CodeBlock));
        return spans;
    }

    // Blockquote: > text
    if trimmed.starts_with('>') {
        spans.push(span(0, line.len(), TokenKind::Blockquote));
        return spans;
    }

    // List markers: - item, * item, + item, 1. item
    let prefix_len = line.len() - trimmed.len();
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ")
    {
        spans.push(span(prefix_len, prefix_len + 2, TokenKind::ListMarker));
        // Continue to highlight inline elements in the rest
        highlight_markdown_inline(&mut spans, line, prefix_len + 2);
        return spans;
    }
    // Numbered list: 1. 2. etc.
    {
        let digits: usize = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && trimmed[digits..].starts_with(". ") {
            let marker_end = prefix_len + digits + 2;
            spans.push(span(prefix_len, marker_end, TokenKind::ListMarker));
            highlight_markdown_inline(&mut spans, line, marker_end);
            return spans;
        }
    }

    // Default: highlight inline elements
    highlight_markdown_inline(&mut spans, line, 0);
    spans
}

/// Highlight inline Markdown elements: **bold**, *italic*, `code`, [links](url), ![images](url)
fn highlight_markdown_inline(spans: &mut Vec<Span>, line: &str, start_from: usize) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = start_from;

    while i < len {
        // Inline code: `...`
        if bytes[i] == b'`' && !(i + 1 < len && bytes[i + 1] == b'`') {
            let start = i;
            i += 1;
            while i < len && bytes[i] != b'`' { i += 1; }
            if i < len { i += 1; } // consume closing `
            spans.push(span(start, i, TokenKind::CodeInline));
            continue;
        }

        // Bold: **...**
        if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'*') { i += 1; }
            if i + 1 < len { i += 2; } // consume closing **
            spans.push(span(start, i, TokenKind::Strong));
            continue;
        }

        // Bold: __...__
        if i + 1 < len && bytes[i] == b'_' && bytes[i + 1] == b'_' {
            let start = i;
            i += 2;
            while i + 1 < len && !(bytes[i] == b'_' && bytes[i + 1] == b'_') { i += 1; }
            if i + 1 < len { i += 2; }
            spans.push(span(start, i, TokenKind::Strong));
            continue;
        }

        // Italic: *...*
        if bytes[i] == b'*' {
            let start = i;
            i += 1;
            while i < len && bytes[i] != b'*' { i += 1; }
            if i < len { i += 1; }
            spans.push(span(start, i, TokenKind::Emphasis));
            continue;
        }

        // Italic: _..._
        if bytes[i] == b'_' {
            let start = i;
            i += 1;
            while i < len && bytes[i] != b'_' { i += 1; }
            if i < len { i += 1; }
            spans.push(span(start, i, TokenKind::Emphasis));
            continue;
        }

        // Link: [text](url) or image: ![alt](url)
        if bytes[i] == b'[' || (bytes[i] == b'!' && i + 1 < len && bytes[i + 1] == b'[') {
            let start = i;
            if bytes[i] == b'!' { i += 1; }
            i += 1; // skip [
            while i < len && bytes[i] != b']' { i += 1; }
            if i < len { i += 1; } // skip ]
            if i < len && bytes[i] == b'(' {
                i += 1;
                while i < len && bytes[i] != b')' { i += 1; }
                if i < len { i += 1; } // skip )
            }
            spans.push(span(start, i, TokenKind::Link));
            continue;
        }

        i += 1;
    }
}

// ── Rust highlighter ──────────────────────────────────────────────────────────

fn highlight_rust(line: &str) -> Vec<Span> {
    let mut spans = vec![];
    let trimmed = line.trim_start();

    // Line comments
    if trimmed.starts_with("//") {
        spans.push(span(0, line.len(), TokenKind::Comment));
        return spans;
    }

    // Attributes: #[...] or #![...]
    if trimmed.starts_with("#[") || trimmed.starts_with("#![") {
        spans.push(span(0, line.len(), TokenKind::Attribute));
        return spans;
    }

    const KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn",
        "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
        "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
        "self", "Self", "static", "struct", "super", "trait", "true", "type",
        "unsafe", "use", "where", "while", "yield",
    ];
    add_keyword_spans(&mut spans, line, KEYWORDS, TokenKind::Keyword);

    // Macros: word! or word!(
    let mut i = 0usize;
    let bytes = line.as_bytes();
    while i < bytes.len() {
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1; }
            if i < bytes.len() && bytes[i] == b'!' {
                spans.push(span(start, i + 1, TokenKind::Macro));
                i += 1;
            }
            continue;
        }
        // Lifetime: 'a, 'static, etc.
        if bytes[i] == b'\'' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_alphabetic() {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1; }
            spans.push(span(start, i, TokenKind::Lifetime));
            continue;
        }
        i += 1;
    }

    add_string_spans(&mut spans, line);
    // Number literals
    add_number_spans(&mut spans, line);
    spans
}

fn add_number_spans(spans: &mut Vec<Span>, line: &str) {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            // Check that previous char is not alphanumeric (word boundary)
            if i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_') {
                i += 1;
                continue;
            }
            let start = i;
            // Hex: 0x...
            if bytes[i] == b'0' && i + 1 < bytes.len() && (bytes[i + 1] == b'x' || bytes[i + 1] == b'b' || bytes[i + 1] == b'o') {
                i += 2;
                while i < bytes.len() && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') { i += 1; }
            } else {
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b'_' || bytes[i] == b'e' || bytes[i] == b'E') { i += 1; }
            }
            // Type suffix: u8, i32, f64, usize, etc.
            if i < bytes.len() && (bytes[i] == b'u' || bytes[i] == b'i' || bytes[i] == b'f') {
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1; }
            }
            spans.push(span(start, i, TokenKind::Number));
            continue;
        }
        i += 1;
    }
}

// ── Semantic token types (Chroma-inspired) ───────────────────────────────────
//
// Instead of relying on syntect theme rules (which collapse many scopes to the
// same colour), we classify each token's scope stack into a semantic type and
// map that type to a hand-picked colour.  This is exactly how glow/Chroma gets
// distinct colours for JSON keys vs values, function names vs keywords, etc.

/// Semantic token type — determines colour via our custom palette.
///
/// Neon-Minimalist palette: Tokyo Night base with selective neon accents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SemanticToken {
    Keyword,            // fn, let, if, for, class, def, import …
    KeywordReserved,    // control-flow: return, break, continue, yield …
    KeywordType,        // i32, str, bool, String …
    NameFunction,       // function/method names, macros
    NameBuiltin,        // built-in functions: len, print, println …
    NameTag,            // JSON keys, HTML/XML tags, YAML keys
    NameAttribute,      // HTML attributes, decorators
    NameDecorator,      // @decorator, #[attr]
    LiteralString,      // "hello", 'world'
    LiteralStringEscape,// \n, \t, \x1b, \u{…}
    LiteralNumber,      // 42, 3.14, 0xff
    LiteralConst,       // true, false, null, None
    Comment,            // // …, # …, /* … */
    Operator,           // =, +, -, ::, ->
    Punctuation,        // { } [ ] ( ) , ; :
    Variable,           // $VAR, self, this
    // ── Diff / patch ──
    GenericInserted,    // + lines in diffs
    GenericDeleted,     // - lines in diffs
    // ── Markup (Markdown / RST / AsciiDoc) ──
    MarkupHeading,      // # Heading
    MarkupBold,         // **bold**
    MarkupItalic,       // *italic*
    MarkupCode,         // `inline code` and fenced code blocks
    MarkupLink,         // [text](url) — the URL part
    MarkupLinkText,     // [text](url) — the text part
    MarkupList,         // list markers (-, *, 1.)
    MarkupQuote,        // > blockquote
    MarkupRaw,          // raw / verbatim blocks
    MarkupSection,      // entity.name.section (heading text)
    // ── Shell-specific ──
    ShellBuiltin,       // echo, cd, export, source …
    Text,               // everything else
}

impl SemanticToken {
    /// Look up the RGB colour for this token in the given palette.
    pub fn rgb(self, palette: &CodePalette) -> (u8, u8, u8) {
        palette.get(self)
    }

    /// Convert to a crossterm `Color` using the given palette.
    pub fn to_color(self, palette: &CodePalette) -> Color {
        let (r, g, b) = self.rgb(palette);
        Color::Rgb { r, g, b }
    }

    pub fn bold(self) -> bool {
        matches!(self, SemanticToken::Keyword | SemanticToken::KeywordReserved
            | SemanticToken::NameTag
            | SemanticToken::MarkupHeading | SemanticToken::MarkupBold
            | SemanticToken::MarkupSection)
    }

    pub fn italic(self) -> bool {
        matches!(self, SemanticToken::Comment | SemanticToken::MarkupItalic
            | SemanticToken::MarkupQuote)
    }
}

// ── Code palette — per-theme colour table for SemanticToken ──────────────────

/// A complete colour palette for syntax highlighting.
///
/// Each field maps to a `SemanticToken` variant.  Themes provide different
/// palettes; the active palette is stored in `SyntectHighlighter` and
/// `MdRenderer` and can be swapped at runtime via `:theme`.
#[derive(Debug, Clone)]
pub struct CodePalette {
    pub keyword:              (u8, u8, u8),
    pub keyword_reserved:     (u8, u8, u8),
    pub keyword_type:         (u8, u8, u8),
    pub name_function:        (u8, u8, u8),
    pub name_builtin:         (u8, u8, u8),
    pub name_tag:             (u8, u8, u8),
    pub name_attribute:       (u8, u8, u8),
    pub name_decorator:       (u8, u8, u8),
    pub literal_string:       (u8, u8, u8),
    pub literal_string_escape:(u8, u8, u8),
    pub literal_number:       (u8, u8, u8),
    pub literal_const:        (u8, u8, u8),
    pub comment:              (u8, u8, u8),
    pub operator:             (u8, u8, u8),
    pub punctuation:          (u8, u8, u8),
    pub variable:             (u8, u8, u8),
    pub generic_inserted:     (u8, u8, u8),
    pub generic_deleted:      (u8, u8, u8),
    pub markup_heading:       (u8, u8, u8),
    pub markup_bold:          (u8, u8, u8),
    pub markup_italic:        (u8, u8, u8),
    pub markup_code:          (u8, u8, u8),
    pub markup_link:          (u8, u8, u8),
    pub markup_link_text:     (u8, u8, u8),
    pub markup_list:          (u8, u8, u8),
    pub markup_quote:         (u8, u8, u8),
    pub markup_raw:           (u8, u8, u8),
    pub markup_section:       (u8, u8, u8),
    pub shell_builtin:        (u8, u8, u8),
    pub text:                 (u8, u8, u8),
}

impl CodePalette {
    /// Look up the colour for a semantic token.
    pub fn get(&self, token: SemanticToken) -> (u8, u8, u8) {
        match token {
            SemanticToken::Keyword          => self.keyword,
            SemanticToken::KeywordReserved  => self.keyword_reserved,
            SemanticToken::KeywordType      => self.keyword_type,
            SemanticToken::NameFunction     => self.name_function,
            SemanticToken::NameBuiltin      => self.name_builtin,
            SemanticToken::NameTag          => self.name_tag,
            SemanticToken::NameAttribute    => self.name_attribute,
            SemanticToken::NameDecorator    => self.name_decorator,
            SemanticToken::LiteralString    => self.literal_string,
            SemanticToken::LiteralStringEscape => self.literal_string_escape,
            SemanticToken::LiteralNumber    => self.literal_number,
            SemanticToken::LiteralConst     => self.literal_const,
            SemanticToken::Comment          => self.comment,
            SemanticToken::Operator         => self.operator,
            SemanticToken::Punctuation      => self.punctuation,
            SemanticToken::Variable         => self.variable,
            SemanticToken::GenericInserted  => self.generic_inserted,
            SemanticToken::GenericDeleted   => self.generic_deleted,
            SemanticToken::MarkupHeading    => self.markup_heading,
            SemanticToken::MarkupBold       => self.markup_bold,
            SemanticToken::MarkupItalic     => self.markup_italic,
            SemanticToken::MarkupCode       => self.markup_code,
            SemanticToken::MarkupLink       => self.markup_link,
            SemanticToken::MarkupLinkText   => self.markup_link_text,
            SemanticToken::MarkupList       => self.markup_list,
            SemanticToken::MarkupQuote      => self.markup_quote,
            SemanticToken::MarkupRaw        => self.markup_raw,
            SemanticToken::MarkupSection    => self.markup_section,
            SemanticToken::ShellBuiltin     => self.shell_builtin,
            SemanticToken::Text             => self.text,
        }
    }

    // ── Built-in palettes ─────────────────────────────────────────────────

    /// Neon-Minimalist — Tokyo Night base with selective neon accents.
    pub fn neon_minimalist() -> Self {
        Self {
            keyword:              (187, 154, 247), // #BB9AF7  soft purple
            keyword_reserved:     (187, 154, 247), // #BB9AF7
            keyword_type:         ( 42, 195, 222), // #2AC3DE  bright cyan
            name_function:        (122, 162, 247), // #7AA2F7  vivid blue
            name_builtin:         ( 13, 185, 215), // #0DB9D7  neon teal
            name_tag:             (122, 162, 247), // #7AA2F7  blue (JSON keys)
            name_attribute:       (224, 175, 104), // #E0AF68  warm gold
            name_decorator:       (224, 175, 104), // #E0AF68
            literal_string:       (158, 206, 106), // #9ECE6A  fresh green
            literal_string_escape:(224, 175, 104), // #E0AF68  gold
            literal_number:       (255, 158, 100), // #FF9E64  warm orange
            literal_const:        (255, 158, 100), // #FF9E64
            comment:              ( 86,  95, 137), // #565F89  muted slate
            operator:             (247, 118, 142), // #F7768E  neon pink
            punctuation:          (137, 221, 255), // #89DDFF  light cyan
            variable:             (192, 202, 245), // #C0CAF5  soft lavender
            generic_inserted:     ( 68, 157,  68), // #449D44
            generic_deleted:      (176,  57,  57), // #B03939
            markup_heading:       (187, 154, 247), // #BB9AF7
            markup_bold:          (255, 158, 100), // #FF9E64
            markup_italic:        (180, 249, 248), // #B4F9F8  mint
            markup_code:          (224, 175, 104), // #E0AF68
            markup_link:          ( 42, 195, 222), // #2AC3DE
            markup_link_text:     (125, 207, 255), // #7DCFFF
            markup_list:          (122, 162, 247), // #7AA2F7
            markup_quote:         ( 86,  95, 137), // #565F89
            markup_raw:           (224, 175, 104), // #E0AF68
            markup_section:       (169, 177, 214), // #A9B1D6
            shell_builtin:        ( 13, 185, 215), // #0DB9D7
            text:                 (169, 177, 214), // #A9B1D6
        }
    }

    /// Glow Dark — exact colours from glow's Dark Chroma style.
    pub fn glow_dark() -> Self {
        Self {
            keyword:              (  0, 170, 255), // #00AAFF  Keyword
            keyword_reserved:     (  0, 170, 255), // #00AAFF  KeywordReserved
            keyword_type:         (  0, 170, 255), // #00AAFF  KeywordType
            name_function:        (  0, 215, 135), // #00D787  NameFunction
            name_builtin:         (255, 142, 199), // #FF8EC7  NameBuiltin
            name_tag:             (  0, 170, 255), // #00AAFF  NameTag
            name_attribute:       (  0, 215, 135), // #00D787  NameAttribute
            name_decorator:       (  0, 215, 135), // #00D787  NameDecorator
            literal_string:       (198, 150, 105), // #C69669  String
            literal_string_escape:(175, 255, 215), // #AFFFD7  StringEscape
            literal_number:       (110, 239, 192), // #6EEFC0  LiteralNumber
            literal_const:        (  0, 170, 255), // #00AAFF  KeywordConstant
            comment:              (102, 102, 102), // #666666  Comment
            operator:             (239, 128, 128), // #EF8080  Operator
            punctuation:          (232, 232, 168), // #E8E8A8  Punctuation
            variable:             (255, 142, 199), // #FF8EC7  NameVariable
            generic_inserted:     (  0, 215, 135), // #00D787  GenericInserted
            generic_deleted:      (239, 128, 128), // #EF8080  GenericDeleted
            markup_heading:       (  0, 170, 255), // #00AAFF
            markup_bold:          (232, 232, 168), // #E8E8A8
            markup_italic:        (198, 150, 105), // #C69669
            markup_code:          (175, 255, 215), // #AFFFD7
            markup_link:          (  0, 170, 255), // #00AAFF
            markup_link_text:     (  0, 215, 135), // #00D787
            markup_list:          (232, 232, 168), // #E8E8A8
            markup_quote:         (102, 102, 102), // #666666
            markup_raw:           (175, 255, 215), // #AFFFD7
            markup_section:       (232, 232, 232), // #E8E8E8
            shell_builtin:        (255, 142, 199), // #FF8EC7
            text:                 (232, 232, 232), // #E8E8E8
        }
    }

    /// Monokai Pro — warm, high-contrast palette.
    pub fn monokai_pro() -> Self {
        Self {
            keyword:              (255,  97, 136), // #FF6188  red-pink
            keyword_reserved:     (255,  97, 136), // #FF6188
            keyword_type:         (120, 220, 232), // #78DCE8  cyan
            name_function:        (166, 226, 046), // #A6E22E  green
            name_builtin:         (120, 220, 232), // #78DCE8
            name_tag:             (255,  97, 136), // #FF6188
            name_attribute:       (166, 226,  46), // #A6E22E
            name_decorator:       (166, 226,  46), // #A6E22E
            literal_string:       (255, 216, 102), // #FFD866  yellow
            literal_string_escape:(171, 157, 242), // #AB9DF2  purple
            literal_number:       (171, 157, 242), // #AB9DF2  purple
            literal_const:        (171, 157, 242), // #AB9DF2
            comment:              (117, 113, 094), // #75715E  grey-brown
            operator:             (255,  97, 136), // #FF6188
            punctuation:          (200, 200, 200), // #C8C8C8
            variable:             (252, 152, 103), // #FC9867  orange
            generic_inserted:     (166, 226,  46), // #A6E22E
            generic_deleted:      (255,  97, 136), // #FF6188
            markup_heading:       (255,  97, 136), // #FF6188
            markup_bold:          (252, 152, 103), // #FC9867
            markup_italic:        (120, 220, 232), // #78DCE8
            markup_code:          (255, 216, 102), // #FFD866
            markup_link:          (120, 220, 232), // #78DCE8
            markup_link_text:     (166, 226,  46), // #A6E22E
            markup_list:          (255,  97, 136), // #FF6188
            markup_quote:         (117, 113,  94), // #75715E
            markup_raw:           (255, 216, 102), // #FFD866
            markup_section:       (248, 248, 242), // #F8F8F2
            shell_builtin:        (120, 220, 232), // #78DCE8
            text:                 (248, 248, 242), // #F8F8F2
        }
    }

    /// GitHub Dark — clean, professional palette.
    pub fn github_dark() -> Self {
        Self {
            keyword:              (255, 123, 114), // #FF7B72  coral red
            keyword_reserved:     (255, 123, 114), // #FF7B72
            keyword_type:         (255, 123, 114), // #FF7B72
            name_function:        (210, 168, 255), // #D2A8FF  light purple
            name_builtin:         (121, 192, 255), // #79C0FF  blue
            name_tag:             (126, 231, 135), // #7EE787  green
            name_attribute:       (121, 192, 255), // #79C0FF
            name_decorator:       (210, 168, 255), // #D2A8FF
            literal_string:       (165, 214, 255), // #A5D6FF  light blue
            literal_string_escape:(121, 192, 255), // #79C0FF
            literal_number:       (121, 192, 255), // #79C0FF
            literal_const:        (121, 192, 255), // #79C0FF
            comment:              (139, 148, 158), // #8B949E  grey
            operator:             (255, 123, 114), // #FF7B72
            punctuation:          (201, 209, 217), // #C9D1D9
            variable:             (255, 166, 87),  // #FFA657  orange
            generic_inserted:     ( 63, 185, 80),  // #3FB950
            generic_deleted:      (248,  81, 73),  // #F85149
            markup_heading:       (121, 192, 255), // #79C0FF
            markup_bold:          (201, 209, 217), // #C9D1D9
            markup_italic:        (201, 209, 217), // #C9D1D9
            markup_code:          (165, 214, 255), // #A5D6FF
            markup_link:          ( 88, 166, 255), // #58A6FF
            markup_link_text:     (210, 168, 255), // #D2A8FF
            markup_list:          (255, 123, 114), // #FF7B72
            markup_quote:         (139, 148, 158), // #8B949E
            markup_raw:           (165, 214, 255), // #A5D6FF
            markup_section:       (201, 209, 217), // #C9D1D9
            shell_builtin:        (121, 192, 255), // #79C0FF
            text:                 (201, 209, 217), // #C9D1D9
        }
    }

    /// One Dark Pro — Atom-inspired palette.
    pub fn one_dark_pro() -> Self {
        Self {
            keyword:              (198, 120, 221), // #C678DD  purple
            keyword_reserved:     (198, 120, 221), // #C678DD
            keyword_type:         (229, 192, 123), // #E5C07B  yellow
            name_function:        ( 97, 175, 239), // #61AFEF  blue
            name_builtin:         ( 86, 182, 194), // #56B6C2  cyan
            name_tag:             (224, 108, 117), // #E06C75  red
            name_attribute:       (209, 154, 102), // #D19A66  orange
            name_decorator:       (209, 154, 102), // #D19A66
            literal_string:       (152, 195, 121), // #98C379  green
            literal_string_escape:(209, 154, 102), // #D19A66
            literal_number:       (209, 154, 102), // #D19A66
            literal_const:        (209, 154, 102), // #D19A66
            comment:              ( 92, 99,  112), // #5C6370  grey
            operator:             ( 86, 182, 194), // #56B6C2  cyan
            punctuation:          (171, 178, 191), // #ABB2BF
            variable:             (224, 108, 117), // #E06C75  red
            generic_inserted:     (152, 195, 121), // #98C379
            generic_deleted:      (224, 108, 117), // #E06C75
            markup_heading:       (224, 108, 117), // #E06C75
            markup_bold:          (209, 154, 102), // #D19A66
            markup_italic:        (198, 120, 221), // #C678DD
            markup_code:          (152, 195, 121), // #98C379
            markup_link:          ( 97, 175, 239), // #61AFEF
            markup_link_text:     ( 86, 182, 194), // #56B6C2
            markup_list:          (224, 108, 117), // #E06C75
            markup_quote:         ( 92,  99, 112), // #5C6370
            markup_raw:           (152, 195, 121), // #98C379
            markup_section:       (171, 178, 191), // #ABB2BF
            shell_builtin:        ( 86, 182, 194), // #56B6C2
            text:                 (171, 178, 191), // #ABB2BF
        }
    }

    /// Dracula — classic dark theme.
    pub fn dracula() -> Self {
        Self {
            keyword:              (255, 121, 198), // #FF79C6  pink
            keyword_reserved:     (255, 121, 198), // #FF79C6
            keyword_type:         (139, 233, 253), // #8BE9FD  cyan
            name_function:        ( 80, 250, 123), // #50FA7B  green
            name_builtin:         (139, 233, 253), // #8BE9FD
            name_tag:             (255, 121, 198), // #FF79C6
            name_attribute:       ( 80, 250, 123), // #50FA7B
            name_decorator:       ( 80, 250, 123), // #50FA7B
            literal_string:       (241, 250, 140), // #F1FA8C  yellow
            literal_string_escape:(255, 184, 108), // #FFB86C  orange
            literal_number:       (189, 147, 249), // #BD93F9  purple
            literal_const:        (189, 147, 249), // #BD93F9
            comment:              ( 98, 114, 164), // #6272A4  blue-grey
            operator:             (255, 121, 198), // #FF79C6
            punctuation:          (248, 248, 242), // #F8F8F2
            variable:             (248, 248, 242), // #F8F8F2
            generic_inserted:     ( 80, 250, 123), // #50FA7B
            generic_deleted:      (255,  85,  85), // #FF5555
            markup_heading:       (189, 147, 249), // #BD93F9
            markup_bold:          (255, 184, 108), // #FFB86C
            markup_italic:        (139, 233, 253), // #8BE9FD
            markup_code:          (241, 250, 140), // #F1FA8C
            markup_link:          (139, 233, 253), // #8BE9FD
            markup_link_text:     ( 80, 250, 123), // #50FA7B
            markup_list:          (255, 121, 198), // #FF79C6
            markup_quote:         ( 98, 114, 164), // #6272A4
            markup_raw:           (241, 250, 140), // #F1FA8C
            markup_section:       (248, 248, 242), // #F8F8F2
            shell_builtin:        (139, 233, 253), // #8BE9FD
            text:                 (248, 248, 242), // #F8F8F2
        }
    }

    /// Electric Impressionism — vibrant, self-luminous neon palette.
    ///
    /// Inspired by the "glow" aesthetic: high-saturation colours that appear
    /// to emit light against a dark background.
    pub fn electric_impressionism() -> Self {
        Self {
            keyword:              (  0, 245, 255), // #00F5FF  electric cyan
            keyword_reserved:     (183, 138, 255), // #B78AFF  soft violet
            keyword_type:         (  0, 245, 255), // #00F5FF
            name_function:        (  0, 215, 135), // #00D787  neon green
            name_builtin:         (255,  77, 148), // #FF4D94  hot pink
            name_tag:             (  0, 245, 255), // #00F5FF
            name_attribute:       (166, 226,  46), // #A6E22E  lime
            name_decorator:       (183, 138, 255), // #B78AFF
            literal_string:       (255, 230,  80), // #FFE650  warm yellow
            literal_string_escape:(255, 160,  50), // #FFA032  amber
            literal_number:       (110, 239, 192), // #6EEFC0  mint green
            literal_const:        (183, 138, 255), // #B78AFF
            comment:              ( 90, 100, 120), // #5A6478  steel grey
            operator:             (255,  77, 148), // #FF4D94  hot pink
            punctuation:          (200, 220, 255), // #C8DCFF  ice blue
            variable:             (255, 200, 100), // #FFC864  golden
            generic_inserted:     (  0, 215, 135), // #00D787
            generic_deleted:      (255,  77, 148), // #FF4D94
            markup_heading:       (  0, 245, 255), // #00F5FF
            markup_bold:          (255, 200, 100), // #FFC864
            markup_italic:        (183, 138, 255), // #B78AFF
            markup_code:          (166, 226,  46), // #A6E22E
            markup_link:          (  0, 245, 255), // #00F5FF
            markup_link_text:     (  0, 215, 135), // #00D787
            markup_list:          (255,  77, 148), // #FF4D94
            markup_quote:         ( 90, 100, 120), // #5A6478
            markup_raw:           (166, 226,  46), // #A6E22E
            markup_section:       (230, 235, 255), // #E6EBFF
            shell_builtin:        (255,  77, 148), // #FF4D94
            text:                 (230, 235, 255), // #E6EBFF
        }
    }

    /// Synthwave '84 — retro-futuristic neon palette.
    ///
    /// Purple-pink gradients with cyan and yellow accents, evoking
    /// 1980s synthwave album art and retrowave aesthetics.
    pub fn synthwave() -> Self {
        Self {
            keyword:              (255,  40, 150), // #FF2896  neon magenta
            keyword_reserved:     (255,  40, 150), // #FF2896
            keyword_type:         (254, 78,  210), // #FE4ED2  fuchsia
            name_function:        ( 54, 243, 240), // #36F3F0  turquoise
            name_builtin:         (254,  78, 210), // #FE4ED2
            name_tag:             (255,  40, 150), // #FF2896
            name_attribute:       ( 54, 243, 240), // #36F3F0
            name_decorator:       (254,  78, 210), // #FE4ED2
            literal_string:       (255, 241, 118), // #FFF176  bright yellow
            literal_string_escape:(255, 183,  77), // #FFB74D  orange
            literal_number:       (255, 183,  77), // #FFB74D
            literal_const:        (254,  78, 210), // #FE4ED2
            comment:              (105,  90, 140), // #695A8C  muted purple
            operator:             (255,  40, 150), // #FF2896
            punctuation:          (200, 180, 230), // #C8B4E6  lavender
            variable:             ( 54, 243, 240), // #36F3F0
            generic_inserted:     ( 54, 243, 240), // #36F3F0
            generic_deleted:      (255,  40, 150), // #FF2896
            markup_heading:       (255,  40, 150), // #FF2896
            markup_bold:          (255, 241, 118), // #FFF176
            markup_italic:        (254,  78, 210), // #FE4ED2
            markup_code:          (255, 241, 118), // #FFF176
            markup_link:          ( 54, 243, 240), // #36F3F0
            markup_link_text:     (254,  78, 210), // #FE4ED2
            markup_list:          (255,  40, 150), // #FF2896
            markup_quote:         (105,  90, 140), // #695A8C
            markup_raw:           (255, 241, 118), // #FFF176
            markup_section:       (230, 220, 245), // #E6DCF5
            shell_builtin:        (254,  78, 210), // #FE4ED2
            text:                 (230, 220, 245), // #E6DCF5
        }
    }

    /// Look up a palette by name.  Returns `None` for unknown names.
    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "neon-minimalist" | "neon_minimalist" | "dark" | "default"
                => Some(Self::neon_minimalist()),
            "glow-dark" | "glow_dark" | "glow"
                => Some(Self::glow_dark()),
            "monokai-pro" | "monokai_pro" | "monokai"
                => Some(Self::monokai_pro()),
            "github-dark" | "github_dark" | "github"
                => Some(Self::github_dark()),
            "one-dark-pro" | "one_dark_pro" | "one-dark" | "onedark"
                => Some(Self::one_dark_pro()),
            "dracula"
                => Some(Self::dracula()),
            "electric-impressionism" | "electric_impressionism" | "electric"
                => Some(Self::electric_impressionism()),
            "synthwave" | "synthwave-84" | "synthwave_84"
                => Some(Self::synthwave()),
            _ => None,
        }
    }

    /// Return a list of all available theme names (canonical form).
    pub fn available_themes() -> &'static [&'static str] {
        &[
            "neon-minimalist",
            "glow-dark",
            "monokai-pro",
            "github-dark",
            "one-dark-pro",
            "dracula",
            "electric-impressionism",
            "synthwave",
        ]
    }
}

/// Classify a syntect scope stack into a semantic token type.
pub fn classify_scope(scope_stack: &syntect::parsing::ScopeStack, ss: &SyntaxSet) -> SemanticToken {
    let scopes = scope_stack.as_slice();
    let scope_strs: Vec<String> = scopes.iter()
        .map(|s| s.build_string())
        .collect();

    // ── Markup scopes (Markdown / RST / AsciiDoc) ──
    // Check these first because markup files have `text.html.markdown` as the
    // root scope and we want to classify their tokens before falling through
    // to the generic programming-language rules.
    for s in &scope_strs {
        // Headings: markup.heading.1.markdown, entity.name.section.markdown
        if s.starts_with("markup.heading") {
            return SemanticToken::MarkupHeading;
        }
        if s.starts_with("entity.name.section") {
            return SemanticToken::MarkupSection;
        }
        // Bold / italic
        if s.starts_with("markup.bold") {
            return SemanticToken::MarkupBold;
        }
        if s.starts_with("markup.italic") {
            return SemanticToken::MarkupItalic;
        }
        // Inline code and fenced code blocks
        if s.starts_with("markup.raw") {
            return SemanticToken::MarkupCode;
        }
        // Links: meta.link, markup.underline.link
        if s.starts_with("markup.underline.link") || s.starts_with("meta.link.inet") {
            return SemanticToken::MarkupLink;
        }
        if s.starts_with("meta.link.inline") || s.starts_with("meta.image.inline") {
            return SemanticToken::MarkupLinkText;
        }
        // Lists
        if s.starts_with("markup.list") {
            return SemanticToken::MarkupList;
        }
        // Blockquotes
        if s.starts_with("markup.quote") {
            return SemanticToken::MarkupQuote;
        }
        // Language name in fenced code blocks: constant.other.language-name
        if s.starts_with("constant.other.language-name") {
            return SemanticToken::MarkupCode;
        }
        // Punctuation inside markup (**, `, ```, #, etc.)
        if s.starts_with("punctuation.definition.heading") {
            return SemanticToken::MarkupHeading;
        }
        if s.starts_with("punctuation.definition.bold") {
            return SemanticToken::MarkupBold;
        }
        if s.starts_with("punctuation.definition.italic") {
            return SemanticToken::MarkupItalic;
        }
        if s.starts_with("punctuation.definition.raw")
            || s.starts_with("punctuation.definition.code-fence")
        {
            return SemanticToken::MarkupCode;
        }
        if s.starts_with("punctuation.definition.link")
            || s.starts_with("punctuation.definition.metadata")
            || s.starts_with("punctuation.definition.string.begin.markdown")
            || s.starts_with("punctuation.definition.string.end.markdown")
        {
            return SemanticToken::MarkupLink;
        }
        if s.starts_with("punctuation.definition.blockquote") {
            return SemanticToken::MarkupQuote;
        }
        if s.starts_with("punctuation.definition.list_item") {
            return SemanticToken::MarkupList;
        }
    }

    // ── JSON / YAML / TOML keys (must check BEFORE strings) ──
    for s in &scope_strs {
        if s.contains("meta.structure.dictionary.key")
            || s.contains("entity.name.tag")
            || s.contains("support.type.property-name")
            || s.contains("meta.mapping.key")
        {
            return SemanticToken::NameTag;
        }
    }

    for s in &scope_strs {
        if s.starts_with("comment") {
            return SemanticToken::Comment;
        }
        // String escape sequences: constant.character.escape
        if s.starts_with("constant.character.escape") {
            return SemanticToken::LiteralStringEscape;
        }
        if s.starts_with("string") {
            return SemanticToken::LiteralString;
        }
        if s.starts_with("constant.numeric") {
            return SemanticToken::LiteralNumber;
        }
        if s.starts_with("constant.language") {
            return SemanticToken::LiteralConst;
        }
        // Diff / patch scopes
        if s.starts_with("markup.inserted") {
            return SemanticToken::GenericInserted;
        }
        if s.starts_with("markup.deleted") {
            return SemanticToken::GenericDeleted;
        }
        // Control-flow keywords → KeywordReserved
        if s.starts_with("keyword.control.return")
            || s.starts_with("keyword.control.break")
            || s.starts_with("keyword.control.continue")
            || s.starts_with("keyword.control.yield")
            || s.starts_with("keyword.control.throw")
            || s.starts_with("keyword.control.raise")
        {
            return SemanticToken::KeywordReserved;
        }
        if s.starts_with("keyword.control")
            || s.starts_with("keyword.other")
            || s == "keyword"
            || s.starts_with("storage.type")
            || s.starts_with("storage.modifier")
        {
            return SemanticToken::Keyword;
        }
        if s.starts_with("support.type")
            || s.starts_with("entity.name.type")
            || s.starts_with("storage.type.primitive")
        {
            return SemanticToken::KeywordType;
        }
        // Decorators / annotations → NameDecorator
        if s.starts_with("entity.name.decorator")
            || s.starts_with("meta.annotation")
            || s.starts_with("punctuation.definition.annotation")
        {
            return SemanticToken::NameDecorator;
        }
        if s.starts_with("entity.name.function")
            || s.starts_with("meta.function-call")
            || s.starts_with("support.macro")
            || s.starts_with("entity.name.macro")
        {
            return SemanticToken::NameFunction;
        }
        // Built-in functions: support.function.builtin.*
        if s.starts_with("support.function.builtin") {
            return SemanticToken::NameBuiltin;
        }
        if s.starts_with("entity.other.attribute") {
            return SemanticToken::NameAttribute;
        }
        // Shell builtins & commands: support.function.* (echo, cd, test …)
        if s.starts_with("support.function") {
            return SemanticToken::ShellBuiltin;
        }
        // External commands (cat, grep, awk …): variable.function.shell
        if s.starts_with("variable.function") {
            return SemanticToken::NameFunction;
        }
        if s.starts_with("variable.other")
            || s.starts_with("variable.parameter")
            || s.starts_with("variable.language")
        {
            return SemanticToken::Variable;
        }
        if s.starts_with("keyword.operator") {
            return SemanticToken::Operator;
        }
        if s.starts_with("punctuation") {
            return SemanticToken::Punctuation;
        }
    }

    let _ = ss;
    SemanticToken::Text
}

// ── Syntect-backed highlighter ────────────────────────────────────────────────

/// A single syntect-highlighted span: byte range + RGB colours + font style.
#[derive(Debug, Clone)]
pub struct SyntectSpan {
    /// Byte start offset in the source line.
    pub start: usize,
    /// Byte end offset (exclusive).
    pub end: usize,
    /// Foreground colour.
    pub fg: Color,
    /// `true` when the token should be bold.
    pub bold: bool,
    /// `true` when the token should be italic.
    pub italic: bool,
    /// Special overlay kind (search match, visual block).  `None` for normal
    /// syntect tokens.
    pub overlay: Option<OverlayKind>,
}

/// Overlay kinds that are painted on top of syntect colours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayKind {
    SearchMatch,
    SearchMatchCurrent,
    VisualBlock,
    VisualChar,
    VisualLine,
}

impl OverlayKind {
    pub fn bg_color(self) -> Color {
        match self {
            OverlayKind::SearchMatch        => Color::DarkGrey,
            OverlayKind::SearchMatchCurrent => Color::Yellow,
            OverlayKind::VisualBlock        => Color::DarkGrey,
            OverlayKind::VisualChar         => Color::Rgb { r: 68, g: 68, b: 120 },
            OverlayKind::VisualLine         => Color::Rgb { r: 68, g: 68, b: 120 },
        }
    }
    pub fn fg_color(self) -> Option<Color> {
        match self {
            OverlayKind::SearchMatchCurrent => Some(Color::Black),
            _ => None,
        }
    }
}

/// Syntect-backed, stateful per-file highlighter.
///
/// Uses `ParseState` + `ScopeStack` (not `HighlightLines`) so we can inspect
/// the raw scope stack and classify tokens ourselves via `classify_scope()`.
/// This gives us Chroma-quality colour differentiation (e.g. JSON key ≠ value).
pub struct SyntectHighlighter {
    filetype: FileType,
    syntax_set: SyntaxSet,
    /// Active theme name — kept for API compat but no longer drives colours.
    theme_name: String,
    /// Active code palette — determines colours for each SemanticToken.
    palette: CodePalette,
    /// Per-line parse state — `None` when filetype is `Plain`.
    parse_state: Option<ParseState>,
    /// Scope stack tracks the current nesting of scopes across lines.
    scope_stack: ScopeStack,
}

unsafe impl Send for SyntectHighlighter {}

impl SyntectHighlighter {
    /// Create a new highlighter for the given filetype.
    pub fn new(filetype: FileType, theme_name: impl Into<String>) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_name = theme_name.into();
        let palette = CodePalette::by_name(&theme_name)
            .unwrap_or_else(CodePalette::neon_minimalist);
        let mut h = Self {
            filetype,
            syntax_set,
            theme_name,
            palette,
            parse_state: None,
            scope_stack: ScopeStack::new(),
        };
        h.reset_state();
        h
    }

    /// Convenience constructor.
    pub fn with_default_theme(filetype: FileType) -> Self {
        Self::new(filetype, "neon-minimalist")
    }

    /// Change the active filetype and reset the parse state.
    pub fn set_filetype(&mut self, ft: FileType) {
        self.filetype = ft;
        self.reset_state();
    }

    /// Switch the theme (palette + name).
    pub fn set_theme(&mut self, theme_name: impl Into<String>) {
        let name = theme_name.into();
        if let Some(p) = CodePalette::by_name(&name) {
            self.palette = p;
        }
        self.theme_name = name;
    }

    /// Return the current theme name.
    pub fn theme_name(&self) -> &str { &self.theme_name }

    /// Return a reference to the active code palette.
    pub fn palette(&self) -> &CodePalette { &self.palette }

    pub fn filetype(&self) -> FileType { self.filetype }

    /// Reset the per-line parse state (call after seeking / reloading the file).
    pub fn reset_state(&mut self) {
        self.scope_stack = ScopeStack::new();
        if self.filetype == FileType::Plain {
            self.parse_state = None;
            return;
        }
        let syntax = self.find_syntax();
        // SAFETY: ParseState borrows from SyntaxSet which we own and never drop.
        let ps: ParseState = ParseState::new(syntax);
        let ps: ParseState = unsafe { std::mem::transmute(ps) };
        self.parse_state = Some(ps);
    }

    /// Highlight one line and return `SyntectSpan`s.
    ///
    /// The internal parse state is advanced so the next call picks up where
    /// this one left off (important for multi-line strings / comments).
    ///
    /// **Important**: `line` from the editor buffer has its trailing `\n`
    /// stripped, but `SyntaxSet::load_defaults_newlines()` grammars rely on
    /// `\n` to close line-scoped constructs (e.g. `comment.line`).  We
    /// therefore append `\n` before parsing and clamp all byte offsets to
    /// `line.len()` so the caller never sees the synthetic newline.
    pub fn highlight_line(&mut self, line: &str) -> Vec<SyntectSpan> {
        let Some(ref mut ps) = self.parse_state else {
            return vec![];
        };
        // Append \n so line-end anchors in grammars fire correctly.
        let line_with_nl = format!("{}\n", line);
        let ops = match ps.parse_line(&line_with_nl, &self.syntax_set) {
            Ok(ops) => ops,
            Err(_) => return vec![],
        };

        let orig_len = line.len(); // byte length without the synthetic \n
        let mut spans = Vec::new();
        let mut byte_pos = 0usize;

        for &(offset, ref op) in &ops {
            // Clamp offset to orig_len so we never emit spans covering the \n.
            let clamped = offset.min(orig_len);
            if clamped > byte_pos {
                let text = &line[byte_pos..clamped];
                if !text.is_empty() {
                    let tt = classify_scope(&self.scope_stack, &self.syntax_set);
                    spans.push(SyntectSpan {
                        start: byte_pos,
                        end: clamped,
                        fg: tt.to_color(&self.palette),
                        bold: tt.bold(),
                        italic: tt.italic(),
                        overlay: None,
                    });
                }
                byte_pos = clamped;
            }
            self.scope_stack.apply(op).ok();
        }

        // Remaining text after last scope operation (up to orig_len only).
        if byte_pos < orig_len {
            let tt = classify_scope(&self.scope_stack, &self.syntax_set);
            spans.push(SyntectSpan {
                start: byte_pos,
                end: orig_len,
                fg: tt.to_color(&self.palette),
                bold: tt.bold(),
                italic: tt.italic(),
                overlay: None,
            });
        }

        spans
    }

    /// Access the syntax set (needed by mdrender for code blocks).
    pub fn syntax_set(&self) -> &SyntaxSet {
        &self.syntax_set
    }

    // ── private helpers ───────────────────────────────────────────────────────

    fn find_syntax(&self) -> &syntect::parsing::SyntaxReference {
        let token = self.filetype.syntect_token();
        self.syntax_set
            .find_syntax_by_token(token)
            .or_else(|| self.syntax_set.find_syntax_by_extension(token))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text())
    }
}

impl FileType {
    /// Return the token/extension string that syntect uses to look up the
    /// `SyntaxDefinition` for this filetype.
    pub fn syntect_token(self) -> &'static str {
        match self {
            FileType::Plain      => "txt",
            FileType::Yaml       => "yaml",
            FileType::Json       => "json",
            FileType::Toml       => "toml",
            FileType::Properties => "properties",
            FileType::Xml        => "xml",
            FileType::Html       => "html",
            FileType::Shell      => "sh",
            FileType::Log        => "log",
            FileType::Java       => "java",
            FileType::Python     => "py",
            FileType::Markdown   => "md",
            FileType::Rust       => "rs",
            FileType::Go         => "go",
            FileType::JavaScript => "js",
            FileType::TypeScript => "ts",
        }
    }
}
