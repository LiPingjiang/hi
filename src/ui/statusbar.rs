//! Status bar rendering: info line + context-aware hint line.
use crate::editor::Editor;
use crate::locale::Locale;
use crate::mode::Mode;
use crate::syntax::highlight::FileType;

impl Editor {
    pub fn hint_line(&self, locale: &Locale) -> String {
        let ui = &locale.ui;

        // File tree has focus — override all other hints
        if self.filetree_visible && self.filetree_focus {
            return ui.hint_filetree.clone();
        }
        match &self.mode {
            Mode::Visual { .. } => ui.hint_visual.clone(),
            Mode::Insert       => ui.hint_insert.clone(),
            Mode::Command(_)   => ui.hint_command.clone(),
            Mode::Search(_)    => ui.hint_search.clone(),
            Mode::Ai(_)        => ui.hint_ai.clone(),
            Mode::Normal => {
                // Macro recording takes highest priority
                if let Some(reg) = self.macro_recording {
                    return ui.hint_normal_macro.replace("{reg}", &reg.to_string());
                }
                // Register prefix waiting
                if self.active_register.is_some() || self.pending_key == Some('"') {
                    return ui.hint_normal_register.clone();
                }
                // Search highlight active
                if self.search_highlight {
                    return ui.hint_normal_search.clone();
                }

                let line = self.buffer.line_str(self.cursor_line);
                let col = self.cursor_col;
                let chars: Vec<char> = line.chars().collect();
                let trimmed = line.trim();

                // Empty line
                if trimmed.is_empty() {
                    return ui.hint_normal_empty.clone();
                }

                // Comment line (// # -- /* )
                if trimmed.starts_with("//") || trimmed.starts_with('#')
                    || trimmed.starts_with("--") || trimmed.starts_with("/*")
                {
                    return ui.hint_normal_comment.clone();
                }

                // Cursor on '<' (XML/HTML tag)
                if col < chars.len() && chars[col] == '<' {
                    return ui.hint_normal_tag.clone();
                }

                let word = word_at(&chars, col);

                // Cursor on URL or file path
                if word.starts_with("http://") || word.starts_with("https://")
                    || word.starts_with('/') || (word.contains('/') && !word.is_empty())
                {
                    return ui.hint_normal_url.clone();
                }

                // Cursor on a number
                if !word.is_empty() && word.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') {
                    return ui.hint_normal_number.clone();
                }

                // Cursor inside a string (on or after a quote char)
                let on_quote = col < chars.len() && (chars[col] == '"' || chars[col] == '\'' || chars[col] == '`');
                let in_string = !on_quote && col > 0 && is_inside_quotes(&chars, col);
                if on_quote || in_string {
                    return ui.hint_normal_string.clone();
                }

                // Cursor on a word (identifier / keyword)
                if !word.is_empty() && word.chars().next().map(|c| c.is_alphabetic() || c == '_').unwrap_or(false) {
                    return ui.hint_normal_word.clone();
                }

                // Default Normal
                ui.hint_normal.clone()
            }
        }
    }

    pub fn info_line(&self, filetype: FileType) -> String {
        let mode = self.mode.name();
        let filename = self.buffer.display_name();
        let modified = if self.buffer.modified { " [+]" } else { "" };
        let line = self.cursor_line + 1;
        let col  = self.cursor_col + 1;
        let ft   = filetype.name();
        format!("{:<8} {}{:<30}  {:>5}:{:<5} {}", mode, filename, modified, line, col, ft)
    }
}

fn word_at(chars: &[char], col: usize) -> String {
    if col >= chars.len() { return String::new(); }
    let is_word = |c: char| c.is_alphanumeric() || "/_.-:".contains(c);
    let mut s = col;
    let mut e = col;
    while s > 0 && is_word(chars[s-1]) { s -= 1; }
    while e < chars.len() && is_word(chars[e]) { e += 1; }
    chars[s..e].iter().collect()
}

/// Returns true if `col` is inside a matched pair of " ' or ` on the same line.
fn is_inside_quotes(chars: &[char], col: usize) -> bool {
    for &q in &['"', '\'', '`'] {
        let mut inside = false;
        for (i, &c) in chars.iter().enumerate() {
            if c == q {
                inside = !inside;
            }
            if i == col {
                return inside;
            }
        }
    }
    false
}
