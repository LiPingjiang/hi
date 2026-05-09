//! Rule-based syntax highlighting (no tree-sitter in Phase 1).
use std::path::Path;
use crossterm::style::Color;

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
        matches!(self, TokenKind::Keyword | TokenKind::Tag | TokenKind::Section)
    }

    pub fn italic(&self) -> bool {
        matches!(self, TokenKind::Comment)
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
