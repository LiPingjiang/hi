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
use syntect::easy::HighlightLines;
use syntect::highlighting::{ThemeSet, Style as SynStyle};
use syntect::parsing::SyntaxSet;

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

// ── Syntect-backed highlighter ────────────────────────────────────────────────

/// A single syntect-highlighted span: byte range + RGB colours + font style.
///
/// Unlike the legacy `Span` / `TokenKind`, colours here are exact RGB values
/// taken directly from the Sublime Text theme, so they match the Chat panel
/// code-block colours perfectly.
#[derive(Debug, Clone)]
pub struct SyntectSpan {
    /// Byte start offset in the source line.
    pub start: usize,
    /// Byte end offset (exclusive).
    pub end: usize,
    /// Foreground colour from the theme.
    pub fg: Color,
    /// `true` when the theme marks this token bold.
    pub bold: bool,
    /// `true` when the theme marks this token italic.
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
}

impl OverlayKind {
    pub fn bg_color(self) -> Color {
        match self {
            OverlayKind::SearchMatch        => Color::DarkGrey,
            OverlayKind::SearchMatchCurrent => Color::Yellow,
            OverlayKind::VisualBlock        => Color::DarkGrey,
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
/// Holds a `HighlightLines` state machine so that multi-line constructs
/// (block comments, heredocs, …) are tracked correctly across successive
/// `highlight_line` calls.  Call `reset()` whenever the file or filetype
/// changes.
pub struct SyntectHighlighter {
    filetype: FileType,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    /// Active theme name (matches `MdTheme::syntect_theme`).
    theme_name: String,
    /// Per-line state machine — `None` when filetype is `Plain`.
    state: Option<HighlightLines<'static>>,
}

// SAFETY: `HighlightLines` holds a `&'static SyntaxDefinition` reference
// obtained from a `SyntaxSet` that we own.  We never expose the raw reference
// and always reset the state when the syntax set changes.
unsafe impl Send for SyntectHighlighter {}

impl SyntectHighlighter {
    /// Create a new highlighter for the given filetype.
    ///
    /// `theme_name` should be one of the syntect built-in names, e.g.
    /// `"base16-ocean.dark"`, `"Solarized (dark)"`, etc.  Falls back to the
    /// first available theme if the name is not found.
    pub fn new(filetype: FileType, theme_name: impl Into<String>) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set  = ThemeSet::load_defaults();
        let theme_name = theme_name.into();
        let mut h = Self {
            filetype,
            syntax_set,
            theme_set,
            theme_name,
            state: None,
        };
        h.reset_state();
        h
    }

    /// Convenience constructor that mirrors `MdRenderer::with_default_theme`.
    pub fn with_default_theme(filetype: FileType) -> Self {
        Self::new(filetype, "base16-ocean.dark")
    }

    /// Change the active filetype and reset the parse state.
    pub fn set_filetype(&mut self, ft: FileType) {
        self.filetype = ft;
        self.reset_state();
    }

    /// Switch the syntect colour theme at runtime and reset the parse state.
    pub fn set_theme(&mut self, theme_name: impl Into<String>) {
        self.theme_name = theme_name.into();
        self.reset_state();
    }

    /// Return the current theme name.
    pub fn theme_name(&self) -> &str { &self.theme_name }

    pub fn filetype(&self) -> FileType { self.filetype }

    /// Reset the per-line parse state (call after seeking / reloading the file).
    pub fn reset_state(&mut self) {
        if self.filetype == FileType::Plain {
            self.state = None;
            return;
        }
        let syntax = self.find_syntax();
        let theme  = self.active_theme();
        // SAFETY: we extend the lifetime to 'static because both `syntax_set`
        // and `theme_set` are owned by `self` and outlive `state`.
        let hl: HighlightLines<'_> = HighlightLines::new(syntax, theme);
        let hl: HighlightLines<'static> = unsafe {
            std::mem::transmute(hl)
        };
        self.state = Some(hl);
    }

    /// Highlight one line and return `SyntectSpan`s.
    ///
    /// The internal `HighlightLines` state is advanced so the next call picks
    /// up where this one left off (important for multi-line strings / comments).
    pub fn highlight_line(&mut self, line: &str) -> Vec<SyntectSpan> {
        let Some(ref mut hl) = self.state else {
            return vec![];
        };
        match hl.highlight_line(line, &self.syntax_set) {
            Ok(ranges) => {
                let mut spans = Vec::with_capacity(ranges.len());
                let mut byte_pos = 0usize;
                for (style, text) in &ranges {
                    if text.is_empty() { continue; }
                    let end = byte_pos + text.len();
                    spans.push(SyntectSpan {
                        start: byte_pos,
                        end,
                        fg: syn_color_to_crossterm(style),
                        bold:   style.font_style.contains(syntect::highlighting::FontStyle::BOLD),
                        italic: style.font_style.contains(syntect::highlighting::FontStyle::ITALIC),
                        overlay: None,
                    });
                    byte_pos = end;
                }
                spans
            }
            Err(_) => vec![],
        }
    }

    // ── private helpers ───────────────────────────────────────────────────────

    fn find_syntax(&self) -> &syntect::parsing::SyntaxReference {
        let token = self.filetype.syntect_token();
        self.syntax_set
            .find_syntax_by_token(token)
            .or_else(|| self.syntax_set.find_syntax_by_extension(token))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text())
    }

    fn active_theme(&self) -> &syntect::highlighting::Theme {
        self.theme_set.themes.get(&self.theme_name)
            .unwrap_or_else(|| self.theme_set.themes.values().next().unwrap())
    }
}

/// Convert a syntect `Style` foreground to a crossterm `Color`.
fn syn_color_to_crossterm(style: &SynStyle) -> Color {
    let fg = style.foreground;
    Color::Rgb { r: fg.r, g: fg.g, b: fg.b }
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
        }
    }
}
