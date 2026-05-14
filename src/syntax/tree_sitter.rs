//! Tree-sitter incremental syntax highlighter.
//!
//! # Architecture
//!
//! Unlike syntect's stateful `ParseState` (which must replay from line 0),
//! Tree-sitter builds a full Concrete Syntax Tree (CST) and supports:
//!
//! 1. **Incremental edit** — `tree.edit(InputEdit)` marks only the changed
//!    subtree as dirty; the rest of the tree is reused in O(1).
//! 2. **Re-parse** — `parser.parse(source, old_tree)` only re-parses dirty
//!    nodes, giving O(changed_bytes × log n) complexity.
//! 3. **Range-limited highlight** — `query.matches(node, source, range)`
//!    only walks nodes that intersect the requested byte range, so we only
//!    pay for the visible viewport.
//!
//! # Highlight strategy
//!
//! We use Tree-sitter's built-in `highlights.scm` queries (bundled with each
//! grammar crate) to map capture names like `@keyword`, `@string`, etc. to
//! our `SemanticToken` enum, which is then coloured by `CodePalette`.
//!
//! For languages without a bundled query (or `FileType::Plain`) we fall back
//! to returning empty spans (plain text).

use tree_sitter::{InputEdit, Language, Node, Parser, Point, Query, QueryCursor, StreamingIterator, Tree};
use crate::syntax::highlight::{CodePalette, FileType, SemanticToken, SyntectSpan};

// ── Language registry ─────────────────────────────────────────────────────────

/// Return the Tree-sitter `Language` for a given `FileType`, or `None` for
/// `Plain` (no grammar available).
fn ts_language(ft: FileType) -> Option<Language> {
    match ft {
        FileType::Rust       => Some(tree_sitter_rust::LANGUAGE.into()),
        FileType::Python     => Some(tree_sitter_python::LANGUAGE.into()),
        FileType::Java       => Some(tree_sitter_java::LANGUAGE.into()),
        FileType::Go         => Some(tree_sitter_go::LANGUAGE.into()),
        FileType::Json       => Some(tree_sitter_json::LANGUAGE.into()),
        FileType::Yaml       => Some(tree_sitter_yaml::LANGUAGE.into()),
        FileType::Toml       => Some(tree_sitter_toml_ng::LANGUAGE.into()),
        FileType::Shell      => Some(tree_sitter_bash::LANGUAGE.into()),
        FileType::Html       => Some(tree_sitter_html::LANGUAGE.into()),
        FileType::Xml        => Some(tree_sitter_xml::LANGUAGE_XML.into()),
        FileType::Markdown   => Some(tree_sitter_md::LANGUAGE.into()),
        FileType::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
        FileType::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        FileType::Plain | FileType::Log | FileType::Properties => None,
    }
}

/// Return the `highlights.scm` query source bundled with each grammar crate.
/// Returns `None` for languages without a bundled query.
fn highlights_query(ft: FileType) -> Option<&'static str> {
    match ft {
        FileType::Rust       => Some(tree_sitter_rust::HIGHLIGHTS_QUERY),
        FileType::Python     => Some(tree_sitter_python::HIGHLIGHTS_QUERY),
        FileType::Java       => Some(tree_sitter_java::HIGHLIGHTS_QUERY),
        FileType::Go         => Some(tree_sitter_go::HIGHLIGHTS_QUERY),
        FileType::Json       => Some(tree_sitter_json::HIGHLIGHTS_QUERY),
        FileType::Yaml       => Some(tree_sitter_yaml::HIGHLIGHTS_QUERY),
        FileType::Shell      => Some(tree_sitter_bash::HIGHLIGHT_QUERY),
        FileType::Html       => Some(tree_sitter_html::HIGHLIGHTS_QUERY),
        FileType::JavaScript => Some(tree_sitter_javascript::HIGHLIGHT_QUERY),
        FileType::TypeScript => Some(tree_sitter_typescript::HIGHLIGHTS_QUERY),
        FileType::Toml       => Some(tree_sitter_toml_ng::HIGHLIGHTS_QUERY),
        // tree-sitter-md splits into block + inline grammars; concatenate both queries.
        // The combined string is valid because highlights.scm files are independent.
        FileType::Markdown   => Some(tree_sitter_md::HIGHLIGHT_QUERY_BLOCK),
        // xml crate doesn't expose a highlights query constant
        FileType::Xml => None,
        FileType::Plain | FileType::Log | FileType::Properties => None,
    }
}

// ── Capture-name → SemanticToken mapping ─────────────────────────────────────

/// Map a Tree-sitter capture name (e.g. `"keyword"`, `"string"`) to our
/// `SemanticToken` enum.  The capture name is the part after `@` in the
/// `highlights.scm` query, with the leading `@` stripped.
fn capture_to_token(name: &str) -> SemanticToken {
    // Exact matches first, then prefix matches
    match name {
        "keyword" | "keyword.control" | "keyword.operator" | "keyword.return"
        | "keyword.function" | "keyword.import" | "keyword.repeat"
        | "keyword.conditional" | "keyword.exception" | "keyword.coroutine" => SemanticToken::Keyword,

        "keyword.type" | "type" | "type.builtin" => SemanticToken::KeywordType,

        "string" | "string.special" | "string.escape" | "string.regexp"
        | "character" | "character.special" => SemanticToken::LiteralString,

        "number" | "number.float" | "float" => SemanticToken::LiteralNumber,

        "comment" | "comment.line" | "comment.block" | "comment.documentation" => SemanticToken::Comment,

        "function" | "function.call" | "function.method" | "function.method.call"
        | "method" | "method.call" => SemanticToken::NameFunction,

        "function.builtin" | "method.builtin" => SemanticToken::NameBuiltin,

        "variable" | "variable.member" | "variable.parameter" => SemanticToken::Variable,

        "variable.builtin" => SemanticToken::NameBuiltin,

        "constant" | "constant.builtin" | "constant.macro" => SemanticToken::LiteralNumber,

        "operator" => SemanticToken::Operator,

        "punctuation" | "punctuation.bracket" | "punctuation.delimiter"
        | "punctuation.special" => SemanticToken::Punctuation,

        "attribute" | "attribute.builtin" => SemanticToken::NameAttribute,

        "constructor" => SemanticToken::NameFunction,

        "label" => SemanticToken::Keyword,

        "namespace" | "module" => SemanticToken::KeywordType,

        "property" => SemanticToken::NameAttribute,

        "tag" | "tag.builtin" => SemanticToken::Keyword,

        "decorator" => SemanticToken::NameDecorator,

        "escape" => SemanticToken::LiteralString,

        "boolean" => SemanticToken::Keyword,

        "conditional" | "repeat" | "include" | "exception" => SemanticToken::Keyword,

        "field" => SemanticToken::NameAttribute,

        "lifetime" => SemanticToken::Variable,

        "macro" => SemanticToken::NameDecorator,

        "storageclass" => SemanticToken::Keyword,

        "structure" => SemanticToken::KeywordType,

        "text.title" | "markup.heading" => SemanticToken::Keyword,

        "text.uri" | "markup.link.url" => SemanticToken::LiteralString,

        "text.literal" | "markup.raw" | "markup.raw.inline" | "markup.raw.block" => SemanticToken::LiteralString,

        "text.strong" | "markup.bold" => SemanticToken::Keyword,

        "text.emphasis" | "markup.italic" => SemanticToken::Comment,

        _ => SemanticToken::Text,
    }
}

// ── TsHighlighter ─────────────────────────────────────────────────────────────

/// Tree-sitter backed incremental highlighter.
///
/// Holds the parsed `Tree` and re-uses it across frames.  On each buffer edit
/// call `edit()` to mark the changed region, then `highlight_viewport()` to
/// get spans for the visible lines — no full re-parse needed unless the tree
/// is dirty.
pub struct TsHighlighter {
    filetype: FileType,
    palette: CodePalette,
    /// The Tree-sitter parser (stateless between parses — state lives in Tree).
    parser: Parser,
    /// The current parsed tree.  `None` for `Plain` / unsupported languages.
    tree: Option<Tree>,
    /// Compiled highlight query for the current language.
    query: Option<Query>,
    /// Whether the tree needs a full re-parse (e.g. after `set_filetype`).
    needs_full_parse: bool,
}

// Parser is not Send by default because of the raw pointer inside, but we
// only ever access it from the main thread.
unsafe impl Send for TsHighlighter {}

impl TsHighlighter {
    /// Create a new highlighter.  Call `full_parse(source)` before the first
    /// `highlight_viewport()` call.
    pub fn new(filetype: FileType, palette: CodePalette) -> Self {
        let (parser, query) = Self::build_parser_and_query(filetype);
        Self {
            filetype,
            palette,
            parser,
            tree: None,
            query,
            needs_full_parse: true,
        }
    }

    /// Change the active filetype.  Triggers a full re-parse on the next
    /// `highlight_viewport()` call.
    pub fn set_filetype(&mut self, ft: FileType) {
        if self.filetype == ft { return; }
        self.filetype = ft;
        let (parser, query) = Self::build_parser_and_query(ft);
        self.parser = parser;
        self.query = query;
        self.tree = None;
        self.needs_full_parse = true;
    }

    /// Update the colour palette (called by `:theme`).
    pub fn set_palette(&mut self, palette: CodePalette) {
        self.palette = palette;
    }

    pub fn filetype(&self) -> FileType { self.filetype }

    /// Force a full re-parse on the next `incremental_parse()` call.
    /// Used after undo/redo/reload where the tree state is complex to reconstruct.
    pub fn force_full_parse(&mut self) {
        self.needs_full_parse = true;
        self.tree = None;
    }

    // ── Parse API ─────────────────────────────────────────────────────────────

    /// Full parse of the entire source.  Must be called after loading a new
    /// file or after `set_filetype`.
    pub fn full_parse(&mut self, source: &str) {
        self.needs_full_parse = false;
        if self.query.is_none() {
            self.tree = None;
            return;
        }
        self.tree = self.parser.parse(source, None);
    }

    /// Notify the highlighter of a single-character or multi-character edit.
    ///
    /// Parameters mirror `nvim_buf_attach` `on_bytes`:
    /// - `start_byte` / `start_row` / `start_col` — edit start position
    /// - `old_end_byte` / `old_end_row` / `old_end_col` — old end position
    /// - `new_end_byte` / `new_end_row` / `new_end_col` — new end position
    ///
    /// After calling `edit()`, call `incremental_parse(source)` to re-parse
    /// only the dirty subtree.
    #[allow(clippy::too_many_arguments)]
    pub fn edit(
        &mut self,
        start_byte: usize,
        old_end_byte: usize,
        new_end_byte: usize,
        start_row: usize,
        start_col: usize,
        old_end_row: usize,
        old_end_col: usize,
        new_end_row: usize,
        new_end_col: usize,
    ) {
        let Some(ref mut tree) = self.tree else { return; };
        tree.edit(&InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_position:   Point { row: start_row,   column: start_col },
            old_end_position: Point { row: old_end_row, column: old_end_col },
            new_end_position: Point { row: new_end_row, column: new_end_col },
        });
    }

    /// Incremental re-parse after `edit()`.  Only dirty subtrees are
    /// re-parsed; the rest of the tree is reused in O(1).
    pub fn incremental_parse(&mut self, source: &str) {
        if self.query.is_none() { return; }
        if self.needs_full_parse {
            self.full_parse(source);
            return;
        }
        let old_tree = self.tree.take();
        self.tree = self.parser.parse(source, old_tree.as_ref());
    }

    // ── Highlight API ─────────────────────────────────────────────────────────

    /// Return `SyntectSpan`s for a single line.
    ///
    /// `buf_line` is the 0-based line index in the buffer.
    /// `line` is the text of that line (without trailing `\n`).
    /// `source` is the full buffer text (needed for byte-offset queries).
    ///
    /// This is O(tokens in line) — no replay from line 0.
    pub fn highlight_line(
        &self,
        line: &str,
        buf_line: usize,
        source: &str,
    ) -> Vec<SyntectSpan> {
        let (Some(ref tree), Some(ref query)) = (&self.tree, &self.query) else {
            return vec![];
        };

        // Compute the byte range for this line within the full source.
        let line_start_byte = byte_offset_of_line(source, buf_line);
        let line_end_byte = line_start_byte + line.len();

        let root = tree.root_node();
        let mut cursor = QueryCursor::new();
        // Restrict the query to this line's byte range — O(tokens in line).
        cursor.set_byte_range(line_start_byte..line_end_byte);

        let source_bytes = source.as_bytes();
        let mut raw_spans: Vec<(usize, usize, SemanticToken)> = Vec::new();

        let mut matches = cursor.matches(query, root, source_bytes);
        while let Some(m) = matches.next() {
            for cap in m.captures {
                let node: Node = cap.node;
                let node_start = node.start_byte();
                let node_end   = node.end_byte();

                // Clamp to this line's byte range
                let span_start = node_start.max(line_start_byte);
                let span_end   = node_end.min(line_end_byte);
                if span_start >= span_end { continue; }

                // Convert to line-local byte offsets
                let local_start = span_start - line_start_byte;
                let local_end   = span_end   - line_start_byte;

                let cap_name = query.capture_names()[cap.index as usize];
                let token = capture_to_token(cap_name);
                raw_spans.push((local_start, local_end, token));
            }
        }

        if raw_spans.is_empty() {
            return vec![];
        }

        // Sort by start, then by end descending (longer spans first = more specific)
        raw_spans.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));

        // Deduplicate overlapping spans: keep the first (most specific) for each byte
        let line_len = line.len();
        let mut byte_token: Vec<Option<SemanticToken>> = vec![None; line_len];
        for (start, end, token) in &raw_spans {
            let s = (*start).min(line_len);
            let e = (*end).min(line_len);
            for b in s..e {
                if byte_token[b].is_none() {
                    byte_token[b] = Some(*token);
                }
            }
        }

        // Merge consecutive bytes with the same token into spans
        spans_from_byte_tokens(&byte_token, &self.palette)
    }

    /// Highlight a range of lines (the visible viewport).
    ///
    /// Returns a `Vec` of `(line_index, Vec<SyntectSpan>)` pairs.
    /// Only lines that have at least one token are included; callers should
    /// treat missing lines as plain text.
    pub fn highlight_viewport(
        &self,
        source: &str,
        start_line: usize,
        end_line: usize,  // exclusive
    ) -> Vec<(usize, Vec<SyntectSpan>)> {
        let (Some(ref tree), Some(ref query)) = (&self.tree, &self.query) else {
            return vec![];
        };

        let line_count = source.lines().count();
        let end_line = end_line.min(line_count);
        if start_line >= end_line { return vec![]; }

        // Compute byte range for the entire viewport
        let viewport_start_byte = byte_offset_of_line(source, start_line);
        let viewport_end_byte   = byte_offset_of_line(source, end_line);

        let root = tree.root_node();
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(viewport_start_byte..viewport_end_byte);

        let source_bytes = source.as_bytes();

        // Collect all captures in the viewport, grouped by line
        let mut line_spans: Vec<Vec<(usize, usize, SemanticToken)>> =
            vec![vec![]; end_line - start_line];

        let mut matches = cursor.matches(query, root, source_bytes);
        while let Some(m) = matches.next() {
            for cap in m.captures {
                let node: Node = cap.node;
                let node_start = node.start_byte();
                let node_end   = node.end_byte();

                // A node may span multiple lines — split it per line
                let cap_name = query.capture_names()[cap.index as usize];
                let token = capture_to_token(cap_name);

                // Find which lines this node touches
                let node_start_row = node.start_position().row;
                let node_end_row   = node.end_position().row;

                for row in node_start_row..=node_end_row {
                    if row < start_line || row >= end_line { continue; }
                    let li = row - start_line;

                    let line_start = byte_offset_of_line(source, row);
                    let line_text  = source_line(source, row);
                    let line_end   = line_start + line_text.len();

                    let span_start = node_start.max(line_start);
                    let span_end   = node_end.min(line_end);
                    if span_start >= span_end { continue; }

                    let local_start = span_start - line_start;
                    let local_end   = span_end   - line_start;
                    line_spans[li].push((local_start, local_end, token));
                }
            }
        }

        // Convert per-line raw spans to SyntectSpan vecs
        let mut result = Vec::new();
        for (li, raw) in line_spans.into_iter().enumerate() {
            if raw.is_empty() { continue; }
            let abs_line = start_line + li;
            let line_text = source_line(source, abs_line);
            let line_len = line_text.len();

            let mut sorted = raw;
            sorted.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));

            let mut byte_token: Vec<Option<SemanticToken>> = vec![None; line_len];
            for (start, end, token) in &sorted {
                let s = (*start).min(line_len);
                let e = (*end).min(line_len);
                for b in s..e {
                    if byte_token[b].is_none() {
                        byte_token[b] = Some(*token);
                    }
                }
            }

            let spans = spans_from_byte_tokens(&byte_token, &self.palette);
            if !spans.is_empty() {
                result.push((abs_line, spans));
            }
        }

        result
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn build_parser_and_query(ft: FileType) -> (Parser, Option<Query>) {
        let mut parser = Parser::new();
        let lang = match ts_language(ft) {
            Some(l) => l,
            None => return (parser, None),
        };
        if parser.set_language(&lang).is_err() {
            return (parser, None);
        }
        let query_src = match highlights_query(ft) {
            Some(s) => s,
            None => {
                // Language supported but no bundled query — use a minimal fallback
                return (parser, None);
            }
        };
        let query = Query::new(&lang, query_src).ok();
        (parser, query)
    }
}

// ── Utility functions ─────────────────────────────────────────────────────────

/// Return the byte offset of the start of `line_idx` (0-based) in `source`.
fn byte_offset_of_line(source: &str, line_idx: usize) -> usize {
    if line_idx == 0 { return 0; }
    let mut count = 0usize;
    let mut offset = 0usize;
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            count += 1;
            if count == line_idx {
                return i + 1;
            }
        }
        offset = i + 1;
    }
    offset
}

/// Return the text of line `line_idx` (0-based) without the trailing `\n`.
fn source_line(source: &str, line_idx: usize) -> &str {
    source
        .lines()
        .nth(line_idx)
        .unwrap_or("")
}

/// Convert a per-byte token array into a `Vec<SyntectSpan>` by merging
/// consecutive bytes with the same token.
fn spans_from_byte_tokens(
    byte_token: &[Option<SemanticToken>],
    palette: &CodePalette,
) -> Vec<SyntectSpan> {
    let mut spans = Vec::new();
    let mut i = 0usize;
    while i < byte_token.len() {
        let tok = byte_token[i];
        let start = i;
        while i < byte_token.len() && byte_token[i] == tok {
            i += 1;
        }
        if let Some(t) = tok {
            if t != SemanticToken::Text {
                spans.push(SyntectSpan {
                    start,
                    end: i,
                    fg: t.to_color(palette),
                    bold: t.bold(),
                    italic: t.italic(),
                    overlay: None,
                });
            }
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Query;

    fn check_query(name: &str, lang: tree_sitter::Language, src: &str) {
        match Query::new(&lang, src) {
            Ok(q)  => println!("{}: OK ({} captures)", name, q.capture_names().len()),
            Err(e) => panic!("{}: query error: {:?}", name, e),
        }
    }

    #[test]
    fn markdown_produces_spans() {
        let palette = CodePalette::neon_minimalist();
        let mut hl = TsHighlighter::new(FileType::Markdown, palette);
        let source = "# Hello\n\nThis is `code` and **bold**.\n";
        hl.full_parse(source);
        let spans = hl.highlight_viewport(source, 0, 3);
        println!("markdown spans: {:?}", spans);
        assert!(!spans.is_empty(), "expected at least one highlighted line");
    }

    #[test]
    fn rust_produces_spans() {
        let palette = CodePalette::neon_minimalist();
        let mut hl = TsHighlighter::new(FileType::Rust, palette);
        let source = "fn main() {\n    let x = 42;\n    println!(\"{}\", x);\n}\n";
        hl.full_parse(source);
        let spans = hl.highlight_viewport(source, 0, 4);
        println!("rust spans ({} lines with highlights):", spans.len());
        for (line, s) in &spans {
            println!("  line {}: {} spans", line, s.len());
        }
        assert!(!spans.is_empty(), "expected rust highlights");
    }

    #[test]
    fn toml_produces_spans() {
        let palette = CodePalette::neon_minimalist();
        let mut hl = TsHighlighter::new(FileType::Toml, palette);
        let source = "[package]\nname = \"hi\"\nversion = \"0.1.0\"\n";
        hl.full_parse(source);
        let spans = hl.highlight_viewport(source, 0, 3);
        println!("toml spans ({} lines with highlights):", spans.len());
        for (line, s) in &spans {
            println!("  line {}: {} spans", line, s.len());
        }
        assert!(!spans.is_empty(), "expected toml highlights");
    }

    #[test]
    fn all_highlight_queries_compile() {
        check_query("rust",       tree_sitter_rust::LANGUAGE.into(),       tree_sitter_rust::HIGHLIGHTS_QUERY);
        check_query("python",     tree_sitter_python::LANGUAGE.into(),     tree_sitter_python::HIGHLIGHTS_QUERY);
        check_query("java",       tree_sitter_java::LANGUAGE.into(),       tree_sitter_java::HIGHLIGHTS_QUERY);
        check_query("go",         tree_sitter_go::LANGUAGE.into(),         tree_sitter_go::HIGHLIGHTS_QUERY);
        check_query("json",       tree_sitter_json::LANGUAGE.into(),       tree_sitter_json::HIGHLIGHTS_QUERY);
        check_query("yaml",       tree_sitter_yaml::LANGUAGE.into(),       tree_sitter_yaml::HIGHLIGHTS_QUERY);
        check_query("toml",       tree_sitter_toml_ng::LANGUAGE.into(),    tree_sitter_toml_ng::HIGHLIGHTS_QUERY);
        check_query("bash",       tree_sitter_bash::LANGUAGE.into(),       tree_sitter_bash::HIGHLIGHT_QUERY);
        check_query("html",       tree_sitter_html::LANGUAGE.into(),       tree_sitter_html::HIGHLIGHTS_QUERY);
        check_query("javascript", tree_sitter_javascript::LANGUAGE.into(), tree_sitter_javascript::HIGHLIGHT_QUERY);
        check_query("typescript", tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(), tree_sitter_typescript::HIGHLIGHTS_QUERY);
        check_query("markdown",   tree_sitter_md::LANGUAGE.into(),         tree_sitter_md::HIGHLIGHT_QUERY_BLOCK);
    }
}
