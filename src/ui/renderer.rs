//! Main TUI renderer using crossterm.
//!
//! Performance-critical: every public `render*` method uses `queue!` (buffered)
//! instead of `execute!` (immediate flush) and writes into a `BufWriter<Stdout>`.
//! A single `self.stdout.flush()` at the end of each public entry-point ensures
//! the entire frame reaches the terminal in one write-syscall batch.
use crossterm::{
    cursor,
    event::{EnableMouseCapture, DisableMouseCapture},
    execute, queue,
    style::{Attribute, Color, SetForegroundColor, SetBackgroundColor, ResetColor, SetAttribute},
    terminal,
};
use unicode_width::UnicodeWidthChar;
use std::io::{self, Write, Stdout, BufWriter};
use std::path::Path;
use std::time::Instant;

use crate::ui::perf_log::FrameTimer;

use crate::app::{AiStatus, FocusZone, ThemePicker};
use crate::mode::cmd_completion::CmdCompletionState;
use crate::config::Config;
use crate::editor::Editor;
use crate::locale::Locale;
use crate::mode::{Mode, VisualKind};
use crate::syntax::highlight::{FileType, Highlighter, SyntectSpan, OverlayKind, CodePalette};
use crate::syntax::TsHighlighter;
use crate::ui::chatpanel::{ChatPanel, ChatRole};
use crate::ui::filetree::FileTree;
use crate::ui::ghost::GhostText;
use crate::ui::mdrender::{MdRenderer, MdLine, MdTheme};
use crate::ui::tutorial::{TutorialBoard, tutorial_content};

pub struct Renderer {
    pub stdout: BufWriter<Stdout>,
    /// Legacy rule-based highlighter — kept only for search-match / visual-block
    /// overlay spans that are merged on top of tree-sitter output.
    pub highlighter: Highlighter,
    /// Tree-sitter incremental highlighter for the editor text area.
    pub ts_hl: TsHighlighter,
    pub md_renderer: MdRenderer,
    /// Cached full-buffer string to avoid O(n) `rope.to_string()` every frame.
    /// Invalidated when `source_generation` differs from `buffer.generation`.
    source_cache: String,
    source_generation: u64,
    /// Whether mouse mode is active (mouse events are processed normally).
    /// When false, mouse events trigger a "drop the mouse" reminder.
    /// Always true for mouse capture at the terminal level — we capture
    /// mouse events regardless so we can show the reminder.
    pub mouse_enabled: bool,
}

impl Renderer {
    /// Create a renderer driven by the user's `~/.hirc` theme configuration.
    pub fn new(filetype: FileType, config: &Config) -> Self {
        let chat_theme = MdTheme::by_name(&config.theme.chat_theme);
        let palette = CodePalette::by_name(&config.theme.chat_theme)
            .unwrap_or_else(CodePalette::neon_minimalist);
        Self {
            stdout: BufWriter::new(io::stdout()),
            highlighter: Highlighter::new(filetype),
            ts_hl: TsHighlighter::new(filetype, palette.clone()),
            md_renderer: MdRenderer::new_with_palette(chat_theme, palette),
            source_cache: String::new(),
            source_generation: u64::MAX, // force refresh on first frame
            mouse_enabled: config.general.mouse,
        }
    }

    pub fn set_filetype(&mut self, ft: FileType) {
        self.highlighter = Highlighter::new(ft);
        self.ts_hl.set_filetype(ft);
    }

    /// Switch both the editor syntax palette and the Markdown chat theme at
    /// runtime.  Called by `:theme <name>`.
    pub fn set_theme(&mut self, name: &str) {
        if let Some(p) = CodePalette::by_name(name) {
            self.ts_hl.set_palette(p.clone());
            self.md_renderer.palette = p;
        }
        self.md_renderer.theme = MdTheme::by_name(name);
    }

    pub fn init(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        // Always capture mouse events so we can show the "drop the mouse"
        // reminder when mouse mode is off.
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

    /// Render the welcome/splash screen when no file is opened.
    pub fn render_welcome(&mut self, w: usize, h: usize) -> io::Result<()> {
        let bg = Color::Rgb { r: 24, g: 24, b: 30 };
        let logo_color = Color::Rgb { r: 120, g: 180, b: 255 };
        let accent = Color::Rgb { r: 80, g: 140, b: 220 };
        let dim = Color::Rgb { r: 90, g: 90, b: 110 };
        let hint_color = Color::Rgb { r: 140, g: 140, b: 160 };
        let version_color = Color::Rgb { r: 100, g: 200, b: 160 };

        // ASCII art logo
        let logo: &[&str] = &[
            r"  ██╗  ██╗ ██╗",
            r"  ██║  ██║ ██║",
            r"  ███████║ ██║",
            r"  ██╔══██║ ██║",
            r"  ██║  ██║ ██║",
            r"  ╚═╝  ╚═╝ ╚═╝",
        ];

        let tagline = "A terminal editor for the AI era.";
        let version = concat!("v", env!("CARGO_PKG_VERSION"));
        let hints: &[&str] = &[
            "",
            "Press : to enter command mode",
            "Press q to quit",
            "",
            "hi <file>       Open a file",
            "hi .            Open file tree",
            ":e <file>       Open file from command mode",
            "Ctrl+P          Fuzzy file picker",
            "?               Ask AI anything",
        ];

        // Clear entire screen
        for row in 0..h {
            queue!(self.stdout, cursor::MoveTo(0, row as u16), SetBackgroundColor(bg))?;
            write!(self.stdout, "{:width$}", "", width = w)?;
        }

        // Calculate vertical center
        let total_block = logo.len() + 2 + hints.len() + 2; // logo + gap + tagline + gap + hints
        let start_y = h.saturating_sub(total_block) / 2;

        // Draw logo centered
        for (i, line) in logo.iter().enumerate() {
            let y = start_y + i;
            if y >= h { break; }
            let dw = display_width_str(line);
            let pad = w.saturating_sub(dw) / 2;
            queue!(self.stdout,
                cursor::MoveTo(0, y as u16),
                SetBackgroundColor(bg),
                SetForegroundColor(logo_color),
            )?;
            write!(self.stdout, "{:pad$}{}", "", line, pad = pad)?;
            // Fill rest of line
            let used = pad + dw;
            if used < w {
                write!(self.stdout, "{:width$}", "", width = w - used)?;
            }
        }

        // Tagline + version
        let tag_y = start_y + logo.len() + 1;
        if tag_y < h {
            let tag_with_ver = format!("{}  {}", tagline, version);
            let dw = display_width_str(&tag_with_ver);
            let pad = w.saturating_sub(dw) / 2;
            queue!(self.stdout,
                cursor::MoveTo(0, tag_y as u16),
                SetBackgroundColor(bg),
                SetForegroundColor(accent),
            )?;
            write!(self.stdout, "{:pad$}{}", "", tagline, pad = pad)?;
            queue!(self.stdout, SetForegroundColor(version_color))?;
            write!(self.stdout, "  {}", version)?;
            let used = pad + dw;
            if used < w {
                write!(self.stdout, "{:width$}", "", width = w - used)?;
            }
        }

        // Separator
        let sep_y = tag_y + 1;
        if sep_y < h {
            let sep = "─".repeat(36.min(w));
            let dw = display_width_str(&sep);
            let pad = w.saturating_sub(dw) / 2;
            queue!(self.stdout,
                cursor::MoveTo(0, sep_y as u16),
                SetBackgroundColor(bg),
                SetForegroundColor(dim),
            )?;
            write!(self.stdout, "{:pad$}{}", "", sep, pad = pad)?;
            let used = pad + dw;
            if used < w {
                write!(self.stdout, "{:width$}", "", width = w - used)?;
            }
        }

        // Hints
        let hints_start = sep_y + 1;
        for (i, line) in hints.iter().enumerate() {
            let y = hints_start + i;
            if y >= h { break; }
            let dw = display_width_str(line);
            let pad = w.saturating_sub(dw) / 2;
            queue!(self.stdout,
                cursor::MoveTo(0, y as u16),
                SetBackgroundColor(bg),
                SetForegroundColor(hint_color),
            )?;
            write!(self.stdout, "{:pad$}{}", "", line, pad = pad)?;
            let used = pad + dw;
            if used < w {
                write!(self.stdout, "{:width$}", "", width = w - used)?;
            }
        }

        // Hide cursor on welcome screen
        queue!(self.stdout, cursor::Hide)?;
        self.stdout.flush()?;
        Ok(())
    }

    pub fn render(
        &mut self,
        editor: &mut Editor,
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
        locale: &Locale,
        tutorial_board: &TutorialBoard,
        tutorial_visible: bool,
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
        let tut_width = if tutorial_visible { 32usize } else { 0 };

        // ── Perf: start frame timer ──────────────────────
        let mut perf = FrameTimer::start();

        queue!(self.stdout,
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
                    queue!(self.stdout, cursor::MoveTo(0, row as u16))?;
                    write!(self.stdout, "{:width$}", "", width = ft_width)?;
                }
            }
        }

        // ── Editing area ──────────────────────────────────
        let edit_x = ft_width + if ft_width > 0 { 1 } else { 0 };
        let chat_total = chat_width + if chat_width > 0 { 1 } else { 0 }; // +1 for separator
        let tut_total = tut_width + if tut_width > 0 { 1 } else { 0 };   // +1 for separator
        let edit_w = w.saturating_sub(edit_x).saturating_sub(chat_total).saturating_sub(tut_total);
        let edit_h = h.saturating_sub(2);
        let gutter = if editor.config.general.line_numbers { editor.gutter_width() } else { 0 };

        let text_w = edit_w.saturating_sub(gutter);

        // Draw search highlights as a sorted list of (line,col) pairs
        let search_set: std::collections::HashSet<(usize,usize)> = editor.search_matches.iter().cloned().collect();
        let current_match = editor.search_matches.get(editor.search_match_idx).cloned();

        // ── Tree-sitter: incremental parse + viewport highlight ──────────
        // 1. source_cache is refreshed only when buffer.generation changes —
        //    O(n) rope.to_string() is skipped on frames with no edits.
        // 2. incremental_parse() re-uses the old TSTree; only dirty subtrees
        //    are re-parsed (O(changed_bytes × log n)).
        // 3. highlight_viewport() queries only the visible byte range —
        //    O(tokens in viewport), regardless of scroll position.
        let preparse_start = Instant::now();
        if editor.buffer.generation != self.source_generation {
            // Drain pending InputEdits into the tree-sitter tree so that
            // incremental_parse() only re-parses the dirty subtrees.
            // If pending_edits is empty (e.g. after undo/redo) we fall back
            // to a full re-parse via needs_full_parse flag.
            let edits: Vec<_> = editor.buffer.pending_edits.drain(..).collect();
            if edits.is_empty() {
                // undo/redo or reload: force full re-parse
                self.ts_hl.force_full_parse();
            } else {
                for ei in &edits {
                    self.ts_hl.edit(
                        ei.start_byte, ei.old_end_byte, ei.new_end_byte,
                        ei.start_row,  ei.start_col,
                        ei.old_end_row, ei.old_end_col,
                        ei.new_end_row, ei.new_end_col,
                    );
                }
            }
            self.source_cache = editor.buffer.rope.to_string();
            self.source_generation = editor.buffer.generation;
        }
        let source = &self.source_cache;
        self.ts_hl.incremental_parse(source);
        let viewport_spans = self.ts_hl.highlight_viewport(
            &source,
            editor.scroll_line,
            editor.scroll_line + edit_h,
        );
        // Build a line→spans lookup for O(1) access in the render loop below.
        let mut viewport_map: std::collections::HashMap<usize, Vec<SyntectSpan>> =
            viewport_spans.into_iter().collect();
        if let Some(ref mut p) = perf {
            p.set_preparse(preparse_start.elapsed(), edit_h);
        }

        // Ensure no colour state leaks from previous frame's status bar or
        // overlays into the editing area.
        queue!(self.stdout, ResetColor, SetAttribute(Attribute::Reset))?;

        let lines_start = Instant::now();
        for screen_row in 0..edit_h {
            let buf_line = editor.scroll_line + screen_row;
            queue!(self.stdout, cursor::MoveTo(edit_x as u16, screen_row as u16))?;

            // Draw gutter
            if editor.config.general.line_numbers {
                if buf_line < editor.buffer.line_count() {
                    let lnum = format!("{:>width$} ", buf_line + 1, width = gutter - 1);
                    queue!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
                    write!(self.stdout, "{}", lnum)?;
                    queue!(self.stdout, ResetColor)?;
                } else {
                    let blank = " ".repeat(gutter);
                    write!(self.stdout, "{}", blank)?;
                }
            }

            // Draw text
            if buf_line < editor.buffer.line_count() {
                let line = editor.buffer.line_str(buf_line);

                let mut spans: Vec<SyntectSpan> = if editor.search_highlight && !editor.search_pattern.is_empty() {
                    // Merge tree-sitter spans + search highlight overlays
                    self.spans_with_search_ts(
                        &line, buf_line, &search_set, current_match,
                        viewport_map.remove(&buf_line).unwrap_or_default(),
                    )
                } else {
                    viewport_map.remove(&buf_line).unwrap_or_default()
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

                // Visual Char highlight: highlight from anchor to cursor (inclusive)
                if let Mode::Visual { kind: VisualKind::Char, anchor } = &editor.mode {
                    let cursor_idx = editor.cursor_char_idx();
                    let (sel_start, sel_end) = if *anchor <= cursor_idx {
                        (*anchor, cursor_idx + 1)
                    } else {
                        (cursor_idx, anchor + 1)
                    };
                    let line_start = editor.buffer.line_to_char(buf_line);
                    let line_end   = line_start + editor.buffer.line_len(buf_line);
                    // Clamp selection to this line's byte range
                    if sel_start < line_end && sel_end > line_start {
                        let char_s = sel_start.saturating_sub(line_start);
                        let char_e = (sel_end - line_start).min(editor.buffer.line_len(buf_line));
                        let chars: Vec<char> = line.chars().collect();
                        let byte_s: usize = chars[..char_s.min(chars.len())].iter().map(|c| c.len_utf8()).sum();
                        let byte_e: usize = chars[..char_e.min(chars.len())].iter().map(|c| c.len_utf8()).sum();
                        if byte_s < byte_e {
                            spans.push(SyntectSpan {
                                start: byte_s,
                                end:   byte_e,
                                fg: Color::White,
                                bold: false,
                                italic: false,
                                overlay: Some(OverlayKind::VisualChar),
                            });
                        }
                    }
                }

                // Visual Line highlight: highlight every character on selected lines
                if let Mode::Visual { kind: VisualKind::Line, anchor } = &editor.mode {
                    let anchor_line = editor.buffer.char_to_line(*anchor);
                    let cursor_line = editor.cursor_line;
                    let (sel_start_line, sel_end_line) = if cursor_line <= anchor_line {
                        (cursor_line, anchor_line)
                    } else {
                        (anchor_line, cursor_line)
                    };
                    if buf_line >= sel_start_line && buf_line <= sel_end_line {
                        let line_len_bytes: usize = line.len();
                        if line_len_bytes > 0 {
                            spans.push(SyntectSpan {
                                start: 0,
                                end:   line_len_bytes,
                                fg: Color::White,
                                bold: false,
                                italic: false,
                                overlay: Some(OverlayKind::VisualLine),
                            });
                        }
                    }
                }

                self.render_line_with_spans(&line, &spans, text_w, buf_line, editor)?;
            } else {
                // Empty rows past EOF
                queue!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
                write!(self.stdout, "~")?;
                queue!(self.stdout, ResetColor)?;
                let padding = edit_w.saturating_sub(gutter + 1);
                write!(self.stdout, "{:padding$}", "", padding = padding)?;
            }
        }

        if let Some(ref mut p) = perf {
            p.set_lines(lines_start.elapsed(), edit_h);
        }

        let overlays_start = Instant::now();
        // ── Separator between file tree and edit area ─────
        if ft_width > 0 {
            queue!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
            for row in 0..edit_h {
                queue!(self.stdout, cursor::MoveTo(ft_width as u16, row as u16))?;
                write!(self.stdout, "│")?;
            }
            queue!(self.stdout, ResetColor)?;
        }

        // ── Tutorial board (right side, left of chat) ────
        if tutorial_visible && tut_width > 0 {
            // Tutorial sits to the left of chat (or at the right edge if no chat)
            let tut_x = w.saturating_sub(chat_total).saturating_sub(tut_width);
            let tut_sep_x = tut_x.saturating_sub(1);
            // Separator
            queue!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
            for row in 0..edit_h {
                queue!(self.stdout, cursor::MoveTo(tut_sep_x as u16, row as u16))?;
                write!(self.stdout, "│")?;
            }
            queue!(self.stdout, ResetColor)?;
            self.render_tutorial_panel(tutorial_board, tut_x, tut_width, edit_h, locale)?;
        }

        // ── Chat panel (right side) ─────────────────────
        if chat_visible && chat_width > 0 {
            let chat_x = w.saturating_sub(chat_width);
            let sep_x = chat_x.saturating_sub(1);
            // Separator
            queue!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
            for row in 0..edit_h {
                queue!(self.stdout, cursor::MoveTo(sep_x as u16, row as u16))?;
                write!(self.stdout, "│")?;
            }
            queue!(self.stdout, ResetColor)?;
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

        queue!(self.stdout, cursor::MoveTo(0, hint_row))?;
        if ai_pending {
            // Animated spinner while AI is working
            let spinner_frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let frame = spinner_frames[(ai_tick as usize) % spinner_frames.len()];
            let spinner_msg = format!("{} AI 思考中...", frame);
            queue!(self.stdout, SetForegroundColor(Color::Cyan))?;
            let spinner_trunc = truncate(&spinner_msg, w);
            let spinner_dw = display_width_str(&spinner_trunc);
            write!(self.stdout, "{}", spinner_trunc)?;
            if spinner_dw < w {
                write!(self.stdout, "{:padding$}", "", padding = w - spinner_dw)?;
            }
            queue!(self.stdout, ResetColor)?;
        } else {
            queue!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
            let hint = if let Some(msg) = ai_query_msg {
                // AI query result displayed in hint line
                truncate(msg, w)
            } else if ghost.visible {
                format!("[Tab]确认执行  [Esc]取消  {}", ghost.explanation)
            } else {
                let base = editor.hint_line(locale, self.highlighter.filetype());
                // When multiple panels are visible, append the zone-switch hint
                // so the user always knows how to navigate between areas.
                let multi_panel = (editor.filetree_visible && !editor.filetree_focus)
                    || (chat_visible && focus != FocusZone::Chat)
                    || tutorial_visible;
                if multi_panel && !editor.filetree_focus {
                    format!("{}  {}", base, locale.ui.hint_switch_zone)
                } else {
                    base
                }
            };
            let hint_trunc = truncate(&hint, w);
            let hint_dw = display_width_str(&hint_trunc);
            write!(self.stdout, "{}", hint_trunc)?;
            if hint_dw < w {
                write!(self.stdout, "{:padding$}", "", padding = w - hint_dw)?;
            }
            queue!(self.stdout, ResetColor)?;
        }

        queue!(self.stdout, cursor::MoveTo(0, info_row))?;
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
            queue!(self.stdout, cursor::MoveTo((w.saturating_sub(ghost_dw.min(w))) as u16, info_row))?;
            queue!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
            write!(self.stdout, "{}", truncate(&ghost_str, w))?;
            queue!(self.stdout, ResetColor)?;
        }

        // ── Command / Search / AI input line ──────────────
        match &editor.mode {
            Mode::Command(s) => {
                queue!(self.stdout, cursor::MoveTo(0, info_row))?;
                queue!(self.stdout, SetForegroundColor(Color::White))?;
                let s_trunc = truncate(s, w.saturating_sub(1));
                let s_dw = display_width_str(&s_trunc) + 1; // +1 for ':'
                write!(self.stdout, ":{}", s_trunc)?;
                if s_dw < w {
                    write!(self.stdout, "{:padding$}", "", padding = w - s_dw)?;
                }
                queue!(self.stdout, ResetColor)?;
            }
            Mode::Search(s) => {
                queue!(self.stdout, cursor::MoveTo(0, info_row))?;
                queue!(self.stdout, SetForegroundColor(Color::White))?;
                let s_trunc = truncate(s, w.saturating_sub(1));
                let s_dw = display_width_str(&s_trunc) + 1; // +1 for '/'
                write!(self.stdout, "/{}", s_trunc)?;
                if s_dw < w {
                    write!(self.stdout, "{:padding$}", "", padding = w - s_dw)?;
                }
                queue!(self.stdout, ResetColor)?;
            }
            Mode::Ai(s) => {
                queue!(self.stdout, cursor::MoveTo(0, info_row))?;
                queue!(self.stdout, SetForegroundColor(Color::Cyan))?;
                let s_trunc = truncate(s, w.saturating_sub(1));
                let s_dw = display_width_str(&s_trunc) + 1; // +1 for '?'
                write!(self.stdout, "?{}", s_trunc)?;
                if s_dw < w {
                    write!(self.stdout, "{:padding$}", "", padding = w - s_dw)?;
                }
                queue!(self.stdout, ResetColor)?;
            }
            _ => {}
        }

        // ── Command completion popup ──────────────────────
        if editor.mode.is_command() && cmd_completion.visible() {
            self.render_cmd_completion(cmd_completion, w, hint_row)?;
        }

        // ── File tree prompt overlay ──────────────────────
        // Search prompt is rendered inside the file tree panel itself, skip it here.
        if let Some(prompt) = filetree_prompt {
            if !matches!(prompt, crate::app::FileTreePrompt::Search { .. }) {
                let label = prompt.label();
                let input = match prompt {
                    crate::app::FileTreePrompt::NewFile  { input } => input.as_str(),
                    crate::app::FileTreePrompt::NewDir   { input } => input.as_str(),
                    crate::app::FileTreePrompt::Rename   { input, .. } => input.as_str(),
                    crate::app::FileTreePrompt::Delete   { path, .. } => {
                        // Show path in hint line
                        let path_str = path.to_string_lossy();
                        queue!(self.stdout, cursor::MoveTo(0, hint_row))?;
                        queue!(self.stdout, SetForegroundColor(Color::Yellow))?;
                        write!(self.stdout, "{}{}  {:<width$}", label, path_str, "", width = w.saturating_sub(label.len() + path_str.len() + 2))?;
                        queue!(self.stdout, ResetColor)?;
                        // Overwrite info row with prompt
                        queue!(self.stdout, cursor::MoveTo(0, info_row))?;
                        queue!(self.stdout, SetBackgroundColor(Color::DarkYellow), SetForegroundColor(Color::Black))?;
                        write!(self.stdout, "{:<width$}", "按 y 确认删除，n 取消", width = w)?;
                        queue!(self.stdout, ResetColor)?;
                        // Flush before early return
                        self.stdout.flush()?;
                        return Ok(());
                    }
                    crate::app::FileTreePrompt::Search { .. } => unreachable!(),
                };
                queue!(self.stdout, cursor::MoveTo(0, info_row))?;
                queue!(self.stdout, SetBackgroundColor(Color::DarkGreen), SetForegroundColor(Color::Black))?;
                write!(self.stdout, "{}{:<width$}", label, input, width = w.saturating_sub(label.len()))?;
                queue!(self.stdout, ResetColor)?;
                // Show cursor at end of input
                let cursor_x = (display_width_str(label) + display_width_str(input)).min(w.saturating_sub(1));
                queue!(self.stdout, cursor::MoveTo(cursor_x as u16, info_row), cursor::Show)?;
            }
        }

        if let Some(ref mut p) = perf {
            p.set_overlays(overlays_start.elapsed());
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
            queue!(self.stdout,
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
                        queue!(self.stdout,
                            cursor::Show,
                            cursor::MoveTo(x as u16, vis_line as u16),
                        )?;
                        // Block vs beam
                        if editor.mode.is_insert() {
                            queue!(self.stdout, cursor::SetCursorStyle::BlinkingBar)?;
                        } else {
                            queue!(self.stdout, cursor::SetCursorStyle::SteadyBlock)?;
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
                    queue!(self.stdout,
                        cursor::Show,
                        cursor::MoveTo(input_len as u16, info_row),
                        cursor::SetCursorStyle::BlinkingBar,
                    )?;
                }
            }
        }

        // ── Single flush for the entire frame ─────────────
        let flush_start = Instant::now();
        self.stdout.flush()?;
        if let Some(mut p) = perf.take() {
            p.set_flush(flush_start.elapsed(), 0);
            p.set_viewport(editor.scroll_line, edit_h, editor.buffer.line_count());
            p.finish();
        }
        Ok(())
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
                queue!(self.stdout, ResetColor, SetAttribute(Attribute::Reset))?;
                if let Some(idx) = cur_idx {
                    let sp = &spans[idx];
                    if let Some(ov) = sp.overlay {
                        // Overlay: use overlay bg, optionally override fg
                        queue!(self.stdout, SetBackgroundColor(ov.bg_color()))?;
                        if let Some(fg) = ov.fg_color() {
                            queue!(self.stdout, SetForegroundColor(fg))?;
                        } else {
                            queue!(self.stdout, SetForegroundColor(sp.fg))?;
                        }
                    } else {
                        queue!(self.stdout, SetForegroundColor(sp.fg))?;
                    }
                    if sp.bold   { queue!(self.stdout, SetAttribute(Attribute::Bold))?; }
                    if sp.italic { queue!(self.stdout, SetAttribute(Attribute::Italic))?; }
                }
                last_idx = cur_idx;
            }

            write!(self.stdout, "{}", ch)?;
            byte_pos += ch_len;
            col += UnicodeWidthChar::width(*ch).unwrap_or(0);
        }

        queue!(self.stdout, ResetColor, SetAttribute(Attribute::Reset))?;
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

    /// Overlay search-match highlights on top of pre-computed tree-sitter spans.
    fn spans_with_search_ts(
        &self,
        line: &str,
        buf_line: usize,
        search_set: &std::collections::HashSet<(usize,usize)>,
        current_match: Option<(usize,usize)>,
        mut spans: Vec<SyntectSpan>,
    ) -> Vec<SyntectSpan> {
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
        // Reserve bottom row for search input if search is active
        let is_searching = ft.filter.is_some();
        let tree_height = if is_searching { height.saturating_sub(1) } else { height };

        // Scroll offset: keep cursor visible within tree_height
        let scroll = if ft.cursor >= tree_height {
            ft.cursor - tree_height + 1
        } else {
            0
        };

        // Get the filter query for highlighting
        let filter_query = ft.filter.as_deref().unwrap_or("");

        for row in 0..tree_height {
            queue!(self.stdout, cursor::MoveTo(0, row as u16))?;
            let vis_idx = row + scroll; // index into visible_indices
            if let Some(&real_idx) = ft.visible_indices.get(vis_idx) {
                let node = &ft.nodes[real_idx];
                let is_cursor = vis_idx == ft.cursor;
                if is_cursor && focused {
                    queue!(self.stdout, SetBackgroundColor(Color::Rgb { r: 30, g: 60, b: 90 }))?;
                }

                // Indent
                let indent = "  ".repeat(node.depth);
                let indent_dw = node.depth * 2;

                // Icon + color based on type
                let (icon, fg) = if node.is_dir {
                    let ic = if node.expanded { "▾ " } else { "▸ " };
                    (ic, Color::Rgb { r: 80, g: 180, b: 255 }) // blue for dirs
                } else {
                    filetree_file_style(&node.path)
                };

                // Write indent + icon with base color
                queue!(self.stdout, SetForegroundColor(fg))?;
                if node.is_dir {
                    queue!(self.stdout, SetAttribute(Attribute::Bold))?;
                }
                write!(self.stdout, "{}{}", indent, icon)?;

                // Write name with match highlighting
                let name_budget = width.saturating_sub(indent_dw + display_width_str(icon));
                let name_trunc = truncate(&node.name, name_budget);

                if !filter_query.is_empty() && !node.is_dir {
                    // Highlight matching substring
                    self.write_highlighted_name(&name_trunc, filter_query, fg)?;
                } else {
                    write!(self.stdout, "{}", name_trunc)?;
                }

                // Pad remaining width
                let used = indent_dw + display_width_str(icon) + display_width_str(&name_trunc);
                if used < width {
                    write!(self.stdout, "{:padding$}", "", padding = width - used)?;
                }

                queue!(self.stdout, SetAttribute(Attribute::Reset), ResetColor)?;
            } else {
                write!(self.stdout, "{:width$}", "", width = width)?;
            }
        }

        // Render search input bar at bottom
        if is_searching {
            let search_row = tree_height;
            queue!(self.stdout, cursor::MoveTo(0, search_row as u16))?;
            queue!(self.stdout, SetBackgroundColor(Color::Rgb { r: 40, g: 40, b: 50 }), SetForegroundColor(Color::Rgb { r: 255, g: 200, b: 80 }))?;
            let prompt_str = format!("/{}", filter_query);
            let prompt_trunc = truncate(&prompt_str, width);
            let prompt_dw = display_width_str(&prompt_trunc);
            write!(self.stdout, "{}", prompt_trunc)?;
            if prompt_dw < width {
                write!(self.stdout, "{:padding$}", "", padding = width - prompt_dw)?;
            }
            queue!(self.stdout, ResetColor)?;
        }

        Ok(())
    }

    /// Write a file name with the matching substring highlighted.
    fn write_highlighted_name(&mut self, name: &str, query: &str, base_fg: Color) -> io::Result<()> {
        let name_lower = name.to_lowercase();
        let query_lower = query.to_lowercase();
        let highlight_fg = Color::Rgb { r: 255, g: 100, b: 100 }; // bright red for match

        if let Some(start) = name_lower.find(&query_lower) {
            let end = start + query.len();
            // Ensure we split at char boundaries
            let (before, rest) = name.split_at(
                name.char_indices().nth(start).map(|(i, _)| i).unwrap_or(name.len())
            );
            let match_end_byte = rest.char_indices()
                .nth(end - start)
                .map(|(i, _)| i)
                .unwrap_or(rest.len());
            let (matched, after) = rest.split_at(match_end_byte);

            // Before match
            write!(self.stdout, "{}", before)?;
            // Matched part — highlighted
            queue!(self.stdout, SetForegroundColor(highlight_fg), SetAttribute(Attribute::Bold))?;
            write!(self.stdout, "{}", matched)?;
            // After match — restore base color
            queue!(self.stdout, SetAttribute(Attribute::NoBold), SetForegroundColor(base_fg))?;
            write!(self.stdout, "{}", after)?;
        } else {
            write!(self.stdout, "{}", name)?;
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
        queue!(self.stdout, cursor::MoveTo(x as u16, 0))?;
        if focused {
            queue!(self.stdout, SetBackgroundColor(Color::DarkCyan), SetForegroundColor(Color::White))?;
        } else {
            queue!(self.stdout, SetBackgroundColor(Color::DarkGrey), SetForegroundColor(Color::White))?;
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
        queue!(self.stdout, ResetColor)?;

        // Content area — render styled MdLine spans
        let content_h = usable_h.saturating_sub(1);
        let visible_start = total.saturating_sub(content_h + panel.scroll);
        let visible_end = total.saturating_sub(panel.scroll);

        let visible: Vec<&(ChatRole, MdLine)> = all_lines[visible_start..visible_end].iter().collect();

        for row in 0..content_h {
            queue!(self.stdout, cursor::MoveTo(x as u16, (row + 1) as u16))?;
            if let Some((_role, md_line)) = visible.get(row) {
                // Render border (blockquote decoration)
                let mut col = 0usize;
                if let Some((ref border_str, border_color)) = md_line.border {
                    queue!(self.stdout, SetForegroundColor(border_color.clone()))?;
                    write!(self.stdout, " {}", border_str)?;
                    col += 1 + display_width_str(border_str);
                    queue!(self.stdout, ResetColor)?;
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
                        queue!(self.stdout, SetForegroundColor(fg))?;
                    }
                    if let Some(bg) = span.bg {
                        queue!(self.stdout, SetBackgroundColor(bg))?;
                    }
                    if span.bold {
                        queue!(self.stdout, SetAttribute(Attribute::Bold))?;
                    }
                    if span.italic {
                        queue!(self.stdout, SetAttribute(Attribute::Italic))?;
                    }
                    if span.underline {
                        queue!(self.stdout, SetAttribute(Attribute::Underlined))?;
                    }
                    if span.strikethrough {
                        queue!(self.stdout, SetAttribute(Attribute::CrossedOut))?;
                    }
                    if span.dim {
                        queue!(self.stdout, SetAttribute(Attribute::Dim))?;
                    }

                    write!(self.stdout, "{}", span_text)?;
                    queue!(self.stdout, ResetColor, SetAttribute(Attribute::Reset))?;
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
        queue!(self.stdout, cursor::MoveTo(x as u16, input_y))?;
        if chat_input_active {
            queue!(self.stdout, SetBackgroundColor(Color::DarkBlue), SetForegroundColor(Color::White))?;
            let prompt_str = format!("▶ {}", chat_input);
            let prompt_trunc = truncate(&prompt_str, width);
            let prompt_dw = display_width_str(&prompt_trunc);
            write!(self.stdout, "{}", prompt_trunc)?;
            if prompt_dw < width {
                write!(self.stdout, "{:padding$}", "", padding = width - prompt_dw)?;
            }
            queue!(self.stdout, ResetColor)?;
        } else if focused {
            queue!(self.stdout, SetForegroundColor(Color::DarkGrey))?;
            let hint = "[i]输入  [Esc]返回";
            let hint_trunc = truncate(hint, width);
            let hint_dw = display_width_str(&hint_trunc);
            write!(self.stdout, "{}", hint_trunc)?;
            if hint_dw < width {
                write!(self.stdout, "{:padding$}", "", padding = width - hint_dw)?;
            }
            queue!(self.stdout, ResetColor)?;
        } else {
            write!(self.stdout, "{:width$}", "", width = width)?;
        }

        Ok(())
    }

    fn render_tutorial_panel(
        &mut self,
        _board: &TutorialBoard,
        x: usize,
        width: usize,
        height: usize,
        locale: &Locale,
    ) -> io::Result<()> {
        // Detect language from locale
        let lang = if locale.messages.saved == "已保存" { "zh-CN" } else { "en-US" };
        let lines = tutorial_content(lang);
        let content_h = height.saturating_sub(1); // 1 row for title

        // Title bar
        queue!(self.stdout, cursor::MoveTo(x as u16, 0))?;
        queue!(self.stdout, SetBackgroundColor(Color::DarkYellow), SetForegroundColor(Color::Black))?;
        let title = if lang.starts_with("zh") { "教学板" } else { "Tutorial" };
        let title_trunc = truncate(title, width);
        let title_dw = display_width_str(&title_trunc);
        write!(self.stdout, "{}", title_trunc)?;
        if title_dw < width {
            write!(self.stdout, "{:padding$}", "", padding = width - title_dw)?;
        }
        queue!(self.stdout, ResetColor)?;

        // Content area
        for row in 0..content_h {
            queue!(self.stdout, cursor::MoveTo(x as u16, (row + 1) as u16))?;
            if let Some(line) = lines.get(row) {
                let mut col = 0usize;
                for span in &line.spans {
                    let avail = width.saturating_sub(col);
                    if avail == 0 { break; }
                    let span_text = truncate(&span.text, avail);
                    let span_dw = display_width_str(&span_text);

                    if let Some(fg) = span.fg {
                        queue!(self.stdout, SetForegroundColor(fg))?;
                    }
                    if span.bold {
                        queue!(self.stdout, SetAttribute(Attribute::Bold))?;
                    }
                    write!(self.stdout, "{}", span_text)?;
                    col += span_dw;
                    queue!(self.stdout, ResetColor)?;
                    if span.bold {
                        queue!(self.stdout, SetAttribute(Attribute::Reset))?;
                    }
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

        Ok(())
    }

    fn render_info_line(&mut self, info: &str, w: usize, editor: &Editor, ai_indicator: &str, ai_status: &AiStatus) -> io::Result<()> {
        let (mode_bg, mode_fg) = match &editor.mode {
            Mode::Normal       => (Color::Blue,    Color::White),
            Mode::Insert       => (Color::Green,   Color::Black),
            Mode::Visual { kind: VisualKind::Block, .. } => (Color::DarkMagenta, Color::White),
            Mode::Visual { .. } => (Color::Magenta, Color::White),
            Mode::Command(_)   => (Color::Yellow,  Color::Black),
            Mode::Ai(_)        => (Color::Cyan,    Color::Black),
            Mode::Search(_)    => (Color::Yellow,  Color::Black),
        };
        queue!(self.stdout,
            SetBackgroundColor(mode_bg),
            SetForegroundColor(mode_fg),
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
        queue!(self.stdout, SetForegroundColor(ai_fg))?;
        write!(self.stdout, "{}", ai_indicator)?;

        queue!(self.stdout, ResetColor)?;
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

        queue!(self.stdout, SetBackgroundColor(Color::DarkGrey), SetForegroundColor(Color::White))?;
        // Top border
        queue!(self.stdout, cursor::MoveTo(start_x as u16, start_y as u16))?;
        write!(self.stdout, "┌{}┐", "─".repeat(overlay_w.saturating_sub(2)))?;

        // Title
        queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 1) as u16))?;
        write!(self.stdout, "│{}│", center_to_dw("Shell 输出", overlay_w.saturating_sub(2)))?;

        let inner_w = overlay_w.saturating_sub(3);
        for (i, line) in visible.iter().enumerate() {
            queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 2 + i) as u16))?;
            let content = truncate(line, inner_w);
            write!(self.stdout, "│ {}│", pad_to_dw(&content, inner_w))?;
        }

        // Footer
        let footer_y = start_y + 2 + visible.len();
        queue!(self.stdout, cursor::MoveTo(start_x as u16, footer_y as u16))?;
        let hint = "[任意键关闭]";
        write!(self.stdout, "│{}│", center_to_dw(hint, overlay_w.saturating_sub(2)))?;

        // Bottom border
        queue!(self.stdout, cursor::MoveTo(start_x as u16, (footer_y + 1) as u16))?;
        write!(self.stdout, "└{}┘", "─".repeat(overlay_w.saturating_sub(2)))?;

        queue!(self.stdout, ResetColor)?;
        Ok(())
    }

    fn render_plan_overlay(&mut self, plan: &[String], w: usize, h: usize) -> io::Result<()> {
        let overlay_w = (w * 3 / 4).min(w.saturating_sub(4));
        let overlay_h = plan.len() + 4;
        let start_x = (w.saturating_sub(overlay_w)) / 2;
        let start_y = (h.saturating_sub(overlay_h)) / 2;

        queue!(self.stdout, SetBackgroundColor(Color::DarkBlue), SetForegroundColor(Color::White))?;
        // Top border
        queue!(self.stdout, cursor::MoveTo(start_x as u16, start_y as u16))?;
        write!(self.stdout, "┌{}┐", "─".repeat(overlay_w.saturating_sub(2)))?;

        // Title
        queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 1) as u16))?;
        write!(self.stdout, "│{}│", center_to_dw("AI 执行计划", overlay_w.saturating_sub(2)))?;

        let inner_w = overlay_w.saturating_sub(3); // "│ " + content + "│"
        for (i, line) in plan.iter().enumerate() {
            queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 2 + i) as u16))?;
            let content = truncate(line, inner_w);
            write!(self.stdout, "│ {}│", pad_to_dw(&content, inner_w))?;
        }

        // Footer
        let footer_y = start_y + 2 + plan.len();
        queue!(self.stdout, cursor::MoveTo(start_x as u16, footer_y as u16))?;
        let hint = "[y]确认执行  [n]取消  [e]编辑计划";
        let footer_inner = overlay_w.saturating_sub(2); // "│" + content + "│"
        write!(self.stdout, "│{}│", center_to_dw(hint, footer_inner))?;

        // Bottom border
        queue!(self.stdout, cursor::MoveTo(start_x as u16, (footer_y + 1) as u16))?;
        write!(self.stdout, "└{}┘", "─".repeat(overlay_w.saturating_sub(2)))?;

        queue!(self.stdout, ResetColor)?;
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
            .map(|c| display_width_str(&c.desc))
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

            queue!(self.stdout, cursor::MoveTo(0, row))?;

            if is_selected {
                queue!(self.stdout,
                    SetBackgroundColor(Color::Rgb { r: 68, g: 71, b: 90 }),
                    SetForegroundColor(Color::Rgb { r: 189, g: 147, b: 249 }),
                )?;
            } else {
                queue!(self.stdout,
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
                queue!(self.stdout, SetForegroundColor(Color::Rgb { r: 166, g: 227, b: 161 }))?;
            } else {
                queue!(self.stdout, SetForegroundColor(Color::Rgb { r: 108, g: 112, b: 134 }))?;
            }
            let desc_trunc = truncate(&item.desc, popup_w.saturating_sub(trigger_dw + gap + 1));
            write!(self.stdout, "{}", desc_trunc)?;

            // Pad to popup width
            let used = trigger_dw + gap + display_width_str(&desc_trunc);
            let pad = popup_w.saturating_sub(used);
            if pad > 0 {
                write!(self.stdout, "{:pad$}", "", pad = pad)?;
            }

            queue!(self.stdout, ResetColor)?;

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
        queue!(self.stdout, cursor::MoveTo(start_x as u16, start_y as u16))?;
        queue!(self.stdout, SetBackgroundColor(Color::Rgb { r: 30, g: 30, b: 46 }), SetForegroundColor(Color::Rgb { r: 180, g: 190, b: 254 }))?;
        write!(self.stdout, "┌{}┐", "─".repeat(inner_w))?;

        // Title — truncate to fit inside the box, then centre-pad
        let title = "选择主题 j/k Enter Esc";
        let title_trunc = truncate(title, inner_w.saturating_sub(2)); // leave 1 col padding each side
        let title_dw = display_width_str(&title_trunc);
        let pad_total = inner_w.saturating_sub(title_dw);
        let pad_left = pad_total / 2;
        let pad_right = pad_total - pad_left;
        queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 1) as u16))?;
        queue!(self.stdout, SetForegroundColor(Color::Rgb { r: 180, g: 190, b: 254 }))?;
        write!(self.stdout, "│{:pl$}{}{:pr$}│", "", title_trunc, "", pl = pad_left, pr = pad_right)?;

        // Separator
        queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 2) as u16))?;
        write!(self.stdout, "├{}┤", "─".repeat(inner_w))?;

        // Theme items
        for (i, theme_name) in picker.themes.iter().enumerate() {
            let row = start_y + 3 + i;
            queue!(self.stdout, cursor::MoveTo(start_x as u16, row as u16))?;

            if i == picker.cursor {
                // Selected item — highlighted
                queue!(self.stdout,
                    SetBackgroundColor(Color::Rgb { r: 88, g: 91, b: 112 }),
                    SetForegroundColor(Color::Rgb { r: 166, g: 227, b: 161 }),
                )?;
                let label = format!("  ● {}  ", theme_name);
                let label_trunc = truncate(&label, inner_w);
                let label_dw = display_width_str(&label_trunc);
                write!(self.stdout, "│{}{:pad$}│", label_trunc, "", pad = inner_w.saturating_sub(label_dw))?;
            } else {
                queue!(self.stdout,
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
        queue!(self.stdout, cursor::MoveTo(start_x as u16, bottom_y as u16))?;
        queue!(self.stdout,
            SetBackgroundColor(Color::Rgb { r: 30, g: 30, b: 46 }),
            SetForegroundColor(Color::Rgb { r: 180, g: 190, b: 254 }),
        )?;
        write!(self.stdout, "└{}┘", "─".repeat(inner_w))?;

        queue!(self.stdout, ResetColor)?;
        Ok(())
    }

    /// Render the fuzzy file picker overlay.
    pub fn render_file_picker(
        &mut self,
        picker: &crate::ui::picker::FilePicker,
        term_w: usize,
        term_h: usize,
    ) -> io::Result<()> {
        // Overlay dimensions
        let box_w = (term_w * 2 / 3).max(50).min(term_w.saturating_sub(4));
        let max_results = 12usize;
        let box_h = max_results + 4; // border + query line + separator + results + border
        let start_x = (term_w.saturating_sub(box_w)) / 2;
        let start_y = (term_h.saturating_sub(box_h)) / 3; // upper-third of screen
        let inner_w = box_w.saturating_sub(2);

        // Colour palette (Catppuccin Mocha-ish)
        let bg       = Color::Rgb { r: 24,  g: 24,  b: 37  };
        let border   = Color::Rgb { r: 137, g: 180, b: 250 }; // blue
        let fg_dim   = Color::Rgb { r: 108, g: 112, b: 134 };
        let fg_main  = Color::Rgb { r: 205, g: 214, b: 244 };
        let sel_bg   = Color::Rgb { r: 49,  g: 50,  b: 68  };
        let sel_fg   = Color::Rgb { r: 166, g: 227, b: 161 }; // green
        let match_fg = Color::Rgb { r: 250, g: 179, b: 135 }; // peach — matched chars

        // ── Top border ──
        queue!(self.stdout, cursor::MoveTo(start_x as u16, start_y as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        write!(self.stdout, "╭{}╮", "─".repeat(inner_w))?;

        // ── Query line ──
        queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 1) as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        write!(self.stdout, "│")?;
        queue!(self.stdout, SetForegroundColor(fg_dim))?;
        write!(self.stdout, " 🔍 ")?;
        queue!(self.stdout, SetForegroundColor(fg_main))?;
        let q_display = truncate(&picker.query, inner_w.saturating_sub(5));
        let q_dw = display_width_str(&q_display);
        write!(self.stdout, "{}{:pad$}", q_display, "", pad = inner_w.saturating_sub(4 + q_dw))?;
        queue!(self.stdout, SetForegroundColor(border))?;
        write!(self.stdout, "│")?;

        // ── Separator ──
        queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 2) as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        write!(self.stdout, "├{}┤", "─".repeat(inner_w))?;

        // ── Results ──
        let (window_start, window) = picker.visible_window(max_results);
        for (row_i, (file_idx, _score)) in window.iter().enumerate() {
            let abs_i = window_start + row_i;
            let is_selected = abs_i == picker.cursor;
            let path_str = picker.all_files[*file_idx].to_string_lossy().to_string();

            queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 3 + row_i) as u16))?;
            queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
            write!(self.stdout, "│")?;

            if is_selected {
                queue!(self.stdout, SetBackgroundColor(sel_bg))?;
            }

            // Prefix arrow for selected
            if is_selected {
                queue!(self.stdout, SetForegroundColor(sel_fg))?;
                write!(self.stdout, " ▶ ")?;
            } else {
                queue!(self.stdout, SetForegroundColor(fg_dim))?;
                write!(self.stdout, "   ")?;
            }

            // Render path with matched characters highlighted
            let q_lower = picker.query.to_lowercase();
            let path_lower = path_str.to_lowercase();
            let mut qi = 0usize;
            let q_chars: Vec<char> = q_lower.chars().collect();
            let mut rendered_w = 3usize; // prefix width
            let max_path_w = inner_w.saturating_sub(4); // 3 prefix + 1 right margin

            for ch in path_str.chars() {
                let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
                if rendered_w + cw > max_path_w + 3 { break; }
                let ch_lower = ch.to_lowercase().next().unwrap_or(ch);
                if qi < q_chars.len() && ch_lower == q_chars[qi] {
                    // Matched character
                    if is_selected {
                        queue!(self.stdout, SetForegroundColor(match_fg), SetBackgroundColor(sel_bg))?;
                    } else {
                        queue!(self.stdout, SetForegroundColor(match_fg), SetBackgroundColor(bg))?;
                    }
                    write!(self.stdout, "{}", ch)?;
                    qi += 1;
                } else {
                    if is_selected {
                        queue!(self.stdout, SetForegroundColor(fg_main), SetBackgroundColor(sel_bg))?;
                    } else {
                        queue!(self.stdout, SetForegroundColor(fg_main), SetBackgroundColor(bg))?;
                    }
                    write!(self.stdout, "{}", ch)?;
                }
                rendered_w += cw;
            }
            let _ = path_lower; // suppress unused warning

            // Pad to end of inner width
            let pad = inner_w.saturating_sub(rendered_w);
            if is_selected {
                queue!(self.stdout, SetBackgroundColor(sel_bg))?;
            } else {
                queue!(self.stdout, SetBackgroundColor(bg))?;
            }
            write!(self.stdout, "{:pad$}", "", pad = pad)?;

            queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
            write!(self.stdout, "│")?;
        }

        // Fill empty rows if fewer results than max_results
        for row_i in window.len()..max_results {
            queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 3 + row_i) as u16))?;
            queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
            write!(self.stdout, "│{:width$}│", "", width = inner_w)?;
        }

        // ── Bottom border with hint ──
        let bottom_y = start_y + 3 + max_results;
        queue!(self.stdout, cursor::MoveTo(start_x as u16, bottom_y as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        let total = picker.matches.len();
        let hint = format!(" ↑↓ navigate  Enter open  Esc cancel  ({} files) ", total);
        let hint_trunc = truncate(&hint, inner_w);
        let hint_dw = display_width_str(&hint_trunc);
        queue!(self.stdout, SetForegroundColor(fg_dim))?;
        write!(self.stdout, "╰")?;
        write!(self.stdout, "{}{:pad$}", hint_trunc, "", pad = inner_w.saturating_sub(hint_dw))?;
        queue!(self.stdout, SetForegroundColor(border))?;
        write!(self.stdout, "╯")?;

        // Position cursor inside the query box (after the prompt)
        let cursor_x = start_x + 4 + display_width_str(&q_display);
        queue!(self.stdout,
            cursor::MoveTo(cursor_x as u16, (start_y + 1) as u16),
            cursor::Show,
        )?;

        queue!(self.stdout, ResetColor)?;
        // Flush here since render_file_picker is a public entry-point
        self.stdout.flush()
    }

    /// Render the global grep panel overlay.
    pub fn render_grep_panel(
        &mut self,
        panel: &crate::ui::grep_panel::GrepPanel,
        term_w: usize,
        term_h: usize,
    ) -> io::Result<()> {
        let box_w = (term_w * 3 / 4).max(60).min(term_w.saturating_sub(4));
        let max_results = 14usize;
        let box_h = max_results + 5; // border + query + separator + results + status + border
        let start_x = (term_w.saturating_sub(box_w)) / 2;
        let start_y = (term_h.saturating_sub(box_h)) / 3;
        let inner_w = box_w.saturating_sub(2);

        // Catppuccin Mocha palette
        let bg       = Color::Rgb { r: 24,  g: 24,  b: 37  };
        let border   = Color::Rgb { r: 203, g: 166, b: 247 }; // mauve
        let fg_dim   = Color::Rgb { r: 108, g: 112, b: 134 };
        let fg_main  = Color::Rgb { r: 205, g: 214, b: 244 };
        let sel_bg   = Color::Rgb { r: 49,  g: 50,  b: 68  };
        let sel_fg   = Color::Rgb { r: 166, g: 227, b: 161 }; // green
        let match_fg = Color::Rgb { r: 250, g: 179, b: 135 }; // peach
        let lnum_fg  = Color::Rgb { r: 148, g: 226, b: 213 }; // teal
        let regex_fg = Color::Rgb { r: 249, g: 226, b: 175 }; // yellow

        // ── Top border ──
        queue!(self.stdout, cursor::MoveTo(start_x as u16, start_y as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        let title = if panel.is_regex { " 🔎 Grep (regex) " } else { " 🔎 Grep " };
        let title_dw = display_width_str(title);
        let dashes_left = (inner_w.saturating_sub(title_dw)) / 2;
        let dashes_right = inner_w.saturating_sub(title_dw + dashes_left);
        write!(self.stdout, "╭{}{}{}╮",
            "─".repeat(dashes_left), title, "─".repeat(dashes_right))?;

        // ── Query line ──
        queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 1) as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        write!(self.stdout, "│")?;
        queue!(self.stdout, SetForegroundColor(fg_dim))?;
        write!(self.stdout, " / ")?;
        let q_color = if panel.is_regex { regex_fg } else { fg_main };
        queue!(self.stdout, SetForegroundColor(q_color))?;
        let q_display = truncate(&panel.query, inner_w.saturating_sub(5));
        let q_dw = display_width_str(&q_display);
        write!(self.stdout, "{}{:pad$}", q_display, "", pad = inner_w.saturating_sub(3 + q_dw))?;
        queue!(self.stdout, SetForegroundColor(border))?;
        write!(self.stdout, "│")?;

        // ── Separator ──
        queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 2) as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        write!(self.stdout, "├{}┤", "─".repeat(inner_w))?;

        // ── Results ──
        let (window_start, window) = panel.visible_window(max_results);
        for (row_i, m) in window.iter().enumerate() {
            let abs_i = window_start + row_i;
            let is_selected = abs_i == panel.cursor;

            queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 3 + row_i) as u16))?;
            queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
            write!(self.stdout, "│")?;

            if is_selected {
                queue!(self.stdout, SetBackgroundColor(sel_bg))?;
            }

            // Selection arrow
            if is_selected {
                queue!(self.stdout, SetForegroundColor(sel_fg))?;
                write!(self.stdout, " ▶ ")?;
            } else {
                queue!(self.stdout, SetForegroundColor(fg_dim))?;
                write!(self.stdout, "   ")?;
            }

            // File name (basename only) + line number
            let fname = m.path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| m.path.to_string_lossy().to_string());
            let lnum_str = format!(":{}", m.line_no);
            let header = format!("{}{}", fname, lnum_str);
            let header_trunc = truncate(&header, (inner_w / 3).max(20));
            let header_dw = display_width_str(&header_trunc);

            queue!(self.stdout, SetForegroundColor(if is_selected { sel_fg } else { lnum_fg }))?;
            write!(self.stdout, "{}", header_trunc)?;
            queue!(self.stdout, SetForegroundColor(fg_dim))?;
            write!(self.stdout, " │ ")?;

            // Line text with match highlighted
            let sep_w = 3usize;
            let prefix_w = 3 + header_dw + sep_w;
            let avail = inner_w.saturating_sub(prefix_w + 1);
            let line_bg = if is_selected { sel_bg } else { bg };

            let mut rendered_w = 0usize;
            let mut byte_pos = 0usize;
            for ch in m.line_text.chars() {
                let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
                if rendered_w + cw > avail { break; }
                let in_match = byte_pos >= m.match_start && byte_pos < m.match_end;
                if in_match {
                    queue!(self.stdout, SetForegroundColor(match_fg), SetBackgroundColor(line_bg))?;
                } else {
                    queue!(self.stdout, SetForegroundColor(fg_main), SetBackgroundColor(line_bg))?;
                }
                write!(self.stdout, "{}", ch)?;
                rendered_w += cw;
                byte_pos += ch.len_utf8();
            }
            // Pad remainder
            queue!(self.stdout, SetBackgroundColor(line_bg))?;
            write!(self.stdout, "{:pad$}", "", pad = avail.saturating_sub(rendered_w))?;

            queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
            write!(self.stdout, "│")?;
        }

        // Fill empty rows
        for row_i in window.len()..max_results {
            queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 3 + row_i) as u16))?;
            queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
            write!(self.stdout, "│{:width$}│", "", width = inner_w)?;
        }

        // ── Status bar ──
        let status_y = start_y + 3 + max_results;
        queue!(self.stdout, cursor::MoveTo(start_x as u16, status_y as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        write!(self.stdout, "├{}┤", "─".repeat(inner_w))?;

        queue!(self.stdout, cursor::MoveTo(start_x as u16, (status_y + 1) as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        write!(self.stdout, "│")?;
        let status_msg = if !panel.searched {
            format!(" Enter to search  Esc cancel  (type query above)")
        } else {
            let total = panel.matches.len();
            if total == 0 {
                format!(" No matches found")
            } else {
                format!(" {}/{} matches  ↑↓ navigate  Enter jump  Esc cancel", panel.cursor + 1, total)
            }
        };
        let status_trunc = truncate(&status_msg, inner_w.saturating_sub(2));
        let status_dw = display_width_str(&status_trunc);
        queue!(self.stdout, SetForegroundColor(fg_dim))?;
        write!(self.stdout, "{}{:pad$}", status_trunc, "", pad = inner_w.saturating_sub(status_dw))?;
        queue!(self.stdout, SetForegroundColor(border))?;
        write!(self.stdout, "│")?;

        // ── Bottom border ──
        queue!(self.stdout, cursor::MoveTo(start_x as u16, (status_y + 2) as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        write!(self.stdout, "╰{}╯", "─".repeat(inner_w))?;

        // Cursor inside query box
        let cursor_x = start_x + 3 + display_width_str(&q_display);
        queue!(self.stdout,
            cursor::MoveTo(cursor_x as u16, (start_y + 1) as u16),
            cursor::Show,
        )?;

        queue!(self.stdout, ResetColor)?;
        // Flush here since render_grep_panel is a public entry-point
        self.stdout.flush()
    }

    /// Render the AI diff confirmation panel overlay.
    pub fn render_diff_panel(
        &mut self,
        panel: &crate::ui::diff_panel::DiffPanel,
        diff: &crate::ai::EditDiff,
        status_text: &str,
        term_w: usize,
        term_h: usize,
    ) -> io::Result<()> {
        use crate::ui::diff_panel::{all_display_lines, DiffLineKind};

        let box_w = (term_w * 4 / 5).max(70).min(term_w.saturating_sub(4));
        let max_content_rows = (term_h / 2).max(10).min(20);
        let box_h = max_content_rows + 5; // border + summary + separator + content + status + border
        let start_x = (term_w.saturating_sub(box_w)) / 2;
        let start_y = (term_h.saturating_sub(box_h)) / 4;
        let inner_w = box_w.saturating_sub(2);

        // Colour palette (Catppuccin Mocha)
        let bg       = Color::Rgb { r: 24,  g: 24,  b: 37  };
        let border   = Color::Rgb { r: 137, g: 180, b: 250 }; // blue
        let fg_dim   = Color::Rgb { r: 108, g: 112, b: 134 };
        let fg_main  = Color::Rgb { r: 205, g: 214, b: 244 };
        let red_fg   = Color::Rgb { r: 243, g: 139, b: 168 }; // red
        let green_fg = Color::Rgb { r: 166, g: 227, b: 161 }; // green
        let yellow_fg= Color::Rgb { r: 249, g: 226, b: 175 }; // yellow

        // ── Top border ──
        queue!(self.stdout, cursor::MoveTo(start_x as u16, start_y as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        let title = " ✦ AI Edit Diff ";
        let title_dw = display_width_str(title);
        let dashes_left = (inner_w.saturating_sub(title_dw)) / 2;
        let dashes_right = inner_w.saturating_sub(title_dw + dashes_left);
        write!(self.stdout, "╭{}{}{}╮",
            "─".repeat(dashes_left), title, "─".repeat(dashes_right))?;

        // ── Summary line ──
        queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 1) as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        write!(self.stdout, "│")?;
        queue!(self.stdout, SetForegroundColor(yellow_fg))?;
        let summary_trunc = truncate(&panel.summary, inner_w.saturating_sub(2));
        let summary_dw = display_width_str(&summary_trunc);
        write!(self.stdout, " {}{:pad$}", summary_trunc, "", pad = inner_w.saturating_sub(1 + summary_dw))?;
        queue!(self.stdout, SetForegroundColor(border))?;
        write!(self.stdout, "│")?;

        // ── Separator ──
        queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 2) as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        write!(self.stdout, "├{}┤", "─".repeat(inner_w))?;

        // ── Diff content ──
        let all_lines = all_display_lines(diff);
        let total_lines = all_lines.len();
        let scroll = panel.scroll.min(total_lines.saturating_sub(max_content_rows));
        let visible = &all_lines[scroll..total_lines.min(scroll + max_content_rows)];

        for (row_i, dl) in visible.iter().enumerate() {
            queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 3 + row_i) as u16))?;
            queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
            write!(self.stdout, "│")?;

            let (color, prefix) = match dl.kind {
                DiffLineKind::Header => (fg_dim, ""),
                DiffLineKind::Removed => (red_fg, ""),
                DiffLineKind::Added => (green_fg, ""),
                DiffLineKind::Hint => (yellow_fg, ""),
            };
            queue!(self.stdout, SetForegroundColor(color))?;
            let text_trunc = truncate(&dl.text, inner_w.saturating_sub(1));
            let text_dw = display_width_str(&text_trunc);
            write!(self.stdout, " {}{:pad$}", text_trunc, "", pad = inner_w.saturating_sub(1 + text_dw))?;
            let _ = prefix; // used for future prefix chars
            queue!(self.stdout, SetForegroundColor(border))?;
            write!(self.stdout, "│")?;
        }

        // Fill empty rows if content is shorter than max_content_rows
        for row_i in visible.len()..max_content_rows {
            queue!(self.stdout, cursor::MoveTo(start_x as u16, (start_y + 3 + row_i) as u16))?;
            queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
            write!(self.stdout, "│{:pad$}│", "", pad = inner_w)?;
        }

        // ── Status / hint line ──
        let status_y = start_y + 3 + max_content_rows;
        queue!(self.stdout, cursor::MoveTo(start_x as u16, status_y as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        write!(self.stdout, "├{}┤", "─".repeat(inner_w))?;

        queue!(self.stdout, cursor::MoveTo(start_x as u16, (status_y + 1) as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        write!(self.stdout, "│")?;
        queue!(self.stdout, SetForegroundColor(fg_main))?;
        let st_trunc = truncate(status_text, inner_w.saturating_sub(2));
        let st_dw = display_width_str(&st_trunc);
        write!(self.stdout, " {}{:pad$}", st_trunc, "", pad = inner_w.saturating_sub(1 + st_dw))?;
        queue!(self.stdout, SetForegroundColor(border))?;
        write!(self.stdout, "│")?;

        // ── Bottom border ──
        queue!(self.stdout, cursor::MoveTo(start_x as u16, (status_y + 2) as u16))?;
        queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(border))?;
        write!(self.stdout, "╰{}╯", "─".repeat(inner_w))?;

        // Hide cursor while diff panel is shown
        queue!(self.stdout, cursor::Hide)?;

        queue!(self.stdout, ResetColor)?;
        self.stdout.flush()
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

/// Word-wrap text to fit within `max_width` display columns.
/// Respects existing newlines and wraps long lines at word boundaries.
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut result = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            result.push(String::new());
            continue;
        }
        let line_dw = display_width_str(line);
        if line_dw <= max_width {
            result.push(line.to_string());
            continue;
        }
        // Need to wrap this line
        let mut current = String::new();
        let mut current_w = 0usize;
        for word in line.split_inclusive(|c: char| c == ' ' || c == ',' || c == '.' || c == ';') {
            let word_w = display_width_str(word);
            if current_w + word_w <= max_width {
                current.push_str(word);
                current_w += word_w;
            } else if current.is_empty() {
                // Single word wider than max_width — break by character
                for ch in word.chars() {
                    let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
                    if current_w + cw > max_width {
                        result.push(current.clone());
                        current.clear();
                        current_w = 0;
                    }
                    current.push(ch);
                    current_w += cw;
                }
            } else {
                result.push(current.clone());
                current = word.to_string();
                current_w = word_w;
            }
        }
        if !current.is_empty() {
            result.push(current);
        }
    }
    result
}

/// Pad `s` with trailing spaces so its display width equals `target_dw`.
/// Unlike `{:<width$}`, this accounts for CJK double-width characters.
fn pad_to_dw(s: &str, target_dw: usize) -> String {
    let dw = display_width_str(s);
    if dw >= target_dw {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(target_dw - dw))
    }
}

/// Truncate a string to fit within `max_cols` display-width columns.
/// Safe for multi-byte UTF-8 — never splits a character.
fn truncate_to_width(s: &str, max_cols: usize) -> &str {
    let mut col = 0;
    for (i, c) in s.char_indices() {
        let cw = UnicodeWidthChar::width(c).unwrap_or(0);
        if col + cw > max_cols {
            return &s[..i];
        }
        col += cw;
    }
    s
}

/// Returns (icon, foreground_color) for a file based on its extension.
fn filetree_file_style(path: &Path) -> (&'static str, Color) {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        // Rust
        "rs" => ("  ", Color::Rgb { r: 255, g: 140, b: 60 }),   // orange
        // Config / data
        "toml" | "yaml" | "yml" | "json" | "ini" | "cfg" =>
            ("  ", Color::Rgb { r: 220, g: 200, b: 80 }),        // yellow
        // Documentation
        "md" | "txt" | "rst" | "adoc" =>
            ("  ", Color::Rgb { r: 100, g: 200, b: 120 }),       // green
        // Shell / scripts
        "sh" | "bash" | "zsh" | "fish" =>
            ("  ", Color::Rgb { r: 180, g: 130, b: 255 }),       // purple
        // Python
        "py" | "pyi" =>
            ("  ", Color::Rgb { r: 80, g: 200, b: 180 }),        // teal
        // JavaScript / TypeScript / Web
        "js" | "mjs" | "cjs" =>
            ("  ", Color::Rgb { r: 240, g: 220, b: 80 }),        // JS yellow
        "ts" | "mts" | "cts" =>
            ("  ", Color::Rgb { r: 60, g: 160, b: 255 }),        // TS blue
        "jsx" | "tsx" =>
            ("  ", Color::Rgb { r: 100, g: 220, b: 240 }),       // React cyan
        "html" | "htm" =>
            ("  ", Color::Rgb { r: 255, g: 120, b: 60 }),        // HTML orange
        "css" | "scss" | "less" =>
            ("  ", Color::Rgb { r: 120, g: 140, b: 255 }),       // CSS blue-purple
        // Go
        "go" => ("  ", Color::Rgb { r: 80, g: 200, b: 220 }),   // Go cyan
        // Java / Kotlin
        "java" => ("  ", Color::Rgb { r: 255, g: 100, b: 80 }), // red
        "kt" | "kts" =>
            ("  ", Color::Rgb { r: 180, g: 120, b: 255 }),       // Kotlin purple
        // C / C++
        "c" | "h" =>
            ("  ", Color::Rgb { r: 100, g: 160, b: 255 }),       // C blue
        "cpp" | "cc" | "cxx" | "hpp" =>
            ("  ", Color::Rgb { r: 120, g: 140, b: 220 }),       // C++ blue
        // Lock / generated
        "lock" | "sum" =>
            ("  ", Color::Rgb { r: 120, g: 120, b: 120 }),       // dim gray
        // Images
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "ico" | "webp" =>
            ("  ", Color::Rgb { r: 200, g: 160, b: 255 }),       // light purple
        // Binary / archive
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" =>
            ("  ", Color::Rgb { r: 180, g: 140, b: 100 }),       // brown
        // Default
        _ => ("  ", Color::Rgb { r: 190, g: 190, b: 190 }),      // light gray
    }
}

/// Center `s` within `target_dw` columns using display-width-aware padding.
fn center_to_dw(s: &str, target_dw: usize) -> String {
    let dw = display_width_str(s);
    if dw >= target_dw {
        return s.to_string();
    }
    let total_pad = target_dw - dw;
    let left = total_pad / 2;
    let right = total_pad - left;
    format!("{}{}{}", " ".repeat(left), s, " ".repeat(right))
}

// ── LeetCode "古法时代" full-screen overlay renderer ─────────────────────────
#[cfg(feature = "leetcode")]
impl Renderer {
    /// Render the LeetCode panel as a full-screen overlay.
    pub fn render_leetcode_panel(
        &mut self,
        panel: &mut crate::leetcode::LeetCodePanel,
        w: usize,
        h: usize,
    ) -> io::Result<()> {
        use crate::leetcode::panel::{LeetCodeView, RetroColors};

        let bg = Color::Rgb { r: RetroColors::BG.0, g: RetroColors::BG.1, b: RetroColors::BG.2 };
        let green = Color::Rgb { r: RetroColors::GREEN.0, g: RetroColors::GREEN.1, b: RetroColors::GREEN.2 };
        let dim_green = Color::Rgb { r: RetroColors::DIM_GREEN.0, g: RetroColors::DIM_GREEN.1, b: RetroColors::DIM_GREEN.2 };
        let amber = Color::Rgb { r: RetroColors::AMBER.0, g: RetroColors::AMBER.1, b: RetroColors::AMBER.2 };
        let highlight_bg = Color::Rgb { r: RetroColors::HIGHLIGHT.0, g: RetroColors::HIGHLIGHT.1, b: RetroColors::HIGHLIGHT.2 };

        // Clear entire screen with retro background
        queue!(self.stdout, cursor::Hide, cursor::MoveTo(0, 0))?;
        for row in 0..h {
            queue!(self.stdout, cursor::MoveTo(0, row as u16), SetBackgroundColor(bg), SetForegroundColor(dim_green))?;
            write!(self.stdout, "{:width$}", "", width = w)?;
        }

        match panel.view {
            LeetCodeView::Splash => {
                // Use the user's chosen splash color for the logo
                let sc = panel.splash_color;
                let splash_fg = Color::Rgb { r: sc.fg().0, g: sc.fg().1, b: sc.fg().2 };
                let splash_dim = Color::Rgb { r: sc.dim().0, g: sc.dim().1, b: sc.dim().2 };
                let splash_accent = Color::Rgb { r: sc.accent().0, g: sc.accent().1, b: sc.accent().2 };

                // Draw splash lines centered vertically with per-line coloring
                let splash_lines = panel.splash_lines();
                let block_height = splash_lines.len() + 3; // lines + 2 gap + 1 status
                let start_y = h.saturating_sub(block_height) / 2;

                // Find max display width across all lines so they align as a block
                let max_logo_dw = splash_lines.iter()
                    .map(|(l, _)| display_width_str(l))
                    .max()
                    .unwrap_or(0);

                let splash_cool = Color::Rgb { r: sc.cool().0, g: sc.cool().1, b: sc.cool().2 };
                let splash_warm = Color::Rgb { r: sc.warm().0, g: sc.warm().1, b: sc.warm().2 };

                for (i, (line, zone)) in splash_lines.iter().enumerate() {
                    let y = start_y + i;
                    if y >= h { break; }
                    // Pick color based on zone tag
                    let fg_color = match zone {
                        'L' => splash_fg,      // Logo text — bright
                        'T' => splash_fg,      // Text — bright
                        'B' => splash_dim,     // Border — dim
                        'D' => splash_accent,  // Decoration — subtle accent
                        'C' => splash_cool,    // Cool — blue/cyan tones
                        'W' => splash_warm,    // Warm — mint/aqua tones
                        _   => splash_fg,
                    };
                    queue!(self.stdout, cursor::MoveTo(0, y as u16), SetBackgroundColor(bg), SetForegroundColor(fg_color))?;
                    // Pad each line to max_logo_dw (right-pad) so they form a uniform block
                    let line_dw = display_width_str(line);
                    let padded_line = if line_dw < max_logo_dw {
                        format!("{}{}", line, " ".repeat(max_logo_dw - line_dw))
                    } else {
                        line.to_string()
                    };
                    // Now center the uniform-width block within terminal width
                    let centered = center_to_dw(&padded_line, w);
                    let truncated = truncate_to_width(&centered, w);
                    write!(self.stdout, "{}", truncated)?;
                }

                // Bottom area: color hint (h-2) + command input or status (h-1)
                if h >= 3 {
                    use crate::leetcode::panel::SplashColor;

                    // h-2: Hint bar (always visible, uses splash color)
                    let colors_list = SplashColor::all()
                        .iter()
                        .map(|c| c.name())
                        .collect::<Vec<_>>()
                        .join("/");
                    let hint = format!(
                        ":color <{}>  |  :q quit  |  current: {}",
                        colors_list,
                        panel.splash_color.name()
                    );
                    queue!(self.stdout, cursor::MoveTo(0, (h - 2) as u16), SetBackgroundColor(bg), SetForegroundColor(splash_dim))?;
                    let hint_centered = center_to_dw(&hint, w);
                    let hint_truncated = truncate_to_width(&hint_centered, w);
                    write!(self.stdout, "{}", hint_truncated)?;

                    // h-1: Command input (when active) or status message
                    if panel.splash_cmd_active {
                        queue!(self.stdout, cursor::MoveTo(0, (h - 1) as u16), SetBackgroundColor(bg), SetForegroundColor(splash_fg))?;
                        let cmd_line = format!(":{}", &panel.splash_cmd_input);
                        // Show completion hint inline (dimmed)
                        let completion_hint = if !panel.cmd_completions.is_empty() {
                            let first = &panel.cmd_completions[0];
                            if first.starts_with(&panel.splash_cmd_input) && first.len() > panel.splash_cmd_input.len() {
                                first[panel.splash_cmd_input.len()..].to_string()
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };
                        write!(self.stdout, "{}", cmd_line)?;
                        // Draw ghost completion text
                        if !completion_hint.is_empty() {
                            queue!(self.stdout, SetForegroundColor(splash_dim))?;
                            write!(self.stdout, "{}", completion_hint)?;
                            queue!(self.stdout, SetForegroundColor(splash_fg))?;
                        }
                        // Pad rest of line
                        let used = display_width_str(&cmd_line) + display_width_str(&completion_hint);
                        if used < w {
                            write!(self.stdout, "{}", " ".repeat(w - used))?;
                        }
                        let cursor_x = display_width_str(&cmd_line);
                        queue!(self.stdout, cursor::Show, cursor::MoveTo(cursor_x as u16, (h - 1) as u16), cursor::SetCursorStyle::BlinkingBar)?;
                    } else {
                        queue!(self.stdout, cursor::MoveTo(0, (h - 1) as u16), SetBackgroundColor(bg), SetForegroundColor(amber))?;
                        let status = center_to_dw(&panel.status_msg, w);
                        let truncated = truncate_to_width(&status, w);
                        write!(self.stdout, "{}", truncated)?;
                    }
                }
            }
            LeetCodeView::ProblemList => {
                use crate::leetcode::panel::ListMode;

                // Header with filter indicators
                queue!(self.stdout, cursor::MoveTo(0, 0), SetBackgroundColor(highlight_bg), SetForegroundColor(green))?;
                // Build filter indicator string
                let diff_indicator = match panel.filter.difficulty {
                    Some(crate::leetcode::Difficulty::Easy) => "[1:Easy]",
                    Some(crate::leetcode::Difficulty::Medium) => "[2:Med]",
                    Some(crate::leetcode::Difficulty::Hard) => "[3:Hard]",
                    None => "[All]",
                };
                let status_indicator = match panel.filter.status {
                    Some(crate::leetcode::SolveStatus::Solved) => " [s:✓]",
                    Some(crate::leetcode::SolveStatus::Attempted) => " [s:○]",
                    Some(crate::leetcode::SolveStatus::NotStarted) => " [s:·]",
                    None => "",
                };
                let header = format!(
                    " ╔═ LeetCode 古法时代 ═╗  {} {}  {}/{}",
                    diff_indicator, status_indicator, panel.filtered.len(), panel.problems.len()
                );
                write!(self.stdout, "{:width$}", header, width = w)?;

                // Column header row — must align with data row:
                // data: " ▸ ✓ " (5 chars) + difficulty (8 chars) + rate (6 chars) + "  " + id.title
                queue!(self.stdout, cursor::MoveTo(0, 1), SetBackgroundColor(bg), SetForegroundColor(dim_green))?;
                let col_header = format!("     {:<8}{:>6}  {}", "DIFF", "RATE", "PROBLEM");
                write!(self.stdout, "{:width$}", col_header, width = w)?;

                // Separator
                queue!(self.stdout, cursor::MoveTo(0, 2), SetBackgroundColor(bg), SetForegroundColor(dim_green))?;
                let sep: String = "─".repeat(w);
                let truncated = truncate_to_width(&sep, w);
                write!(self.stdout, "{}", truncated)?;

                // Problem rows
                let visible = panel.visible_problems(h);
                let cursor_in_view = panel.cursor_in_view(h);

                for (i, problem) in visible.iter().enumerate() {
                    let y = i + 3; // after header(1) + col_header(1) + separator(1)
                    if y >= h.saturating_sub(2) { break; }

                    let is_selected = i == cursor_in_view;
                    let row_bg = if is_selected { highlight_bg } else { bg };
                    let diff_color = match problem.difficulty {
                        crate::leetcode::Difficulty::Easy => Color::Rgb { r: RetroColors::EASY.0, g: RetroColors::EASY.1, b: RetroColors::EASY.2 },
                        crate::leetcode::Difficulty::Medium => Color::Rgb { r: RetroColors::MEDIUM.0, g: RetroColors::MEDIUM.1, b: RetroColors::MEDIUM.2 },
                        crate::leetcode::Difficulty::Hard => Color::Rgb { r: RetroColors::HARD.0, g: RetroColors::HARD.1, b: RetroColors::HARD.2 },
                    };

                    queue!(self.stdout, cursor::MoveTo(0, y as u16), SetBackgroundColor(row_bg))?;

                    // Pointer + status icon
                    let pointer = if is_selected { "▸" } else { " " };
                    let status_icon = problem.status.icon();
                    queue!(self.stdout, SetForegroundColor(green))?;
                    write!(self.stdout, " {} {} ", pointer, status_icon)?;

                    // Difficulty (colored, 8 chars padded)
                    queue!(self.stdout, SetForegroundColor(diff_color))?;
                    write!(self.stdout, "{:<8}", problem.difficulty.label())?;

                    // Acceptance rate (dim green, 6 chars)
                    queue!(self.stdout, SetForegroundColor(dim_green))?;
                    write!(self.stdout, "{:>5.1}%", problem.acceptance)?;

                    // Problem number + title (green)
                    queue!(self.stdout, SetForegroundColor(green))?;
                    let title_w = w.saturating_sub(26); // 4(ptr+status) + 8(diff) + 6(rate) + 8(spacing)
                    let id_title = format!("  {:>4}. {}", problem.frontend_id, problem.title);
                    let id_title_display: String = id_title.chars().take(title_w).collect();
                    write!(self.stdout, "{:<width$}", id_title_display, width = title_w)?;
                }

                // Footer separator + status/search
                let footer_y = h.saturating_sub(2);
                queue!(self.stdout, cursor::MoveTo(0, footer_y as u16), SetBackgroundColor(bg), SetForegroundColor(dim_green))?;
                write!(self.stdout, "{:width$}", "─".repeat(w), width = w)?;

                queue!(self.stdout, cursor::MoveTo(0, h.saturating_sub(1) as u16), SetBackgroundColor(highlight_bg), SetForegroundColor(amber))?;
                match panel.list_mode {
                    ListMode::Search => {
                        let footer = format!(" /{}█  (Enter:confirm  Esc:cancel)", &panel.search_input);
                        write!(self.stdout, "{:width$}", footer, width = w)?;
                    }
                    ListMode::Command => {
                        // Show command with ghost completion
                        let cmd_part = format!(" :{}", &panel.search_input);
                        write!(self.stdout, "{}", cmd_part)?;
                        let ghost = if !panel.cmd_completions.is_empty() {
                            let first = &panel.cmd_completions[0];
                            if first.starts_with(panel.search_input.as_str()) && first.len() > panel.search_input.len() {
                                first[panel.search_input.len()..].to_string()
                            } else { String::new() }
                        } else { String::new() };
                        if !ghost.is_empty() {
                            queue!(self.stdout, SetForegroundColor(dim_green))?;
                            write!(self.stdout, "{}", ghost)?;
                            queue!(self.stdout, SetForegroundColor(amber))?;
                        }
                        let tail = "  (Tab:complete  Enter:exec  Esc:cancel)";
                        write!(self.stdout, "{}", tail)?;
                        let used = display_width_str(&cmd_part) + display_width_str(&ghost) + display_width_str(tail);
                        if used < w { write!(self.stdout, "{}", " ".repeat(w - used))?; }
                    }
                    ListMode::Normal => {
                        let footer = format!(" {} │ j/k:↕ ^d/^u:page /:search s:过滤 1-3:难度 0:clear m:图谱 :q:quit", &panel.status_msg);
                        write!(self.stdout, "{:width$}", footer, width = w)?;
                    }
                };
            }
            LeetCodeView::Coding => {
                use crate::leetcode::panel::CodingFocus;

                if let Some(ref mut coding) = panel.coding {
                    // ── Layout calculation ──
                    // Title bar: 1 row, Footer: 2 rows, Content area: h-3
                    let full_content_h = h.saturating_sub(3);
                    // Result panel takes bottom portion when shown
                    let result_panel_h = if coding.show_result_panel { (full_content_h / 3).max(5) } else { 0 };
                    let content_h = full_content_h.saturating_sub(result_panel_h);
                    // Horizontal split: description | editor | ai
                    let desc_w = if coding.show_description { w * 2 / 5 } else { 0 };
                    let ai_w = if coding.show_ai { w / 4 } else { 0 };
                    let editor_w = w.saturating_sub(desc_w).saturating_sub(ai_w);
                    let editor_x = desc_w;

                    // ── Title bar ──
                    queue!(self.stdout, cursor::MoveTo(0, 0), SetBackgroundColor(highlight_bg), SetForegroundColor(green))?;
                    let lang_name = coding.detail.code_snippets.get(coding.lang_index)
                        .map(|s| s.lang.as_str()).unwrap_or("?");
                    let mode_name = coding.editor.mode.name();
                    let title_bar = format!(
                        " #{} {} │ [{}] │ {} │ --{}--",
                        coding.detail.summary.frontend_id,
                        coding.detail.summary.title,
                        lang_name,
                        match coding.focus {
                            CodingFocus::Editor => "EDITOR",
                            CodingFocus::Description => "DESC",
                            CodingFocus::AiChat => "AI",
                            CodingFocus::Result => "RESULT",
                        },
                        mode_name,
                    );
                    write!(self.stdout, "{:width$}", title_bar, width = w)?;

                    // ── Description panel (left) with word wrap ──
                    if coding.show_description && desc_w > 2 {
                        let inner_w = desc_w.saturating_sub(1); // 1 for border
                        let is_focused = coding.focus == CodingFocus::Description;
                        let border_color = if is_focused { green } else { dim_green };

                        // Word-wrap the description text to fit inner_w
                        let wrapped_lines = wrap_text(&coding.detail.content_text, inner_w);

                        for row in 0..content_h {
                            let y = row + 1;
                            queue!(self.stdout, cursor::MoveTo(0, y as u16), SetBackgroundColor(bg), SetForegroundColor(border_color))?;
                            let line_idx = row + coding.desc_scroll;
                            if let Some(line) = wrapped_lines.get(line_idx) {
                                queue!(self.stdout, SetForegroundColor(if is_focused { green } else { dim_green }))?;
                                let dw = display_width_str(line);
                                write!(self.stdout, "{}", line)?;
                                if dw < inner_w {
                                    write!(self.stdout, "{}", " ".repeat(inner_w - dw))?;
                                }
                            } else {
                                write!(self.stdout, "{:width$}", "", width = inner_w)?;
                            }
                            // Vertical border
                            queue!(self.stdout, SetForegroundColor(dim_green))?;
                            write!(self.stdout, "│")?;
                        }
                    }

                    // ── Code editor (center) with syntax highlighting ──
                    {
                        // Drain pending edits into tree-sitter for proper incremental parsing
                        let edits: Vec<_> = coding.editor.buffer.pending_edits.drain(..).collect();
                        if !edits.is_empty() {
                            for ei in &edits {
                                coding.ts_hl.edit(
                                    ei.start_byte, ei.old_end_byte, ei.new_end_byte,
                                    ei.start_row,  ei.start_col,
                                    ei.old_end_row, ei.old_end_col,
                                    ei.new_end_row, ei.new_end_col,
                                );
                            }
                        }

                        let code_text = coding.editor.buffer.rope.to_string();
                        let code_lines: Vec<&str> = code_text.lines().collect();
                        let is_focused = coding.focus == CodingFocus::Editor;
                        let line_num_w = 4; // "  1 "
                        let code_inner_w = editor_w.saturating_sub(line_num_w + 1); // +1 for right border if ai panel

                        // Incremental parse for syntax highlighting
                        coding.ts_hl.incremental_parse(&code_text);
                        let viewport_end = (coding.editor.scroll_line + content_h).min(code_lines.len());
                        let viewport_spans = coding.ts_hl.highlight_viewport(
                            &code_text,
                            coding.editor.scroll_line,
                            viewport_end,
                        );
                        // Build line→spans lookup
                        let viewport_map: std::collections::HashMap<usize, Vec<SyntectSpan>> =
                            viewport_spans.into_iter().collect();

                        // Error line uses red background for highlighting
                    let error_line_bg = Color::Rgb { r: 80, g: 20, b: 20 };
                    let error_line_num_fg = Color::Rgb { r: 255, g: 80, b: 80 };

                    for row in 0..content_h {
                            let y = row + 1;
                            let line_idx = row + coding.editor.scroll_line;
                            // Check if this is the error line (1-based in coding.error_line)
                            let is_error_line = coding.error_line > 0 && (line_idx + 1) == coding.error_line;
                            let row_bg = if is_error_line { error_line_bg } else { bg };
                            queue!(self.stdout, cursor::MoveTo(editor_x as u16, y as u16), SetBackgroundColor(row_bg))?;

                            // Line number
                            let ln_fg = if is_error_line { error_line_num_fg } else { dim_green };
                            queue!(self.stdout, SetForegroundColor(ln_fg))?;
                            if line_idx < code_lines.len() {
                                write!(self.stdout, "{:>3} ", line_idx + 1)?;
                            } else {
                                write!(self.stdout, "  ~ ")?;
                            }

                            // Code content with syntax highlighting
                            if let Some(line) = code_lines.get(line_idx) {
                                if let Some(spans) = viewport_map.get(&line_idx) {
                                    // Render with syntax colors
                                    let mut col = 0usize; // display column
                                    let mut byte_pos = 0usize;
                                    let mut span_idx = 0;
                                    for ch in line.chars() {
                                        if col >= code_inner_w { break; }
                                        let ch_len = ch.len_utf8();
                                        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(1);
                                        // Find the span covering this byte position
                                        while span_idx < spans.len() && spans[span_idx].end <= byte_pos {
                                            span_idx += 1;
                                        }
                                        let fg = if !is_focused {
                                            dim_green
                                        } else if span_idx < spans.len() && byte_pos >= spans[span_idx].start && byte_pos < spans[span_idx].end {
                                            spans[span_idx].fg
                                        } else {
                                            green
                                        };
                                        queue!(self.stdout, SetBackgroundColor(row_bg), SetForegroundColor(fg))?;
                                        write!(self.stdout, "{}", ch)?;
                                        byte_pos += ch_len;
                                        col += ch_width;
                                    }
                                    // Pad remaining space
                                    if col < code_inner_w {
                                        queue!(self.stdout, SetForegroundColor(row_bg))?;
                                        write!(self.stdout, "{:width$}", "", width = code_inner_w - col)?;
                                    }
                                } else {
                                    // No spans — plain text fallback
                                    let fg = if is_focused { green } else { dim_green };
                                    queue!(self.stdout, SetForegroundColor(fg))?;
                                    let display: String = line.chars().take(code_inner_w).collect();
                                    write!(self.stdout, "{:<width$}", display, width = code_inner_w)?;
                                }
                            } else {
                                write!(self.stdout, "{:width$}", "", width = code_inner_w)?;
                            }

                            // Right border if AI panel is shown
                            if coding.show_ai {
                                queue!(self.stdout, SetBackgroundColor(bg), SetForegroundColor(dim_green))?;
                                write!(self.stdout, "│")?;
                            }
                        }
                    }

                    // ── AI panel (right) with chat messages ──
                    if coding.show_ai && ai_w > 2 {
                        let ai_x = w.saturating_sub(ai_w);
                        let is_focused = coding.focus == CodingFocus::AiChat;
                        let border_fg = if is_focused { green } else { dim_green };
                        let ai_inner_w = ai_w.saturating_sub(1); // 1 for left border
                        let _input_rows = 1usize; // input line at bottom
                        let chat_rows = content_h.saturating_sub(2); // -1 header -1 input

                        // Header row
                        let header_y = 1;
                        queue!(self.stdout, cursor::MoveTo(ai_x as u16, header_y as u16), SetBackgroundColor(bg), SetForegroundColor(border_fg))?;
                        let pending_indicator = if coding.ai_pending { " ⟳" } else { "" };
                        let header = format!(" AI Chat{} ", pending_indicator);
                        let header_fill = ai_w.saturating_sub(display_width_str(&header));
                        write!(self.stdout, "{}{}", header, "─".repeat(header_fill / 2))?;

                        // Chat messages area
                        let chat_lines = coding.ai_chat.render_lines(ai_inner_w.saturating_sub(1));
                        let total_chat_lines = chat_lines.len();
                        let scroll = coding.ai_chat.scroll;
                        // Show from bottom (most recent), scrolled up by `scroll`
                        let visible_end = total_chat_lines.saturating_sub(scroll);
                        let visible_start = visible_end.saturating_sub(chat_rows);

                        let user_fg = Color::Rgb { r: 100, g: 200, b: 255 };
                        let asst_fg = Color::Rgb { r: 150, g: 255, b: 150 };
                        let system_fg = Color::Rgb { r: 255, g: 200, b: 100 };

                        for row in 0..chat_rows {
                            let y = 2 + row; // after header
                            queue!(self.stdout, cursor::MoveTo(ai_x as u16, y as u16), SetBackgroundColor(bg), SetForegroundColor(border_fg))?;
                            write!(self.stdout, "│")?;

                            let line_idx = visible_start + row;
                            if line_idx < visible_end {
                                if let Some((role, text)) = chat_lines.get(line_idx) {
                                    let msg_fg = match role {
                                        crate::ui::chatpanel::ChatRole::User => user_fg,
                                        crate::ui::chatpanel::ChatRole::Assistant => asst_fg,
                                        crate::ui::chatpanel::ChatRole::System => system_fg,
                                    };
                                    queue!(self.stdout, SetForegroundColor(msg_fg))?;
                                    let display: String = text.chars().take(ai_inner_w.saturating_sub(1)).collect();
                                    let dw = display_width_str(&display);
                                    write!(self.stdout, "{}", display)?;
                                    if dw < ai_inner_w.saturating_sub(1) {
                                        write!(self.stdout, "{:width$}", "", width = ai_inner_w.saturating_sub(1) - dw)?;
                                    }
                                } else {
                                    write!(self.stdout, "{:width$}", "", width = ai_inner_w.saturating_sub(1))?;
                                }
                            } else {
                                write!(self.stdout, "{:width$}", "", width = ai_inner_w.saturating_sub(1))?;
                            }
                        }

                        // Input line at bottom of AI panel
                        let input_y = 2 + chat_rows;
                        queue!(self.stdout, cursor::MoveTo(ai_x as u16, input_y as u16), SetBackgroundColor(bg), SetForegroundColor(border_fg))?;
                        write!(self.stdout, "│")?;
                        let input_fg = if is_focused { Color::Rgb { r: 255, g: 255, b: 255 } } else { dim_green };
                        queue!(self.stdout, SetForegroundColor(input_fg))?;
                        let prompt_char = "▸ ";
                        let input_display_w = ai_inner_w.saturating_sub(3); // "│▸ " prefix
                        let input_text: String = coding.ai_input.chars().take(input_display_w).collect();
                        let input_dw = display_width_str(&input_text);
                        write!(self.stdout, "{}{}", prompt_char, input_text)?;
                        if input_dw + 2 < ai_inner_w.saturating_sub(1) {
                            write!(self.stdout, "{:width$}", "", width = ai_inner_w.saturating_sub(1) - input_dw - 2)?;
                        }
                    }

                    // ── Result/Error panel (bottom split) ──
                    if coding.show_result_panel && result_panel_h > 0 {
                        let panel_y_start = 1 + content_h; // after title bar + editor area
                        let panel_border_fg = Color::Rgb { r: 255, g: 80, b: 80 };
                        let panel_text_fg = Color::Rgb { r: 220, g: 180, b: 180 };
                        let panel_header_fg = Color::Rgb { r: 255, g: 100, b: 100 };

                        // Border top row
                        queue!(self.stdout, cursor::MoveTo(0, panel_y_start as u16), SetBackgroundColor(bg), SetForegroundColor(panel_border_fg))?;
                        let border_title = if let Some(ref res) = coding.result {
                            if !res.compile_error.is_empty() {
                                " ─── Compile Error "
                            } else if !res.runtime_error.is_empty() {
                                " ─── Runtime Error "
                            } else {
                                " ─── Result "
                            }
                        } else {
                            " ─── Result "
                        };
                        let border_line = format!("{}{}", border_title, "─".repeat(w.saturating_sub(display_width_str(border_title))));
                        write!(self.stdout, "{}", &border_line[..border_line.len().min(w * 4)])?;

                        // Panel content
                        if let Some(ref result) = coding.result {
                            let mut lines: Vec<String> = Vec::new();

                            // Status line
                            lines.push(format!("Status: {}", result.status_msg));

                            // Passed/Total + Runtime info (always show when available)
                            if result.total_testcases > 0 {
                                lines.push(format!("Passed: {}/{}", result.total_correct, result.total_testcases));
                            }
                            if !result.runtime.is_empty() {
                                lines.push(format!("Runtime: {} │ Memory: {}", result.runtime, result.memory));
                            }

                            // Compile error section
                            if !result.compile_error.is_empty() || !result.full_compile_error.is_empty() {
                                lines.push(String::new());
                                lines.push(String::from("── Compile Error ──"));
                                let full_err = if !result.full_compile_error.is_empty() {
                                    &result.full_compile_error
                                } else {
                                    &result.compile_error
                                };
                                for l in full_err.lines() {
                                    lines.push(l.to_string());
                                }
                            }

                            // Runtime error section
                            if !result.runtime_error.is_empty() || !result.full_runtime_error.is_empty() {
                                lines.push(String::new());
                                lines.push(String::from("── Runtime Error ──"));
                                let full_err = if !result.full_runtime_error.is_empty() {
                                    &result.full_runtime_error
                                } else {
                                    &result.runtime_error
                                };
                                for l in full_err.lines() {
                                    lines.push(l.to_string());
                                }
                            }

                            // Test case diff section (Wrong Answer / TLE etc.)
                            if !result.last_testcase.is_empty() || !result.expected_output.is_empty() || !result.code_output.is_empty() {
                                lines.push(String::new());
                                lines.push(String::from("── Failed Test Case ──"));
                                if !result.last_testcase.is_empty() {
                                    lines.push(String::from("Input:"));
                                    for l in result.last_testcase.lines() {
                                        lines.push(format!("  {}", l));
                                    }
                                }
                                if !result.expected_output.is_empty() {
                                    lines.push(String::from("Expected:"));
                                    for l in result.expected_output.lines() {
                                        lines.push(format!("  {}", l));
                                    }
                                }
                                if !result.code_output.is_empty() {
                                    lines.push(String::from("Your Output:"));
                                    for l in result.code_output.lines() {
                                        lines.push(format!("  {}", l));
                                    }
                                }
                            }

                            // Stdout section
                            if !result.std_output.is_empty() {
                                lines.push(String::new());
                                lines.push(String::from("── Stdout ──"));
                                for l in result.std_output.lines() {
                                    lines.push(format!("  {}", l));
                                }
                            }

                            // If Wrong Answer but no details available, show hint
                            if result.status_msg != "Accepted"
                                && result.compile_error.is_empty()
                                && result.full_compile_error.is_empty()
                                && result.runtime_error.is_empty()
                                && result.full_runtime_error.is_empty()
                                && result.last_testcase.is_empty()
                                && result.expected_output.is_empty()
                                && result.code_output.is_empty()
                                && result.std_output.is_empty()
                            {
                                lines.push(String::new());
                                lines.push(String::from("(No detailed error info available)"));
                                lines.push(String::from("Try :run to test with specific inputs"));
                            }

                            // Render visible lines (scrollable)
                            let visible_rows = result_panel_h.saturating_sub(1); // -1 for border
                            for row in 0..visible_rows {
                                let y = panel_y_start + 1 + row;
                                queue!(self.stdout, cursor::MoveTo(0, y as u16), SetBackgroundColor(bg))?;
                                let line_idx = row + coding.result_scroll;
                                if let Some(line) = lines.get(line_idx) {
                                    let fg = if line.starts_with("Status:") || line.starts_with("──") {
                                        panel_header_fg
                                    } else {
                                        panel_text_fg
                                    };
                                    queue!(self.stdout, SetForegroundColor(fg))?;
                                    let display: String = line.chars().take(w).collect();
                                    let dw = display_width_str(&display);
                                    write!(self.stdout, " {}", display)?;
                                    if dw + 1 < w {
                                        write!(self.stdout, "{:width$}", "", width = w - dw - 1)?;
                                    }
                                } else {
                                    write!(self.stdout, "{:width$}", "", width = w)?;
                                }
                            }
                        }
                    }

                    // ── Language selection overlay ──
                    if coding.selecting_lang {
                        let overlay_w = 30.min(w.saturating_sub(4));
                        let overlay_h = (coding.detail.code_snippets.len() + 2).min(content_h);
                        let ox = (w.saturating_sub(overlay_w)) / 2;
                        let oy = (h.saturating_sub(overlay_h)) / 2;

                        // Border top
                        queue!(self.stdout, cursor::MoveTo(ox as u16, oy as u16), SetBackgroundColor(bg), SetForegroundColor(amber))?;
                        write!(self.stdout, "╔{}╗", "═".repeat(overlay_w.saturating_sub(2)))?;

                        for (i, snippet) in coding.detail.code_snippets.iter().enumerate() {
                            let y = oy + 1 + i;
                            if y >= oy + overlay_h - 1 { break; }
                            let is_sel = i == coding.lang_cursor;
                            let sel_bg = if is_sel { highlight_bg } else { bg };
                            queue!(self.stdout, cursor::MoveTo(ox as u16, y as u16), SetBackgroundColor(sel_bg), SetForegroundColor(if is_sel { amber } else { green }))?;
                            let ptr = if is_sel { "▸" } else { " " };
                            let inner_w = overlay_w.saturating_sub(4);
                            let lang_display: String = snippet.lang.chars().take(inner_w).collect();
                            write!(self.stdout, "║{} {:<width$}║", ptr, lang_display, width = inner_w)?;
                        }

                        // Border bottom
                        let bot_y = oy + overlay_h - 1;
                        queue!(self.stdout, cursor::MoveTo(ox as u16, bot_y as u16), SetBackgroundColor(bg), SetForegroundColor(amber))?;
                        write!(self.stdout, "╚{}╝", "═".repeat(overlay_w.saturating_sub(2)))?;
                    }

                    // ── History selection overlay ──
                    if coding.selecting_history && !coding.submissions.is_empty() {
                        let overlay_w = 60.min(w.saturating_sub(4));
                        let visible_count = coding.submissions.len().min(content_h.saturating_sub(2));
                        let overlay_h = visible_count + 2; // +2 for top/bottom border
                        let ox = (w.saturating_sub(overlay_w)) / 2;
                        let oy = (h.saturating_sub(overlay_h)) / 2;

                        // Border top
                        queue!(self.stdout, cursor::MoveTo(ox as u16, oy as u16), SetBackgroundColor(bg), SetForegroundColor(amber))?;
                        write!(self.stdout, "╔{}╗", "═".repeat(overlay_w.saturating_sub(2)))?;

                        // Scrolling: keep cursor visible
                        let max_visible = visible_count;
                        let scroll = if coding.history_cursor >= coding.history_scroll + max_visible {
                            coding.history_cursor - max_visible + 1
                        } else if coding.history_cursor < coding.history_scroll {
                            coding.history_cursor
                        } else {
                            coding.history_scroll
                        };

                        for vi in 0..max_visible {
                            let i = scroll + vi;
                            if i >= coding.submissions.len() { break; }
                            let entry = &coding.submissions[i];
                            let y = oy + 1 + vi;
                            let is_sel = i == coding.history_cursor;
                            let sel_bg = if is_sel { highlight_bg } else { bg };
                            let status_color = if entry.status_display == "Accepted" {
                                green
                            } else {
                                Color::Rgb { r: 255, g: 80, b: 80 }
                            };
                            queue!(self.stdout, cursor::MoveTo(ox as u16, y as u16), SetBackgroundColor(sel_bg), SetForegroundColor(if is_sel { amber } else { status_color }))?;
                            let ptr = if is_sel { "▸" } else { " " };
                            let inner_w = overlay_w.saturating_sub(4);
                            // Format: "▸ Accepted | Python3 | 4 ms | 2024-01-15"
                            let ts_str = {
                                // Format timestamp as YYYY-MM-DD (approximate)
                                let secs = entry.timestamp as i64;
                                let days = secs / 86400;
                                // Calculate date from days since epoch
                                let mut y = 1970i64;
                                let mut remaining = days;
                                loop {
                                    let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
                                    if remaining < days_in_year { break; }
                                    remaining -= days_in_year;
                                    y += 1;
                                }
                                let months_days = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                                    [31,29,31,30,31,30,31,31,30,31,30,31]
                                } else {
                                    [31,28,31,30,31,30,31,31,30,31,30,31]
                                };
                                let mut m = 1u32;
                                for &md in months_days.iter() {
                                    if remaining < md { break; }
                                    remaining -= md;
                                    m += 1;
                                }
                                let d = remaining + 1;
                                format!("{:04}-{:02}-{:02}", y, m, d)
                            };
                            let line = format!(
                                "{} {} │ {} │ {}",
                                entry.status_display, entry.lang, entry.runtime, ts_str
                            );
                            let display: String = line.chars().take(inner_w).collect();
                            write!(self.stdout, "║{} {:<width$}║", ptr, display, width = inner_w)?;
                        }

                        // Border bottom
                        let bot_y = oy + overlay_h - 1;
                        queue!(self.stdout, cursor::MoveTo(ox as u16, bot_y as u16), SetBackgroundColor(bg), SetForegroundColor(amber))?;
                        write!(self.stdout, "╚{}╝", "═".repeat(overlay_w.saturating_sub(2)))?;
                    }

                    // ── Footer (2 lines) ──
                    let footer_y1 = h.saturating_sub(2) as u16; // Line 1: vim keys
                    let footer_y2 = h.saturating_sub(1) as u16; // Line 2: commands / input

                    // Footer line 1: Vim shortcuts (always visible)
                    {
                        let key_color = Color::Rgb { r: 180, g: 140, b: 255 }; // purple for keys
                        let sep_color = Color::Rgb { r: 80, g: 80, b: 80 };    // dim separator
                        let desc_color = Color::Rgb { r: 140, g: 140, b: 140 }; // dim desc
                        queue!(self.stdout, cursor::MoveTo(0, footer_y1), SetBackgroundColor(highlight_bg))?;
                        // Format: key=desc separated by │
                        // Show Tab hint when multiple panels are visible
                        let has_multi_panel = coding.show_description || coding.show_ai || coding.show_result_panel;
                        let keys: Vec<(&str, &str)> = {
                            let mut v: Vec<(&str, &str)> = vec![
                                ("i", "edit"), ("A", "append"), ("o/O", "newline"),
                                ("dd", "del line"), ("u", "undo"),
                                ("hjkl", "move"), ("w/b", "word"),
                                ("gg/G", "top/bot"), ("/", "search"), ("?", "ai"),
                            ];
                            if has_multi_panel {
                                v.push(("Tab", "切换面板"));
                            }
                            v
                        };
                        let mut col = 1usize;
                        for (idx, (key, desc)) in keys.iter().enumerate() {
                            if idx > 0 {
                                queue!(self.stdout, SetForegroundColor(sep_color))?;
                                write!(self.stdout, " │ ")?;
                                col += 3;
                            }
                            queue!(self.stdout, SetForegroundColor(key_color))?;
                            write!(self.stdout, "{}", key)?;
                            col += display_width_str(key);
                            queue!(self.stdout, SetForegroundColor(desc_color))?;
                            write!(self.stdout, " {}", desc)?;
                            col += 1 + display_width_str(desc);
                            if col >= w.saturating_sub(4) { break; }
                        }
                        // Fill remaining
                        if col < w {
                            write!(self.stdout, "{:width$}", "", width = w - col)?;
                        }
                    }

                    // Footer line 2: : commands or active input
                    queue!(self.stdout, cursor::MoveTo(0, footer_y2), SetBackgroundColor(highlight_bg), SetForegroundColor(amber))?;
                    match &coding.editor.mode {
                        crate::mode::Mode::Command(s) => {
                            let cmd_part = format!(":{}", s);
                            write!(self.stdout, "{}", cmd_part)?;
                            // Show ghost completion hint (dimmed)
                            let ghost = if !panel.cmd_completions.is_empty() {
                                let first = &panel.cmd_completions[0];
                                if first.starts_with(s.as_str()) && first.len() > s.len() {
                                    first[s.len()..].to_string()
                                } else { String::new() }
                            } else { String::new() };
                            if !ghost.is_empty() {
                                queue!(self.stdout, SetForegroundColor(dim_green))?;
                                write!(self.stdout, "{}", ghost)?;
                                queue!(self.stdout, SetForegroundColor(amber))?;
                            }
                            // Fill remaining width
                            let used = display_width_str(&cmd_part) + display_width_str(&ghost);
                            if used < w {
                                write!(self.stdout, "{:width$}", "", width = w - used)?;
                            }
                        }
                        crate::mode::Mode::Search(s) => {
                            let footer_text = format!("/{}", s);
                            write!(self.stdout, "{:width$}", footer_text, width = w)?;
                        }
                        _ => {
                            // Show : commands on the left
                            let cmd_color = Color::Rgb { r: 100, g: 200, b: 130 }; // green for commands
                            let sep_color = Color::Rgb { r: 80, g: 80, b: 80 };
                            let cmds = [":submit", ":run", ":lang", ":hist", ":desc", ":ai", ":result", ":q"];
                            let mut col = 1usize;
                            queue!(self.stdout, SetForegroundColor(cmd_color))?;
                            for (idx, cmd) in cmds.iter().enumerate() {
                                if idx > 0 {
                                    queue!(self.stdout, SetForegroundColor(sep_color))?;
                                    write!(self.stdout, "  ")?;
                                    col += 2;
                                    queue!(self.stdout, SetForegroundColor(cmd_color))?;
                                }
                                write!(self.stdout, "{}", cmd)?;
                                col += display_width_str(cmd);
                                if col >= w.saturating_sub(4) { break; }
                            }
                            // Show status_msg on the right if there's room
                            if !coding.status_msg.is_empty() && w > col + 4 {
                                let max_status = w - col - 3;
                                let status_display = if display_width_str(&coding.status_msg) > max_status {
                                    coding.status_msg.chars().take(max_status).collect::<String>()
                                } else {
                                    coding.status_msg.clone()
                                };
                                let status_w = display_width_str(&status_display);
                                let gap = w - col - status_w - 3;
                                write!(self.stdout, "{:gap$}", "", gap = gap)?;
                                queue!(self.stdout, SetForegroundColor(amber))?;
                                write!(self.stdout, " │ {}", status_display)?;
                            } else if col < w {
                                write!(self.stdout, "{:width$}", "", width = w - col)?;
                            }
                        }
                    }
                }
            }
            LeetCodeView::Login => {
                use crate::leetcode::panel::LoginStep;
                // Login instructions — vertically centered block
                // Adapt content based on current login step
                let (prompt_text, step_hint) = match panel.login_step {
                    LoginStep::Session => (
                        "Step 1/2: Paste LEETCODE_SESSION value (Esc to cancel):",
                        "  (Copy the value of LEETCODE_SESSION from browser cookies)",
                    ),
                    LoginStep::CsrfToken => (
                        "Step 2/2: Paste csrftoken value (Esc to cancel):",
                        "  (Copy the value of csrftoken from browser cookies)",
                    ),
                };

                let lines: Vec<(&str, bool)> = vec![
                    ("╔══════════════════════════════════════╗", true),
                    ("║        LeetCode Login                ║", true),
                    ("╚══════════════════════════════════════╝", true),
                    ("", false),
                    ("How to get your cookie:", false),
                    ("", false),
                    ("  1. Open https://leetcode.cn (or leetcode.com)", false),
                    ("  2. Log in to your account", false),
                    ("  3. Press F12 → Application → Cookies", false),
                    ("  4. Copy the value of LEETCODE_SESSION", false),
                    ("  5. Copy the value of csrftoken", false),
                    ("", false),
                    (prompt_text, false),
                    (step_hint, false),
                ];
                let total_lines = lines.len() + 2; // +2 for input line + blank
                let start_y = h.saturating_sub(total_lines) / 2;

                for (i, (text, is_amber)) in lines.iter().enumerate() {
                    let y = start_y + i;
                    if y >= h { break; }
                    let fg = if *is_amber { amber } else { green };
                    queue!(self.stdout, cursor::MoveTo(0, y as u16), SetBackgroundColor(bg), SetForegroundColor(fg))?;
                    let centered = center_to_dw(text, w);
                    let truncated = truncate_to_width(&centered, w);
                    write!(self.stdout, "{}", truncated)?;
                }

                // Input line
                let input_y = start_y + lines.len() + 1;
                if input_y < h {
                    queue!(self.stdout, cursor::MoveTo(0, input_y as u16), SetBackgroundColor(bg), SetForegroundColor(green))?;
                    let input_display = format!("  > {}_", &panel.login_input);
                    let centered = center_to_dw(&input_display, w);
                    let truncated = truncate_to_width(&centered, w);
                    write!(self.stdout, "{}", truncated)?;
                }
            }
            LeetCodeView::KnowledgeMap => {
                if let Some(ref kp) = panel.knowledge_panel {
                    // ── Tab bar (row 0) ──
                    let tab_bar = kp.render_tab_bar(w);
                    queue!(self.stdout, cursor::MoveTo(0, 0), SetBackgroundColor(bg))?;
                    write!(self.stdout, "{}", tab_bar)?;

                    // ── Filter bar (rows 1-3) ──
                    let filter_bar = kp.render_filter_bar(w);
                    for (i, line) in filter_bar.lines().enumerate() {
                        let row = 1 + i;
                        if row >= h { break; }
                        queue!(self.stdout, cursor::MoveTo(0, row as u16), SetBackgroundColor(bg))?;
                        write!(self.stdout, "{:width$}", "", width = w)?;
                        queue!(self.stdout, cursor::MoveTo(0, row as u16))?;
                        write!(self.stdout, "{}", line)?;
                    }

                    // ── Content area (row 4 onwards) ──
                    let content_start = 4;
                    let content_height = h.saturating_sub(content_start + 1); // -1 for footer

                    use crate::leetcode::knowledge_panel::PanelTab;
                    let content_lines = match kp.selected_tab {
                        PanelTab::Overview => kp.render_overview(w),
                        PanelTab::Topics => kp.render_topics(w),
                        PanelTab::Techniques => kp.render_techniques(w),
                        PanelTab::Problems => kp.render_problem_list(w, content_height),
                    };
                    for (i, line) in content_lines.iter().enumerate() {
                        let row = content_start + i;
                        if row >= h.saturating_sub(1) { break; }
                        queue!(self.stdout, cursor::MoveTo(0, row as u16), SetBackgroundColor(bg))?;
                        write!(self.stdout, "{:width$}", "", width = w)?;
                        queue!(self.stdout, cursor::MoveTo(0, row as u16))?;
                        write!(self.stdout, "{}", line)?;
                    }

                    // ── Footer ──
                    let footer_y = h.saturating_sub(1);
                    queue!(self.stdout, cursor::MoveTo(0, footer_y as u16), SetBackgroundColor(highlight_bg), SetForegroundColor(green))?;
                    let footer = format!(" Knowledge Map │ Tab:切换 1-4:面板 j/k:滚动 q:返回 │ {} problems", kp.filtered_ids.len());
                    write!(self.stdout, "{:width$}", footer, width = w)?;
                }
            }
        }

        // ── Hardware cursor positioning for LeetCode views ──
        match panel.view {
            LeetCodeView::Coding => {
                use crate::leetcode::panel::CodingFocus;
                use crate::mode::Mode as EditorMode;
                if let Some(ref coding) = panel.coding {
                    // Command/Search mode: cursor in footer line
                    match &coding.editor.mode {
                        EditorMode::Command(s) => {
                            let cursor_x = (display_width_str(":") + display_width_str(s)).min(w.saturating_sub(1));
                            let cursor_y = h.saturating_sub(1);
                            queue!(self.stdout,
                                cursor::Show,
                                cursor::MoveTo(cursor_x as u16, cursor_y as u16),
                                cursor::SetCursorStyle::BlinkingBar,
                            )?;
                        }
                        EditorMode::Search(s) => {
                            let cursor_x = (display_width_str("/") + display_width_str(s)).min(w.saturating_sub(1));
                            let cursor_y = h.saturating_sub(1);
                            queue!(self.stdout,
                                cursor::Show,
                                cursor::MoveTo(cursor_x as u16, cursor_y as u16),
                                cursor::SetCursorStyle::BlinkingBar,
                            )?;
                        }
                        _ if coding.focus == CodingFocus::AiChat => {
                            // Position cursor in the AI input line
                            let ai_w = if coding.show_ai { w / 4 } else { 0 };
                            let ai_x = w.saturating_sub(ai_w);
                            let full_content_h = h.saturating_sub(3); // title(1) + footer(2)
                            let result_panel_h = if coding.show_result_panel { (full_content_h / 3).max(5) } else { 0 };
                            let content_h = full_content_h.saturating_sub(result_panel_h);
                            let chat_rows = content_h.saturating_sub(2);
                            let input_y = 2 + chat_rows;
                            // "│▸ " = 4 display columns offset, then cursor at input position
                            let input_prefix_w = 3; // "│▸ "
                            let cursor_col: usize = coding.ai_input.chars()
                                .take(coding.ai_input_cursor)
                                .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
                                .sum();
                            let cursor_x = ai_x + input_prefix_w + cursor_col;
                            queue!(self.stdout,
                                cursor::Show,
                                cursor::MoveTo(cursor_x as u16, input_y as u16),
                                cursor::SetCursorStyle::BlinkingBar,
                            )?;
                        }
                        _ if coding.focus == CodingFocus::Editor && !coding.selecting_lang && !coding.selecting_history && !coding.lang_confirm_pending => {
                            // Position cursor in the code editor area
                            let full_content_h = h.saturating_sub(3); // title(1) + footer(2)
                            let result_panel_h = if coding.show_result_panel { (full_content_h / 3).max(5) } else { 0 };
                            let content_h = full_content_h.saturating_sub(result_panel_h);
                            let desc_w = if coding.show_description { w * 2 / 5 } else { 0 };
                            let editor_x = desc_w;
                            let line_num_w: usize = 4; // "  1 "

                            // Only show cursor if cursor_line is visible
                            let vis_line = coding.editor.cursor_line.saturating_sub(coding.editor.scroll_line);
                            if vis_line < content_h {
                                let code_text = coding.editor.buffer.rope.to_string();
                                let code_lines: Vec<&str> = code_text.lines().collect();
                                let current_line = code_lines.get(coding.editor.cursor_line).unwrap_or(&"");
                                let display_col: usize = current_line.chars()
                                    .take(coding.editor.cursor_col)
                                    .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
                                    .sum();
                                let cursor_x = editor_x + line_num_w + display_col;
                                let cursor_y = vis_line + 1; // +1 for title bar
                                // Cursor style depends on editor mode
                                let cursor_style = match coding.editor.mode {
                                    EditorMode::Insert => cursor::SetCursorStyle::BlinkingBar,
                                    _ => cursor::SetCursorStyle::SteadyBlock,
                                };
                                queue!(self.stdout,
                                    cursor::Show,
                                    cursor::MoveTo(cursor_x as u16, cursor_y as u16),
                                    cursor_style,
                                )?;
                            } else {
                                queue!(self.stdout, cursor::Hide)?;
                            }
                        }
                        _ => {
                            queue!(self.stdout, cursor::Hide)?;
                        }
                    }
                } else {
                    queue!(self.stdout, cursor::Hide)?;
                }
            }
            LeetCodeView::ProblemList => {
                use crate::leetcode::panel::ListMode;
                match panel.list_mode {
                    ListMode::Search => {
                        let prefix = " /";
                        let input_dw = display_width_str(&panel.search_input);
                        let cursor_x = (display_width_str(prefix) + input_dw).min(w.saturating_sub(1));
                        let cursor_y = h.saturating_sub(1);
                        queue!(self.stdout,
                            cursor::Show,
                            cursor::MoveTo(cursor_x as u16, cursor_y as u16),
                            cursor::SetCursorStyle::BlinkingBar,
                        )?;
                    }
                    ListMode::Command => {
                        let prefix = " :";
                        let input_dw = display_width_str(&panel.search_input);
                        let cursor_x = (display_width_str(prefix) + input_dw).min(w.saturating_sub(1));
                        let cursor_y = h.saturating_sub(1);
                        queue!(self.stdout,
                            cursor::Show,
                            cursor::MoveTo(cursor_x as u16, cursor_y as u16),
                            cursor::SetCursorStyle::BlinkingBar,
                        )?;
                    }
                    ListMode::Normal => {
                        queue!(self.stdout, cursor::Hide)?;
                    }
                }
            }
            LeetCodeView::Login => {
                // Show cursor at end of login input
                let total_lines = 15; // approximate lines count from login view
                let start_y = h.saturating_sub(total_lines + 2) / 2;
                let input_y = start_y + total_lines;
                if input_y < h {
                    let prefix = "  > ";
                    let input_dw = display_width_str(&panel.login_input);
                    let prefix_dw = display_width_str(prefix);
                    // The input is centered, so calculate offset
                    let full_text = format!("  > {}_", &panel.login_input);
                    let full_dw = display_width_str(&full_text);
                    let left_pad = w.saturating_sub(full_dw) / 2;
                    let cursor_x = (left_pad + prefix_dw + input_dw).min(w.saturating_sub(1));
                    queue!(self.stdout,
                        cursor::Show,
                        cursor::MoveTo(cursor_x as u16, input_y as u16),
                        cursor::SetCursorStyle::BlinkingBar,
                    )?;
                } else {
                    queue!(self.stdout, cursor::Hide)?;
                }
            }
            LeetCodeView::Splash => {
                // Cursor is handled in the render section above (shown when cmd active)
                if !panel.splash_cmd_active {
                    queue!(self.stdout, cursor::Hide)?;
                }
            }
            _ => {
                queue!(self.stdout, cursor::Hide)?;
            }
        }

        queue!(self.stdout, ResetColor)?;
        self.stdout.flush()?;
        Ok(())
    }
}