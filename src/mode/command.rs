//! Command mode (:) — parsing and execution of Ex commands.
use std::path::PathBuf;
use crossterm::event::{KeyCode, KeyEvent};
use crate::editor::Editor;

pub enum CommandAction {
    None,
    ExitToNormal,
    Quit { force: bool },
    SaveAndQuit,
    SetMsg(String),
    OpenFile(PathBuf),
    RunSubstitution { range: SubRange, pattern: String, replacement: String, flags: String },
    GoToLine(usize),
    ToggleLineNumbers(bool),
    SetTabWidth(usize),
    ClearSearch,
    /// :!{cmd} — run shell command and show output
    ShellCommand(String),
    /// :d / :{n}d / :{n,m}d — delete lines
    DeleteLines { start: usize, end: usize },
    /// :u — undo (same as `u` in normal mode)
    Undo,
    /// :theme {name} — switch colour theme at runtime
    SetTheme(String),
    /// :theme (no arg) — open interactive theme picker
    OpenThemePicker,
    /// :preview — render current Markdown file as HTML and open in browser
    Preview,
    /// :grep {pattern} — open global grep panel
    Grep { pattern: String, is_regex: bool },
    /// :filetree / :FT — toggle file tree sidebar
    ToggleFileTree,
    /// :tutorial — toggle tutorial board
    ToggleTutorial,
    /// :mouse — toggle mouse mode
    ToggleMouse,
}

#[derive(Debug, Clone)]
pub enum SubRange {
    CurrentLine,
    WholeFile,
    Lines(usize, usize),
}

impl Editor {
    /// Handle a key event while in Command mode.
    /// `input` is the mutable accumulator for the command string.
    pub fn handle_command_key(&mut self, key: KeyEvent, input: &mut String) -> CommandAction {
        match key.code {
            KeyCode::Esc => {
                input.clear();
                CommandAction::ExitToNormal
            }
            KeyCode::Enter => {
                let cmd = input.trim().to_string();
                input.clear();
                self.parse_and_execute_command(&cmd)
            }
            KeyCode::Backspace => {
                if input.is_empty() {
                    CommandAction::ExitToNormal
                } else {
                    input.pop();
                    CommandAction::None
                }
            }
            KeyCode::Up => {
                self.cmd_history_idx = Some(
                    self.cmd_history_idx
                        .map(|i| i.saturating_sub(1))
                        .unwrap_or(self.cmd_history.len().saturating_sub(1))
                );
                if let Some(i) = self.cmd_history_idx {
                    if let Some(h) = self.cmd_history.get(i) {
                        *input = h.clone();
                    }
                }
                CommandAction::None
            }
            KeyCode::Down => {
                if let Some(i) = self.cmd_history_idx {
                    let next = i + 1;
                    if next >= self.cmd_history.len() {
                        self.cmd_history_idx = None;
                        input.clear();
                    } else {
                        self.cmd_history_idx = Some(next);
                        *input = self.cmd_history[next].clone();
                    }
                }
                CommandAction::None
            }
            KeyCode::Char(c) => {
                input.push(c);
                CommandAction::None
            }
            _ => CommandAction::None,
        }
    }

    pub(crate) fn parse_and_execute_command(&mut self, cmd: &str) -> CommandAction {
        // Save to history
        if !cmd.is_empty() {
            self.cmd_history.push(cmd.to_string());
            self.cmd_history_idx = None;
        }

        // :q! :wq :x :w :q :e :set :noh :{n} :%s :range s
        match cmd {
            "q"  => return CommandAction::Quit { force: false },
            "q!" => return CommandAction::Quit { force: true },
            "wq" | "wq!" | "x" | "x!" => return CommandAction::SaveAndQuit,
            "w"  => {
                match self.buffer.save() {
                    Ok(_) => return CommandAction::SetMsg(format!("\"{}\" written", self.buffer.display_name())),
                    Err(e) if e.to_string().contains("No file path") => {
                        return CommandAction::SetMsg("No file name — use :w <filename> to save".to_string());
                    }
                    Err(e) => return CommandAction::SetMsg(format!("Save failed: {}", e)),
                }
            }
            "e!" => {
                match self.buffer.reload() {
                    Ok(_) => return CommandAction::SetMsg("File reloaded".to_string()),
                    Err(e) => return CommandAction::SetMsg(format!("Error: {}", e)),
                }
            }
            "noh" | "nohl" | "nohlsearch" => {
                self.search_highlight = false;
                return CommandAction::ClearSearch;
            }
            _ => {}
        }

        // :w {file}
        if let Some(path) = cmd.strip_prefix("w ") {
            let p = PathBuf::from(path.trim());
            match self.buffer.save_as(p) {
                Ok(_) => return CommandAction::SetMsg(format!("\"{}\" written", self.buffer.display_name())),
                Err(e) => return CommandAction::SetMsg(format!("Error: {}", e)),
            }
        }

        // :e {file}
        if let Some(path) = cmd.strip_prefix("e ") {
            return CommandAction::OpenFile(PathBuf::from(path.trim()));
        }

        // :set ...
        if let Some(rest) = cmd.strip_prefix("set ").or_else(|| cmd.strip_prefix("set\t")) {
            return self.handle_set_command(rest.trim());
        }

        // :{n} — go to line
        if let Ok(n) = cmd.parse::<usize>() {
            return CommandAction::GoToLine(n);
        }

        // Substitution: [range]s/pat/rep/flags
        if let Some(action) = self.parse_substitution(cmd) {
            return action;
        }

        // :!{cmd} — shell command
        if let Some(shell_cmd) = cmd.strip_prefix('!') {
            return CommandAction::ShellCommand(shell_cmd.trim().to_string());
        }

        // :u — undo
        if cmd == "u" {
            return CommandAction::Undo;
        }

        // :theme {name} — switch colour theme
        if let Some(name) = cmd.strip_prefix("theme ").or_else(|| cmd.strip_prefix("theme\t")) {
            let name = name.trim();
            if name.is_empty() {
                return CommandAction::SetMsg("Usage: :theme <name>  (available: neon-minimalist, glow-dark, monokai-pro, github-dark, one-dark-pro, dracula, electric-impressionism, synthwave)".to_string());
            }
            return CommandAction::SetTheme(name.to_string());
        }
        // bare :theme — open interactive theme picker
        if cmd == "theme" {
            return CommandAction::OpenThemePicker;
        }

        // :preview — open Markdown preview in browser
        if cmd == "preview" {
            return CommandAction::Preview;
        }

        // :filetree / :FT — toggle file tree sidebar
        if cmd == "filetree" || cmd == "FT" || cmd == "ft" || cmd == "FileTree" || cmd == "filetreeToggle" || cmd == "FileTreeToggle" {
            return CommandAction::ToggleFileTree;
        }

        // :tutorial — toggle tutorial board
        if cmd == "tutorial" || cmd == "tut" || cmd == "Tutorial" {
            return CommandAction::ToggleTutorial;
        }

        // :mouse — toggle mouse mode
        if cmd == "mouse" {
            return CommandAction::ToggleMouse;
        }

        // :grep {pattern}  or  :grep /{pattern}/  (regex)
        if let Some(rest) = cmd.strip_prefix("grep ").or_else(|| cmd.strip_prefix("grep\t")) {
            let rest = rest.trim();
            // Detect /pattern/ syntax → regex mode
            if rest.starts_with('/') && rest.ends_with('/') && rest.len() > 2 {
                let pat = &rest[1..rest.len()-1];
                return CommandAction::Grep { pattern: pat.to_string(), is_regex: true };
            }
            return CommandAction::Grep { pattern: rest.to_string(), is_regex: false };
        }
        // bare :grep — show usage
        if cmd == "grep" {
            return CommandAction::SetMsg(":grep <pattern>  or  :grep /<regex>/".to_string());
        }

        // :d / :{n}d / :{n,m}d — delete lines
        if let Some(action) = self.parse_delete_lines(cmd) {
            return action;
        }

        CommandAction::SetMsg(format!("Unknown command: {}", cmd))
    }

    fn handle_set_command(&mut self, rest: &str) -> CommandAction {
        match rest {
            "nu" | "number"    => return CommandAction::ToggleLineNumbers(true),
            "nonu" | "nonumber" => return CommandAction::ToggleLineNumbers(false),
            _ => {}
        }
        if let Some(n) = rest.strip_prefix("tabstop=").or_else(|| rest.strip_prefix("ts=")) {
            if let Ok(v) = n.parse::<usize>() {
                return CommandAction::SetTabWidth(v);
            }
        }
        CommandAction::SetMsg(format!("Unknown option: {}", rest))
    }

    fn parse_substitution(&self, cmd: &str) -> Option<CommandAction> {
        // Patterns: s/p/r/flags   %s/p/r/flags   n,ms/p/r/flags
        let (range, rest) = if let Some(r) = cmd.strip_prefix('%') {
            (SubRange::WholeFile, r)
        } else if let Some(idx) = cmd.find(',') {
            let left = &cmd[..idx];
            let right = &cmd[idx+1..];
            let n: usize = left.trim().parse().ok()?;
            let (m_str, tail) = right.split_once('s')?;
            let m: usize = m_str.trim().parse().ok()?;
            (SubRange::Lines(n.saturating_sub(1), m.saturating_sub(1)), tail)
        } else {
            (SubRange::CurrentLine, cmd)
        };

        let rest = rest.strip_prefix('s')?;
        let sep = rest.chars().next()?;
        let parts: Vec<&str> = rest[1..].splitn(3, sep).collect();
        if parts.len() < 2 { return None; }
        let pattern = parts[0].to_string();
        let replacement = parts[1].to_string();
        let flags = parts.get(2).unwrap_or(&"").to_string();

        Some(CommandAction::RunSubstitution { range, pattern, replacement, flags })
    }

    /// Parse :d / :{n}d / :{n,m}d
    fn parse_delete_lines(&self, cmd: &str) -> Option<CommandAction> {
        // bare :d — delete current line
        if cmd == "d" {
            let line = self.cursor_line;
            return Some(CommandAction::DeleteLines { start: line, end: line });
        }
        // {n}d — delete line n (1-based)
        if let Some(n_str) = cmd.strip_suffix('d') {
            let n_str = n_str.trim();
            if !n_str.is_empty() {
                if let Some(comma) = n_str.find(',') {
                    // {n,m}d
                    let a: usize = n_str[..comma].trim().parse().ok()?;
                    let b: usize = n_str[comma+1..].trim().parse().ok()?;
                    let start = a.saturating_sub(1);
                    let end   = b.saturating_sub(1);
                    return Some(CommandAction::DeleteLines { start, end });
                } else {
                    // {n}d
                    let n: usize = n_str.parse().ok()?;
                    let line = n.saturating_sub(1);
                    return Some(CommandAction::DeleteLines { start: line, end: line });
                }
            }
        }
        None
    }
}
