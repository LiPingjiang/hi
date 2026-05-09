//! Cursor motion helpers used by normal.rs and visual.rs.
use crate::editor::Editor;

impl Editor {
    // ── Basic directional moves ────────────────────────────

    pub fn move_left(&mut self, count: usize) {
        for _ in 0..count {
            if self.cursor_col > 0 {
                self.cursor_col -= 1;
            }
        }
        self.scroll_to_cursor();
    }

    pub fn move_right(&mut self, count: usize) {
        let line_len = self.buffer.line_len(self.cursor_line);
        let max_col = if self.mode.is_insert() { line_len } else { line_len.saturating_sub(1) };
        for _ in 0..count {
            if self.cursor_col < max_col {
                self.cursor_col += 1;
            }
        }
        self.scroll_to_cursor();
    }

    pub fn move_up(&mut self, count: usize) {
        self.cursor_line = self.cursor_line.saturating_sub(count);
        self.clamp_cursor();
        self.scroll_to_cursor();
    }

    pub fn move_down(&mut self, count: usize) {
        let max = self.buffer.line_count().saturating_sub(1);
        self.cursor_line = (self.cursor_line + count).min(max);
        self.clamp_cursor();
        self.scroll_to_cursor();
    }

    // ── Line boundary ──────────────────────────────────────

    pub fn move_line_start(&mut self) {
        self.cursor_col = 0;
        self.scroll_to_cursor();
    }

    pub fn move_line_start_nonblank(&mut self) {
        let line = self.buffer.line_str(self.cursor_line);
        self.cursor_col = line.chars().take_while(|c| c.is_whitespace()).count();
        self.scroll_to_cursor();
    }

    pub fn move_line_end(&mut self) {
        let len = self.buffer.line_len(self.cursor_line);
        self.cursor_col = if len == 0 { 0 } else { len - 1 };
        self.scroll_to_cursor();
    }

    // ── Word motions ───────────────────────────────────────

    pub fn move_word_forward(&mut self, count: usize) {
        for _ in 0..count {
            self._word_forward(false);
        }
        self.scroll_to_cursor();
    }

    pub fn move_word_back(&mut self, count: usize) {
        for _ in 0..count {
            self._word_back(false);
        }
        self.scroll_to_cursor();
    }

    pub fn move_word_end(&mut self, count: usize) {
        for _ in 0..count {
            self._word_end(false);
        }
        self.scroll_to_cursor();
    }

    fn _word_forward(&mut self, big: bool) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let mut col = self.cursor_col;

        let is_word = |c: char| if big { !c.is_whitespace() } else { c.is_alphanumeric() || c == '_' };

        // Skip current word chars
        while col < chars.len() && is_word(chars[col]) { col += 1; }
        // Skip whitespace
        while col < chars.len() && chars[col].is_whitespace() { col += 1; }

        if col >= chars.len() {
            // Move to next line
            if self.cursor_line + 1 < self.buffer.line_count() {
                self.cursor_line += 1;
                self.cursor_col = 0;
                self.move_line_start_nonblank();
            } else {
                self.cursor_col = chars.len().saturating_sub(1);
            }
        } else {
            self.cursor_col = col;
        }
    }

    fn _word_back(&mut self, big: bool) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let is_word = |c: char| if big { !c.is_whitespace() } else { c.is_alphanumeric() || c == '_' };

        if self.cursor_col == 0 {
            if self.cursor_line > 0 {
                self.cursor_line -= 1;
                let prev_len = self.buffer.line_len(self.cursor_line).saturating_sub(1);
                self.cursor_col = prev_len;
            }
            return;
        }

        let mut col = self.cursor_col.saturating_sub(1);
        // Skip whitespace backward
        while col > 0 && chars[col].is_whitespace() { col -= 1; }
        // Skip word backward
        while col > 0 && is_word(chars[col]) { col -= 1; }
        // If we stopped on a non-word char, move forward one
        if col > 0 || (!chars.is_empty() && !is_word(chars[col])) {
            if !chars.is_empty() && !is_word(chars[col]) && col + 1 <= self.cursor_col {
                col += 1;
            }
        }
        self.cursor_col = col;
    }

    fn _word_end(&mut self, big: bool) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let is_word = |c: char| if big { !c.is_whitespace() } else { c.is_alphanumeric() || c == '_' };

        let mut col = self.cursor_col + 1;
        if col >= chars.len() {
            if self.cursor_line + 1 < self.buffer.line_count() {
                self.cursor_line += 1;
                self.cursor_col = 0;
            }
            return;
        }
        // Skip whitespace
        while col < chars.len() && chars[col].is_whitespace() { col += 1; }
        // Go to end of word
        while col + 1 < chars.len() && is_word(chars[col + 1]) { col += 1; }
        self.cursor_col = col;
    }

    // ── Paragraph / empty-line jumps ───────────────────────

    pub fn move_paragraph_forward(&mut self, count: usize) {
        for _ in 0..count {
            let mut l = self.cursor_line + 1;
            while l < self.buffer.line_count() && !self.buffer.line_str(l).trim().is_empty() {
                l += 1;
            }
            self.cursor_line = l.min(self.buffer.line_count().saturating_sub(1));
        }
        self.cursor_col = 0;
        self.scroll_to_cursor();
    }

    pub fn move_paragraph_back(&mut self, count: usize) {
        for _ in 0..count {
            let mut l = self.cursor_line.saturating_sub(1);
            while l > 0 && !self.buffer.line_str(l).trim().is_empty() {
                l -= 1;
            }
            self.cursor_line = l;
        }
        self.cursor_col = 0;
        self.scroll_to_cursor();
    }

    // ── File top / bottom ──────────────────────────────────

    pub fn move_file_top(&mut self) {
        self.push_jump();
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.scroll_line = 0;
    }

    pub fn move_file_bottom(&mut self) {
        self.push_jump();
        self.cursor_line = self.buffer.line_count().saturating_sub(1);
        self.cursor_col = 0;
        self.scroll_to_cursor();
    }

    pub fn move_to_line(&mut self, n: usize) {
        self.push_jump();
        let line = n.saturating_sub(1).min(self.buffer.line_count().saturating_sub(1));
        self.cursor_line = line;
        self.cursor_col = 0;
        self.scroll_to_cursor();
    }

    // ── Scroll ─────────────────────────────────────────────

    pub fn scroll_half_down(&mut self) {
        let half = self.edit_height() / 2;
        self.cursor_line = (self.cursor_line + half).min(self.buffer.line_count().saturating_sub(1));
        self.clamp_cursor();
        self.scroll_to_cursor();
    }

    pub fn scroll_half_up(&mut self) {
        let half = self.edit_height() / 2;
        self.cursor_line = self.cursor_line.saturating_sub(half);
        self.scroll_to_cursor();
    }

    pub fn scroll_page_down(&mut self) {
        let h = self.edit_height();
        self.cursor_line = (self.cursor_line + h).min(self.buffer.line_count().saturating_sub(1));
        self.clamp_cursor();
        self.scroll_to_cursor();
    }

    pub fn scroll_page_up(&mut self) {
        let h = self.edit_height();
        self.cursor_line = self.cursor_line.saturating_sub(h);
        self.scroll_to_cursor();
    }

    pub fn scroll_cursor_center(&mut self) {
        let half = self.edit_height() / 2;
        self.scroll_line = self.cursor_line.saturating_sub(half);
    }

    pub fn scroll_cursor_top(&mut self) {
        self.scroll_line = self.cursor_line;
    }

    pub fn scroll_cursor_bottom(&mut self) {
        let h = self.edit_height();
        self.scroll_line = self.cursor_line.saturating_sub(h.saturating_sub(1));
    }

    // ── f / F / t / T ──────────────────────────────────────

    pub fn find_char_forward(&mut self, ch: char, till: bool) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        for i in (self.cursor_col + 1)..chars.len() {
            if chars[i] == ch {
                self.cursor_col = if till { i.saturating_sub(1) } else { i };
                break;
            }
        }
        self.scroll_to_cursor();
    }

    pub fn find_char_backward(&mut self, ch: char, till: bool) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        for i in (0..self.cursor_col).rev() {
            if chars[i] == ch {
                self.cursor_col = if till { i + 1 } else { i };
                break;
            }
        }
        self.scroll_to_cursor();
    }

    // ── % bracket match ────────────────────────────────────

    pub fn move_matching_bracket(&mut self) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        if self.cursor_col >= chars.len() { return; }
        let ch = chars[self.cursor_col];
        let (open, close, forward) = match ch {
            '(' => ('(', ')', true),
            ')' => ('(', ')', false),
            '[' => ('[', ']', true),
            ']' => ('[', ']', false),
            '{' => ('{', '}', true),
            '}' => ('{', '}', false),
            _ => return,
        };

        // Search through entire buffer as a char sequence
        let full: Vec<char> = self.buffer.rope.chars().collect();
        let start_char = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
        let mut depth = 0i32;

        if forward {
            for i in start_char..full.len() {
                if full[i] == open { depth += 1; }
                else if full[i] == close {
                    depth -= 1;
                    if depth == 0 {
                        let line = self.buffer.char_to_line(i);
                        let col = i - self.buffer.line_to_char(line);
                        self.push_jump();
                        self.cursor_line = line;
                        self.cursor_col = col;
                        self.scroll_to_cursor();
                        return;
                    }
                }
            }
        } else {
            for i in (0..=start_char).rev() {
                if full[i] == close { depth += 1; }
                else if full[i] == open {
                    depth -= 1;
                    if depth == 0 {
                        let line = self.buffer.char_to_line(i);
                        let col = i - self.buffer.line_to_char(line);
                        self.push_jump();
                        self.cursor_line = line;
                        self.cursor_col = col;
                        self.scroll_to_cursor();
                        return;
                    }
                }
            }
        }
    }

    // ── H / M / L — screen-relative jumps ─────────────────

    pub fn move_screen_top(&mut self) {
        let off = self.config.general.scroll_off;
        self.cursor_line = (self.scroll_line + off).min(self.buffer.line_count().saturating_sub(1));
        self.move_line_start_nonblank();
    }

    pub fn move_screen_middle(&mut self) {
        let mid = self.scroll_line + self.edit_height() / 2;
        self.cursor_line = mid.min(self.buffer.line_count().saturating_sub(1));
        self.move_line_start_nonblank();
    }

    pub fn move_screen_bottom(&mut self) {
        let off = self.config.general.scroll_off;
        let bottom = self.scroll_line + self.edit_height();
        self.cursor_line = bottom.saturating_sub(off + 1)
            .min(self.buffer.line_count().saturating_sub(1));
        self.move_line_start_nonblank();
    }

    // ── Marks ──────────────────────────────────────────────

    /// Set mark `ch` at current cursor position.
    pub fn set_mark(&mut self, ch: char) {
        self.marks.insert(ch, (self.cursor_line, self.cursor_col));
    }

    /// Jump to exact position of mark `ch` (`` `a ``).
    pub fn jump_to_mark(&mut self, ch: char) {
        if ch == '\'' || ch == '`' {
            // '' / `` — jump to previous position
            if let Some((l, c)) = self.mark_prev.take() {
                let cur = (self.cursor_line, self.cursor_col);
                self.mark_prev = Some(cur);
                self.push_jump();
                self.cursor_line = l;
                self.cursor_col = c;
                self.scroll_to_cursor();
            }
            return;
        }
        if let Some(&(l, c)) = self.marks.get(&ch) {
            let cur = (self.cursor_line, self.cursor_col);
            self.mark_prev = Some(cur);
            self.push_jump();
            self.cursor_line = l.min(self.buffer.line_count().saturating_sub(1));
            self.cursor_col = c;
            self.clamp_cursor();
            self.scroll_to_cursor();
        }
    }

    /// Jump to first non-blank of mark's line (`'a`).
    pub fn jump_to_mark_line(&mut self, ch: char) {
        if ch == '\'' || ch == '`' {
            if let Some(&(l, _)) = self.marks.get(&ch).or(self.mark_prev.as_ref().map(|_| &(0usize, 0usize))) {
                let _ = l; // handled below
            }
            // '' — jump to previous line
            if let Some((l, c)) = self.mark_prev.take() {
                let cur = (self.cursor_line, self.cursor_col);
                self.mark_prev = Some(cur);
                self.push_jump();
                self.cursor_line = l;
                self.cursor_col = c;
                self.move_line_start_nonblank();
                self.scroll_to_cursor();
            }
            return;
        }
        if let Some(&(l, _)) = self.marks.get(&ch) {
            let cur = (self.cursor_line, self.cursor_col);
            self.mark_prev = Some(cur);
            self.push_jump();
            self.cursor_line = l.min(self.buffer.line_count().saturating_sub(1));
            self.move_line_start_nonblank();
            self.scroll_to_cursor();
        }
    }

    // ── Ctrl-a / Ctrl-x — increment / decrement number ────

    /// Find the number under or after the cursor on the current line,
    /// add `delta` to it, and replace it in the buffer.
    pub fn increment_number_at_cursor(&mut self, delta: i64) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();

        // Find the start of the first number at or after cursor_col
        let mut num_start = None;
        let mut search_from = self.cursor_col.min(chars.len().saturating_sub(1));

        // If cursor is already on a digit, scan left to find the start
        if search_from < chars.len() && chars[search_from].is_ascii_digit() {
            while search_from > 0 && chars[search_from - 1].is_ascii_digit() {
                search_from -= 1;
            }
            // Check for leading minus
            if search_from > 0 && chars[search_from - 1] == '-' {
                search_from -= 1;
            }
            num_start = Some(search_from);
        } else {
            // Scan right for a digit
            for i in search_from..chars.len() {
                if chars[i].is_ascii_digit() {
                    let s = if i > 0 && chars[i - 1] == '-' { i - 1 } else { i };
                    num_start = Some(s);
                    break;
                }
            }
        }

        let start_col = match num_start {
            Some(s) => s,
            None => {
                self.set_msg("光标处无数字".to_string());
                return;
            }
        };

        // Find end of number
        let digit_start = if chars[start_col] == '-' { start_col + 1 } else { start_col };
        let mut end_col = digit_start;
        while end_col < chars.len() && chars[end_col].is_ascii_digit() {
            end_col += 1;
        }

        let num_str: String = chars[start_col..end_col].iter().collect();
        let value: i64 = match num_str.parse() {
            Ok(v) => v,
            Err(_) => return,
        };
        let new_value = value + delta;
        let new_str = new_value.to_string();

        // Replace in buffer
        let char_start = self.buffer.line_to_char(self.cursor_line) + start_col;
        let char_end   = self.buffer.line_to_char(self.cursor_line) + end_col;
        self.buffer.delete_range(char_start, char_end);
        self.buffer.insert_str(char_start, &new_str);

        // Move cursor to end of new number
        self.cursor_col = start_col + new_str.len().saturating_sub(1);
        self.clamp_cursor();
    }

    // ── gd — go to definition (file-local) ────────────────
    //
    // Strategy: extract the identifier under the cursor, then scan upward
    // from the current line for the first line that looks like a definition:
    //   - function/fn/def/class/let/const/var/type declaration containing the word
    //   - or simply the first occurrence of the word above the cursor
    // Falls back to the first occurrence in the file if nothing is found above.
    pub fn goto_definition(&mut self) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let col = self.cursor_col.min(chars.len().saturating_sub(1));

        // Extract identifier under cursor
        let is_ident = |c: char| c.is_alphanumeric() || c == '_';
        let mut s = col;
        let mut e = col;
        if col < chars.len() && is_ident(chars[col]) {
            while s > 0 && is_ident(chars[s - 1]) { s -= 1; }
            while e + 1 < chars.len() && is_ident(chars[e + 1]) { e += 1; }
            e += 1;
        }
        if s == e { return; }
        let word: String = chars[s..e].iter().collect();

        // Definition-like keywords (language-agnostic heuristic)
        let def_keywords = ["fn ", "def ", "function ", "class ", "let ", "const ",
                            "var ", "type ", "struct ", "enum ", "interface ",
                            "func ", "sub ", "proc "];

        let lc = self.buffer.line_count();

        // First pass: scan upward for a definition line
        let mut found: Option<(usize, usize)> = None;
        'outer: for ln in (0..self.cursor_line).rev() {
            let ls = self.buffer.line_str(ln);
            if !ls.contains(&*word) { continue; }
            let lchars: Vec<char> = ls.chars().collect();
            // Check if this line looks like a definition
            let is_def = def_keywords.iter().any(|kw| ls.contains(kw));
            if is_def {
                // Find the column of the word
                let lstr = ls.as_str();
                if let Some(byte_pos) = lstr.find(&*word) {
                    let col_pos = lstr[..byte_pos].chars().count();
                    found = Some((ln, col_pos));
                    break 'outer;
                }
            }
            let _ = lchars;
        }

        // Second pass: if no definition found above, take first occurrence in file
        if found.is_none() {
            'outer2: for ln in 0..lc {
                if ln == self.cursor_line { continue; }
                let ls = self.buffer.line_str(ln);
                if let Some(byte_pos) = ls.find(&*word) {
                    let col_pos = ls[..byte_pos].chars().count();
                    found = Some((ln, col_pos));
                    break 'outer2;
                }
            }
        }

        if let Some((target_line, target_col)) = found {
            self.push_jump();
            self.cursor_line = target_line;
            self.cursor_col  = target_col;
            self.clamp_cursor();
            self.scroll_to_cursor();
            self.set_msg(format!("gd → 第 {} 行", target_line + 1));
        } else {
            self.set_msg(format!("未找到 '{}' 的定义", word));
        }
    }
}
