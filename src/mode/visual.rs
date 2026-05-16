//! Visual mode key handling (Char, Line, Block).
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::editor::Editor;
use crate::mode::{Mode, VisualKind};

pub enum VisualAction {
    None,
    ExitToNormal,
    EnterInsert,
    /// Enter block-insert mode: insert `text` at the start col of every selected line.
    EnterBlockInsert { start_line: usize, end_line: usize, col: usize },
    EnterAi(String), // selected text passed to AI
    /// Copy selected text to system clipboard (handled by App layer via pbcopy/xclip).
    CopyToClipboard(String),
}

impl Editor {
    pub fn handle_visual_key(&mut self, key: KeyEvent, anchor: usize, kind: VisualKind) -> VisualAction {
        let cursor = self.cursor_char_idx();
        let (sel_start, sel_end) = if anchor <= cursor {
            (anchor, cursor + 1)
        } else {
            (cursor, anchor + 1)
        };

        // ── Block-mode helpers ────────────────────────────
        // For Block mode we work in (line, col) space rather than char indices.
        let anchor_line = self.buffer.char_to_line(anchor);
        let anchor_col  = anchor - self.buffer.line_to_char(anchor_line);
        let (block_start_line, block_end_line) = if self.cursor_line <= anchor_line {
            (self.cursor_line, anchor_line)
        } else {
            (anchor_line, self.cursor_line)
        };
        let (block_left_col, block_right_col) = if self.cursor_col <= anchor_col {
            (self.cursor_col, anchor_col)
        } else {
            (anchor_col, self.cursor_col)
        };

        match key.code {
            KeyCode::Esc => VisualAction::ExitToNormal,

            // Switch between visual sub-modes
            KeyCode::Char('v') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if kind == VisualKind::Char { VisualAction::ExitToNormal }
                else {
                    self.mode = Mode::Visual { kind: VisualKind::Char, anchor };
                    VisualAction::None
                }
            }
            KeyCode::Char('V') => {
                if kind == VisualKind::Line { VisualAction::ExitToNormal }
                else {
                    self.mode = Mode::Visual { kind: VisualKind::Line, anchor };
                    VisualAction::None
                }
            }
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if kind == VisualKind::Block { VisualAction::ExitToNormal }
                else {
                    self.mode = Mode::Visual { kind: VisualKind::Block, anchor };
                    VisualAction::None
                }
            }

            // o — swap cursor and anchor
            KeyCode::Char('o') => {
                let new_anchor = cursor;
                self.set_cursor_from_char_idx(anchor);
                self.mode = Mode::Visual { kind, anchor: new_anchor };
                VisualAction::None
            }

            // Movement — same as Normal
            KeyCode::Char('h') | KeyCode::Left  => { self.move_left(1);  VisualAction::None }
            KeyCode::Char('l') | KeyCode::Right => { self.move_right(1); VisualAction::None }
            KeyCode::Char('j') | KeyCode::Down  => { self.move_down(1);  VisualAction::None }
            KeyCode::Char('k') | KeyCode::Up    => { self.move_up(1);    VisualAction::None }
            KeyCode::Char('w') => { self.move_word_forward(1); VisualAction::None }
            KeyCode::Char('b') => { self.move_word_back(1);    VisualAction::None }
            KeyCode::Char('e') => { self.move_word_end(1);     VisualAction::None }
            KeyCode::Char('0') => { self.move_line_start();          VisualAction::None }
            KeyCode::Char('^') => { self.move_line_start_nonblank(); VisualAction::None }
            KeyCode::Char('$') => { self.move_line_end();            VisualAction::None }
            KeyCode::Char('G') => { self.move_file_bottom();         VisualAction::None }
            KeyCode::Char('{') => { self.move_paragraph_back(1);     VisualAction::None }
            KeyCode::Char('}') => { self.move_paragraph_forward(1);  VisualAction::None }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_half_down(); VisualAction::None
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_half_up(); VisualAction::None
            }

            // ── Yank ──────────────────────────────────────
            KeyCode::Char('y') => {
                match kind {
                    VisualKind::Block => {
                        let text = self.block_extract(block_start_line, block_end_line,
                                                      block_left_col, block_right_col);
                        self.buffer.register = text;
                        self.buffer.register_linewise = false;
                    }
                    _ => {
                        let text: String = self.buffer.rope
                            .slice(sel_start..sel_end.min(self.buffer.len_chars()))
                            .to_string();
                        self.buffer.register = text;
                        self.buffer.register_linewise = matches!(kind, VisualKind::Line);
                    }
                }
                VisualAction::ExitToNormal
            }

            // ── Copy to system clipboard (Ctrl+c) ─────────
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let text = match kind {
                    VisualKind::Block => {
                        self.block_extract(block_start_line, block_end_line,
                                           block_left_col, block_right_col)
                    }
                    _ => {
                        self.buffer.rope
                            .slice(sel_start..sel_end.min(self.buffer.len_chars()))
                            .to_string()
                    }
                };
                // Also sync to internal register so p/P still works
                self.buffer.register = text.clone();
                self.buffer.register_linewise = matches!(kind, VisualKind::Line);
                VisualAction::CopyToClipboard(text)
            }

            // ── Delete ────────────────────────────────────
            KeyCode::Char('d') | KeyCode::Char('x') => {
                match kind {
                    VisualKind::Block => {
                        let text = self.block_extract(block_start_line, block_end_line,
                                                      block_left_col, block_right_col);
                        self.buffer.register = text;
                        self.buffer.register_linewise = false;
                        self.block_delete(block_start_line, block_end_line,
                                          block_left_col, block_right_col);
                        self.cursor_line = block_start_line;
                        self.cursor_col  = block_left_col;
                        self.clamp_cursor();
                    }
                    _ => {
                        let end = sel_end.min(self.buffer.len_chars());
                        let text: String = self.buffer.rope.slice(sel_start..end).to_string();
                        self.buffer.register = text;
                        self.buffer.register_linewise = matches!(kind, VisualKind::Line);
                        self.buffer.delete_range(sel_start, end);
                        self.set_cursor_from_char_idx(sel_start);
                    }
                }
                VisualAction::ExitToNormal
            }

            // ── Change ────────────────────────────────────
            KeyCode::Char('c') => {
                match kind {
                    VisualKind::Block => {
                        // Delete block content, then enter block-insert
                        self.block_delete(block_start_line, block_end_line,
                                          block_left_col, block_right_col);
                        self.cursor_line = block_start_line;
                        self.cursor_col  = block_left_col;
                        self.clamp_cursor();
                        return VisualAction::EnterBlockInsert {
                            start_line: block_start_line,
                            end_line:   block_end_line,
                            col:        block_left_col,
                        };
                    }
                    _ => {
                        let end = sel_end.min(self.buffer.len_chars());
                        let text: String = self.buffer.rope.slice(sel_start..end).to_string();
                        self.buffer.register = text;
                        self.buffer.delete_range(sel_start, end);
                        self.set_cursor_from_char_idx(sel_start);
                        return VisualAction::EnterInsert;
                    }
                }
            }

            // ── Block insert (I) ──────────────────────────
            KeyCode::Char('I') => {
                if kind == VisualKind::Block {
                    return VisualAction::EnterBlockInsert {
                        start_line: block_start_line,
                        end_line:   block_end_line,
                        col:        block_left_col,
                    };
                }
                VisualAction::None
            }

            // ── Indent / dedent ───────────────────────────
            KeyCode::Char('>') => {
                let (start_line, end_line) = self.visual_line_range(anchor);
                for l in start_line..=end_line {
                    self.indent_line_pub(l, true);
                }
                VisualAction::ExitToNormal
            }
            KeyCode::Char('<') => {
                let (start_line, end_line) = self.visual_line_range(anchor);
                for l in start_line..=end_line {
                    self.indent_line_pub(l, false);
                }
                VisualAction::ExitToNormal
            }

            // ── Case toggle ───────────────────────────────
            KeyCode::Char('~') => {
                let end = sel_end.min(self.buffer.len_chars());
                let text: String = self.buffer.rope.slice(sel_start..end).to_string();
                let toggled: String = text.chars().map(|c| {
                    if c.is_uppercase() { c.to_lowercase().next().unwrap_or(c) }
                    else { c.to_uppercase().next().unwrap_or(c) }
                }).collect();
                self.buffer.delete_range(sel_start, end);
                self.buffer.insert_str(sel_start, &toggled);
                self.set_cursor_from_char_idx(sel_start);
                VisualAction::ExitToNormal
            }

            // ── AI mode with selection context ────────────
            KeyCode::Char('?') => {
                let end = sel_end.min(self.buffer.len_chars());
                let selected: String = self.buffer.rope.slice(sel_start..end).to_string();
                VisualAction::EnterAi(selected)
            }

            _ => VisualAction::None,
        }
    }

    fn visual_line_range(&self, anchor: usize) -> (usize, usize) {
        let cursor = self.cursor_char_idx();
        let (s, e) = if anchor <= cursor { (anchor, cursor) } else { (cursor, anchor) };
        let sl = self.buffer.char_to_line(s);
        let el = self.buffer.char_to_line(e);
        (sl, el)
    }

    /// Public proxy for use from visual.rs (indent_line is private in normal.rs impl block).
    pub fn indent_line_pub(&mut self, line: usize, right: bool) {
        let tab_str: String = if self.config.general.expand_tab {
            " ".repeat(self.config.general.tab_width)
        } else {
            "\t".to_string()
        };
        let start = self.buffer.line_to_char(line);
        if right {
            self.buffer.insert_str(start, &tab_str);
        } else {
            let line_s = self.buffer.line_str(line);
            let spaces: usize = line_s.chars().take_while(|c| *c == ' ').count();
            let remove = spaces.min(self.config.general.tab_width);
            if remove > 0 {
                self.buffer.delete_range(start, start + remove);
            }
        }
    }

    // ── Block helpers ─────────────────────────────────────

    /// Extract text from a rectangular block (one line per row, joined by '\n').
    pub fn block_extract(
        &self,
        start_line: usize, end_line: usize,
        left_col: usize,   right_col: usize,
    ) -> String {
        let mut out = String::new();
        for l in start_line..=end_line {
            let line = self.buffer.line_str(l);
            let chars: Vec<char> = line.chars().collect();
            let s = left_col.min(chars.len());
            let e = (right_col + 1).min(chars.len());
            let slice: String = chars[s..e].iter().collect();
            out.push_str(&slice);
            if l < end_line { out.push('\n'); }
        }
        out
    }

    /// Delete a rectangular block from the buffer (process lines in reverse).
    pub fn block_delete(
        &mut self,
        start_line: usize, end_line: usize,
        left_col: usize,   right_col: usize,
    ) {
        for l in (start_line..=end_line).rev() {
            let line = self.buffer.line_str(l);
            let chars: Vec<char> = line.chars().collect();
            let s = left_col.min(chars.len());
            let e = (right_col + 1).min(chars.len());
            if s >= e { continue; }
            let char_start = self.buffer.line_to_char(l) + s;
            let char_end   = self.buffer.line_to_char(l) + e;
            self.buffer.delete_range(char_start, char_end);
        }
    }

    /// Insert `text` at `col` on every line in `start_line..=end_line`.
    pub fn block_insert_text(
        &mut self,
        start_line: usize, end_line: usize,
        col: usize, text: &str,
    ) {
        for l in (start_line..=end_line).rev() {
            let line_len = self.buffer.line_len(l);
            let insert_col = col.min(line_len);
            let char_idx = self.buffer.line_to_char(l) + insert_col;
            self.buffer.insert_str(char_idx, text);
        }
    }

    /// Compute the block selection rectangle for rendering.
    /// Returns (start_line, end_line, left_col, right_col).
    pub fn block_rect(&self, anchor: usize) -> (usize, usize, usize, usize) {
        let anchor_line = self.buffer.char_to_line(anchor);
        let anchor_col  = anchor - self.buffer.line_to_char(anchor_line);
        let (sl, el) = if self.cursor_line <= anchor_line {
            (self.cursor_line, anchor_line)
        } else {
            (anchor_line, self.cursor_line)
        };
        let (lc, rc) = if self.cursor_col <= anchor_col {
            (self.cursor_col, anchor_col)
        } else {
            (anchor_col, self.cursor_col)
        };
        (sl, el, lc, rc)
    }
}
