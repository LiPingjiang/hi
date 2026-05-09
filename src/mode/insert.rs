//! Insert mode key handling.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::editor::{Editor, RepeatAction};

pub enum InsertAction {
    None,
    ExitToNormal,
}

/// Tracks what was typed during the current Insert session (for dot-repeat).
#[derive(Default)]
pub struct InsertSession {
    pub text: String,
    pub newline_above: bool,
    pub newline_below: bool,
    pub enter_col_offset: i32,
}

impl Editor {
    /// Start tracking a new insert session (called on mode entry).
    pub fn begin_insert_session(&mut self, col_offset: i32, newline_above: bool, newline_below: bool) {
        // Store entry metadata so Esc can build a RepeatAction
        self._insert_col_offset  = col_offset;
        self._insert_newline_above = newline_above;
        self._insert_newline_below = newline_below;
        self._insert_text.clear();
    }

    pub fn handle_insert_key(&mut self, key: KeyEvent) -> InsertAction {
        match key.code {
            KeyCode::Esc => {
                // Save insert session as a repeatable action
                self.last_action = Some(RepeatAction::Insert {
                    enter_col_offset: self._insert_col_offset,
                    newline_above: self._insert_newline_above,
                    newline_below: self._insert_newline_below,
                    text: self._insert_text.clone(),
                });
                self._insert_text.clear();
                // Move cursor left by 1 (vim behaviour on Esc)
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                }
                return InsertAction::ExitToNormal;
            }

            KeyCode::Enter => {
                let indent = if self.config.general.auto_indent {
                    self.buffer.indent_of_line(self.cursor_line)
                } else {
                    String::new()
                };
                let char_idx = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                let new_pos = self.buffer.insert_newline(char_idx, &indent);
                self.set_cursor_from_char_idx(new_pos);
            }

            KeyCode::Backspace => {
                if self.cursor_col > 0 {
                    let char_idx = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                    self.buffer.delete_range(char_idx - 1, char_idx);
                    self.cursor_col -= 1;
                } else if self.cursor_line > 0 {
                    // Join with previous line
                    let prev_len = self.buffer.line_len(self.cursor_line - 1);
                    let char_idx = self.buffer.pos_to_char(self.cursor_line, 0);
                    // Remove the newline before this line
                    self.buffer.delete_range(char_idx - 1, char_idx);
                    self.cursor_line -= 1;
                    self.cursor_col = prev_len;
                }
                self.scroll_to_cursor();
            }

            KeyCode::Delete => {
                let line_len = self.buffer.line_len(self.cursor_line);
                if self.cursor_col < line_len {
                    let char_idx = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                    self.buffer.delete_range(char_idx, char_idx + 1);
                }
            }

            KeyCode::Tab => {
                let tab_str: String = if self.config.general.expand_tab {
                    " ".repeat(self.config.general.tab_width)
                } else {
                    "\t".to_string()
                };
                let char_idx = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                let len = tab_str.chars().count();
                self.buffer.insert_str(char_idx, &tab_str);
                self.cursor_col += len;
                self.scroll_to_cursor();
            }

            // Ctrl+w: delete previous word
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.cursor_col == 0 { return InsertAction::None; }
                let line = self.buffer.line_str(self.cursor_line);
                let chars: Vec<char> = line.chars().collect();
                let mut col = self.cursor_col.saturating_sub(1);
                // Skip trailing whitespace
                while col > 0 && chars[col].is_whitespace() { col -= 1; }
                // Skip word
                let is_w = |c: char| c.is_alphanumeric() || c == '_';
                if is_w(chars[col]) {
                    while col > 0 && is_w(chars[col-1]) { col -= 1; }
                } else {
                    while col > 0 && !chars[col-1].is_whitespace() && !is_w(chars[col-1]) { col -= 1; }
                }
                let start = self.buffer.pos_to_char(self.cursor_line, col);
                let end   = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                self.buffer.delete_range(start, end);
                self.cursor_col = col;
                self.scroll_to_cursor();
            }

            // Ctrl+u: delete to line start
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let start = self.buffer.pos_to_char(self.cursor_line, 0);
                let end   = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                if start < end {
                    self.buffer.delete_range(start, end);
                    self.cursor_col = 0;
                    self.scroll_to_cursor();
                }
            }

            KeyCode::Left  => { if self.cursor_col > 0 { self.cursor_col -= 1; } }
            KeyCode::Right => {
                let len = self.buffer.line_len(self.cursor_line);
                if self.cursor_col < len { self.cursor_col += 1; }
            }
            KeyCode::Up    => { self.move_up(1); }
            KeyCode::Down  => { self.move_down(1); }

            KeyCode::Char(c) => {
                // Track typed chars for dot-repeat
                self._insert_text.push(c);
                let char_idx = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                self.buffer.insert_char(char_idx, c);
                self.cursor_col += 1;
                self.scroll_to_cursor();
            }

            _ => {}
        }
        InsertAction::None
    }
}
