//! Tests for the syntax highlighter: FileType detection, span output.

use hi::syntax::highlight::{FileType, Highlighter, TokenKind};

// ── FileType detection ────────────────────────────────────────────────────────

#[test]
fn test_filetype_from_ext() {
    assert_eq!(FileType::from_ext("yaml"), FileType::Yaml);
    assert_eq!(FileType::from_ext("yml"),  FileType::Yaml);
    assert_eq!(FileType::from_ext("json"), FileType::Json);
    assert_eq!(FileType::from_ext("toml"), FileType::Toml);
    assert_eq!(FileType::from_ext("py"),   FileType::Python);
    assert_eq!(FileType::from_ext("java"), FileType::Java);
    assert_eq!(FileType::from_ext("sh"),   FileType::Shell);
    assert_eq!(FileType::from_ext("log"),  FileType::Log);
    assert_eq!(FileType::from_ext("xml"),  FileType::Xml);
    assert_eq!(FileType::from_ext("html"), FileType::Html);
    assert_eq!(FileType::from_ext("txt"),  FileType::Plain);
    assert_eq!(FileType::from_ext(""),     FileType::Plain);
    // case insensitive
    assert_eq!(FileType::from_ext("YAML"), FileType::Yaml);
    assert_eq!(FileType::from_ext("JSON"), FileType::Json);
}

#[test]
fn test_filetype_from_path() {
    use std::path::Path;
    assert_eq!(FileType::from_path(Path::new("config.yaml")), FileType::Yaml);
    assert_eq!(FileType::from_path(Path::new("Makefile")),    FileType::Shell);
    assert_eq!(FileType::from_path(Path::new("Dockerfile")),  FileType::Shell);
}

// ── Highlighter output ────────────────────────────────────────────────────────

#[test]
fn test_plain_no_spans() {
    let h = Highlighter::new(FileType::Plain);
    let spans = h.highlight_line("just plain text");
    assert!(spans.is_empty(), "plain text should produce no spans");
}

// JSON
#[test]
fn test_json_string_span() {
    let h = Highlighter::new(FileType::Json);
    let spans = h.highlight_line("  \"name\": \"Alice\"");
    let has_string = spans.iter().any(|s| s.kind == TokenKind::String);
    assert!(has_string, "JSON strings should be highlighted: {:?}", spans);
}

#[test]
fn test_json_number_span() {
    let h = Highlighter::new(FileType::Json);
    let spans = h.highlight_line("  \"age\": 42");
    let has_number = spans.iter().any(|s| s.kind == TokenKind::Number);
    assert!(has_number, "JSON numbers should be highlighted: {:?}", spans);
}

// YAML
#[test]
fn test_yaml_key_span() {
    let h = Highlighter::new(FileType::Yaml);
    let spans = h.highlight_line("name: Alice");
    let has_key = spans.iter().any(|s| s.kind == TokenKind::Key);
    assert!(has_key, "YAML keys should be highlighted: {:?}", spans);
}

#[test]
fn test_yaml_comment_span() {
    let h = Highlighter::new(FileType::Yaml);
    let spans = h.highlight_line("# this is a comment");
    let has_comment = spans.iter().any(|s| s.kind == TokenKind::Comment);
    assert!(has_comment, "YAML comments should be highlighted: {:?}", spans);
}

// TOML
#[test]
fn test_toml_section_span() {
    let h = Highlighter::new(FileType::Toml);
    let spans = h.highlight_line("[dependencies]");
    let has_section = spans.iter().any(|s| s.kind == TokenKind::Section);
    assert!(has_section, "TOML section headers should be highlighted: {:?}", spans);
}

// Log
#[test]
fn test_log_error_span() {
    let h = Highlighter::new(FileType::Log);
    let spans = h.highlight_line("2024-01-01 ERROR: something went wrong");
    let has_error = spans.iter().any(|s| s.kind == TokenKind::Error);
    assert!(has_error, "Log ERROR lines should be highlighted: {:?}", spans);
}

#[test]
fn test_log_warn_span() {
    let h = Highlighter::new(FileType::Log);
    let spans = h.highlight_line("WARN: disk almost full");
    let has_warn = spans.iter().any(|s| s.kind == TokenKind::Warning);
    assert!(has_warn, "Log WARN lines should be highlighted: {:?}", spans);
}

#[test]
fn test_log_info_span() {
    let h = Highlighter::new(FileType::Log);
    let spans = h.highlight_line("INFO: server started on port 8080");
    let has_info = spans.iter().any(|s| s.kind == TokenKind::Info);
    assert!(has_info, "Log INFO lines should be highlighted: {:?}", spans);
}

// Shell
#[test]
fn test_shell_comment_span() {
    let h = Highlighter::new(FileType::Shell);
    let spans = h.highlight_line("# this is a bash comment");
    let has_comment = spans.iter().any(|s| s.kind == TokenKind::Comment);
    assert!(has_comment, "Shell comments should be highlighted: {:?}", spans);
}

#[test]
fn test_shell_keyword_span() {
    let h = Highlighter::new(FileType::Shell);
    let spans = h.highlight_line("if [ -f file ]; then");
    let has_kw = spans.iter().any(|s| s.kind == TokenKind::Keyword);
    assert!(has_kw, "Shell keywords should be highlighted: {:?}", spans);
}

// Python
#[test]
fn test_python_keyword_span() {
    let h = Highlighter::new(FileType::Python);
    let spans = h.highlight_line("def my_function(x):");
    let has_kw = spans.iter().any(|s| s.kind == TokenKind::Keyword);
    assert!(has_kw, "Python keywords should be highlighted: {:?}", spans);
}

#[test]
fn test_python_comment_span() {
    let h = Highlighter::new(FileType::Python);
    let spans = h.highlight_line("# python comment");
    let has_comment = spans.iter().any(|s| s.kind == TokenKind::Comment);
    assert!(has_comment, "Python comments should be highlighted: {:?}", spans);
}

// Java
#[test]
fn test_java_keyword_span() {
    let h = Highlighter::new(FileType::Java);
    let spans = h.highlight_line("public class Foo {");
    let has_kw = spans.iter().any(|s| s.kind == TokenKind::Keyword);
    assert!(has_kw, "Java keywords should be highlighted: {:?}", spans);
}

// XML
#[test]
fn test_xml_tag_span() {
    let h = Highlighter::new(FileType::Xml);
    let spans = h.highlight_line("<dependency>");
    let has_tag = spans.iter().any(|s| s.kind == TokenKind::Tag);
    assert!(has_tag, "XML tags should be highlighted: {:?}", spans);
}

// Span coverage: no overlapping spans on a single line
#[test]
fn test_no_overlapping_spans() {
    for ft in [FileType::Python, FileType::Java, FileType::Json, FileType::Yaml, FileType::Toml] {
        let h = Highlighter::new(ft);
        let line = "  def foo(x: int) -> str:  # test comment \"hello\" 42";
        let spans = h.highlight_line(line);
        // Check no two spans overlap
        let mut sorted = spans.clone();
        sorted.sort_by_key(|s| s.start);
        for w in sorted.windows(2) {
            assert!(w[0].end <= w[1].start,
                "[{:?}] overlapping spans: {:?} and {:?}", ft, w[0], w[1]);
        }
    }
}
