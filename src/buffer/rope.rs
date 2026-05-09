use std::path::PathBuf;
use anyhow::Result;
use ropey::Rope;
use super::history::{History, ChangeSet};

/// A text buffer backed by a Rope, with undo/redo support.
pub struct Buffer {
    pub rope: Rope,
    pub path: Option<PathBuf>,
    pub modified: bool,
    history: History,
    /// Clipboard (yank register)
    pub register: String,
    /// Whether clipboard contains whole lines (yy) vs chars
    pub register_linewise: bool,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            path: None,
            modified: false,
            history: History::new(),
            register: String::new(),
            register_linewise: false,
        }
    }

    /// Alias for `from_path`, used by app.rs.
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Self::from_path(path.as_ref().to_path_buf())
    }

    pub fn from_path(path: PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_default();
        let rope = Rope::from_str(&content);
        Ok(Self {
            rope,
            path: Some(path),
            modified: false,
            history: History::new(),
            register: String::new(),
            register_linewise: false,
        })
    }

    /// Expose the current file path for context building.
    pub fn filepath(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    // ── Query ─────────────────────────────────────────────

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn line_len(&self, line: usize) -> usize {
        let line_str = self.rope.line(line);
        let s = line_str.to_string();
        // Don't count trailing newline
        s.trim_end_matches('\n').chars().count()
    }

    pub fn line_str(&self, line: usize) -> String {
        self.rope.line(line).to_string()
            .trim_end_matches('\n').to_string()
    }

    pub fn char_to_line(&self, char_idx: usize) -> usize {
        self.rope.char_to_line(char_idx)
    }

    pub fn line_to_char(&self, line: usize) -> usize {
        self.rope.line_to_char(line)
    }

    /// Convert (line, col) to absolute char index.
    pub fn pos_to_char(&self, line: usize, col: usize) -> usize {
        let line_start = self.rope.line_to_char(line);
        line_start + col
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    // ── Mutation (records undo history) ───────────────────

    pub fn insert_char(&mut self, char_idx: usize, ch: char) {
        let before = self.rope.to_string();
        self.rope.insert_char(char_idx, ch);
        self.modified = true;
        let after = self.rope.to_string();
        self.history.push(ChangeSet { before, after, cursor_before: char_idx, cursor_after: char_idx + 1 });
    }

    pub fn insert_str(&mut self, char_idx: usize, s: &str) {
        let before = self.rope.to_string();
        self.rope.insert(char_idx, s);
        self.modified = true;
        let len = s.chars().count();
        let after = self.rope.to_string();
        self.history.push(ChangeSet { before, after, cursor_before: char_idx, cursor_after: char_idx + len });
    }

    pub fn delete_range(&mut self, start: usize, end: usize) {
        if start >= end { return; }
        let before = self.rope.to_string();
        self.rope.remove(start..end);
        self.modified = true;
        let after = self.rope.to_string();
        self.history.push(ChangeSet { before, after, cursor_before: end, cursor_after: start });
    }

    pub fn delete_line(&mut self, line: usize) -> String {
        let line_start = self.rope.line_to_char(line);
        let line_end = if line + 1 < self.rope.len_lines() {
            self.rope.line_to_char(line + 1)
        } else {
            self.rope.len_chars()
        };
        let content = self.rope.slice(line_start..line_end).to_string();
        self.delete_range(line_start, line_end);
        content
    }

    /// Insert a newline + indent at char_idx, return new cursor pos.
    pub fn insert_newline(&mut self, char_idx: usize, indent: &str) -> usize {
        let s = format!("\n{}", indent);
        let new_pos = char_idx + 1 + indent.chars().count();
        self.insert_str(char_idx, &s);
        new_pos
    }

    // ── Undo / Redo ────────────────────────────────────────

    /// Returns Some(cursor_pos) if undo succeeded.
    pub fn undo(&mut self) -> Option<usize> {
        if let Some(cs) = self.history.undo() {
            self.rope = Rope::from_str(&cs.before);
            self.modified = true;
            Some(cs.cursor_before)
        } else {
            None
        }
    }

    /// Returns Some(cursor_pos) if redo succeeded.
    pub fn redo(&mut self) -> Option<usize> {
        if let Some(cs) = self.history.redo() {
            self.rope = Rope::from_str(&cs.after);
            self.modified = true;
            Some(cs.cursor_after)
        } else {
            None
        }
    }

    /// Mark a breakpoint so a sequence of changes can be undone together.
    pub fn undo_breakpoint(&mut self) {
        self.history.breakpoint();
    }

    /// Begin a grouped undo block (AI multi-step operations).
    pub fn begin_group(&mut self) {
        self.history.begin_group();
    }

    /// Low-level insert (no history recording — for internal use).
    pub fn insert(&mut self, char_idx: usize, s: &str) {
        self.insert_str(char_idx, s);
    }

    /// Low-level delete (no history recording — for internal use).
    pub fn delete(&mut self, char_idx: usize, char_count: usize) {
        let end = (char_idx + char_count).min(self.rope.len_chars());
        self.delete_range(char_idx, end);
    }

    // ── Persistence ───────────────────────────────────────

    pub fn save(&mut self) -> Result<()> {
        let path = self.path.clone().ok_or_else(|| anyhow::anyhow!("No file path"))?;
        let content: String = self.rope.to_string();
        std::fs::write(&path, content)?;
        self.modified = false;
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<()> {
        self.path = Some(path);
        self.save()
    }

    pub fn reload(&mut self) -> Result<()> {
        let path = self.path.clone().ok_or_else(|| anyhow::anyhow!("No file path"))?;
        let content = std::fs::read_to_string(&path)?;
        self.rope = Rope::from_str(&content);
        self.modified = false;
        self.history = History::new();
        Ok(())
    }

    // ── Filename display ──────────────────────────────────

    pub fn display_name(&self) -> String {
        self.path.as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or("[No Name]")
            .to_string()
    }

    /// Best-effort auto-indent: return indentation of given line.
    pub fn indent_of_line(&self, line: usize) -> String {
        let s = self.line_str(line);
        let n = s.len() - s.trim_start().len();
        s[..n].to_string()
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
