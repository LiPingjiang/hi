use std::path::PathBuf;
use anyhow::Result;
use ropey::Rope;
use super::history::{History, ChangeSet};

/// Byte-level description of a single edit, suitable for passing to
/// `TsHighlighter::edit()` to keep the Tree-sitter CST in sync.
#[derive(Debug, Clone, Copy)]
pub struct EditInfo {
    pub start_byte:    usize,
    pub old_end_byte:  usize,
    pub new_end_byte:  usize,
    pub start_row:     usize,
    pub start_col:     usize,
    pub old_end_row:   usize,
    pub old_end_col:   usize,
    pub new_end_row:   usize,
    pub new_end_col:   usize,
}

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
    /// Monotonically increasing counter — incremented on every mutation.
    /// Callers can cache `(generation, String)` and skip `rope.to_string()`
    /// when the generation hasn't changed.
    pub generation: u64,
    /// Pending `InputEdit`s to be forwarded to `TsHighlighter::edit()` before
    /// the next incremental parse.  Accumulated here so that all mutation
    /// sites (normal/insert/visual modes) don't need to know about the
    /// highlighter.
    pub pending_edits: Vec<EditInfo>,
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
            generation: 0,
            pending_edits: Vec::new(),
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
            generation: 0,
            pending_edits: Vec::new(),
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

    // ── Mutation (records undo history, accumulates EditInfo) ─────────────────
    //
    // Each mutation method computes an `EditInfo` *before* mutating the rope
    // (so byte offsets are still valid), pushes it to `pending_edits`, then
    // applies the change.  The renderer drains `pending_edits` and forwards
    // them to `TsHighlighter::edit()` before calling `incremental_parse()`.

    pub fn insert_char(&mut self, char_idx: usize, ch: char) {
        let ei = self.edit_info_for_insert(char_idx, ch.len_utf8());
        self.pending_edits.push(ei);
        self.history.push(ChangeSet {
            start: char_idx,
            old_text: String::new(),
            new_text: ch.to_string(),
            cursor_before: char_idx,
            cursor_after: char_idx + 1,
        });
        self.rope.insert_char(char_idx, ch);
        self.modified = true;
        self.generation += 1;
    }

    pub fn insert_str(&mut self, char_idx: usize, s: &str) {
        let byte_len = s.len();
        let ei = self.edit_info_for_insert(char_idx, byte_len);
        self.pending_edits.push(ei);
        let char_len = s.chars().count();
        self.history.push(ChangeSet {
            start: char_idx,
            old_text: String::new(),
            new_text: s.to_string(),
            cursor_before: char_idx,
            cursor_after: char_idx + char_len,
        });
        self.rope.insert(char_idx, s);
        self.modified = true;
        self.generation += 1;
    }

    pub fn delete_range(&mut self, start: usize, end: usize) {
        if start >= end { return; }
        let old_text: String = self.rope.slice(start..end).to_string();
        let old_byte_len = old_text.len();
        let ei = self.edit_info_for_delete(start, old_byte_len);
        self.pending_edits.push(ei);
        self.history.push(ChangeSet {
            start,
            old_text,
            new_text: String::new(),
            cursor_before: end,
            cursor_after: start,
        });
        self.rope.remove(start..end);
        self.modified = true;
        self.generation += 1;
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
        let cs = self.history.undo()?;
        // Invert: remove new_text, insert old_text.
        // We do a full-tree re-parse after undo (clear pending_edits, bump generation).
        let new_end = cs.start + cs.new_text.chars().count();
        if new_end > cs.start {
            self.rope.remove(cs.start..new_end);
        }
        if !cs.old_text.is_empty() {
            self.rope.insert(cs.start, &cs.old_text);
        }
        self.modified = true;
        self.generation += 1;
        // After undo the tree is stale in a complex way; signal a full re-parse
        // by clearing pending_edits (renderer will call full_parse on next frame).
        self.pending_edits.clear();
        Some(cs.cursor_before)
    }

    /// Returns Some(cursor_pos) if redo succeeded.
    pub fn redo(&mut self) -> Option<usize> {
        let cs = self.history.redo()?;
        // Re-apply: remove old_text, insert new_text.
        let old_end = cs.start + cs.old_text.chars().count();
        if old_end > cs.start {
            self.rope.remove(cs.start..old_end);
        }
        if !cs.new_text.is_empty() {
            self.rope.insert(cs.start, &cs.new_text);
        }
        self.modified = true;
        self.generation += 1;
        self.pending_edits.clear();
        Some(cs.cursor_after)
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
        self.generation += 1;
        self.pending_edits.clear();
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

    // ── Private helpers ───────────────────────────────────

    /// Build an `EditInfo` for an insertion of `new_byte_len` bytes at `char_idx`.
    /// Called *before* the rope is mutated.
    fn edit_info_for_insert(&self, char_idx: usize, new_byte_len: usize) -> EditInfo {
        let char_idx = char_idx.min(self.rope.len_chars());
        let start_byte = self.rope.char_to_byte(char_idx);
        let start_row  = self.rope.char_to_line(char_idx);
        let line_start_char = self.rope.line_to_char(start_row);
        let start_col  = char_idx - line_start_char;

        EditInfo {
            start_byte,
            old_end_byte: start_byte,
            new_end_byte: start_byte + new_byte_len,
            start_row,
            start_col,
            old_end_row: start_row,
            old_end_col: start_col,
            // new_end row/col: approximate — same row if no newline in inserted text.
            // The renderer will do a full_parse if the tree becomes inconsistent.
            new_end_row: start_row,
            new_end_col: start_col,
        }
    }

    /// Build an `EditInfo` for a deletion of `old_byte_len` bytes starting at `char_idx`.
    /// Called *before* the rope is mutated.
    fn edit_info_for_delete(&self, char_idx: usize, old_byte_len: usize) -> EditInfo {
        let char_idx = char_idx.min(self.rope.len_chars());
        let start_byte   = self.rope.char_to_byte(char_idx);
        let old_end_byte = (start_byte + old_byte_len).min(self.rope.len_bytes());

        let start_row  = self.rope.char_to_line(char_idx);
        let line_start_char = self.rope.line_to_char(start_row);
        let start_col  = char_idx - line_start_char;

        let old_end_char = self.rope.byte_to_char(old_end_byte);
        let old_end_row  = self.rope.char_to_line(old_end_char.min(self.rope.len_chars()));
        let old_end_line_start = self.rope.line_to_char(old_end_row);
        let old_end_col  = old_end_char.saturating_sub(old_end_line_start);

        EditInfo {
            start_byte,
            old_end_byte,
            new_end_byte: start_byte,
            start_row,
            start_col,
            old_end_row,
            old_end_col,
            new_end_row: start_row,
            new_end_col: start_col,
        }
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
