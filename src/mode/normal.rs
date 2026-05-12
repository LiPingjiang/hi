//! Normal mode key handling.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::editor::{Editor, FindState, RepeatAction};
use crate::mode::VisualKind;

/// What the main loop should do after handling a key in Normal mode.
pub enum NormalAction {
    None,
    EnterInsert { col_offset: i32 },
    EnterInsertNewline { above: bool },
    EnterVisual { kind: VisualKind },
    EnterCommand,
    EnterSearch,
    EnterAi,
    ExecuteCommand(String),
    Quit { force: bool },
    OpenFileAtCursor,
    ToggleFileTree,
    ToggleChatPanel,
    SwitchFocus,
    AiAction(AiSubAction),
    /// Replay last repeatable action
    DotRepeat,
    /// Start macro replay
    PlayMacro(char),
    /// zh — toggle hidden files in file tree
    ToggleHiddenFiles,
}

pub enum AiSubAction {
    Ghost(String),
    Query(String),
}

impl Editor {
    /// Handle one key event in Normal mode. Returns an action for the caller.
    pub fn handle_normal_key(&mut self, key: KeyEvent) -> NormalAction {
        // Collect digit prefix
        if let KeyCode::Char(c) = key.code {
            if c.is_ascii_digit() && (c != '0' || !self.pending_count.is_empty()) {
                self.pending_count.push(c);
                return NormalAction::None;
            }
        }

        let count = self.take_count();

        // Two-key sequences (pending_key set on previous key)
        if let Some(first) = self.pending_key.take() {
            // If recording a macro, capture this key too
            if self.macro_recording.is_some() {
                self.macro_append_key(key);
            }
            return self.handle_two_key_normal(first, key, count);
        }

        // Macro recording: append every normal-mode key
        if self.macro_recording.is_some() {
            self.macro_append_key(key);
        }

        // Single-key dispatch
        match key.code {
            // ── Cursor movement ─────────────────────────
            KeyCode::Char('h') | KeyCode::Left  => { self.move_left(count);  NormalAction::None }
            KeyCode::Char('l') if !key.modifiers.contains(KeyModifiers::CONTROL) => { self.move_right(count); NormalAction::None }
            KeyCode::Right => { self.move_right(count); NormalAction::None }
            KeyCode::Char('j') | KeyCode::Down  => { self.move_down(count);  NormalAction::None }
            KeyCode::Char('k') | KeyCode::Up    => { self.move_up(count);    NormalAction::None }
            KeyCode::Char('w') if !key.modifiers.contains(KeyModifiers::CONTROL) => { self.move_word_forward(count); NormalAction::None }
            KeyCode::Char('b') if !key.modifiers.contains(KeyModifiers::CONTROL) => { self.move_word_back(count);    NormalAction::None }
            KeyCode::Char('e') => { self.move_word_end(count);     NormalAction::None }
            KeyCode::Char('W') => { for _ in 0..count { self.move_word_forward_big(); } NormalAction::None }
            KeyCode::Char('B') => { for _ in 0..count { self.move_word_back_big(); }    NormalAction::None }
            KeyCode::Char('E') => { for _ in 0..count { self.move_word_end_big(); }     NormalAction::None }
            KeyCode::Char('0') => { self.move_line_start();          NormalAction::None }
            KeyCode::Char('^') => { self.move_line_start_nonblank(); NormalAction::None }
            KeyCode::Char('$') => { self.move_line_end();            NormalAction::None }
            KeyCode::Char('G') if count > 1 => { self.move_to_line(count); NormalAction::None }
            KeyCode::Char('G') => { self.move_file_bottom(); NormalAction::None }
            KeyCode::Char('{') => { self.move_paragraph_back(count);   NormalAction::None }
            KeyCode::Char('}') => { self.move_paragraph_forward(count); NormalAction::None }
            KeyCode::Char('%') => { self.move_matching_bracket();       NormalAction::None }
            KeyCode::Char('H') => { self.move_screen_top();    NormalAction::None }
            KeyCode::Char('M') => { self.move_screen_middle(); NormalAction::None }
            KeyCode::Char('L') => { self.move_screen_bottom(); NormalAction::None }
            // m{a-z} — set mark; `{a-z} — jump exact; '{a-z} — jump line
            KeyCode::Char('m') => { self.pending_key = Some('m'); NormalAction::None }
            KeyCode::Char('`') => { self.pending_key = Some('`'); NormalAction::None }
            KeyCode::Char('\'') => { self.pending_key = Some('\''); NormalAction::None }
            KeyCode::Char('n') => { for _ in 0..count { self.search_next(); } NormalAction::None }
            KeyCode::Char('N') => { for _ in 0..count { self.search_prev(); } NormalAction::None }
            KeyCode::Char('*') => { self.search_word_under_cursor(true);  NormalAction::None }
            KeyCode::Char('#') => { self.search_word_under_cursor(false); NormalAction::None }

            // ; — repeat last f/F/t/T in same direction
            KeyCode::Char(';') => {
                if let Some(fs) = self.last_find.clone() {
                    for _ in 0..count {
                        if fs.forward {
                            self.find_char_forward(fs.ch, fs.till);
                        } else {
                            self.find_char_backward(fs.ch, fs.till);
                        }
                    }
                }
                NormalAction::None
            }
            // , — repeat last f/F/t/T in reverse direction
            KeyCode::Char(',') => {
                if let Some(fs) = self.last_find.clone() {
                    for _ in 0..count {
                        if fs.forward {
                            self.find_char_backward(fs.ch, fs.till);
                        } else {
                            self.find_char_forward(fs.ch, fs.till);
                        }
                    }
                }
                NormalAction::None
            }

            // Ctrl variants
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.increment_number_at_cursor(count as i64);
                NormalAction::None
            }
            KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.increment_number_at_cursor(-(count as i64));
                NormalAction::None
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_half_down(); NormalAction::None
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_half_up(); NormalAction::None
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_page_down(); NormalAction::None
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_page_up(); NormalAction::None
            }
            KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.jump_back(); NormalAction::None
            }
            KeyCode::Char('i') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.jump_forward(); NormalAction::None
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(pos) = self.buffer.redo() {
                    self.set_cursor_from_char_idx(pos);
                }
                NormalAction::None
            }
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                NormalAction::ToggleFileTree
            }
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                NormalAction::ToggleChatPanel
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                NormalAction::SwitchFocus
            }

            // ── Mode transitions ────────────────────────
            KeyCode::Char('i') => NormalAction::EnterInsert { col_offset: 0 },
            KeyCode::Char('a') => NormalAction::EnterInsert { col_offset: 1 },
            KeyCode::Char('I') => {
                self.move_line_start_nonblank();
                NormalAction::EnterInsert { col_offset: 0 }
            }
            KeyCode::Char('A') => {
                self.move_line_end();
                // In insert mode col can be at line_len
                NormalAction::EnterInsert { col_offset: 1 }
            }
            KeyCode::Char('o') => NormalAction::EnterInsertNewline { above: false },
            KeyCode::Char('O') => NormalAction::EnterInsertNewline { above: true },
            KeyCode::Char('s') => {
                self.delete_char_at_cursor();
                NormalAction::EnterInsert { col_offset: 0 }
            }
            KeyCode::Char('S') => {
                self.clear_current_line();
                NormalAction::EnterInsert { col_offset: 0 }
            }
            KeyCode::Char('C') => {
                self.delete_to_eol();
                NormalAction::EnterInsert { col_offset: 0 }
            }
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                NormalAction::EnterVisual { kind: VisualKind::Block }
            }
            KeyCode::Char('v') => NormalAction::EnterVisual { kind: VisualKind::Char },
            KeyCode::Char('V') => NormalAction::EnterVisual { kind: VisualKind::Line },
            KeyCode::Char(':') => NormalAction::EnterCommand,
            KeyCode::Char('/') => NormalAction::EnterSearch,
            KeyCode::Char('?') => NormalAction::EnterAi,

            // ── Focus cycling ────────────────────────────
            KeyCode::Tab => NormalAction::SwitchFocus,

            // ── Editing ─────────────────────────────────
            KeyCode::Char('x') => {
                for _ in 0..count { self.delete_char_at_cursor(); }
                NormalAction::None
            }
            KeyCode::Char('X') => {
                for _ in 0..count { self.delete_char_before_cursor(); }
                NormalAction::None
            }
            KeyCode::Char('D') => { self.delete_to_eol(); NormalAction::None }
            KeyCode::Char('J') => { for _ in 0..count { self.join_line(); } NormalAction::None }
            KeyCode::Char('p') => { self.paste_after(count);  NormalAction::None }
            KeyCode::Char('P') => { self.paste_before(count); NormalAction::None }
            KeyCode::Char('u') => {
                if let Some(pos) = self.buffer.undo() {
                    self.set_cursor_from_char_idx(pos);
                }
                NormalAction::None
            }
            KeyCode::Char('~') => { self.toggle_case_at_cursor(); NormalAction::None }

            // ── Dot-repeat ──────────────────────────────
            KeyCode::Char('.') => NormalAction::DotRepeat,

            // ── Register prefix: "{char} ────────────────
            KeyCode::Char('"') => {
                self.pending_key = Some('"');
                NormalAction::None
            }

            // ── Macro: q{reg} start/stop, @{reg} play ───
            KeyCode::Char('q') => {
                if self.macro_recording.is_some() {
                    // Stop recording (q pressed again)
                    self.macro_recording = None;
                    self.editor_msg_stop_macro();
                    NormalAction::None
                } else {
                    self.pending_key = Some('q');
                    NormalAction::None
                }
            }
            KeyCode::Char('@') => {
                self.pending_key = Some('@');
                NormalAction::None
            }

            // ── zz / zt / zb via pending 'z' ────────────
            // ── gg / dd / yy / cc / >> / << / gU / gu via pending ──
            KeyCode::Char(c) if matches!(c, 'g' | 'd' | 'y' | 'c' | 'r' | 'f' | 'F' | 't' | 'T' | 'z' | '>' | '<') => {
                self.pending_key = Some(c);
                NormalAction::None
            }
            // gf needs to be triggered after pending 'g' is set, handled in two-key
            KeyCode::Char('f') if self.pending_key.is_none() => {
                // standalone f — wait for next char
                self.pending_key = Some('f');
                NormalAction::None
            }

            _ => NormalAction::None,
        }
    }

    fn handle_two_key_normal(&mut self, first: char, key: KeyEvent, count: usize) -> NormalAction {
        // ── Three-key text-object: operator + (i/a) + delimiter ─────────
        // Stage 1: op + i/a  →  store op in pending_operator, wait for delimiter
        if matches!(first, 'd' | 'y' | 'c') {
            if let KeyCode::Char(motion) = key.code {
                if matches!(motion, 'i' | 'a') {
                    self.pending_operator = Some(first);
                    self.pending_key = Some(motion); // 'i' or 'a'
                    return NormalAction::None;
                }
            }
        }
        // Stage 2: (i/a) + delimiter  →  pending_operator is set
        if let Some(op) = self.pending_operator.take() {
            let inner = first == 'i';
            if let KeyCode::Char(delim) = key.code {
                self.execute_text_obj(op, inner, delim, count);
                if op == 'c' {
                    return NormalAction::EnterInsert { col_offset: 0 };
                }
            }
            return NormalAction::None;
        }

        match (first, key.code) {
            // gg → go to top
            ('g', KeyCode::Char('g')) => { self.move_file_top(); NormalAction::None }
            // gd → go to definition (file-local)
            ('g', KeyCode::Char('d')) => { self.goto_definition(); NormalAction::None }
            // gf → open file under cursor
            ('g', KeyCode::Char('f')) => NormalAction::OpenFileAtCursor,
            // gU → uppercase operator (wait for motion)
            ('g', KeyCode::Char('U')) => {
                self.pending_operator = Some('U');
                self.pending_key = Some('U'); // reuse pending_key as motion-wait marker
                NormalAction::None
            }
            // gu → lowercase operator (wait for motion)
            ('g', KeyCode::Char('u')) => {
                self.pending_operator = Some('u');
                self.pending_key = Some('u'); // reuse pending_key as motion-wait marker
                NormalAction::None
            }
            // gUU → uppercase current line
            ('U', KeyCode::Char('U')) => {
                self.pending_operator = None;
                self.apply_case_line(true);
                NormalAction::None
            }
            // guu → lowercase current line
            ('u', KeyCode::Char('u')) => {
                self.pending_operator = None;
                self.apply_case_line(false);
                NormalAction::None
            }
            // gU{motion} or gu{motion}
            ('U', _) | ('u', _) => {
                let upper = first == 'U';
                self.pending_operator = None;
                self.apply_case_operator_key(upper, key, count);
                NormalAction::None
            }

            // dd → delete line(s)
            ('d', KeyCode::Char('d')) => {
                use crate::editor::RegisterEntry;
                let reg = self.active_register.take();
                for _ in 0..count {
                    let yanked = self.buffer.delete_line(self.cursor_line.min(self.buffer.line_count().saturating_sub(1)));
                    if let Some(r) = reg {
                        self.named_registers.insert(r, RegisterEntry { text: yanked.clone(), linewise: true });
                    }
                    self.buffer.register = yanked;
                    self.buffer.register_linewise = true;
                }
                self.clamp_cursor();
                NormalAction::None
            }
            // d + motion/text-object
            ('d', _) => {
                self.execute_operator_key('d', key, count);
                NormalAction::None
            }

            // yy → yank line(s)
            ('y', KeyCode::Char('y')) => {
                use crate::editor::RegisterEntry;
                let reg = self.active_register.take();
                let mut yanked = String::new();
                for i in 0..count {
                    let l = (self.cursor_line + i).min(self.buffer.line_count().saturating_sub(1));
                    yanked.push_str(&self.buffer.line_str(l));
                    yanked.push('\n');
                }
                if let Some(r) = reg {
                    self.named_registers.insert(r, RegisterEntry { text: yanked.clone(), linewise: true });
                }
                self.buffer.register = yanked;
                self.buffer.register_linewise = true;
                NormalAction::None
            }
            ('y', _) => {
                self.execute_operator_key('y', key, count);
                NormalAction::None
            }

            // cc → change line
            ('c', KeyCode::Char('c')) => {
                self.clear_current_line();
                NormalAction::EnterInsert { col_offset: 0 }
            }
            ('c', _) => {
                self.execute_operator_key('c', key, count);
                NormalAction::EnterInsert { col_offset: 0 }
            }

            // r{char} → replace char
            ('r', KeyCode::Char(c)) => {
                self.replace_char_at_cursor(c);
                self.last_action = Some(RepeatAction::ReplaceChar(c));
                NormalAction::None
            }

            // f{char}, F{char}, t{char}, T{char}
            ('f', KeyCode::Char(c)) => {
                self.last_find = Some(FindState { ch: c, forward: true, till: false });
                for _ in 0..count { self.find_char_forward(c, false); }
                NormalAction::None
            }
            ('F', KeyCode::Char(c)) => {
                self.last_find = Some(FindState { ch: c, forward: false, till: false });
                for _ in 0..count { self.find_char_backward(c, false); }
                NormalAction::None
            }
            ('t', KeyCode::Char(c)) => {
                self.last_find = Some(FindState { ch: c, forward: true, till: true });
                for _ in 0..count { self.find_char_forward(c, true); }
                NormalAction::None
            }
            ('T', KeyCode::Char(c)) => {
                self.last_find = Some(FindState { ch: c, forward: false, till: true });
                for _ in 0..count { self.find_char_backward(c, true); }
                NormalAction::None
            }

            // >> and <<
            ('>', KeyCode::Char('>')) => {
                for _ in 0..count { self.indent_line(self.cursor_line, true); }
                self.last_action = Some(RepeatAction::Indent(true));
                NormalAction::None
            }
            ('<', KeyCode::Char('<')) => {
                for _ in 0..count { self.indent_line(self.cursor_line, false); }
                self.last_action = Some(RepeatAction::Indent(false));
                NormalAction::None
            }

            // zz / zt / zb
            ('z', KeyCode::Char('z')) => { self.scroll_cursor_center(); NormalAction::None }
            ('z', KeyCode::Char('t')) => { self.scroll_cursor_top();    NormalAction::None }
            ('z', KeyCode::Char('b')) => { self.scroll_cursor_bottom();  NormalAction::None }
            // zh — toggle hidden files in filetree
            ('z', KeyCode::Char('h')) => NormalAction::ToggleHiddenFiles,

            // ── Register prefix "{reg}{op} ────────────
            ('"', KeyCode::Char(c)) => {
                self.active_register = Some(c);
                NormalAction::None
            }

            // ── Macro: q{reg} → start recording ─────────
            ('q', KeyCode::Char(c)) if c.is_ascii_alphabetic() || c.is_ascii_digit() => {
                self.macro_recording = Some(c);
                self.macros.insert(c, Vec::new());
                self.set_msg(format!("录制宏 @{}", c));
                NormalAction::None
            }

            // ── Macro: @{reg} → play back ────────────────
            ('@', KeyCode::Char(c)) => NormalAction::PlayMacro(c),

            // ── Marks ─────────────────────────────────────
            // m{a-z/A-Z/0-9} — set mark
            ('m', KeyCode::Char(c)) if c.is_ascii_alphanumeric() => {
                self.set_mark(c);
                self.set_msg(format!("标记 '{}", c));
                NormalAction::None
            }
            // `{ch} — jump to exact position
            ('`', KeyCode::Char(c)) => {
                self.jump_to_mark(c);
                NormalAction::None
            }
            // '{ch} — jump to first non-blank of mark's line
            ('\'', KeyCode::Char(c)) => {
                self.jump_to_mark_line(c);
                NormalAction::None
            }

            _ => NormalAction::None,
        }
    }

    // ── Helpers ────────────────────────────────────────────

    pub fn take_count(&mut self) -> usize {
        if self.pending_count.is_empty() {
            1
        } else {
            let n: usize = self.pending_count.parse().unwrap_or(1);
            self.pending_count.clear();
            n
        }
    }

    pub fn set_cursor_from_char_idx(&mut self, char_idx: usize) {
        let line = self.buffer.char_to_line(char_idx.min(self.buffer.len_chars().saturating_sub(1)));
        let line_start = self.buffer.line_to_char(line);
        let col = char_idx.saturating_sub(line_start);
        self.cursor_line = line;
        self.cursor_col = col;
        self.clamp_cursor();
        self.scroll_to_cursor();
    }

    fn delete_char_at_cursor(&mut self) {
        let line = self.buffer.line_str(self.cursor_line);
        if line.is_empty() { return; }
        let chars: Vec<char> = line.chars().collect();
        if self.cursor_col >= chars.len() { return; }
        let char_idx = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
        let yanked: String = chars[self.cursor_col..self.cursor_col+1].iter().collect();
        self.buffer.register = yanked;
        self.buffer.register_linewise = false;
        self.buffer.delete_range(char_idx, char_idx + 1);
        self.clamp_cursor();
    }

    fn delete_char_before_cursor(&mut self) {
        if self.cursor_col == 0 { return; }
        self.cursor_col -= 1;
        let char_idx = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
        self.buffer.delete_range(char_idx, char_idx + 1);
        self.clamp_cursor();
    }

    fn delete_to_eol(&mut self) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        if self.cursor_col >= chars.len() { return; }
        let start = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
        let end   = self.buffer.pos_to_char(self.cursor_line, chars.len());
        let yanked: String = chars[self.cursor_col..].iter().collect();
        self.buffer.register = yanked;
        self.buffer.register_linewise = false;
        self.buffer.delete_range(start, end);
        self.clamp_cursor();
    }

    fn clear_current_line(&mut self) {
        let line = self.buffer.line_str(self.cursor_line);
        let start = self.buffer.pos_to_char(self.cursor_line, 0);
        let end = start + line.chars().count();
        self.buffer.register = line;
        self.buffer.register_linewise = false;
        self.buffer.delete_range(start, end);
        self.cursor_col = 0;
    }

    pub fn replace_char_at_cursor(&mut self, ch: char) {
        let char_idx = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
        let line = self.buffer.line_str(self.cursor_line);
        if self.cursor_col < line.chars().count() {
            self.buffer.delete_range(char_idx, char_idx + 1);
            self.buffer.insert_char(char_idx, ch);
        }
    }

    fn toggle_case_at_cursor(&mut self) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        if self.cursor_col >= chars.len() { return; }
        let c = chars[self.cursor_col];
        let toggled = if c.is_uppercase() {
            c.to_lowercase().next().unwrap_or(c)
        } else {
            c.to_uppercase().next().unwrap_or(c)
        };
        let char_idx = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
        self.buffer.delete_range(char_idx, char_idx + 1);
        self.buffer.insert_char(char_idx, toggled);
        self.move_right(1);
    }

    fn join_line(&mut self) {
        let lc = self.buffer.line_count();
        if self.cursor_line + 1 >= lc { return; }
        let next_trimmed = self.buffer.line_str(self.cursor_line + 1)
            .trim_start().to_string();
        let this_end = self.buffer.pos_to_char(
            self.cursor_line,
            self.buffer.line_len(self.cursor_line)
        );
        // Remove the newline at end of this line
        self.buffer.delete_range(this_end, this_end + 1);
        // Insert space + trimmed next line content
        self.buffer.insert_str(this_end, &format!(" {}", next_trimmed));
        self.cursor_col = self.buffer.line_len(self.cursor_line).saturating_sub(next_trimmed.chars().count()).saturating_sub(1);
    }

    fn paste_after(&mut self, count: usize) {
        let (reg, linewise) = self.get_paste_register();
        if linewise {
            let insert_line = self.cursor_line + 1;
            let insert_char = if insert_line < self.buffer.line_count() {
                self.buffer.line_to_char(insert_line)
            } else {
                // After last line: append newline then content
                let last_char = self.buffer.len_chars();
                self.buffer.insert_char(last_char, '\n');
                self.buffer.len_chars()
            };
            for _ in 0..count {
                self.buffer.insert_str(insert_char, &reg);
            }
            self.cursor_line = insert_line;
            self.cursor_col = 0;
        } else {
            let char_idx = self.buffer.pos_to_char(self.cursor_line, self.cursor_col) + 1;
            let char_idx = char_idx.min(self.buffer.len_chars());
            for _ in 0..count {
                self.buffer.insert_str(char_idx, &reg);
            }
            let reg_len = reg.chars().count();
            self.cursor_col += reg_len * count;
            self.clamp_cursor();
        }
    }

    fn paste_before(&mut self, count: usize) {
        let (reg, linewise) = self.get_paste_register();
        if linewise {
            let insert_char = self.buffer.line_to_char(self.cursor_line);
            for _ in 0..count {
                self.buffer.insert_str(insert_char, &reg);
            }
            // cursor stays on same line (now content shifted)
        } else {
            let char_idx = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
            for _ in 0..count {
                self.buffer.insert_str(char_idx, &reg);
            }
        }
    }

    /// Return (text, linewise) from the active named register or the default register.
    fn get_paste_register(&mut self) -> (String, bool) {
        if let Some(reg) = self.active_register.take() {
            if let Some(entry) = self.named_registers.get(&reg) {
                return (entry.text.clone(), entry.linewise);
            }
        }
        (self.buffer.register.clone(), self.buffer.register_linewise)
    }

    pub fn indent_line(&mut self, line: usize, right: bool) {
        let tab_str: String = if self.config.general.expand_tab {
            " ".repeat(self.config.general.tab_width)
        } else {
            "\t".to_string()
        };
        let start = self.buffer.line_to_char(line);
        if right {
            self.buffer.insert_str(start, &tab_str);
        } else {
            // Remove up to tab_width spaces from start
            let line_s = self.buffer.line_str(line);
            let spaces: usize = line_s.chars().take_while(|c| *c == ' ').count();
            let remove = spaces.min(self.config.general.tab_width);
            if remove > 0 {
                self.buffer.delete_range(start, start + remove);
            }
        }
    }

    fn search_word_under_cursor(&mut self, forward: bool) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let col = self.cursor_col.min(chars.len().saturating_sub(1));
        // Find word boundaries
        let mut start = col;
        let mut end = col;
        while start > 0 && (chars[start-1].is_alphanumeric() || chars[start-1] == '_') {
            start -= 1;
        }
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }
        if start == end { return; }
        let word: String = chars[start..end].iter().collect();
        let ic = self.config.general.ignore_case;
        self.run_search(&word, ic);
        if !forward {
            self.search_prev();
        }
    }

    /// Execute operator (d/y/c) + text object / motion from a KeyEvent.
    pub(crate) fn execute_operator_key(&mut self, op: char, key: KeyEvent, count: usize) {
        match key.code {
            // ── word text objects ────────────────────────────────────────
            KeyCode::Char('w') => {
                let (start, end) = self.text_obj_word(false);
                self.apply_operator(op, start, end, false);
            }
            KeyCode::Char('W') => {
                let (start, end) = self.text_obj_word(true);
                self.apply_operator(op, start, end, false);
            }
            KeyCode::Char('p') => {
                let (start, end) = self.text_obj_paragraph();
                self.apply_operator(op, start, end, true);
            }

            // ── motion: $ (to end of line) ───────────────────────────────
            KeyCode::Char('$') => {
                let line = self.buffer.line_str(self.cursor_line);
                let line_chars = line.chars().count();
                let start = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                let end   = self.buffer.pos_to_char(self.cursor_line, line_chars);
                if start < end { self.apply_operator(op, start, end, false); }
            }

            // ── motion: 0 (to start of line) ────────────────────────────
            KeyCode::Char('0') => {
                let end   = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                let start = self.buffer.line_to_char(self.cursor_line);
                if start < end { self.apply_operator(op, start, end, false); }
            }

            // ── motion: ^ (to first non-blank) ──────────────────────────
            KeyCode::Char('^') => {
                let line = self.buffer.line_str(self.cursor_line);
                let first_nb = line.chars().take_while(|c| c.is_whitespace()).count();
                let cur = self.cursor_col;
                let ls  = self.buffer.line_to_char(self.cursor_line);
                let (start, end) = if cur >= first_nb {
                    (ls + first_nb, ls + cur)
                } else {
                    (ls + cur, ls + first_nb)
                };
                if start < end { self.apply_operator(op, start, end, false); }
            }

            // ── motion: G (to end of file, linewise) ────────────────────
            KeyCode::Char('G') => {
                let target_line = if count > 1 { count.saturating_sub(1) } else { self.buffer.line_count().saturating_sub(1) };
                let (from, to) = if self.cursor_line <= target_line {
                    (self.cursor_line, target_line)
                } else {
                    (target_line, self.cursor_line)
                };
                let start = self.buffer.line_to_char(from);
                let end   = if to + 1 < self.buffer.line_count() {
                    self.buffer.line_to_char(to + 1)
                } else {
                    self.buffer.len_chars()
                };
                if start < end { self.apply_operator(op, start, end, true); }
            }

            // ── motion: j (down N lines, linewise) ──────────────────────
            KeyCode::Char('j') => {
                let from = self.cursor_line;
                let to   = (from + count).min(self.buffer.line_count().saturating_sub(1));
                let start = self.buffer.line_to_char(from);
                let end   = if to + 1 < self.buffer.line_count() {
                    self.buffer.line_to_char(to + 1)
                } else {
                    self.buffer.len_chars()
                };
                if start < end { self.apply_operator(op, start, end, true); }
            }

            // ── motion: k (up N lines, linewise) ────────────────────────
            KeyCode::Char('k') => {
                let to   = self.cursor_line;
                let from = to.saturating_sub(count);
                let start = self.buffer.line_to_char(from);
                let end   = if to + 1 < self.buffer.line_count() {
                    self.buffer.line_to_char(to + 1)
                } else {
                    self.buffer.len_chars()
                };
                if start < end { self.apply_operator(op, start, end, true); }
            }

            // ── motion: e (to end of word) ───────────────────────────────
            KeyCode::Char('e') => {
                let start = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                // move_word_end advances cursor; capture position after
                let saved_line = self.cursor_line;
                let saved_col  = self.cursor_col;
                self.move_word_end(count);
                let end = self.buffer.pos_to_char(self.cursor_line, self.cursor_col) + 1;
                self.cursor_line = saved_line;
                self.cursor_col  = saved_col;
                let end = end.min(self.buffer.len_chars());
                if start < end { self.apply_operator(op, start, end, false); }
            }

            // ── motion: b (back word) ────────────────────────────────────
            KeyCode::Char('b') => {
                let end   = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                let saved_line = self.cursor_line;
                let saved_col  = self.cursor_col;
                self.move_word_back(count);
                let start = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                self.cursor_line = saved_line;
                self.cursor_col  = saved_col;
                if start < end { self.apply_operator(op, start, end, false); }
            }

            _ => {}
        }
    }

    /// `iw` / `aw` — word text object (alphanumeric + `_` delimited).
    /// `inner=false` (aw) includes trailing whitespace.
    fn text_obj_word(&self, inner: bool) -> (usize, usize) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let col = self.cursor_col.min(chars.len().saturating_sub(1));
        let is_w = |c: char| c.is_alphanumeric() || c == '_';
        let mut s = col;
        let mut e = col;
        if col < chars.len() && is_w(chars[col]) {
            while s > 0 && is_w(chars[s-1]) { s -= 1; }
            while e + 1 < chars.len() && is_w(chars[e+1]) { e += 1; }
            e += 1;
        }
        if !inner {
            // aw: include trailing whitespace
            while e < chars.len() && chars[e].is_whitespace() { e += 1; }
        }
        let start = self.buffer.pos_to_char(self.cursor_line, s);
        let end   = self.buffer.pos_to_char(self.cursor_line, e);
        (start, end)
    }

    /// `iW` / `aW` — WORD text object (whitespace-delimited).
    /// `inner=false` (aW) includes trailing whitespace.
    fn text_obj_word_big(&self, inner: bool) -> (usize, usize) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let col = self.cursor_col.min(chars.len().saturating_sub(1));
        let is_w = |c: char| !c.is_whitespace();
        let mut s = col;
        let mut e = col;
        if col < chars.len() && is_w(chars[col]) {
            while s > 0 && is_w(chars[s-1]) { s -= 1; }
            while e + 1 < chars.len() && is_w(chars[e+1]) { e += 1; }
            e += 1;
        }
        if !inner {
            // aW: include trailing whitespace
            while e < chars.len() && chars[e].is_whitespace() { e += 1; }
        }
        let start = self.buffer.pos_to_char(self.cursor_line, s);
        let end   = self.buffer.pos_to_char(self.cursor_line, e);
        (start, end)
    }

    /// Execute operator `op` on a text object (`inner` = true for `i`, false for `a`).
    pub(crate) fn execute_text_obj(&mut self, op: char, inner: bool, delim: char, count: usize) {
        let _ = count;
        let (start, end) = match delim {
            '"' | '\'' | '`' => self.text_obj_quotes(delim, inner),
            '(' | ')' | 'b' => self.text_obj_pair('(', ')', inner),
            '{' | '}' | 'B' => self.text_obj_pair('{', '}', inner),
            '[' | ']'       => self.text_obj_pair('[', ']', inner),
            '<' | '>'       => self.text_obj_pair('<', '>', inner),
            'w'             => self.text_obj_word(inner),
            'W'             => self.text_obj_word_big(inner),
            'p'             => self.text_obj_paragraph(),
            's'             => self.text_obj_sentence(inner),
            't'             => self.text_obj_tag(inner),
            _               => return,
        };
        if start < end {
            self.apply_operator(op, start, end, false);
        }
    }

    /// text object for quote pairs: `"…"`, `'…'`, `` `…` ``
    fn text_obj_quotes(&self, quote: char, inner: bool) -> (usize, usize) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let col = self.cursor_col.min(chars.len().saturating_sub(1));
        // find opening quote to the left (or at col)
        let mut open = None;
        let mut close = None;
        // scan left for opening
        let mut i = col as isize;
        while i >= 0 {
            if chars[i as usize] == quote {
                open = Some(i as usize);
                break;
            }
            i -= 1;
        }
        // scan right for closing
        if let Some(o) = open {
            let mut j = o + 1;
            while j < chars.len() {
                if chars[j] == quote {
                    close = Some(j);
                    break;
                }
                j += 1;
            }
        }
        if let (Some(o), Some(c)) = (open, close) {
            let line_start = self.buffer.line_to_char(self.cursor_line);
            if inner {
                (line_start + o + 1, line_start + c)
            } else {
                (line_start + o, line_start + c + 1)
            }
        } else {
            let idx = self.buffer.pos_to_char(self.cursor_line, col);
            (idx, idx)
        }
    }

    /// text object for bracket pairs: `(…)`, `{…}`, `[…]`, `<…>`
    fn text_obj_pair(&self, open_ch: char, close_ch: char, inner: bool) -> (usize, usize) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let col = self.cursor_col.min(chars.len().saturating_sub(1));
        let mut open = None;
        let mut close = None;
        let mut depth = 0i32;
        // scan left for opener
        let mut i = col as isize;
        while i >= 0 {
            let c = chars[i as usize];
            if c == close_ch { depth += 1; }
            else if c == open_ch {
                if depth == 0 { open = Some(i as usize); break; }
                else { depth -= 1; }
            }
            i -= 1;
        }
        // scan right for closer
        if let Some(o) = open {
            depth = 0;
            let mut j = o;
            while j < chars.len() {
                if chars[j] == open_ch { depth += 1; }
                else if chars[j] == close_ch {
                    depth -= 1;
                    if depth == 0 { close = Some(j); break; }
                }
                j += 1;
            }
        }
        if let (Some(o), Some(c)) = (open, close) {
            let ls = self.buffer.line_to_char(self.cursor_line);
            if inner { (ls + o + 1, ls + c) } else { (ls + o, ls + c + 1) }
        } else {
            let idx = self.buffer.pos_to_char(self.cursor_line, col);
            (idx, idx)
        }
    }

    /// text object for sentence (simple: up to `.`/`?`/`!` + whitespace)
    fn text_obj_sentence(&self, inner: bool) -> (usize, usize) {
        // Simple: sentence within current line only
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let col = self.cursor_col.min(chars.len().saturating_sub(1));
        let is_end = |c: char| matches!(c, '.' | '?' | '!');
        // find sentence start (after previous sentence end + spaces)
        let mut s = col;
        while s > 0 && !is_end(chars[s-1]) { s -= 1; }
        while s < chars.len() && chars[s] == ' ' { s += 1; }
        // find sentence end
        let mut e = col;
        while e < chars.len() && !is_end(chars[e]) { e += 1; }
        if e < chars.len() { e += 1; } // include the punctuation
        let ls = self.buffer.line_to_char(self.cursor_line);
        if inner {
            // trim trailing spaces
            let mut ee = e;
            while ee > s && chars[ee-1] == ' ' { ee -= 1; }
            (ls + s, ls + ee)
        } else {
            // include trailing spaces
            while e < chars.len() && chars[e] == ' ' { e += 1; }
            (ls + s, ls + e)
        }
    }

    fn text_obj_paragraph(&self) -> (usize, usize) {
        let lc = self.buffer.line_count();
        let mut top = self.cursor_line;
        let mut bot = self.cursor_line;
        while top > 0 && !self.buffer.line_str(top-1).trim().is_empty() { top -= 1; }
        while bot + 1 < lc && !self.buffer.line_str(bot+1).trim().is_empty() { bot += 1; }
        let start = self.buffer.line_to_char(top);
        let end   = if bot + 1 < lc { self.buffer.line_to_char(bot + 1) } else { self.buffer.len_chars() };
        (start, end)
    }

    /// text object for XML/HTML tags: `it` (inner tag) / `at` (a tag, includes the tags).
    ///
    /// Strategy: search backwards from cursor for `<tag …>`, then forward for `</tag>`.
    /// Works on multi-line content by scanning the full buffer as a flat char sequence.
    fn text_obj_tag(&self, inner: bool) -> (usize, usize) {
        let total = self.buffer.len_chars();
        let cursor_char = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);

        // Helper: collect all chars as a Vec for easy indexing
        let text: String = self.buffer.rope.slice(0..total).to_string();
        let chars: Vec<char> = text.chars().collect();
        let n = chars.len();

        // Find the opening tag that contains the cursor.
        // Scan backwards for '<' that starts an opening tag (not '</' or '<!').
        let mut open_start: Option<usize> = None;
        let mut open_end: Option<usize> = None;   // position after '>'
        let mut tag_name = String::new();

        let mut i = cursor_char.min(n.saturating_sub(1)) as isize;
        while i >= 0 {
            let idx = i as usize;
            if chars[idx] == '<' {
                // Check it's not a closing tag or comment
                if idx + 1 < n && chars[idx + 1] != '/' && chars[idx + 1] != '!' {
                    // Extract tag name
                    let mut j = idx + 1;
                    while j < n && (chars[j].is_alphanumeric() || chars[j] == '-' || chars[j] == '_' || chars[j] == ':') {
                        j += 1;
                    }
                    if j > idx + 1 {
                        let name: String = chars[idx+1..j].iter().collect();
                        // Find the closing '>' of this opening tag
                        let mut k = j;
                        while k < n && chars[k] != '>' { k += 1; }
                        if k < n {
                            // Self-closing tags (<br/>) — skip
                            if chars[k-1] != '/' {
                                open_start = Some(idx);
                                open_end   = Some(k + 1);
                                tag_name   = name;
                                break;
                            }
                        }
                    }
                }
            }
            i -= 1;
        }

        let (open_start, open_end) = match (open_start, open_end) {
            (Some(s), Some(e)) => (s, e),
            _ => return (cursor_char, cursor_char),
        };

        // Find matching closing tag </tag_name>
        let close_tag: String = format!("</{}>", tag_name);
        let close_chars: Vec<char> = close_tag.chars().collect();
        let mut depth = 1usize;
        let mut pos = open_end;
        let mut close_start: Option<usize> = None;
        let mut close_end:   Option<usize> = None;

        while pos < n {
            if chars[pos] == '<' {
                // Check for opening tag (same name) — increase depth
                if pos + 1 < n && chars[pos + 1] != '/' && chars[pos + 1] != '!' {
                    let mut j = pos + 1;
                    while j < n && (chars[j].is_alphanumeric() || chars[j] == '-' || chars[j] == '_' || chars[j] == ':') {
                        j += 1;
                    }
                    let name: String = chars[pos+1..j].iter().collect();
                    if name == tag_name {
                        // Find its '>'
                        let mut k = j;
                        while k < n && chars[k] != '>' { k += 1; }
                        if k < n && chars[k-1] != '/' {
                            depth += 1;
                            pos = k + 1;
                            continue;
                        }
                    }
                }
                // Check for closing tag
                if pos + close_chars.len() <= n {
                    let slice: Vec<char> = chars[pos..pos+close_chars.len()].to_vec();
                    if slice == close_chars {
                        depth -= 1;
                        if depth == 0 {
                            close_start = Some(pos);
                            close_end   = Some(pos + close_chars.len());
                            break;
                        }
                        pos += close_chars.len();
                        continue;
                    }
                }
            }
            pos += 1;
        }

        match (close_start, close_end) {
            (Some(cs), Some(ce)) => {
                if inner {
                    (open_end, cs)
                } else {
                    (open_start, ce)
                }
            }
            _ => (cursor_char, cursor_char),
        }
    }

    fn apply_operator(&mut self, op: char, start: usize, end: usize, linewise: bool) {
        use crate::editor::RegisterEntry;
        let text: String = self.buffer.rope.slice(start..end).to_string();
        // Store in named register if active, otherwise default register
        let store = |editor: &mut Editor, t: String, lw: bool| {
            if let Some(reg) = editor.active_register.take() {
                if reg == '+' || reg == '*' {
                    // system clipboard: fall back to default register (clipboard requires OS integration)
                    editor.buffer.register = t;
                    editor.buffer.register_linewise = lw;
                } else {
                    editor.named_registers.insert(reg, RegisterEntry { text: t.clone(), linewise: lw });
                    // Also keep default register in sync
                    editor.buffer.register = t;
                    editor.buffer.register_linewise = lw;
                }
            } else {
                editor.buffer.register = t;
                editor.buffer.register_linewise = lw;
            }
        };
        match op {
            'd' => {
                store(self, text, linewise);
                self.buffer.delete_range(start, end);
                self.set_cursor_from_char_idx(start);
            }
            'y' => {
                store(self, text, linewise);
            }
            'c' => {
                store(self, text, linewise);
                self.buffer.delete_range(start, end);
                self.set_cursor_from_char_idx(start);
            }
            _ => {}
        }
    }

    // ── Big-word variants (WORD = whitespace-delimited) ────

    fn move_word_forward_big(&mut self) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let mut col = self.cursor_col;
        while col < chars.len() && !chars[col].is_whitespace() { col += 1; }
        while col < chars.len() && chars[col].is_whitespace() { col += 1; }
        if col >= chars.len() && self.cursor_line + 1 < self.buffer.line_count() {
            self.cursor_line += 1;
            self.cursor_col = 0;
            self.move_line_start_nonblank();
        } else {
            self.cursor_col = col;
        }
        self.scroll_to_cursor();
    }

    fn move_word_back_big(&mut self) {
        if self.cursor_col == 0 {
            if self.cursor_line > 0 {
                self.cursor_line -= 1;
                self.cursor_col = self.buffer.line_len(self.cursor_line).saturating_sub(1);
            }
            return;
        }
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let mut col = self.cursor_col.saturating_sub(1);
        while col > 0 && chars[col].is_whitespace() { col -= 1; }
        while col > 0 && !chars[col-1].is_whitespace() { col -= 1; }
        self.cursor_col = col;
        self.scroll_to_cursor();
    }

    fn move_word_end_big(&mut self) {
        let line = self.buffer.line_str(self.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let mut col = self.cursor_col + 1;
        if col >= chars.len() { return; }
        while col < chars.len() && chars[col].is_whitespace() { col += 1; }
        while col + 1 < chars.len() && !chars[col+1].is_whitespace() { col += 1; }
        self.cursor_col = col;
        self.scroll_to_cursor();
    }

    // ── Case operators (gU / gu) ───────────────────────────────────────

    /// Apply upper/lower case to a char range [start, end).
    pub(crate) fn apply_case_range(&mut self, start: usize, end: usize, upper: bool) {
        if start >= end { return; }
        let text: String = self.buffer.rope.slice(start..end).to_string();
        let converted: String = if upper {
            text.chars().map(|c| c.to_uppercase().next().unwrap_or(c)).collect()
        } else {
            text.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect()
        };
        if text != converted {
            self.buffer.delete_range(start, end);
            self.buffer.insert_str(start, &converted);
        }
        self.set_cursor_from_char_idx(start);
    }

    /// gUU / guu — apply case to entire current line (excluding newline).
    pub(crate) fn apply_case_line(&mut self, upper: bool) {
        let line = self.buffer.line_str(self.cursor_line);
        let len = line.chars().count();
        let start = self.buffer.line_to_char(self.cursor_line);
        let end   = start + len;
        self.apply_case_range(start, end, upper);
    }

    /// gU{motion} / gu{motion} — apply case based on motion key.
    pub(crate) fn apply_case_operator_key(&mut self, upper: bool, key: KeyEvent, count: usize) {
        match key.code {
            KeyCode::Char('$') => {
                let line = self.buffer.line_str(self.cursor_line);
                let line_chars = line.chars().count();
                let start = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                let end   = self.buffer.pos_to_char(self.cursor_line, line_chars);
                self.apply_case_range(start, end, upper);
            }
            KeyCode::Char('0') => {
                let end   = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                let start = self.buffer.line_to_char(self.cursor_line);
                self.apply_case_range(start, end, upper);
            }
            KeyCode::Char('w') => {
                let saved_line = self.cursor_line;
                let saved_col  = self.cursor_col;
                self.move_word_forward(count);
                let end = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                let start = self.buffer.pos_to_char(saved_line, saved_col);
                self.cursor_line = saved_line;
                self.cursor_col  = saved_col;
                self.apply_case_range(start, end, upper);
            }
            KeyCode::Char('e') => {
                let start = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                let saved_line = self.cursor_line;
                let saved_col  = self.cursor_col;
                self.move_word_end(count);
                let end = self.buffer.pos_to_char(self.cursor_line, self.cursor_col) + 1;
                self.cursor_line = saved_line;
                self.cursor_col  = saved_col;
                let end = end.min(self.buffer.len_chars());
                self.apply_case_range(start, end, upper);
            }
            KeyCode::Char('b') => {
                let end   = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                let saved_line = self.cursor_line;
                let saved_col  = self.cursor_col;
                self.move_word_back(count);
                let start = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                self.cursor_line = saved_line;
                self.cursor_col  = saved_col;
                self.apply_case_range(start, end, upper);
            }
            KeyCode::Char('j') => {
                let from  = self.cursor_line;
                let to    = (from + count).min(self.buffer.line_count().saturating_sub(1));
                let start = self.buffer.line_to_char(from);
                let end   = if to + 1 < self.buffer.line_count() {
                    self.buffer.line_to_char(to + 1)
                } else {
                    self.buffer.len_chars()
                };
                self.apply_case_range(start, end, upper);
            }
            KeyCode::Char('k') => {
                let to    = self.cursor_line;
                let from  = to.saturating_sub(count);
                let start = self.buffer.line_to_char(from);
                let end   = if to + 1 < self.buffer.line_count() {
                    self.buffer.line_to_char(to + 1)
                } else {
                    self.buffer.len_chars()
                };
                self.apply_case_range(start, end, upper);
            }
            _ => {}
        }
    }
}
