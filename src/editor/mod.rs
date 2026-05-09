pub mod motion;

use std::collections::HashMap;
use crate::buffer::Buffer;
use crate::mode::Mode;
use crate::config::Config;

/// The complete editor state: buffers, cursor, viewport, mode, search state.
pub struct Editor {
    pub buffer: Buffer,
    pub mode: Mode,
    pub config: Config,

    // Cursor position (0-based line and column)
    pub cursor_line: usize,
    pub cursor_col: usize,

    // Scroll offset (top-left of the visible area)
    pub scroll_line: usize,

    // Terminal size
    pub term_width: u16,
    pub term_height: u16,

    // Search state
    pub search_pattern: String,
    pub search_matches: Vec<(usize, usize)>, // (line, col) of each match start
    pub search_match_idx: usize,
    pub search_highlight: bool,

    // Pending normal-mode key accumulation (for gg, dd, yy, etc.)
    pub pending_key: Option<char>,
    pub pending_count: String,  // digit prefix accumulator
    // For three-key text-object sequences: operator (d/y/c) waiting for i/a + delimiter
    pub pending_operator: Option<char>,

    // Last f/F/t/T char for ; and ,
    pub last_find: Option<FindState>,

    // Jump list
    pub jump_list: Vec<(usize, usize)>,
    pub jump_idx: usize,

    // Status bar message (temporary, clears after one render)
    pub status_msg: Option<String>,

    // File tree visibility
    pub filetree_visible: bool,
    pub filetree_focus: bool,

    // Command history for up/down in command mode
    pub cmd_history: Vec<String>,
    pub cmd_history_idx: Option<usize>,

    // ── Dot-repeat ────────────────────────────────────────
    /// The last repeatable action (set after each insert/delete/change)
    pub last_action: Option<RepeatAction>,
    /// Temporary insert-session tracking (populated by begin_insert_session)
    pub _insert_text: String,
    pub _insert_col_offset: i32,
    pub _insert_newline_above: bool,
    pub _insert_newline_below: bool,

    // ── Named registers ───────────────────────────────────
    /// Named registers "a–"z plus special keys like '+'
    pub named_registers: HashMap<char, RegisterEntry>,
    /// The register to use for the next yank/delete/put (set by "{char})
    pub active_register: Option<char>,

    // ── Macro recording/playback ──────────────────────────
    pub macro_recording: Option<char>,          // Some(reg) while recording
    pub macros: HashMap<char, Vec<MacroKey>>,   // stored macros

    // ── Marks ─────────────────────────────────────────────
    /// Named marks 'a–'z  →  (line, col)
    pub marks: HashMap<char, (usize, usize)>,
    /// Last position before a jump (for '' and ``)
    pub mark_prev: Option<(usize, usize)>,

    // ── Shell command output (:!cmd) ──────────────────────
    /// Full output of the last :!{cmd} (None if single-line or no command run)
    pub shell_output: Option<String>,
}

#[derive(Clone)]
pub struct FindState {
    pub ch: char,
    pub forward: bool,
    pub till: bool, // t/T vs f/F
}

/// A single repeatable edit (for the `.` command).
#[derive(Clone, Debug)]
pub enum RepeatAction {
    /// Sequence of chars typed in Insert mode (i/a/o/O/s/S/C/cc)
    Insert {
        enter_col_offset: i32,    // col offset used to enter insert
        newline_above: bool,      // was it `O`?
        newline_below: bool,      // was it `o`?
        text: String,             // chars typed before Esc
    },
    /// Delete operator + range saved as char indices
    DeleteRange { start: usize, end: usize, linewise: bool },
    /// Replace single char (r)
    ReplaceChar(char),
    /// Indent / dedent current line
    Indent(bool),
}

/// Register entry (text + linewise flag)
#[derive(Clone, Debug, Default)]
pub struct RegisterEntry {
    pub text: String,
    pub linewise: bool,
}

/// A single key captured during macro recording.
#[derive(Clone, Debug)]
pub struct MacroKey {
    pub code: crossterm::event::KeyCode,
    pub modifiers: crossterm::event::KeyModifiers,
}

impl Editor {
    pub fn new(config: Config, width: u16, height: u16) -> Self {
        Self {
            buffer: Buffer::new(),
            mode: Mode::Normal,
            config,
            cursor_line: 0,
            cursor_col: 0,
            scroll_line: 0,
            term_width: width,
            term_height: height,
            search_pattern: String::new(),
            search_matches: Vec::new(),
            search_match_idx: 0,
            search_highlight: false,
            pending_key: None,
            pending_count: String::new(),
            pending_operator: None,
            last_find: None,
            jump_list: Vec::new(),
            jump_idx: 0,
            status_msg: None,
            filetree_visible: false,
            filetree_focus: false,
            cmd_history: Vec::new(),
            cmd_history_idx: None,
            last_action: None,
            _insert_text: String::new(),
            _insert_col_offset: 0,
            _insert_newline_above: false,
            _insert_newline_below: false,
            named_registers: HashMap::new(),
            active_register: None,
            macro_recording: None,
            macros: HashMap::new(),
            marks: HashMap::new(),
            mark_prev: None,
            shell_output: None,
        }
    }

    pub fn with_buffer(mut self, buf: Buffer) -> Self {
        self.buffer = buf;
        self
    }

    pub fn with_filetree(mut self) -> Self {
        self.filetree_visible = true;
        self
    }

    // ── Viewport helpers ─────────────────────────────────

    /// Height of the editing area (total - 2 status rows).
    pub fn edit_height(&self) -> usize {
        (self.term_height.saturating_sub(2)) as usize
    }

    /// Width of the editing area (minus line-number gutter and file tree).
    pub fn edit_width(&self) -> usize {
        let mut w = self.term_width as usize;
        if self.config.general.line_numbers {
            w = w.saturating_sub(self.gutter_width());
        }
        if self.filetree_visible {
            w = w.saturating_sub(self.config.filetree.width as usize + 1);
        }
        w
    }

    pub fn gutter_width(&self) -> usize {
        let lines = self.buffer.line_count().max(1);
        let digits = format!("{}", lines).len();
        digits + 1 // space after number
    }

    /// Ensure cursor is within valid bounds.
    pub fn clamp_cursor(&mut self) {
        let line_count = self.buffer.line_count().max(1);
        self.cursor_line = self.cursor_line.min(line_count - 1);
        let line_len = self.buffer.line_len(self.cursor_line);
        if line_len == 0 {
            self.cursor_col = 0;
        } else if self.mode.is_insert() {
            self.cursor_col = self.cursor_col.min(line_len);
        } else {
            self.cursor_col = self.cursor_col.min(line_len.saturating_sub(1));
        }
    }

    /// Scroll to keep cursor visible, respecting scroll_off.
    pub fn scroll_to_cursor(&mut self) {
        let off = self.config.general.scroll_off;
        let height = self.edit_height();
        if height == 0 { return; }

        // Scroll down
        if self.cursor_line + off + 1 > self.scroll_line + height {
            self.scroll_line = (self.cursor_line + off + 1).saturating_sub(height);
        }
        // Scroll up
        if self.cursor_line < self.scroll_line + off {
            self.scroll_line = self.cursor_line.saturating_sub(off);
        }
    }

    // ── Cursor char index ─────────────────────────────────

    pub fn cursor_char_idx(&self) -> usize {
        self.buffer.pos_to_char(self.cursor_line, self.cursor_col)
    }

    // ── Jump list ─────────────────────────────────────────

    pub fn push_jump(&mut self) {
        let pos = (self.cursor_line, self.cursor_col);
        if self.jump_list.last() != Some(&pos) {
            self.jump_list.truncate(self.jump_idx);
            self.jump_list.push(pos);
            self.jump_idx = self.jump_list.len();
        }
    }

    pub fn jump_back(&mut self) -> bool {
        if self.jump_idx == 0 { return false; }
        // Save current position first
        let pos = (self.cursor_line, self.cursor_col);
        if self.jump_idx == self.jump_list.len() {
            self.jump_list.push(pos);
        }
        self.jump_idx = self.jump_idx.saturating_sub(1);
        let (l, c) = self.jump_list[self.jump_idx];
        self.cursor_line = l;
        self.cursor_col = c;
        true
    }

    pub fn jump_forward(&mut self) -> bool {
        if self.jump_idx + 1 >= self.jump_list.len() { return false; }
        self.jump_idx += 1;
        let (l, c) = self.jump_list[self.jump_idx];
        self.cursor_line = l;
        self.cursor_col = c;
        true
    }

    // ── Search ─────────────────────────────────────────────

    pub fn run_search(&mut self, pattern: &str, ignore_case: bool) {
        self.search_pattern = pattern.to_string();
        self.search_matches.clear();
        self.search_highlight = true;
        if pattern.is_empty() { return; }

        let flags = if ignore_case { "(?i)" } else { "" };
        let pat = format!("{}{}", flags, regex::escape(pattern));
        if let Ok(re) = regex::Regex::new(&pat) {
            for line_idx in 0..self.buffer.line_count() {
                let line = self.buffer.line_str(line_idx);
                for m in re.find_iter(&line) {
                    let col = line[..m.start()].chars().count();
                    self.search_matches.push((line_idx, col));
                }
            }
        }

        // Find first match at or after cursor
        self.search_match_idx = self.search_matches.iter().position(|&(l, c)| {
            l > self.cursor_line || (l == self.cursor_line && c >= self.cursor_col)
        }).unwrap_or(0);

        if let Some(&(l, c)) = self.search_matches.get(self.search_match_idx) {
            self.push_jump();
            self.cursor_line = l;
            self.cursor_col = c;
            self.scroll_to_cursor();
        }
    }

    pub fn search_next(&mut self) {
        if self.search_matches.is_empty() { return; }
        self.search_match_idx = (self.search_match_idx + 1) % self.search_matches.len();
        let (l, c) = self.search_matches[self.search_match_idx];
        self.push_jump();
        self.cursor_line = l;
        self.cursor_col = c;
        self.scroll_to_cursor();
    }

    pub fn search_prev(&mut self) {
        if self.search_matches.is_empty() { return; }
        let len = self.search_matches.len();
        self.search_match_idx = (self.search_match_idx + len - 1) % len;
        let (l, c) = self.search_matches[self.search_match_idx];
        self.push_jump();
        self.cursor_line = l;
        self.cursor_col = c;
        self.scroll_to_cursor();
    }

    /// Re-run the last search pattern (e.g. after `*`/`#`).
    pub fn rerun_search(&mut self) {
        let pat = self.search_pattern.clone();
        let ic = self.config.general.ignore_case;
        self.run_search(&pat, ic);
    }

    // ── Status message ────────────────────────────────────

    pub fn set_msg(&mut self, msg: impl Into<String>) {
        self.status_msg = Some(msg.into());
    }

    // ── Macro helpers ─────────────────────────────────────

    /// Append a key event to the currently-recording macro.
    pub fn macro_append_key(&mut self, key: crossterm::event::KeyEvent) {
        if let Some(reg) = self.macro_recording {
            let entry = self.macros.entry(reg).or_default();
            entry.push(MacroKey { code: key.code, modifiers: key.modifiers });
        }
    }

    pub fn editor_msg_stop_macro(&mut self) {
        self.set_msg("宏录制结束".to_string());
    }

    // ── Dot-repeat executor ───────────────────────────────

    /// Replay the last repeatable action at the current cursor position.
    /// Returns `true` if an action was executed.
    pub fn dot_repeat(&mut self) -> bool {
        let action = match self.last_action.clone() {
            Some(a) => a,
            None => return false,
        };
        match action {
            RepeatAction::ReplaceChar(c) => {
                self.replace_char_at_cursor(c);
            }
            RepeatAction::Indent(right) => {
                self.indent_line(self.cursor_line, right);
            }
            RepeatAction::Insert { newline_above, newline_below, text, .. } => {
                let indent = if self.config.general.auto_indent {
                    self.buffer.indent_of_line(self.cursor_line)
                } else {
                    String::new()
                };
                if newline_below {
                    let next = self.buffer.line_to_char(self.cursor_line)
                        + self.buffer.line_len(self.cursor_line);
                    self.buffer.insert_str(next, &format!("\n{}", indent));
                    self.cursor_line += 1;
                    self.cursor_col = indent.len();
                } else if newline_above {
                    let start = self.buffer.line_to_char(self.cursor_line);
                    self.buffer.insert_str(start, &format!("{}\n", indent));
                    self.cursor_col = indent.len();
                }
                let char_idx = self.buffer.pos_to_char(self.cursor_line, self.cursor_col);
                self.buffer.insert_str(char_idx, &text);
                self.cursor_col += text.chars().count();
                self.clamp_cursor();
                self.scroll_to_cursor();
            }
            RepeatAction::DeleteRange { start, end, linewise } => {
                // Re-play a delete; make sure range is still valid
                let len = self.buffer.len_chars();
                if start < len && end <= len && start < end {
                    let text: String = self.buffer.rope.slice(start..end).to_string();
                    self.buffer.register = text;
                    self.buffer.register_linewise = linewise;
                    self.buffer.delete_range(start, end);
                    self.set_cursor_from_char_idx(start);
                }
            }
        }
        true
    }
}
