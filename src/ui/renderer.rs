//! Main TUI renderer using crossterm.
use crossterm::{
    cursor,
    event::{EnableMouseCapture, DisableMouseCapture},
    execute,
    style::{Attribute, Color, SetForegroundColor, SetBackgroundColor, ResetColor, SetAttribute},
    terminal,
};
use std::io::{self, Write, Stdout};

use crate::editor::Editor;
use crate::mode::{Mode, VisualKind};
use crate::syntax::highlight::{FileType, Highlighter, TokenKind};
use crate::ui::filetree::FileTree;
use crate::ui::ghost::GhostText;

pub struct Renderer {
    pub stdout: Stdout,
    pub highlighter: Highlighter,
}

impl Renderer {
    pub fn new(filetype: FileType) -> Self {
        Self {
            stdout: io::stdout(),
            highlighter: Highlighter::new(filetype),
        }
    }

    pub fn set_filetype(&mut self, ft: FileType) {
        self.highlighter = Highlighter::new(ft);
    }

    pub fn init(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        execute!(self.stdout,
            terminal::EnterAlternateScreen,
            EnableMouseCapture,
            cursor::Hide,
        )
    }

    pub fn cleanup(&mut self) -> io::Result<()> {
        terminal::disable_raw_mode()?;
        execute!(self.stdout,
            DisableMouseCapture,
            terminal::LeaveAlternateScreen,
            cursor::Show,
        )
    }

    pub fn render(
        &mut self,
        editor: &Editor,
        filetree: &Option<FileTree>,
        ghost: &GhostText,
        ai_query_msg: &Option<String>,
        plan_lines: &Option<Vec<String>>,
        filetree_prompt: &Option<crate::app::FileTreePrompt>,
    ) -> io::Result<()> {
        let w = editor.term_width as usize;
        let h = editor.term_height as usize;
        let ft_width = if editor.filetree_visible {
            editor.config.filetree.width as usize
        } else { 0 };

        execute!(self.stdout,
            cursor::Hide,
            cursor::MoveTo(0, 0),
        )?;

        // ── File tree panel ──────────────────────────────
        if editor.filetree_visible {
            if let Some(ft) = filetree {
                self.render_filetree(ft, ft_width, h.saturating_sub(2), editor.filetree_focus)?;
            } else {
                // filetree failed to load — clear the panel so no stale content shows
                for row in 0..h.saturating_sub(2) {
                    execute!(self.stdout, cursor::MoveTo(0, row as u16))?;
                    write!(self.stdout, "{:width$}", "", width = ft_width)?;
                }
            }
        }

        // ── Editing area ──────────────────────────────────
        let edit_x = ft_width + if ft_width > 0 { 1 } else { 0 };
        let edit_w = w.saturating_sub(edit_x);
        let edit_h = h.saturating_sub(2);
        let gutter = if editor.config.general.line_numbers { editor.gutter_width() } else { 0 };

        let text_w = edit_w.saturating_sub(gutter);

        // Draw search highlights as a sorted list of (line,col) pairs
        let search_set: std::collections::HashSet<(usize,usize)> = editor.search_matches.iter().cloned().collect();
        let current_match = editor.search_matches.get(editor.search_match_idx).cloned();

        for screen_row in 0..edit_h {
            let buf_line = editor.scroll_line + screen_row;
            execute!(self.stdout, cursor::MoveTo(edit_x as u16, screen_row as u16))?;

            // Draw gutter
            if editor.config.general.line_numbers {
                if buf_line < editor.buffer.line_count() {
                    let lnum = format!("{:>width$} ", buf_line + 1, width = gutter - 1);
                    execute!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
                    write!(self.stdout, "{}", lnum)?;
                    execute!(self.stdout, ResetColor)?;
                } else {
                    let blank = " ".repeat(gutter);
                    write!(self.stdout, "{}", blank)?;
                }
            }

            // Draw text
            if buf_line < editor.buffer.line_count() {
                let line = editor.buffer.line_str(buf_line);
                let mut spans = if editor.search_highlight && !editor.search_pattern.is_empty() {
                    // Merge syntax spans + search highlight spans
                    self.spans_with_search(&line, buf_line, &search_set, current_match)
                } else {
                    self.highlighter.highlight_line(&line)
                };

                // Visual Block highlight: overlay a selection span on the block columns
                if let Mode::Visual { kind: VisualKind::Block, anchor } = &editor.mode {
                    let (sl, el, lc, rc) = editor.block_rect(*anchor);
                    if buf_line >= sl && buf_line <= el {
                        let chars: Vec<char> = line.chars().collect();
                        let s = lc.min(chars.len());
                        let e = (rc + 1).min(chars.len());
                        if s < e {
                            let byte_s: usize = chars[..s].iter().map(|c| c.len_utf8()).sum();
                            let byte_e: usize = chars[..e].iter().map(|c| c.len_utf8()).sum();
                            spans.push(crate::syntax::highlight::Span {
                                start: byte_s,
                                end:   byte_e,
                                kind:  crate::syntax::highlight::TokenKind::SearchMatch,
                            });
                        }
                    }
                }

                self.render_line_with_spans(&line, &spans, text_w, buf_line, editor)?;
            } else {
                // Empty rows past EOF
                execute!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
                write!(self.stdout, "~")?;
                execute!(self.stdout, ResetColor)?;
                let padding = edit_w.saturating_sub(gutter + 1);
                write!(self.stdout, "{:padding$}", "", padding = padding)?;
            }
        }

        // ── Separator between file tree and edit area ─────
        if ft_width > 0 {
            for row in 0..edit_h {
                execute!(self.stdout, cursor::MoveTo(ft_width as u16, row as u16))?;
                execute!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
                write!(self.stdout, "│")?;
                execute!(self.stdout, ResetColor)?;
            }
        }

        // ── Plan overlay ───────────────────────────────────
        if let Some(plan) = plan_lines {
            self.render_plan_overlay(plan, w, h)?;
        }

        // ── Shell output overlay (:!cmd) ───────────────────
        if let Some(output) = &editor.shell_output {
            let lines: Vec<&str> = output.lines().collect();
            self.render_shell_overlay(&lines, w, h)?;
        }

        // ── Status bar (2 rows) ───────────────────────────
        let hint_row = (h - 2) as u16;
        let info_row = (h - 1) as u16;

        execute!(self.stdout, cursor::MoveTo(0, hint_row))?;
        execute!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
        let hint = if let Some(msg) = ai_query_msg {
            // AI query result displayed in hint line
            truncate(msg, w)
        } else if ghost.visible {
            format!("[Tab]确认执行  [Esc]取消  {}", ghost.explanation)
        } else {
            editor.hint_line()
        };
        write!(self.stdout, "{:<width$}", hint, width = w)?;
        execute!(self.stdout, ResetColor)?;

        execute!(self.stdout, cursor::MoveTo(0, info_row))?;
        let info = editor.info_line(self.highlighter.filetype());
        self.render_info_line(&info, w, editor)?;

        // Ghost text in the command prompt area (reuse bottom of info row)
        if ghost.visible {
            let ghost_str = format!("  :{}", ghost.command);
            execute!(self.stdout, cursor::MoveTo((w.saturating_sub(ghost_str.len().min(w))) as u16, info_row))?;
            execute!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
            write!(self.stdout, "{}", truncate(&ghost_str, w))?;
            execute!(self.stdout, ResetColor)?;
        }

        // ── Command / Search / AI input line ──────────────
        match &editor.mode {
            Mode::Command(s) => {
                execute!(self.stdout, cursor::MoveTo(0, info_row))?;
                execute!(self.stdout, SetForegroundColor(Color::White))?;
                write!(self.stdout, ":{:<width$}", s, width = w.saturating_sub(1))?;
                execute!(self.stdout, ResetColor)?;
            }
            Mode::Search(s) => {
                execute!(self.stdout, cursor::MoveTo(0, info_row))?;
                execute!(self.stdout, SetForegroundColor(Color::White))?;
                write!(self.stdout, "/{:<width$}", s, width = w.saturating_sub(1))?;
                execute!(self.stdout, ResetColor)?;
            }
            Mode::Ai(s) => {
                execute!(self.stdout, cursor::MoveTo(0, info_row))?;
                execute!(self.stdout, SetForegroundColor(Color::Cyan))?;
                write!(self.stdout, "?{:<width$}", s, width = w.saturating_sub(1))?;
                execute!(self.stdout, ResetColor)?;
            }
            _ => {}
        }

        // ── File tree prompt overlay ──────────────────────
        if let Some(prompt) = filetree_prompt {
            let label = prompt.label();
            let input = match prompt {
                crate::app::FileTreePrompt::NewFile  { input } => input.as_str(),
                crate::app::FileTreePrompt::NewDir   { input } => input.as_str(),
                crate::app::FileTreePrompt::Rename   { input, .. } => input.as_str(),
                crate::app::FileTreePrompt::Delete   { path, .. } => {
                    // Show path in hint line
                    let path_str = path.to_string_lossy();
                    execute!(self.stdout, cursor::MoveTo(0, hint_row))?;
                    execute!(self.stdout, SetForegroundColor(Color::Yellow))?;
                    write!(self.stdout, "{}{}  {:<width$}", label, path_str, "", width = w.saturating_sub(label.len() + path_str.len() + 2))?;
                    execute!(self.stdout, ResetColor)?;
                    // Overwrite info row with prompt
                    execute!(self.stdout, cursor::MoveTo(0, info_row))?;
                    execute!(self.stdout, SetBackgroundColor(Color::DarkYellow), SetForegroundColor(Color::Black))?;
                    write!(self.stdout, "{:<width$}", "按 y 确认删除，n 取消", width = w)?;
                    execute!(self.stdout, ResetColor)?;
                    return Ok(());
                }
            };
            execute!(self.stdout, cursor::MoveTo(0, info_row))?;
            execute!(self.stdout, SetBackgroundColor(Color::DarkGreen), SetForegroundColor(Color::Black))?;
            write!(self.stdout, "{}{:<width$}", label, input, width = w.saturating_sub(label.len()))?;
            execute!(self.stdout, ResetColor)?;
            // Show cursor at end of input
            let cursor_x = (label.len() + input.len()).min(w.saturating_sub(1));
            execute!(self.stdout, cursor::MoveTo(cursor_x as u16, info_row), cursor::Show)?;
        }

        // ── Hardware cursor position ──────────────────────
        match &editor.mode {
            Mode::Normal | Mode::Insert | Mode::Visual { .. } => {
                let vis_line = editor.cursor_line.saturating_sub(editor.scroll_line);
                if vis_line < edit_h {
                    let x = edit_x + gutter + editor.cursor_col.min(text_w.saturating_sub(1));
                    execute!(self.stdout,
                        cursor::Show,
                        cursor::MoveTo(x as u16, vis_line as u16),
                    )?;
                    // Block vs beam
                    if editor.mode.is_insert() {
                        execute!(self.stdout, cursor::SetCursorStyle::BlinkingBar)?;
                    } else {
                        execute!(self.stdout, cursor::SetCursorStyle::SteadyBlock)?;
                    }
                }
            }
            Mode::Command(_) | Mode::Search(_) | Mode::Ai(_) => {
                let input_len = match &editor.mode {
                    Mode::Command(s) => s.len() + 1,
                    Mode::Search(s)  => s.len() + 1,
                    Mode::Ai(s)      => s.len() + 1,
                    _ => 1,
                };
                execute!(self.stdout,
                    cursor::Show,
                    cursor::MoveTo(input_len as u16, info_row),
                    cursor::SetCursorStyle::BlinkingBar,
                )?;
            }
        }

        self.stdout.flush()
    }

    // ── Private helpers ───────────────────────────────────

    fn render_line_with_spans(
        &mut self,
        line: &str,
        spans: &[crate::syntax::highlight::Span],
        max_width: usize,
        _buf_line: usize,
        _editor: &Editor,
    ) -> io::Result<()> {
        let chars: Vec<char> = line.chars().collect();
        let limit = chars.len().min(max_width);
        let display: String = chars[..limit].iter().collect();

        if spans.is_empty() {
            write!(self.stdout, "{:<width$}", display, width = max_width)?;
            return Ok(());
        }

        // Build a colour map per byte index
        let mut byte_kind: Vec<Option<TokenKind>> = vec![None; line.len() + 1];
        for sp in spans {
            let s = sp.start.min(line.len());
            let e = sp.end.min(line.len());
            for b in s..e {
                byte_kind[b] = Some(sp.kind.clone());
            }
        }

        let mut col = 0usize;
        let mut byte_pos = 0usize;
        let mut last_kind: Option<TokenKind> = None;

        for ch in chars.iter().take(limit) {
            let ch_len = ch.len_utf8();
            let kind = byte_kind[byte_pos].clone();

            if kind != last_kind {
                // Reset previous
                execute!(self.stdout, ResetColor)?;
                if let Some(ref k) = kind {
                    if let Some(fg) = k.fg_color() {
                        execute!(self.stdout, SetForegroundColor(fg))?;
                    }
                    if let Some(bg) = k.bg_color() {
                        execute!(self.stdout, SetBackgroundColor(bg))?;
                    }
                    if k.bold() {
                        execute!(self.stdout, SetAttribute(Attribute::Bold))?;
                    }
                    if k.italic() {
                        execute!(self.stdout, SetAttribute(Attribute::Italic))?;
                    }
                }
                last_kind = kind;
            }

            write!(self.stdout, "{}", ch)?;
            byte_pos += ch_len;
            col += 1;
        }

        execute!(self.stdout, ResetColor)?;
        // Padding
        let pad = max_width.saturating_sub(col);
        if pad > 0 {
            write!(self.stdout, "{:padding$}", "", padding = pad)?;
        }
        Ok(())
    }

    fn spans_with_search(
        &self,
        line: &str,
        buf_line: usize,
        search_set: &std::collections::HashSet<(usize,usize)>,
        current_match: Option<(usize,usize)>,
    ) -> Vec<crate::syntax::highlight::Span> {
        use crate::syntax::highlight::Span;
        let mut spans = self.highlighter.highlight_line(line);
        // Add search highlight spans on top
        let chars: Vec<char> = line.chars().collect();
        let pat_len = 1usize; // minimal; real impl would use pattern length
        for (l, c) in search_set {
            if *l != buf_line { continue; }
            let start: usize = chars[..*c].iter().map(|ch| ch.len_utf8()).sum();
            let end: usize = chars[..(*c + pat_len).min(chars.len())].iter().map(|ch| ch.len_utf8()).sum();
            let kind = if current_match == Some((*l, *c)) {
                TokenKind::SearchMatchCurrent
            } else {
                TokenKind::SearchMatch
            };
            spans.push(Span { start, end, kind });
        }
        spans
    }

    fn render_filetree(
        &mut self,
        ft: &FileTree,
        width: usize,
        height: usize,
        focused: bool,
    ) -> io::Result<()> {
        let lines = ft.render_lines();
        for row in 0..height {
            execute!(self.stdout, cursor::MoveTo(0, row as u16))?;
            if let Some(line) = lines.get(row) {
                let is_cursor = row == ft.cursor;
                if is_cursor && focused {
                    execute!(self.stdout, SetBackgroundColor(Color::DarkBlue))?;
                }
                write!(self.stdout, "{:<width$}", truncate(line, width), width = width)?;
                execute!(self.stdout, ResetColor)?;
            } else {
                write!(self.stdout, "{:width$}", "", width = width)?;
            }
        }
        Ok(())
    }

    fn render_info_line(&mut self, info: &str, w: usize, editor: &Editor) -> io::Result<()> {
        let mode_color = match &editor.mode {
            Mode::Normal       => Color::Blue,
            Mode::Insert       => Color::Green,
            Mode::Visual { kind: VisualKind::Block, .. } => Color::DarkMagenta,
            Mode::Visual { .. } => Color::Magenta,
            Mode::Command(_)   => Color::Yellow,
            Mode::Ai(_)        => Color::Cyan,
            Mode::Search(_)    => Color::Yellow,
        };
        execute!(self.stdout,
            SetBackgroundColor(mode_color),
            SetForegroundColor(Color::Black),
            SetAttribute(Attribute::Bold),
        )?;
        write!(self.stdout, "{:<width$}", truncate(info, w), width = w)?;
        execute!(self.stdout, ResetColor)?;
        Ok(())
    }

    fn render_shell_overlay(&mut self, lines: &[&str], w: usize, h: usize) -> io::Result<()> {
        let max_visible = (h.saturating_sub(6)).min(20);
        let visible: Vec<&str> = lines.iter().take(max_visible).copied().collect();
        let overlay_w = visible.iter().map(|l| l.chars().count()).max().unwrap_or(20)
            .max(30).min(w.saturating_sub(4)) + 4;
        let overlay_h = visible.len() + 4;
        let start_x = (w.saturating_sub(overlay_w)) / 2;
        let start_y = (h.saturating_sub(overlay_h)) / 2;

        execute!(self.stdout, SetBackgroundColor(Color::DarkGrey), SetForegroundColor(Color::White))?;
        // Top border
        execute!(self.stdout, cursor::MoveTo(start_x as u16, start_y as u16))?;
        write!(self.stdout, "┌{}┐", "─".repeat(overlay_w.saturating_sub(2)))?;

        // Title
        execute!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 1) as u16))?;
        write!(self.stdout, "│{:^width$}│", "Shell 输出", width = overlay_w.saturating_sub(2))?;

        for (i, line) in visible.iter().enumerate() {
            execute!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 2 + i) as u16))?;
            write!(self.stdout, "│ {:<width$}│",
                truncate(line, overlay_w.saturating_sub(3)),
                width = overlay_w.saturating_sub(3))?;
        }

        // Footer
        let footer_y = start_y + 2 + visible.len();
        execute!(self.stdout, cursor::MoveTo(start_x as u16, footer_y as u16))?;
        let hint = "[任意键关闭]";
        write!(self.stdout, "│{:^width$}│", hint, width = overlay_w.saturating_sub(2))?;

        // Bottom border
        execute!(self.stdout, cursor::MoveTo(start_x as u16, (footer_y + 1) as u16))?;
        write!(self.stdout, "└{}┘", "─".repeat(overlay_w.saturating_sub(2)))?;

        execute!(self.stdout, ResetColor)?;
        Ok(())
    }

    fn render_plan_overlay(&mut self, plan: &[String], w: usize, h: usize) -> io::Result<()> {
        let overlay_w = (w * 3 / 4).min(w.saturating_sub(4));
        let overlay_h = plan.len() + 4;
        let start_x = (w.saturating_sub(overlay_w)) / 2;
        let start_y = (h.saturating_sub(overlay_h)) / 2;

        execute!(self.stdout, SetBackgroundColor(Color::DarkBlue), SetForegroundColor(Color::White))?;
        // Top border
        execute!(self.stdout, cursor::MoveTo(start_x as u16, start_y as u16))?;
        write!(self.stdout, "┌{}┐", "─".repeat(overlay_w.saturating_sub(2)))?;

        // Title
        execute!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 1) as u16))?;
        write!(self.stdout, "│{:^width$}│", "AI 执行计划", width = overlay_w.saturating_sub(2))?;

        for (i, line) in plan.iter().enumerate() {
            execute!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 2 + i) as u16))?;
            write!(self.stdout, "│ {:<width$}│", truncate(line, overlay_w.saturating_sub(3)), width = overlay_w.saturating_sub(3))?;
        }

        // Footer
        let footer_y = start_y + 2 + plan.len();
        execute!(self.stdout, cursor::MoveTo(start_x as u16, footer_y as u16))?;
        let hint = "[y]确认执行  [n]取消  [e]编辑计划";
        write!(self.stdout, "│{:^width$}│", hint, width = overlay_w.saturating_sub(2))?;

        // Bottom border
        execute!(self.stdout, cursor::MoveTo(start_x as u16, (footer_y + 1) as u16))?;
        write!(self.stdout, "└{}┘", "─".repeat(overlay_w.saturating_sub(2)))?;

        execute!(self.stdout, ResetColor)?;
        Ok(())
    }
}

fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        chars[..max].iter().collect()
    }
}
