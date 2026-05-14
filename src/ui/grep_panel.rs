//! Global grep panel — triggered by `:grep <pattern>` or `Ctrl+F`.
//!
//! Searches all files under the working root for a literal string or regex,
//! displays results in a scrollable overlay, and lets the user jump to any
//! match by pressing Enter.
//!
//! Architecture:
//!   - `GrepPanel` holds the query, the list of `GrepMatch` results, and
//!     the cursor position.
//!   - Searching is synchronous (runs on the main thread) but is fast enough
//!     for typical project sizes (< 50k files, < 100MB total).  For very large
//!     repos the search is capped at MAX_MATCHES results.
//!   - The panel is rendered as a full-width overlay by `Renderer::render_grep_panel`.

use std::path::{Path, PathBuf};
use std::fs;
use regex::Regex;

// ── Match record ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GrepMatch {
    /// Absolute path to the file.
    pub path: PathBuf,
    /// 1-based line number.
    pub line_no: usize,
    /// The full line text (trimmed to MAX_LINE_LEN chars for display).
    pub line_text: String,
    /// Byte offset of the match start within `line_text`.
    pub match_start: usize,
    /// Byte offset of the match end within `line_text`.
    pub match_end: usize,
}

const MAX_MATCHES: usize = 1_000;
const MAX_LINE_LEN: usize = 200;

// ── Search ───────────────────────────────────────────────────────────────────

/// Run a grep search under `root` for `pattern`.
/// If `is_regex` is false the pattern is treated as a literal string.
/// Returns up to MAX_MATCHES results.
pub fn run_grep(root: &Path, pattern: &str, is_regex: bool) -> Vec<GrepMatch> {
    if pattern.is_empty() {
        return vec![];
    }

    let re = if is_regex {
        match Regex::new(pattern) {
            Ok(r) => r,
            Err(_) => return vec![],
        }
    } else {
        // Escape the literal pattern for use as a regex
        match Regex::new(&regex::escape(pattern)) {
            Ok(r) => r,
            Err(_) => return vec![],
        }
    };

    let mut results = Vec::new();
    search_dir(root, root, &re, &mut results);
    results
}

fn search_dir(root: &Path, dir: &Path, re: &Regex, out: &mut Vec<GrepMatch>) {
    if out.len() >= MAX_MATCHES { return; }
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        if out.len() >= MAX_MATCHES { break; }
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden and noise dirs
        if name_str.starts_with('.') { continue; }
        if matches!(name_str.as_ref(), "node_modules" | "target" | "dist" | "build" | "__pycache__") {
            continue;
        }

        if path.is_dir() {
            search_dir(root, &path, re, out);
        } else {
            search_file(&path, re, out);
        }
    }
}

fn search_file(path: &Path, re: &Regex, out: &mut Vec<GrepMatch>) {
    // Skip binary-looking files by extension
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if matches!(ext, "png" | "jpg" | "jpeg" | "gif" | "ico" | "svg" |
                        "pdf" | "zip" | "tar" | "gz" | "bz2" | "xz" |
                        "exe" | "dll" | "so" | "dylib" | "a" | "o" |
                        "class" | "jar" | "wasm" | "bin" | "lock") {
            return;
        }
    }

    let Ok(content) = fs::read_to_string(path) else { return };

    for (line_idx, line) in content.lines().enumerate() {
        if out.len() >= MAX_MATCHES { break; }
        // Truncate very long lines for display
        let display_line = if line.len() > MAX_LINE_LEN {
            &line[..MAX_LINE_LEN]
        } else {
            line
        };

        if let Some(m) = re.find(display_line) {
            out.push(GrepMatch {
                path: path.to_path_buf(),
                line_no: line_idx + 1,
                line_text: display_line.to_string(),
                match_start: m.start(),
                match_end: m.end(),
            });
        }
    }
}

// ── Panel state ──────────────────────────────────────────────────────────────

pub struct GrepPanel {
    /// The search query string.
    pub query: String,
    /// Whether the query is a regex (`:grep /pattern/`) or literal.
    pub is_regex: bool,
    /// All matches found.
    pub matches: Vec<GrepMatch>,
    /// Cursor position within `matches`.
    pub cursor: usize,
    /// Root directory used for the search.
    pub root: PathBuf,
    /// Whether the search has been run (false = still typing).
    pub searched: bool,
}

impl GrepPanel {
    pub fn new(root: PathBuf) -> Self {
        Self {
            query: String::new(),
            is_regex: false,
            matches: vec![],
            cursor: 0,
            root,
            searched: false,
        }
    }

    /// Run the search with the current query.
    pub fn run_search(&mut self) {
        self.matches = run_grep(&self.root, &self.query, self.is_regex);
        self.cursor = 0;
        self.searched = true;
    }

    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
        self.searched = false;
    }

    pub fn pop_char(&mut self) {
        self.query.pop();
        self.searched = false;
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 { self.cursor -= 1; }
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.matches.len() { self.cursor += 1; }
    }

    /// Return the selected match, if any.
    pub fn selected(&self) -> Option<&GrepMatch> {
        self.matches.get(self.cursor)
    }

    /// Visible window of results (up to `max_rows` items).
    pub fn visible_window(&self, max_rows: usize) -> (usize, &[GrepMatch]) {
        let total = self.matches.len();
        if total == 0 { return (0, &[]); }
        let start = if self.cursor >= max_rows.saturating_sub(1) {
            self.cursor.saturating_sub(max_rows / 2)
        } else {
            0
        };
        let end = (start + max_rows).min(total);
        (start, &self.matches[start..end])
    }
}
