//! Main TUI renderer using crossterm.
use crossterm::{
    cursor,
    event::{EnableMouseCapture, DisableMouseCapture},
    execute,
    style::{Attribute, Color, SetForegroundColor, SetBackgroundColor, ResetColor, SetAttribute},
    terminal,
};
use unicode_width::UnicodeWidthChar;
use std::io::{self, Write, Stdout};

use crate::app::{AiStatus, FocusZone, ThemePicker};
use crate::mode::cmd_completion::CmdCompletionState;
use crate::config::Config;
use crate::editor::Editor;
use crate::mode::{Mode, VisualKind};
use crate::syntax::highlight::{FileType, Highlighter, SyntectHighlighter, SyntectSpan, OverlayKind, CodePalette};
use crate::ui::chatpanel::{ChatPanel, ChatRole};
use crate::ui::filetree::FileTree;
use crate::ui::ghost::GhostText;
use crate::ui::mdrender::{MdRenderer, MdLine, MdTheme};

pub struct Renderer {
    pub stdout: Stdout,
    /// Legacy rule-based highlighter — kept only for search-match / visual-block
    /// overlay spans that are merged on top of syntect output.
    pub highlighter: Highlighter,
    /// Syntect-backed highlighter for the editor text area.
    pub syntect_hl: SyntectHighlighter,
    pub md_renderer: MdRenderer,
}

impl Renderer {
    /// Create a renderer driven by the user's `~/.hirc` theme configuration.
    pub fn new(filetype: FileType, config: &Config) -> Self {
        let editor_theme = &config.theme.editor_theme;
        let chat_theme = MdTheme::by_name(&config.theme.chat_theme);
        let palette = CodePalette::by_name(&config.theme.chat_theme)
            .unwrap_or_else(CodePalette::neon_minimalist);
        Self {
            stdout: io::stdout(),
            highlighter: Highlighter::new(filetype),
            syntect_hl: SyntectHighlighter::new(filetype, editor_theme),
            md_renderer: MdRenderer::new_with_palette(chat_theme, palette),
        }
    }

    pub fn set_filetype(&mut self, ft: FileType) {
        self.highlighter = Highlighter::new(ft);
        self.syntect_hl.set_filetype(ft);
    }

    /// Switch both the editor syntax palette and the Markdown chat theme at
    /// runtime.  Called by `:theme <name>`.
    pub fn set_theme(&mut self, name: &str) {
        self.syntect_hl.set_theme(name);
        if let Some(p) = CodePalette::by_name(name) {
            self.md_renderer.palette = p;
        }
        self.md_renderer.theme = MdTheme::by_name(name);
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
        ai_status: &AiStatus,
        ai_pending: bool,
        ai_tick: u64,
        chat_panel: &mut ChatPanel,
        chat_visible: bool,
        focus: FocusZone,
        chat_input: &str,
        chat_input_active: bool,
        chat_input_cursor: usize,
        theme_picker: &Option<ThemePicker>,
        cmd_completion: &CmdCompletionState,
    ) -> io::Result<()> {
        let chat_focus = focus == FocusZone::Chat;
        let ft_focused = focus == FocusZone::FileTree;
        let _editor_focused = focus == FocusZone::Editor;
        let w = editor.term_width as usize;
        let h = editor.term_height as usize;
        let ft_width = if editor.filetree_visible {
            editor.config.filetree.width as usize
        } else { 0 };
        let chat_width = if chat_visible {
            (editor.config.chat.width as usize).min(w / 2)
        } else { 0 };

        execute!(self.stdout,
            cursor::Hide,
            cursor::MoveTo(0, 0),
        )?;

        // ── File tree panel ──────────────────────────────
        if editor.filetree_visible {
            if let Some(ft) = filetree {
                self.render_filetree(ft, ft_width, h.saturating_sub(2), ft_focused)?;
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
        let chat_total = chat_width + if chat_width > 0 { 1 } else { 0 }; // +1 for separator
        let edit_w = w.saturating_sub(edit_x).saturating_sub(chat_total);
        let edit_h = h.saturating_sub(2);
        let gutter = if editor.config.general.line_numbers { editor.gutter_width() } else { 0 };

        let text_w = edit_w.saturating_sub(gutter);

        // Draw search highlights as a sorted list of (line,col) pairs
        let search_set: std::collections::HashSet<(usize,usize)> = editor.search_matches.iter().cloned().collect();
        let current_match = editor.search_matches.get(editor.search_match_idx).cloned();

        // ── Reset syntect state and pre-parse lines above the viewport ──
        // SyntectHighlighter is a stateful state machine: each highlight_line()
        // call advances internal parse state.  We must reset it at the start of
        // every frame and replay all lines from the top of the file up to
        // scroll_line so that multi-line constructs (block comments, heredocs,
        // multi-line strings) that start above the viewport are tracked correctly.
        //
        // Optimisation: only replay the last MAX_PRE_PARSE lines before the
        // viewport instead of the entire file.  200 lines is enough to cover
        // virtually all real-world multi-line constructs (block comments,
        // heredocs, fenced code blocks, etc.) while keeping scroll responsive.
        const MAX_PRE_PARSE: usize = 200;
        self.syntect_hl.reset_state();
        let pre_start = editor.scroll_line.saturating_sub(MAX_PRE_PARSE);
        let pre_end = editor.scroll_line.min(editor.buffer.line_count());
        for pre_line in pre_start..pre_end {
            let text = editor.buffer.line_str(pre_line);
            let _ = self.syntect_hl.highlight_line(&text);
        }

        // Ensure no colour state leaks from previous frame's status bar or
        // overlays into the editing area.
        execute!(self.stdout, ResetColor, SetAttribute(Attribute::Reset))?;

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

                let mut spans: Vec<SyntectSpan> = if editor.search_highlight && !editor.search_pattern.is_empty() {
                    // Merge syntect spans + search highlight overlays
                    self.spans_with_search(&line, buf_line, &search_set, current_match)
                } else {
                    self.syntect_hl.highlight_line(&line)
                };

                // Visual Block highlight: overlay a VisualBlock span on the selected columns
                if let Mode::Visual { kind: VisualKind::Block, anchor } = &editor.mode {
                    let (sl, el, lc, rc) = editor.block_rect(*anchor);
                    if buf_line >= sl && buf_line <= el {
                        let chars: Vec<char> = line.chars().collect();
                        let s = lc.min(chars.len());
                        let e = (rc + 1).min(chars.len());
                        if s < e {
                            let byte_s: usize = chars[..s].iter().map(|c| c.len_utf8()).sum();
                            let byte_e: usize = chars[..e].iter().map(|c| c.len_utf8()).sum();
                            spans.push(SyntectSpan {
                                start: byte_s,
                                end:   byte_e,
                                fg: Color::White,
                                bold: false,
                                italic: false,
                                overlay: Some(OverlayKind::VisualBlock),
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

        // ── Chat panel (right side) ─────────────────────
        if chat_visible && chat_width > 0 {
            let chat_x = w.saturating_sub(chat_width);
            let sep_x = chat_x.saturating_sub(1);
            // Separator
            for row in 0..edit_h {
                execute!(self.stdout, cursor::MoveTo(sep_x as u16, row as u16))?;
                execute!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
                write!(self.stdout, "│")?;
                execute!(self.stdout, ResetColor)?;
            }
            self.render_chat_panel(chat_panel, chat_x, chat_width, edit_h, chat_focus, chat_input, chat_input_active)?;
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

        // ── Theme picker overlay (:theme) ──────────────────
        if let Some(picker) = theme_picker {
            self.render_theme_picker(picker, w, h)?;
        }

        // ── Status bar (2 rows) ───────────────────────────
        let hint_row = (h - 2) as u16;
        let info_row = (h - 1) as u16;

        execute!(self.stdout, cursor::MoveTo(0, hint_row))?;
        if ai_pending {
            // Animated spinner while AI is working
            let spinner_frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let frame = spinner_frames[(ai_tick as usize) % spinner_frames.len()];
            let spinner_msg = format!("{} AI 思考中...", frame);
            execute!(self.stdout, SetForegroundColor(Color::Cyan))?;
            let spinner_trunc = truncate(&spinner_msg, w);
            let spinner_dw = display_width_str(&spinner_trunc);
            write!(self.stdout, "{}", spinner_trunc)?;
            if spinner_dw < w {
                write!(self.stdout, "{:padding$}", "", padding = w - spinner_dw)?;
            }
            execute!(self.stdout, ResetColor)?;
        } else {
            execute!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
            let hint = if let Some(msg) = ai_query_msg {
                // AI query result displayed in hint line
                truncate(msg, w)
            } else if ghost.visible {
                format!("[Tab]确认执行  [Esc]取消  {}", ghost.explanation)
            } else {
                editor.hint_line()
            };
            let hint_trunc = truncate(&hint, w);
            let hint_dw = display_width_str(&hint_trunc);
            write!(self.stdout, "{}", hint_trunc)?;
            if hint_dw < w {
                write!(self.stdout, "{:padding$}", "", padding = w - hint_dw)?;
            }
            execute!(self.stdout, ResetColor)?;
        }

        execute!(self.stdout, cursor::MoveTo(0, info_row))?;
        let ai_indicator = match ai_status {
            AiStatus::Idle         => "[AI ●]",
            AiStatus::NotConfigured => "[AI ○]",
            AiStatus::Requesting   => "[AI ⟳]",
            AiStatus::Error(_)     => "[AI ✗]",
        };
        let info = editor.info_line(self.highlighter.filetype());
        self.render_info_line(&info, w, editor, ai_indicator, ai_status)?;

        // Ghost text in the command prompt area (reuse bottom of info row)
        if ghost.visible {
            let ghost_str = format!("  :{}", ghost.command);
            let ghost_dw = display_width_str(&ghost_str);
            execute!(self.stdout, cursor::MoveTo((w.saturating_sub(ghost_dw.min(w))) as u16, info_row))?;
            execute!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
            write!(self.stdout, "{}", truncate(&ghost_str, w))?;
            execute!(self.stdout, ResetColor)?;
        }

        // ── Command / Search / AI input line ──────────────
        match &editor.mode {
            Mode::Command(s) => {
                execute!(self.stdout, cursor::MoveTo(0, info_row))?;
                execute!(self.stdout, SetForegroundColor(Color::White))?;
                let s_trunc = truncate(s, w.saturating_sub(1));
                let s_dw = display_width_str(&s_trunc) + 1; // +1 for ':'
                write!(self.stdout, ":{}", s_trunc)?;
                if s_dw < w {
                    write!(self.stdout, "{:padding$}", "", padding = w - s_dw)?;
                }
                execute!(self.stdout, ResetColor)?;
            }
            Mode::Search(s) => {
                execute!(self.stdout, cursor::MoveTo(0, info_row))?;
                execute!(self.stdout, SetForegroundColor(Color::White))?;
                let s_trunc = truncate(s, w.saturating_sub(1));
                let s_dw = display_width_str(&s_trunc) + 1; // +1 for '/'
                write!(self.stdout, "/{}", s_trunc)?;
                if s_dw < w {
                    write!(self.stdout, "{:padding$}", "", padding = w - s_dw)?;
                }
                execute!(self.stdout, ResetColor)?;
            }
            Mode::Ai(s) => {
                execute!(self.stdout, cursor::MoveTo(0, info_row))?;
                execute!(self.stdout, SetForegroundColor(Color::Cyan))?;
                let s_trunc = truncate(s, w.saturating_sub(1));
                let s_dw = display_width_str(&s_trunc) + 1; // +1 for '?'
                write!(self.stdout, "?{}", s_trunc)?;
                if s_dw < w {
                    write!(self.stdout, "{:padding$}", "", padding = w - s_dw)?;
                }
                execute!(self.stdout, ResetColor)?;
            }
            _ => {}
        }

        // ── Command completion popup ──────────────────────
        if editor.mode.is_command() && cmd_completion.visible() {
            self.render_cmd_completion(cmd_completion, w, hint_row)?;
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
            let cursor_x = (display_width_str(label) + display_width_str(input)).min(w.saturating_sub(1));
            execute!(self.stdout, cursor::MoveTo(cursor_x as u16, info_row), cursor::Show)?;
        }

        // ── Hardware cursor position ──────────────────────
        // When chat input is active, cursor must be in the chat input line
        // so the IME (input method) composing window appears at the right place.
        if chat_input_active && chat_visible && chat_width > 0 {
            let chat_x = w.saturating_sub(chat_width);
            // Input line is at: title(1) + content_h + 0-based = same row as input_y in render_chat_panel
            let input_row_count = 1;
            let usable_h = edit_h.saturating_sub(input_row_count);
            let content_h = usable_h.saturating_sub(1);
            let input_y = (1 + content_h) as u16;
            // "▶ " prefix is 2 display columns (▶=1 wide + space=1), then text up to cursor
            let prefix_w = display_width_str("▶ ");
            let text_before_cursor: String = chat_input.chars().take(chat_input_cursor).collect();
            let cursor_offset = display_width_str(&text_before_cursor);
            let cursor_x = chat_x + prefix_w + cursor_offset;
            let cursor_x = cursor_x.min(w.saturating_sub(1));
            execute!(self.stdout,
                cursor::Show,
                cursor::MoveTo(cursor_x as u16, input_y),
                cursor::SetCursorStyle::BlinkingBar,
            )?;
        } else {
            match &editor.mode {
                Mode::Normal | Mode::Insert | Mode::Visual { .. } => {
                    let vis_line = editor.cursor_line.saturating_sub(editor.scroll_line);
                    if vis_line < edit_h {
                        // Convert char-index cursor_col to display width for correct terminal positioning
                        let buf_line = editor.cursor_line;
                        let display_col = if buf_line < editor.buffer.line_count() {
                            let line = editor.buffer.line_str(buf_line);
                            line.chars()
                                .take(editor.cursor_col)
                                .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
                                .sum::<usize>()
                        } else {
                            editor.cursor_col
                        };
                        let x = edit_x + gutter + display_col.min(text_w.saturating_sub(1));
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
                        Mode::Command(s) => display_width_str(s) + 1,
                        Mode::Search(s)  => display_width_str(s) + 1,
                        Mode::Ai(s)      => display_width_str(s) + 1,
                        _ => 1,
                    };
                    execute!(self.stdout,
                        cursor::Show,
                        cursor::MoveTo(input_len as u16, info_row),
                        cursor::SetCursorStyle::BlinkingBar,
                    )?;
                }
            }
        }

        self.stdout.flush()
    }

    // ── Private helpers ───────────────────────────────────

    /// Render one editor line using syntect-backed `SyntectSpan`s.
    ///
    /// Overlay spans (search match, visual block) are painted on top of the
    /// syntect colours by overriding the background (and optionally foreground)
    /// for the affected byte ranges.
    fn render_line_with_spans(
        &mut self,
        line: &str,
        spans: &[SyntectSpan],
        max_width: usize,
        _buf_line: usize,
        _editor: &Editor,
    ) -> io::Result<()> {
        let chars: Vec<char> = line.chars().collect();

        // Compute how many chars fit within max_width display columns.
        let mut limit = 0;
        let mut used_width = 0;
        for ch in &chars {
            let w = UnicodeWidthChar::width(*ch).unwrap_or(0);
            if used_width + w > max_width { break; }
            used_width += w;
            limit += 1;
        }

        if spans.is_empty() {
            let display: String = chars[..limit].iter().collect();
            write!(self.stdout, "{}", display)?;
            let pad = max_width.saturating_sub(used_width);
            if pad > 0 {
                write!(self.stdout, "{:padding$}", "", padding = pad)?;
            }
            return Ok(());
        }

        // Build per-byte lookup: (fg, bold, italic, overlay).
        // We store indices into `spans` rather than cloning colours.
        let line_len = line.len();
        let mut byte_span: Vec<Option<usize>> = vec![None; line_len + 1];
        for (idx, sp) in spans.iter().enumerate() {
            let s = sp.start.min(line_len);
            let e = sp.end.min(line_len);
            for b in s..e {
                byte_span[b] = Some(idx);
            }
        }

        let mut col = 0usize;
        let mut byte_pos = 0usize;
        let mut last_idx: Option<usize> = None; // sentinel: "no span applied yet"

        for ch in chars.iter().take(limit) {
            let ch_len = ch.len_utf8();
            let cur_idx = byte_span[byte_pos];

            if cur_idx != last_idx {
                execute!(self.stdout, ResetColor, SetAttribute(Attribute::Reset))?;
                if let Some(idx) = cur_idx {
                    let sp = &spans[idx];
                    if let Some(ov) = sp.overlay {
                        // Overlay: use overlay bg, optionally override fg
                        execute!(self.stdout, SetBackgroundColor(ov.bg_color()))?;
                        if let Some(fg) = ov.fg_color() {
                            execute!(self.stdout, SetForegroundColor(fg))?;
                        } else {
                            execute!(self.stdout, SetForegroundColor(sp.fg))?;
                        }
                    } else {
                        execute!(self.stdout, SetForegroundColor(sp.fg))?;
                    }
                    if sp.bold   { execute!(self.stdout, SetAttribute(Attribute::Bold))?; }
                    if sp.italic { execute!(self.stdout, SetAttribute(Attribute::Italic))?; }
                }
                last_idx = cur_idx;
            }

            write!(self.stdout, "{}", ch)?;
            byte_pos += ch_len;
            col += UnicodeWidthChar::width(*ch).unwrap_or(0);
        }

        execute!(self.stdout, ResetColor, SetAttribute(Attribute::Reset))?;
        let pad = max_width.saturating_sub(col);
        if pad > 0 {
            write!(self.stdout, "{:padding$}", "", padding = pad)?;
        }
        Ok(())
    }

    /// Legacy render path used only when syntect returns no spans (plain text).
    #[allow(dead_code)]
    fn render_line_plain(
        &mut self,
        line: &str,
        max_width: usize,
    ) -> io::Result<()> {
        let chars: Vec<char> = line.chars().collect();
        let mut limit = 0;
        let mut used_width = 0;
        for ch in &chars {
            let w = UnicodeWidthChar::width(*ch).unwrap_or(0);
            if used_width + w > max_width { break; }
            used_width += w;
            limit += 1;
        }
        let display: String = chars[..limit].iter().collect();
        write!(self.stdout, "{}", display)?;
        let pad = max_width.saturating_sub(used_width);
        if pad > 0 {
            write!(self.stdout, "{:padding$}", "", padding = pad)?;
        }
        Ok(())
    }

    /// Build syntect spans for a line, then overlay search-match highlights.
    fn spans_with_search(
        &mut self,
        line: &str,
        buf_line: usize,
        search_set: &std::collections::HashSet<(usize,usize)>,
        current_match: Option<(usize,usize)>,
    ) -> Vec<SyntectSpan> {
        let mut spans = self.syntect_hl.highlight_line(line);
        let chars: Vec<char> = line.chars().collect();
        let pat_len = 1usize; // one char per match position
        for (l, c) in search_set {
            if *l != buf_line { continue; }
            let start: usize = chars[..*c].iter().map(|ch| ch.len_utf8()).sum();
            let end: usize = chars[..(*c + pat_len).min(chars.len())].iter().map(|ch| ch.len_utf8()).sum();
            let overlay = if current_match == Some((*l, *c)) {
                OverlayKind::SearchMatchCurrent
            } else {
                OverlayKind::SearchMatch
            };
            // Push an overlay span; the renderer will paint it on top.
            spans.push(SyntectSpan {
                start,
                end,
                fg: crossterm::style::Color::White,
                bold: false,
                italic: false,
                overlay: Some(overlay),
            });
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
                let ft_trunc = truncate(line, width);
                let ft_dw = display_width_str(&ft_trunc);
                write!(self.stdout, "{}", ft_trunc)?;
                if ft_dw < width {
                    write!(self.stdout, "{:padding$}", "", padding = width - ft_dw)?;
                }
                execute!(self.stdout, ResetColor)?;
            } else {
                write!(self.stdout, "{:width$}", "", width = width)?;
            }
        }
        Ok(())
    }

    fn render_chat_panel(
        &mut self,
        panel: &mut ChatPanel,
        x: usize,
        width: usize,
        height: usize,
        focused: bool,
        chat_input: &str,
        chat_input_active: bool,
    ) -> io::Result<()> {
        let content_w = width.saturating_sub(2); // 1 char padding each side
        let all_lines = panel.render_lines_styled(content_w, &self.md_renderer);
        let total = all_lines.len();

        // Reserve 1 row for input line at the bottom
        let input_row_count = 1;
        let usable_h = height.saturating_sub(input_row_count);

        // Clamp scroll
        let max_scroll = total.saturating_sub(usable_h.saturating_sub(1));
        if panel.scroll > max_scroll {
            panel.scroll = max_scroll;
        }

        // Title bar
        execute!(self.stdout, cursor::MoveTo(x as u16, 0))?;
        if focused {
            execute!(self.stdout, SetBackgroundColor(Color::DarkCyan), SetForegroundColor(Color::White))?;
        } else {
            execute!(self.stdout, SetBackgroundColor(Color::DarkGrey), SetForegroundColor(Color::White))?;
        }
        let title = if panel.messages.is_empty() {
            "AI Chat"
        } else {
            "AI Chat"
        };
        let title_trunc = truncate(title, width);
        let title_dw = display_width_str(&title_trunc);
        write!(self.stdout, "{}", title_trunc)?;
        if title_dw < width {
            write!(self.stdout, "{:padding$}", "", padding = width - title_dw)?;
        }
        execute!(self.stdout, ResetColor)?;

        // Content area — render styled MdLine spans
        let content_h = usable_h.saturating_sub(1);
        let visible_start = total.saturating_sub(content_h + panel.scroll);
        let visible_end = total.saturating_sub(panel.scroll);

        let visible: Vec<&(ChatRole, MdLine)> = all_lines[visible_start..visible_end].iter().collect();

        for row in 0..content_h {
            execute!(self.stdout, cursor::MoveTo(x as u16, (row + 1) as u16))?;
            if let Some((_role, md_line)) = visible.get(row) {
                // Render border (blockquote decoration)
                let mut col = 0usize;
                if let Some((ref border_str, border_color)) = md_line.border {
                    execute!(self.stdout, SetForegroundColor(border_color.clone()))?;
                    write!(self.stdout, " {}", border_str)?;
                    col += 1 + display_width_str(border_str);
                    execute!(self.stdout, ResetColor)?;
                } else {
                    write!(self.stdout, " ")?;
                    col += 1;
                }

                // Render indent
                if md_line.indent > 0 {
                    let indent_str = " ".repeat(md_line.indent);
                    write!(self.stdout, "{}", indent_str)?;
                    col += md_line.indent;
                }

                // Render each styled span
                for span in &md_line.spans {
                    let avail = width.saturating_sub(col);
                    if avail == 0 { break; }
                    let span_text = truncate(&span.text, avail);
                    let span_dw = display_width_str(&span_text);

                    // Apply styles
                    if let Some(fg) = span.fg {
                        execute!(self.stdout, SetForegroundColor(fg))?;
                    }
                    if let Some(bg) = span.bg {
                        execute!(self.stdout, SetBackgroundColor(bg))?;
                    }
                    if span.bold {
                        execute!(self.stdout, SetAttribute(Attribute::Bold))?;
                    }
                    if span.italic {
                        execute!(self.stdout, SetAttribute(Attribute::Italic))?;
                    }
                    if span.underline {
                        execute!(self.stdout, SetAttribute(Attribute::Underlined))?;
                    }
                    if span.strikethrough {
                        execute!(self.stdout, SetAttribute(Attribute::CrossedOut))?;
                    }
                    if span.dim {
                        execute!(self.stdout, SetAttribute(Attribute::Dim))?;
                    }

                    write!(self.stdout, "{}", span_text)?;
                    execute!(self.stdout, ResetColor, SetAttribute(Attribute::Reset))?;
                    col += span_dw;
                }

                // Pad remaining width
                let pad = width.saturating_sub(col);
                if pad > 0 {
                    write!(self.stdout, "{:padding$}", "", padding = pad)?;
                }
            } else {
                write!(self.stdout, "{:width$}", "", width = width)?;
            }
        }

        // ── Input line at bottom of chat panel ──────────
        let input_y = (1 + content_h) as u16;
        execute!(self.stdout, cursor::MoveTo(x as u16, input_y))?;
        if chat_input_active {
            execute!(self.stdout, SetBackgroundColor(Color::DarkBlue), SetForegroundColor(Color::White))?;
            let prompt_str = format!("▶ {}", chat_input);
            let prompt_trunc = truncate(&prompt_str, width);
            let prompt_dw = display_width_str(&prompt_trunc);
            write!(self.stdout, "{}", prompt_trunc)?;
            if prompt_dw < width {
                write!(self.stdout, "{:padding$}", "", padding = width - prompt_dw)?;
            }
            execute!(self.stdout, ResetColor)?;
        } else if focused {
            execute!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
            let hint = "[i]输入  [Esc]返回";
            let hint_trunc = truncate(hint, width);
            let hint_dw = display_width_str(&hint_trunc);
            write!(self.stdout, "{}", hint_trunc)?;
            if hint_dw < width {
                write!(self.stdout, "{:padding$}", "", padding = width - hint_dw)?;
            }
            execute!(self.stdout, ResetColor)?;
        } else {
            write!(self.stdout, "{:width$}", "", width = width)?;
        }

        Ok(())
    }

    fn render_info_line(&mut self, info: &str, w: usize, editor: &Editor, ai_indicator: &str, ai_status: &AiStatus) -> io::Result<()> {
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

        // Reserve space for AI indicator on the right
        let indicator_dw = display_width_str(ai_indicator);
        let info_max = w.saturating_sub(indicator_dw + 1); // +1 for spacing
        let info_trunc = truncate(info, info_max);
        let info_dw = display_width_str(&info_trunc);
        write!(self.stdout, "{}", info_trunc)?;

        // Fill gap between info and AI indicator
        let gap = w.saturating_sub(info_dw + indicator_dw);
        if gap > 0 {
            write!(self.stdout, "{:padding$}", "", padding = gap)?;
        }

        // Draw AI indicator with appropriate color
        let ai_fg = match ai_status {
            AiStatus::Idle          => Color::Green,
            AiStatus::NotConfigured => Color::DarkGrey,
            AiStatus::Requesting    => Color::Yellow,
            AiStatus::Error(_)      => Color::Red,
        };
        execute!(self.stdout, SetForegroundColor(ai_fg))?;
        write!(self.stdout, "{}", ai_indicator)?;

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

    /// Render the command completion popup above the command input line.
    /// The popup grows upward from `anchor_row` (the hint row, one above info_row).
    fn render_cmd_completion(
        &mut self,
        state: &CmdCompletionState,
        term_w: usize,
        anchor_row: u16,
    ) -> io::Result<()> {
        let items = &state.items;
        if items.is_empty() { return Ok(()); }

        // Limit visible items so the popup doesn't eat the whole screen
        let max_visible: usize = 10;
        let visible_count = items.len().min(max_visible);

        // Calculate column widths
        let max_trigger = items.iter().take(visible_count)
            .map(|c| c.trigger.len())
            .max().unwrap_or(4);
        let max_desc = items.iter().take(visible_count)
            .map(|c| display_width_str(c.desc))
            .max().unwrap_or(8);
        // popup width: " :trigger  description "
        let popup_w = (3 + max_trigger + 2 + max_desc + 1).min(term_w.saturating_sub(2));

        // Scroll window: if selected item is outside visible range, shift
        let selected = state.selected.unwrap_or(0);
        let scroll_start = if selected >= visible_count {
            selected - visible_count + 1
        } else {
            0
        };
        let scroll_end = (scroll_start + visible_count).min(items.len());

        // Draw from bottom up: row 0 of popup = anchor_row - visible_count
        let popup_top = (anchor_row as usize).saturating_sub(visible_count);

        for (vi, idx) in (scroll_start..scroll_end).enumerate() {
            let row = (popup_top + vi) as u16;
            let item = &items[idx];
            let is_selected = idx == selected;

            execute!(self.stdout, cursor::MoveTo(0, row))?;

            if is_selected {
                execute!(self.stdout,
                    SetBackgroundColor(Color::Rgb { r: 68, g: 71, b: 90 }),
                    SetForegroundColor(Color::Rgb { r: 189, g: 147, b: 249 }),
                )?;
            } else {
                execute!(self.stdout,
                    SetBackgroundColor(Color::Rgb { r: 40, g: 42, b: 54 }),
                    SetForegroundColor(Color::Rgb { r: 248, g: 248, b: 242 }),
                )?;
            }

            // Format: " :trigger  description "
            let trigger_str = format!(" :{}", item.trigger);
            let trigger_dw = display_width_str(&trigger_str);
            write!(self.stdout, "{}", trigger_str)?;

            // Gap between trigger and description
            let gap = (3 + max_trigger).saturating_sub(trigger_dw - 1);
            if gap > 0 {
                write!(self.stdout, "{:gap$}", "", gap = gap)?;
            }

            // Description in dimmer color
            if is_selected {
                execute!(self.stdout, SetForegroundColor(Color::Rgb { r: 166, g: 227, b: 161 }))?;
            } else {
                execute!(self.stdout, SetForegroundColor(Color::Rgb { r: 108, g: 112, b: 134 }))?;
            }
            let desc_trunc = truncate(item.desc, popup_w.saturating_sub(trigger_dw + gap + 1));
            write!(self.stdout, "{}", desc_trunc)?;

            // Pad to popup width
            let used = trigger_dw + gap + display_width_str(&desc_trunc);
            let pad = popup_w.saturating_sub(used);
            if pad > 0 {
                write!(self.stdout, "{:pad$}", "", pad = pad)?;
            }

            execute!(self.stdout, ResetColor)?;

            // Clear rest of line if popup is narrower than terminal
            if popup_w < term_w {
                // We need to clear the remaining columns on this row
                // to avoid leftover text from the editor area
            }
        }

        Ok(())
    }

    fn render_theme_picker(&mut self, picker: &ThemePicker, w: usize, h: usize) -> io::Result<()> {
        let item_count = picker.themes.len();
        // Each item: "  ● theme-name  " or "    theme-name  "
        let max_name_len = picker.themes.iter().map(|t| t.len()).max().unwrap_or(10);
        // Width based on content only — don't let the title force the box wider
        let overlay_w = (max_name_len + 8).max(36).min(w.saturating_sub(4));
        let inner_w = overlay_w.saturating_sub(2); // space between │…│
        let overlay_h = item_count + 4; // top border + title + items + bottom border
        let start_x = (w.saturating_sub(overlay_w)) / 2;
        let start_y = (h.saturating_sub(overlay_h)) / 2;

        // Top border
        execute!(self.stdout, cursor::MoveTo(start_x as u16, start_y as u16))?;
        execute!(self.stdout, SetBackgroundColor(Color::Rgb { r: 30, g: 30, b: 46 }), SetForegroundColor(Color::Rgb { r: 180, g: 190, b: 254 }))?;
        write!(self.stdout, "┌{}┐", "─".repeat(inner_w))?;

        // Title — truncate to fit inside the box, then centre-pad
        let title = "选择主题 j/k Enter Esc";
        let title_trunc = truncate(title, inner_w.saturating_sub(2)); // leave 1 col padding each side
        let title_dw = display_width_str(&title_trunc);
        let pad_total = inner_w.saturating_sub(title_dw);
        let pad_left = pad_total / 2;
        let pad_right = pad_total - pad_left;
        execute!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 1) as u16))?;
        execute!(self.stdout, SetForegroundColor(Color::Rgb { r: 180, g: 190, b: 254 }))?;
        write!(self.stdout, "│{:pl$}{}{:pr$}│", "", title_trunc, "", pl = pad_left, pr = pad_right)?;

        // Separator
        execute!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 2) as u16))?;
        write!(self.stdout, "├{}┤", "─".repeat(inner_w))?;

        // Theme items
        for (i, theme_name) in picker.themes.iter().enumerate() {
            let row = start_y + 3 + i;
            execute!(self.stdout, cursor::MoveTo(start_x as u16, row as u16))?;

            if i == picker.cursor {
                // Selected item — highlighted
                execute!(self.stdout,
                    SetBackgroundColor(Color::Rgb { r: 88, g: 91, b: 112 }),
                    SetForegroundColor(Color::Rgb { r: 166, g: 227, b: 161 }),
                )?;
                let label = format!("  ● {}  ", theme_name);
                let label_trunc = truncate(&label, inner_w);
                let label_dw = display_width_str(&label_trunc);
                write!(self.stdout, "│{}{:pad$}│", label_trunc, "", pad = inner_w.saturating_sub(label_dw))?;
            } else {
                execute!(self.stdout,
                    SetBackgroundColor(Color::Rgb { r: 30, g: 30, b: 46 }),
                    SetForegroundColor(Color::Rgb { r: 205, g: 214, b: 244 }),
                )?;
                let label = format!("    {}  ", theme_name);
                let label_trunc = truncate(&label, inner_w);
                let label_dw = display_width_str(&label_trunc);
                write!(self.stdout, "│{}{:pad$}│", label_trunc, "", pad = inner_w.saturating_sub(label_dw))?;
            }
        }

        // Bottom border
        let bottom_y = start_y + 3 + item_count;
        execute!(self.stdout, cursor::MoveTo(start_x as u16, bottom_y as u16))?;
        execute!(self.stdout,
            SetBackgroundColor(Color::Rgb { r: 30, g: 30, b: 46 }),
            SetForegroundColor(Color::Rgb { r: 180, g: 190, b: 254 }),
        )?;
        write!(self.stdout, "└{}┘", "─".repeat(inner_w))?;

        execute!(self.stdout, ResetColor)?;
        Ok(())
    }
}

fn truncate(s: &str, max: usize) -> String {
    let mut width = 0;
    let mut result = String::new();
    for ch in s.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + w > max { break; }
        width += w;
        result.push(ch);
    }
    result
}

/// Calculate the display width of a string (accounting for wide chars).
fn display_width_str(s: &str) -> usize {
    s.chars().map(|c| UnicodeWidthChar::width(c).unwrap_or(0)).sum()
}
