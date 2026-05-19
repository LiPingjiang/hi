//! Application glue: event loop, mode dispatch, AI integration.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use crate::ai::{AiClient, AiContext, HintKind};
use crate::ai::log as ai_log;
use crate::ai::{AiEditSession, AiEditStepResult, AiTool, ToolResult, DiffHunk, HunkKind};
use crate::ai::parser::{parse_tool_call, extract_thought};
use crate::ui::chatpanel::ChatPanel;
use crate::ui::diff_panel::DiffPanel;
#[cfg(feature = "leetcode")]
use crate::leetcode::{LeetCodePanel, panel::LeetCodeAction};

/// Which panel currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusZone {
    Editor,
    FileTree,
    Chat,
}

/// AI connection / request status for the status bar indicator.
#[derive(Debug, Clone, PartialEq)]
pub enum AiStatus {
    /// AI is configured and idle (ready to use).
    Idle,
    /// No API key and using default URL — effectively not configured.
    NotConfigured,
    /// A request is in flight.
    Requesting,
    /// Last request returned an error.
    Error(String),
}
use crate::ai::parser::{parse_plan, apply_steps};
use crate::ai::prompt::{build_messages_with_history, PromptKind};
use crate::buffer::Buffer;
use crate::config::Config;
use crate::editor::Editor;
use crate::locale::Locale;
use crate::mode::{Mode, VisualKind};
use crate::mode::command::CommandAction;
use crate::mode::cmd_completion::CmdCompletionState;
use crate::mode::insert::InsertAction;
use crate::mode::normal::NormalAction;
use crate::mode::visual::VisualAction;
use crate::mode::ai::{handle_ai_input_key, AiInputAction};
use crate::syntax::highlight::{FileType, CodePalette};
use crate::ui::filetree::FileTree;
use crate::ui::ghost::GhostText;
use crate::ui::picker::FilePicker;
use crate::ui::grep_panel::GrepPanel;
use crate::ui::renderer::Renderer;
use crate::ui::tutorial::TutorialBoard;

/// Interactive theme picker overlay state.
pub struct ThemePicker {
    pub themes: Vec<&'static str>,
    pub cursor: usize,
    /// Theme name that was active before the picker opened (for Esc restore).
    pub original_theme: String,
}

/// Pending file-tree prompt (new file / new dir / rename / delete confirm / search).
#[derive(Debug, Clone)]
pub enum FileTreePrompt {
    NewFile  { input: String },
    NewDir   { input: String },
    Rename   { original: std::path::PathBuf, input: String },
    Delete   { path: std::path::PathBuf, confirmed: bool },
    Search   { input: String },
}

impl FileTreePrompt {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NewFile  { .. } => "新建文件: ",
            Self::NewDir   { .. } => "新建目录: ",
            Self::Rename   { .. } => "重命名: ",
            Self::Delete   { .. } => "删除? [y/n]: ",
            Self::Search   { .. } => "/",
        }
    }
    pub fn input_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::NewFile  { input } | Self::NewDir { input } | Self::Rename { input, .. } | Self::Search { input } => Some(input),
            Self::Delete   { .. } => None,
        }
    }
}

/// State for Visual Block insert (I / c in V-BLOCK).
struct BlockInsertState {
    start_line: usize,
    end_line:   usize,
    col:        usize,
}

pub struct App {
    pub locale: Locale,
    editor: Editor,
    renderer: Renderer,
    filetree: Option<FileTree>,
    ghost: GhostText,

    // AI async state
    ai_result: Arc<Mutex<Option<HintKind>>>,
    ai_pending: bool,
    ai_query_msg: Option<String>,       // single-line result shown in hint bar
    plan_lines: Option<Vec<String>>,    // plan overlay content
    ai_status: AiStatus,
    ai_tick: u64,                       // incremented every poll cycle (~100ms), drives spinner animation

    // Chat panel (right side)
    chat_panel: ChatPanel,
    chat_visible: bool,
    focus: FocusZone,
    /// Input buffer for typing in the chat panel.
    chat_input: String,
    /// Cursor position (char index) within chat_input.
    chat_input_cursor: usize,
    /// Whether the chat panel is in input mode (typing a message).
    chat_input_active: bool,

    // Tutorial board (right side, left of chat when both visible)
    tutorial_board: TutorialBoard,
    tutorial_visible: bool,

    should_quit: bool,
    filepath: Option<PathBuf>,

    // incsearch: cursor position when '/' was pressed (for Esc restore)
    search_saved_pos: Option<(usize, usize)>,

    // Visual Block insert: when Some, on Esc from Insert we replicate typed text to all lines
    block_insert: Option<BlockInsertState>,

    // File tree prompt (new file / rename / delete confirm)
    filetree_prompt: Option<FileTreePrompt>,

    // Theme picker overlay
    theme_picker: Option<ThemePicker>,

    // Command-line completion state
    cmd_completion: CmdCompletionState,

    // Fuzzy file picker overlay
    file_picker: Option<FilePicker>,

    // Global grep panel overlay
    grep_panel: Option<GrepPanel>,

    // AI agent-edit session (Tool-Use Loop)
    ai_edit_session: Option<AiEditSession>,
    /// Result channel for the agent-edit background thread.
    ai_edit_result: Arc<Mutex<Option<AiEditStepResult>>>,
    /// DiffPanel overlay (shown when session is AwaitingConfirm).
    diff_panel: Option<DiffPanel>,
    /// Monotonically increasing session ID counter.
    ai_edit_session_id: u64,
    /// Pending selection for ga-triggered agent-edit (start_line, end_line, text).
    ai_edit_pending_selection: Option<(usize, usize, String)>,

    // LeetCode "古法时代" panel
    #[cfg(feature = "leetcode")]
    leetcode_panel: Option<LeetCodePanel>,
}

impl App {
    /// Create a new App, optionally loading a file.
    pub fn new(config: Config, locale: Locale, filepath: Option<&Path>, width: u16, height: u16) -> Result<Self> {
        let ft = filepath
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .map(FileType::from_ext)
            .unwrap_or(FileType::Plain);

        let mut editor = Editor::new(config.clone(), width, height);

        if let Some(path) = filepath {
            let buf = Buffer::from_file(path)?;
            editor = editor.with_buffer(buf);
        }

        // Initialize debug logging
        ai_log::init(config.ai.debug);
        // Initialize render-performance logging (HI_PERF=1 to enable)
        crate::ui::perf_log::init();

        let renderer = Renderer::new(ft, &config);
        // Resolve the directory that contains the opened file.
        // filepath may be a bare name like "Cargo.toml" whose .parent() is "",
        // so we canonicalize first and fall back to CWD.
        let filetree_root: PathBuf = filepath
            .and_then(|p| {
                let abs = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
                abs.parent().map(|d| d.to_path_buf())
            })
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let filetree = FileTree::new(&filetree_root, config.filetree.show_hidden).ok();

        Ok(Self {
            locale,
            editor,
            renderer,
            filetree,
            ghost: GhostText::default(),
            ai_result: Arc::new(Mutex::new(None)),
            ai_pending: false,
            ai_query_msg: None,
            plan_lines: None,
            ai_status: {
                let ai = &config.ai;
                if ai.api_key.is_empty() && ai.api_base_url == "https://api.openai.com/v1" {
                    AiStatus::NotConfigured
                } else {
                    AiStatus::Idle
                }
            },
            ai_tick: 0,
            chat_panel: ChatPanel::new(
                config.chat.width as usize,
                config.chat.max_messages,
            ),
            chat_visible: false,
            focus: FocusZone::Editor,
            chat_input: String::new(),
            chat_input_cursor: 0,
            chat_input_active: false,
            tutorial_board: TutorialBoard::new(32),
            tutorial_visible: false,
            should_quit: false,
            filepath: filepath.map(PathBuf::from),
            search_saved_pos: None,
            block_insert: None,
            filetree_prompt: None,
            theme_picker: None,
            cmd_completion: CmdCompletionState::new(),
            file_picker: None,
            grep_panel: None,
            ai_edit_session: None,
            ai_edit_result: Arc::new(Mutex::new(None)),
            diff_panel: None,
            ai_edit_session_id: 0,
            ai_edit_pending_selection: None,
            #[cfg(feature = "leetcode")]
            leetcode_panel: None,
        })
    }

    /// Main run loop.
    pub fn run(&mut self) -> Result<()> {
        // Switch to ASCII input method on startup so Normal-mode keys work
        // immediately without the user having to dismiss a CJK IME.
        crate::ime::switch_to_ascii();

        self.renderer.init()?;
        let mut needs_redraw = true; // first frame always renders

        loop {
            if needs_redraw {
                // LeetCode panel: full-screen overlay, skip normal render
                #[cfg(feature = "leetcode")]
                let leetcode_rendered = if let Some(ref panel) = self.leetcode_panel {
                    let w = self.editor.term_width as usize;
                    let h = self.editor.term_height as usize;
                    self.renderer.render_leetcode_panel(panel, w, h)?;
                    true
                } else {
                    false
                };
                #[cfg(not(feature = "leetcode"))]
                let leetcode_rendered = false;

                if !leetcode_rendered {
                // Normal render
                self.renderer.render(
                    &mut self.editor,
                    &self.filetree,
                    &self.ghost,
                    &self.ai_query_msg,
                    &self.plan_lines,
                    &self.filetree_prompt,
                    &self.ai_status,
                    self.ai_pending,
                    self.ai_tick,
                    &mut self.chat_panel,
                    self.chat_visible,
                    self.focus,
                    &self.chat_input,
                    self.chat_input_active,
                    self.chat_input_cursor,
                    &self.theme_picker,
                    &self.cmd_completion,
                    &self.locale,
                    &self.tutorial_board,
                    self.tutorial_visible,
                )?;

                // Render file picker overlay on top if active
                if let Some(ref picker) = self.file_picker {
                    let w = self.editor.term_width as usize;
                    let h = self.editor.term_height as usize;
                    self.renderer.render_file_picker(picker, w, h)?;
                }

                // Render grep panel overlay on top if active
                if let Some(ref panel) = self.grep_panel {
                    let w = self.editor.term_width as usize;
                    let h = self.editor.term_height as usize;
                    self.renderer.render_grep_panel(panel, w, h)?;
                }

                // Render diff panel overlay on top if active
                if let (Some(ref dp), Some(ref session)) = (&self.diff_panel, &self.ai_edit_session) {
                    let w = self.editor.term_width as usize;
                    let h = self.editor.term_height as usize;
                    self.renderer.render_diff_panel(dp, &session.pending_diff, &session.status_text(), w, h)?;
                }

                // Clear one-shot status message after render
                self.editor.status_msg = None;
                } // end if !leetcode_rendered
                needs_redraw = false;
            }

            // Poll AI result — may set needs_redraw
            let had_ai_result = self.poll_ai_result();
            if had_ai_result {
                needs_redraw = true;
            }

            // Poll AI agent-edit step result
            let had_edit_result = self.poll_ai_edit_result();
            if had_edit_result {
                needs_redraw = true;
            }

            if self.should_quit {
                break;
            }

            // When AI is pending, tick the spinner animation and force redraw
            if self.ai_pending {
                self.ai_tick = self.ai_tick.wrapping_add(1);
                needs_redraw = true;
            }

            // Wait for input (100ms timeout so AI poll runs regularly).
            // After handling the first event, drain any queued events before
            // redrawing.  This coalesces rapid-fire scroll ticks into a single
            // frame, keeping the editor responsive during fast scrolling.
            if event::poll(Duration::from_millis(100))? {
                needs_redraw = true;
                let dispatch_start = Instant::now();
                let mut drain_count = 1usize;
                let mut scroll_count = 0usize;

                let first_ev = event::read()?;
                if matches!(&first_ev, Event::Mouse(me) if matches!(me.kind, crossterm::event::MouseEventKind::ScrollDown | crossterm::event::MouseEventKind::ScrollUp)) {
                    scroll_count += 1;
                }
                self.dispatch_event(first_ev)?;

                // Drain remaining queued events without blocking
                while event::poll(Duration::from_millis(0))? {
                    let ev = event::read()?;
                    if matches!(&ev, Event::Mouse(me) if matches!(me.kind, crossterm::event::MouseEventKind::ScrollDown | crossterm::event::MouseEventKind::ScrollUp)) {
                        scroll_count += 1;
                    }
                    drain_count += 1;
                    self.dispatch_event(ev)?;
                }

                crate::ui::perf_log::log_event_batch(drain_count, scroll_count, dispatch_start.elapsed());
            }
        }

        self.renderer.cleanup()?;
        Ok(())
    }

    // ── Event dispatch ──────────────────────────────────────────────────────

    fn dispatch_event(&mut self, ev: Event) -> Result<()> {
        match ev {
            Event::Key(key) => {
                // LeetCode panel takes priority when open
                #[cfg(feature = "leetcode")]
                if self.leetcode_panel.is_some() {
                    let panel = self.leetcode_panel.as_mut().unwrap();
                    match panel.handle_key(key) {
                        LeetCodeAction::Close => { self.leetcode_panel = None; }
                        LeetCodeAction::Redraw | LeetCodeAction::None => {}
                    }
                    return Ok(());
                }

                if self.diff_panel.is_some() {
                    self.handle_diff_panel_key(key)?;
                } else if self.grep_panel.is_some() {
                    self.handle_grep_panel_key(key)?;
                } else if self.file_picker.is_some() {
                    self.handle_file_picker_key(key)?;
                } else if self.theme_picker.is_some() {
                    self.handle_theme_picker_key(key);
                } else if self.editor.shell_output.is_some() {
                    self.editor.shell_output = None;
                } else if self.filetree_prompt.is_some() {
                    self.handle_filetree_prompt_key(key)?;
                } else if self.focus == FocusZone::Chat && self.chat_visible {
                    self.handle_chat_key(key)?;
                } else if self.focus == FocusZone::FileTree && self.editor.filetree_visible {
                    self.handle_filetree_key(key)?;
                } else {
                    self.handle_key(key)?;
                }
            }
            Event::Resize(w, h) => {
                self.editor.term_width  = w;
                self.editor.term_height = h;
            }
            Event::Mouse(me) => {
                use crossterm::event::{MouseEventKind, MouseButton};
                if self.renderer.mouse_enabled {
                    // Mouse mode ON — process events normally
                    match me.kind {
                        MouseEventKind::ScrollDown => {
                            self.handle_mouse_scroll(me.column, me.row, false);
                        }
                        MouseEventKind::ScrollUp => {
                            self.handle_mouse_scroll(me.column, me.row, true);
                        }
                        MouseEventKind::Down(MouseButton::Left) => {
                            self.handle_mouse_click(me.column, me.row);
                        }
                        MouseEventKind::Drag(MouseButton::Left) => {
                            self.handle_mouse_drag(me.column, me.row);
                        }
                        _ => {}
                    }
                } else {
                    // Mouse mode OFF — show "drop the mouse" reminder on clicks/scrolls
                    match me.kind {
                        MouseEventKind::Down(_) | MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                            self.editor.set_msg(self.locale.messages.mouse_hint.clone());
                        }
                        _ => {} // ignore drag, move, etc. silently
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ── AI polling ────────────────────────────────────────────────────────────

    fn poll_ai_result(&mut self) -> bool {
        if !self.ai_pending { return false; }
        let result = {
            let mut guard = self.ai_result.lock().unwrap();
            guard.take()
        };
        if let Some(hint) = result {
            self.ai_pending = false;
            // Update AI status based on result
            match &hint {
                HintKind::Error(e) => self.ai_status = AiStatus::Error(e.clone()),
                _ => self.ai_status = AiStatus::Idle,
            }
            self.apply_ai_hint(hint);
            true
        } else {
            false
        }
    }

    fn apply_ai_hint(&mut self, hint: HintKind) {
        match &hint {
            HintKind::Advisor(text) => {
                // Push to chat panel; show first line in hint bar as summary
                let first_line = text.lines().next().unwrap_or("").to_string();
                self.ai_query_msg = Some(first_line.clone());
                self.ghost.explanation = first_line;
                self.chat_panel.push_assistant(text);
                // Auto-open chat panel when AI responds
                if !self.chat_visible {
                    self.chat_visible = true;
                }
            }
            HintKind::Plan(steps) => {
                self.plan_lines = Some(steps.clone());
                if self.editor.config.ai.yolo_mode {
                    // yolo_mode: skip confirmation, execute immediately
                    self.editor.set_msg(self.locale.messages.ai_plan_steps_yolo.replace("{n}", &steps.len().to_string()));
                    self.apply_plan();
                } else {
                    self.editor.set_msg(self.locale.messages.ai_plan_steps_confirm.replace("{n}", &steps.len().to_string()));
                    self.editor.mode = Mode::Ai(String::new());
                }
            }
            HintKind::Completion(text) => {
                self.ghost.text = text.clone();
                self.ghost.command = text.lines().next().unwrap_or("").to_string();
                self.ghost.explanation = "AI 建议".to_string();
                self.ghost.visible = true;
            }
            HintKind::Error(e) => {
                self.editor.set_msg(self.locale.messages.ai_error.replace("{err}", &e));
                self.chat_panel.push_system(&format!("Error: {}", e));
            }
        }
    }

    /// Infer the user's intent from the query text alone — no extra API call.
    ///
    /// Rules (checked in order):
    ///  1. `!` prefix  → always Plan  (explicit override)
    ///  2. `?` prefix  → always Advisor (explicit override)
    ///  3. Action verbs present → Plan
    ///  4. Question words / ends with `?` → Advisor
    ///  5. Short query (≤ 15 chars) with no verb/question → Completion
    ///  6. Default → Advisor
    fn infer_intent(query: &str) -> (PromptKind, String) {
        // Explicit overrides
        if let Some(rest) = query.strip_prefix('!') {
            return (PromptKind::Plan, rest.trim().to_string());
        }
        if let Some(rest) = query.strip_prefix('?') {
            return (PromptKind::Advisor, rest.trim().to_string());
        }

        let q = query.trim();
        let lower = q.to_lowercase();

        // Question words → Advisor (checked FIRST so "为什么能修改" doesn't fall into Plan)
        let advisor_triggers = [
            "怎么", "如何", "什么是", "什么意思", "什么", "为什么", "解释", "说明", "介绍",
            "what ", "how ", "why ", "explain", "describe", "what's", "whats",
        ];
        if advisor_triggers.iter().any(|t| lower.contains(t)) || q.ends_with('?') || q.ends_with('？') {
            return (PromptKind::Advisor, q.to_string());
        }

        // Action verbs → Plan (only reached when no question words present)
        let plan_triggers = [
            "帮我改", "帮我修", "帮我删", "帮我加", "帮我添", "帮我替换", "帮我重构",
            "帮我格式", "帮我优化", "帮我补", "帮我写", "帮我生成", "帮我插入",
            "修改", "替换", "删除", "添加", "重构", "格式化", "优化", "插入",
            "rename", "replace", "delete", "remove", "add ", "insert", "refactor",
            "format", "fix ", "rewrite", "change ", "update ",
        ];
        if plan_triggers.iter().any(|t| lower.contains(t)) {
            return (PromptKind::Plan, q.to_string());
        }

        // Short query with no clear signal → Advisor (not Completion)
        // Completion is only triggered by explicit code-like patterns or inline ghost
        // Default → Advisor
        (PromptKind::Advisor, q.to_string())
    }

    fn dispatch_ai_query(&mut self, query: &str) {
        let (kind, real_query) = Self::infer_intent(query);

        // Collect everything we need from editor before any mutable borrow
        let context = AiContext::from_cursor(
            &self.editor.buffer,
            self.editor.buffer.filepath().map(|p| p.to_str().unwrap_or("")).unwrap_or(""),
            self.editor.cursor_line,
            self.editor.cursor_col,
            self.editor.config.ai.context_lines,
            self.renderer.highlighter.filetype().name(),
        );
        let cfg = self.editor.config.ai.clone();

        // Show inferred intent in status bar
        let intent_label = match &kind {
            PromptKind::Plan     => "规划模式",
            PromptKind::Advisor  => "顾问模式",
            PromptKind::Complete => "补全模式",
            PromptKind::Transform(_) => "变换模式",
            PromptKind::AgentEdit { .. } => "编辑模式",
        };
        self.editor.set_msg(self.locale.messages.ai_thinking_plan.clone());

        // Debug: log query dispatch
        ai_log::log(&format!("dispatch_ai_query: intent={}, query={:?}", intent_label, &real_query));
        ai_log::log(&format!("  file={}, line={}, col={}",
            self.editor.buffer.filepath().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "<none>".to_string()),
            self.editor.cursor_line + 1,
            self.editor.cursor_col + 1,
        ));
        ai_log::log(&format!("  model={}, base_url={}", cfg.model, cfg.api_base_url));

        // Collect recent conversation history for multi-turn context
        let history_pairs = self.chat_panel.recent_history(
            self.editor.config.chat.context_pairs,
        );
        let history_owned: Vec<(String, String)> = history_pairs
            .iter()
            .map(|(u, a)| (u.to_string(), a.to_string()))
            .collect();

        let messages = build_messages_with_history(
            &kind,
            &context,
            &real_query,
            &history_owned.iter().map(|(u, a)| (u.as_str(), a.as_str())).collect::<Vec<_>>(),
            &self.locale,
        );
        let result_arc = Arc::clone(&self.ai_result);

        std::thread::spawn(move || {
            let client = AiClient::new(&cfg);
            let res = client.chat(messages);
            let hint = match res {
                Ok(text) => match kind {
                    PromptKind::Plan => {
                        let steps = parse_plan(&text);
                        let step_strs: Vec<String> = steps.iter().map(|s| format!("{:?}", s)).collect();
                        HintKind::Plan(step_strs)
                    }
                    PromptKind::Complete => HintKind::Completion(text),
                    _ => HintKind::Advisor(text),
                },
                Err(e) => HintKind::Error(e.to_string()),
            };
            *result_arc.lock().unwrap() = Some(hint);
        });

        self.ai_pending = true;
        self.ai_status = AiStatus::Requesting;

        // Push user message to chat panel
        self.chat_panel.push_user(&real_query);
        if !self.chat_visible {
            self.chat_visible = true;
        }
    }

    /// Dispatch a query forced to Advisor mode (used from chat panel input).
    /// This bypasses infer_intent so short messages always get AI responses in chat.
    fn dispatch_ai_query_as_advisor(&mut self, query: &str) {
        let context = AiContext::from_cursor(
            &self.editor.buffer,
            self.editor.buffer.filepath().map(|p| p.to_str().unwrap_or("")).unwrap_or(""),
            self.editor.cursor_line,
            self.editor.cursor_col,
            self.editor.config.ai.context_lines,
            self.renderer.highlighter.filetype().name(),
        );
        let cfg = self.editor.config.ai.clone();
        let kind = PromptKind::Advisor;
        let real_query = query.to_string();

        self.editor.set_msg(self.locale.messages.ai_thinking_advisor.clone());
        ai_log::log(&format!("dispatch_ai_query_as_advisor: query={:?}", &real_query));

        let history_pairs = self.chat_panel.recent_history(
            self.editor.config.chat.context_pairs,
        );
        let history_owned: Vec<(String, String)> = history_pairs
            .iter()
            .map(|(u, a)| (u.to_string(), a.to_string()))
            .collect();

        let messages = build_messages_with_history(
            &kind,
            &context,
            &real_query,
            &history_owned.iter().map(|(u, a)| (u.as_str(), a.as_str())).collect::<Vec<_>>(),
            &self.locale,
        );
        let result_arc = Arc::clone(&self.ai_result);

        std::thread::spawn(move || {
            let client = AiClient::new(&cfg);
            let res = client.chat(messages);
            let hint = match res {
                Ok(text) => HintKind::Advisor(text),
                Err(e) => HintKind::Error(e.to_string()),
            };
            *result_arc.lock().unwrap() = Some(hint);
        });

        self.ai_pending = true;
        self.ai_status = AiStatus::Requesting;
        self.chat_panel.push_user(&real_query);
    }

    fn apply_plan(&mut self) {
        if let Some(plan_steps) = self.plan_lines.take() {
            // Re-parse from canonical step format
            let raw = plan_steps.join("\n");
            let steps = parse_plan(&raw);
            self.editor.buffer.begin_group();
            if let Err(e) = apply_steps(&mut self.editor.buffer, &steps) {
                self.editor.set_msg(self.locale.messages.ai_plan_failed.replace("{err}", &e.to_string()));
            } else {
                self.editor.set_msg(self.locale.messages.ai_plan_applied.replace("{n}", &steps.len().to_string()));
            }
            self.editor.clamp_cursor();
            self.editor.scroll_to_cursor();
            self.editor.mode = Mode::Normal;
        }
    }

    // ── Key dispatch ──────────────────────────────────────────────────────────

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.editor.mode.clone() {
            Mode::Normal => self.handle_normal(key)?,
            Mode::Insert => self.handle_insert(key),
            Mode::Visual { kind, anchor } => self.handle_visual(key, kind, anchor),
            Mode::Command(mut input) => {
                // ── Completion interaction ──
                match key.code {
                    KeyCode::Tab => {
                        if self.cmd_completion.visible() {
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                self.cmd_completion.select_prev();
                            } else {
                                self.cmd_completion.select_next();
                            }
                            // Preview: fill input with selected candidate
                            if let Some(text) = self.cmd_completion.accept() {
                                input = text;
                            }
                            self.editor.mode = Mode::Command(input);
                            return Ok(());
                        }
                        // No completions — Tab does nothing
                        return Ok(());
                    }
                    _ => {}
                }

                let action = self.editor.handle_command_key(key, &mut input);
                // Re-sync mode (input may have changed)
                if self.editor.mode.is_command() {
                    // Update completion candidates based on new input
                self.cmd_completion.update(&input, &self.locale);
                self.editor.mode = Mode::Command(input);
                } else {
                // Leaving command mode — clear completions
                self.cmd_completion.update("", &self.locale);
                self.cmd_completion.selected = None;
                }
                self.execute_command_action(action)?;
            }
            Mode::Search(mut pattern) => self.handle_search(key, &mut pattern),
            Mode::Ai(mut input) => self.handle_ai_mode(key, &mut input),
        }
        Ok(())
    }

    fn handle_normal(&mut self, key: KeyEvent) -> Result<()> {
        // Ctrl+P — open fuzzy file picker
        if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.open_file_picker();
            return Ok(());
        }

        // Ctrl+F — open global grep panel (empty query, user types then presses Enter)
        if key.code == KeyCode::Char('f') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.open_grep_panel(String::new(), false);
            return Ok(());
        }

        // Cancel plan/query display on any key
        let had_plan = self.plan_lines.is_some();

        // Special case: y/n while plan is showing
        if had_plan {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.apply_plan();
                    return Ok(());
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.plan_lines = None;
                self.ai_query_msg = None;
                self.editor.set_msg(self.locale.messages.ai_plan_cancelled.clone());
                return Ok(());
                }
                _ => {}
            }
        }

        let action = self.editor.handle_normal_key(key);
        match action {
            NormalAction::None => {}
            NormalAction::EnterInsert { col_offset } => {
                let line_len = self.editor.buffer.line_len(self.editor.cursor_line);
                if col_offset > 0 {
                    self.editor.cursor_col = (self.editor.cursor_col + col_offset as usize).min(line_len);
                }
                self.editor.begin_insert_session(col_offset, false, false);
                self.editor.mode = Mode::Insert;
                self.editor.buffer.begin_group();
            }
            NormalAction::EnterInsertNewline { above } => {
                let line = self.editor.cursor_line;
                let indent = if self.editor.config.general.auto_indent {
                    self.editor.buffer.indent_of_line(line)
                } else {
                    String::new()
                };
                if above {
                    let start = self.editor.buffer.line_to_char(line);
                    self.editor.buffer.insert_str(start, &format!("{}\n", indent));
                    self.editor.cursor_col = indent.len();
                } else {
                    let next = self.editor.buffer.line_to_char(line)
                        + self.editor.buffer.line_len(line);
                    self.editor.buffer.insert_str(next, &format!("\n{}", indent));
                    self.editor.cursor_line += 1;
                    self.editor.cursor_col = indent.len();
                }
                self.editor.begin_insert_session(0, above, !above);
                self.editor.scroll_to_cursor();
                self.editor.mode = Mode::Insert;
                self.editor.buffer.begin_group();
            }
            NormalAction::EnterVisual { kind } => {
                let anchor = self.editor.cursor_char_idx();
                self.editor.mode = Mode::Visual { kind, anchor };
            }
            NormalAction::EnterCommand => {
                self.editor.mode = Mode::Command(String::new());
                // Show all commands when entering command mode
                self.cmd_completion.update("", &self.locale);
            }
            NormalAction::EnterSearch => {
                // Save position for incsearch Esc-restore
                self.search_saved_pos = Some((self.editor.cursor_line, self.editor.cursor_col));
                self.editor.mode = Mode::Search(String::new());
            }
            NormalAction::EnterAi => {
                self.ai_query_msg = None;
                self.editor.mode = Mode::Ai(String::new());
            }
            NormalAction::ExecuteCommand(cmd) => {
                let mut input = cmd;
                let action = self.editor.handle_command_key(
                    KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                    &mut input,
                );
                self.execute_command_action(action)?;
            }
            NormalAction::Quit { force } => {
                if force || !self.editor.buffer.modified {
                    self.should_quit = true;
                } else {
                    self.editor.set_msg(self.locale.messages.unsaved_changes.clone());
                }
            }
            NormalAction::OpenFileAtCursor => {
                let word = self.word_under_cursor();
                let path = PathBuf::from(&word);
                if path.exists() {
                    self.open_file(&path)?;
                } else {
                    self.editor.set_msg(self.locale.messages.file_not_found.replace("{path}", &word));
                }
            }
            NormalAction::ToggleFileTree => {
                self.editor.filetree_visible = !self.editor.filetree_visible;
                if self.editor.filetree_visible && self.filetree.is_none() {
                    // Same canonicalize logic as App::new
                    let root = self.filepath.as_deref()
                        .and_then(|p| {
                            let abs = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
                            abs.parent().map(|d| d.to_path_buf())
                        })
                        .or_else(|| std::env::current_dir().ok())
                        .unwrap_or_else(|| PathBuf::from("."));
                    self.filetree = FileTree::new(&root, self.editor.config.filetree.show_hidden).ok();
                }
            }
            NormalAction::ToggleChatPanel => {
                self.chat_visible = !self.chat_visible;
                if self.chat_visible {
                    // Opening chat — focus it and enter input mode automatically
                    self.set_focus(FocusZone::Chat);
                } else if self.focus == FocusZone::Chat {
                    // Closing chat — return focus to editor
                    self.set_focus(FocusZone::Editor);
                }
            }
            NormalAction::ToggleTutorial => {
                self.tutorial_visible = !self.tutorial_visible;
            }
            NormalAction::SwitchFocus => {
                self.cycle_focus();
            }
            NormalAction::ToggleHiddenFiles => {
                if let Some(ft) = &mut self.filetree {
                    ft.toggle_hidden();
                }
            }
            NormalAction::AiAction(_) => {}
            NormalAction::DotRepeat => {
                self.editor.buffer.begin_group();
                self.editor.dot_repeat();
            }
            NormalAction::PlayMacro(reg) => {
                self.play_macro(reg);
            }
        }
        Ok(())
    }

    /// Cycle focus: Editor → FileTree → Chat → Editor (skipping hidden panels).
    fn cycle_focus(&mut self) {
        let zones = self.available_zones();
        if zones.len() <= 1 { return; }
        let cur_idx = zones.iter().position(|z| *z == self.focus).unwrap_or(0);
        let next = zones[(cur_idx + 1) % zones.len()];
        self.set_focus(next);
    }

    /// Cycle focus in reverse: Editor ← FileTree ← Chat ← Editor.
    fn cycle_focus_reverse(&mut self) {
        let zones = self.available_zones();
        if zones.len() <= 1 { return; }
        let cur_idx = zones.iter().position(|z| *z == self.focus).unwrap_or(0);
        let next = zones[(cur_idx + zones.len() - 1) % zones.len()];
        self.set_focus(next);
    }

    fn available_zones(&self) -> Vec<FocusZone> {
        let mut zones = vec![FocusZone::Editor];
        if self.editor.filetree_visible { zones.push(FocusZone::FileTree); }
        if self.chat_visible { zones.push(FocusZone::Chat); }
        zones
    }

    fn set_focus(&mut self, zone: FocusZone) {
        // Leave old zone
        if self.focus == FocusZone::Chat && zone != FocusZone::Chat {
            self.chat_input_active = false;
        }
        self.focus = zone;
        // Enter chat input mode automatically when focusing Chat
        if zone == FocusZone::Chat {
            self.chat_input_active = true;
        }
        // Sync legacy filetree_focus flag
        self.editor.filetree_focus = zone == FocusZone::FileTree;
    }

    fn play_macro(&mut self, reg: char) {
        use crossterm::event::KeyEvent;
        let keys = match self.editor.macros.get(&reg) {
            Some(k) => k.iter().map(|mk| KeyEvent::new(mk.code, mk.modifiers)).collect::<Vec<_>>(),
            None => {
                self.editor.set_msg(self.locale.messages.macro_not_found.replace("{reg}", &reg.to_string()));
                return;
            }
        };
        for key in keys {
            // Feed each stored key back through the normal handler
            // (Only normal-mode replay for now; insert-mode keys are embedded)
            let _ = self.handle_key(key);
        }
    }

    fn handle_insert(&mut self, key: KeyEvent) {
        // Record insert-mode keys into the active macro
        self.editor.macro_append_key(key);
        match self.editor.handle_insert_key(key) {
            InsertAction::ExitToNormal => {
                // If we were in a block-insert session, replicate typed text to remaining lines
                if let Some(bi) = self.block_insert.take() {
                    let typed = self.editor._insert_text.clone();
                    if !typed.is_empty() && bi.end_line > bi.start_line {
                        // Lines after start_line: insert the same text at bi.col
                        self.editor.block_insert_text(bi.start_line + 1, bi.end_line, bi.col, &typed);
                    }
                }
                self.editor.mode = Mode::Normal;
                self.editor.clamp_cursor();
            }
            InsertAction::None => {}
        }
    }

    fn handle_visual(&mut self, key: KeyEvent, kind: VisualKind, anchor: usize) {
        let action = self.editor.handle_visual_key(key, anchor, kind.clone());
        match action {
            VisualAction::ExitToNormal => {
                self.editor.mode = Mode::Normal;
                self.editor.clamp_cursor();
            }
            VisualAction::EnterInsert => {
                self.editor.begin_insert_session(0, false, false);
                self.editor.mode = Mode::Insert;
                self.editor.buffer.begin_group();
            }
            VisualAction::EnterBlockInsert { start_line, end_line, col } => {
                self.block_insert = Some(BlockInsertState { start_line, end_line, col });
                self.editor.cursor_line = start_line;
                self.editor.cursor_col  = col;
                self.editor.clamp_cursor();
                self.editor.begin_insert_session(0, false, false);
                self.editor.mode = Mode::Insert;
                self.editor.buffer.begin_group();
            }
            VisualAction::EnterAi(selected) => {
                self.ai_query_msg = None;
                self.editor.mode = Mode::Ai(selected);
            }
            VisualAction::EnterAiEdit { start_line, end_line, selected_text } => {
                // Prompt user for instruction via Ai input mode, then start session
                // We store the selection info in a temporary field and enter Ai mode.
                // When the user confirms the instruction, start_ai_edit_session is called.
                self.editor.mode = Mode::Normal;
                // Immediately open an Ai input prompt; the selection context is embedded
                // in the prompt hint so the user knows what they selected.
                let hint = format!("[{}-{}] ", start_line + 1, end_line + 1);
                self.editor.mode = Mode::Ai(hint.clone());
                // Store selection for when the user confirms
                self.ai_edit_pending_selection = Some((start_line, end_line, selected_text));
            }
            VisualAction::CopyToClipboard(text) => {
                // Write to system clipboard via pbcopy (macOS) or xclip/xsel (Linux)
                let escaped = text.replace('\'', "'\\''");
                let _ = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(format!(
                        "printf '%s' '{}' | pbcopy 2>/dev/null || \
                         printf '%s' '{}' | xclip -selection clipboard 2>/dev/null || \
                         printf '%s' '{}' | xsel --clipboard --input 2>/dev/null",
                        escaped, escaped, escaped
                    ))
                    .status();
                self.editor.mode = Mode::Normal;
                self.editor.clamp_cursor();
            }
            VisualAction::None => {
                // Mode stays Visual — refresh anchor in case kind changed
                if self.editor.mode.is_visual() {
                    // mode may have been updated inside handle_visual_key (o, v, V, Ctrl-v)
                } else {
                    self.editor.mode = Mode::Visual { kind, anchor };
                }
            }
        }
    }

    fn handle_search(&mut self, key: KeyEvent, pattern: &mut String) {
        match key.code {
            KeyCode::Esc => {
                // Restore cursor to where it was before search started
                if let Some((l, c)) = self.search_saved_pos.take() {
                    self.editor.cursor_line = l;
                    self.editor.cursor_col = c;
                    self.editor.scroll_to_cursor();
                }
                self.editor.search_highlight = false;
                self.editor.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                // Confirm: keep current position, clear saved pos
                self.search_saved_pos = None;
                let pat = pattern.trim().to_string();
                pattern.clear();
                let ic = self.editor.config.general.ignore_case;
                self.editor.run_search(&pat, ic);
                self.editor.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                pattern.pop();
                self.editor.mode = Mode::Search(pattern.clone());
                // Re-run incremental search with shorter pattern
                self.incsearch_update(pattern);
            }
            KeyCode::Char(c) => {
                pattern.push(c);
                self.editor.mode = Mode::Search(pattern.clone());
                self.incsearch_update(pattern);
            }
            _ => {}
        }
    }

    /// Run search without committing (for incsearch preview).
    fn incsearch_update(&mut self, pattern: &str) {
        if pattern.is_empty() {
            self.editor.search_highlight = false;
            // Restore to saved position
            if let Some((l, c)) = self.search_saved_pos {
                self.editor.cursor_line = l;
                self.editor.cursor_col = c;
                self.editor.scroll_to_cursor();
            }
            return;
        }
        let ic = self.editor.config.general.ignore_case;
        // Temporarily restore saved position so search starts from there
        let saved = self.search_saved_pos.unwrap_or((self.editor.cursor_line, self.editor.cursor_col));
        self.editor.cursor_line = saved.0;
        self.editor.cursor_col = saved.1;
        self.editor.run_search(pattern, ic);
    }

    fn handle_ai_mode(&mut self, key: KeyEvent, input: &mut String) {
        // If plan is pending, handle y/n here too
        if self.plan_lines.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.apply_plan();
                    return;
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.plan_lines = None;
                    self.ai_query_msg = None;
                    self.editor.mode = Mode::Normal;
                    return;
                }
                _ => {}
            }
        }

        let action = handle_ai_input_key(input, key);
        match action {
            AiInputAction::Cancel => {
                self.ai_query_msg = None;
                self.editor.mode = Mode::Normal;
            }
            AiInputAction::Submit(query) => {
                // Check if this is a ga-triggered agent-edit (has pending selection)
                if let Some((start_line, end_line, selected_text)) = self.ai_edit_pending_selection.take() {
                    // Strip the "[N-M] " prefix we injected as a hint
                    let instruction = query
                        .trim_start_matches(|c: char| c == '[' || c.is_ascii_digit() || c == '-' || c == ']' || c == ' ')
                        .to_string();
                    let instruction = if instruction.is_empty() { query } else { instruction };
                    self.start_ai_edit_session_selection(instruction, start_line, end_line, selected_text);
                } else {
                    self.dispatch_ai_query(&query);
                }
                self.editor.mode = Mode::Normal;
            }
            AiInputAction::ConfirmGhost => {
                if self.ghost.visible {
                    let text = self.ghost.text.clone();
                    let char_idx = self.editor.cursor_char_idx();
                    self.editor.buffer.insert_str(char_idx, &text);
                    let added = text.chars().count();
                    self.editor.cursor_col += added;
                    self.ghost.visible = false;
                }
                self.editor.mode = Mode::Normal;
            }
            AiInputAction::None => {
                self.editor.mode = Mode::Ai(input.clone());
            }
        }
    }

    // ── Command actions ───────────────────────────────────────────────────────

    fn execute_command_action(&mut self, action: CommandAction) -> Result<()> {
        match action {
            CommandAction::None => {}
            CommandAction::ExitToNormal => {
                self.editor.mode = Mode::Normal;
            }
            CommandAction::Quit { force } => {
                if force || !self.editor.buffer.modified {
                    self.should_quit = true;
                } else {
                    self.editor.set_msg(self.locale.messages.unsaved_changes.clone());
                    self.editor.mode = Mode::Normal;
                }
            }
            CommandAction::SaveAndQuit => {
                match self.editor.buffer.save() {
                    Ok(_) => { self.should_quit = true; }
                    Err(e) => {
                        let msg = if e.to_string().contains("No file path") {
                            self.locale.messages.no_file_name.clone()
                        } else {
                            self.locale.messages.save_failed.replace("{err}", &e.to_string())
                        };
                        self.editor.set_msg(msg);
                        self.editor.mode = Mode::Normal;
                    }
                }
            }
            CommandAction::SetMsg(msg) => {
                self.editor.set_msg(msg);
                self.editor.mode = Mode::Normal;
            }
            CommandAction::OpenFile(path) => {
                self.open_file(&path)?;
            }
            CommandAction::RunSubstitution { range, pattern, replacement, flags } => {
                self.run_substitution(range, &pattern, &replacement, &flags);
                self.editor.mode = Mode::Normal;
            }
            CommandAction::GoToLine(n) => {
                let target = n.saturating_sub(1);
                self.editor.push_jump();
                self.editor.cursor_line = target.min(self.editor.buffer.line_count().saturating_sub(1));
                self.editor.cursor_col = 0;
                self.editor.scroll_to_cursor();
                self.editor.mode = Mode::Normal;
            }
            CommandAction::ToggleLineNumbers(v) => {
                self.editor.config.general.line_numbers = v;
                self.editor.mode = Mode::Normal;
            }
            CommandAction::SetTabWidth(v) => {
                self.editor.config.general.tab_width = v;
                self.editor.mode = Mode::Normal;
            }
            CommandAction::ClearSearch => {
                self.editor.search_highlight = false;
                self.editor.mode = Mode::Normal;
            }
            CommandAction::ShellCommand(cmd) => {
                self.editor.mode = Mode::Normal;
                self.run_shell_command(&cmd);
            }
            CommandAction::DeleteLines { start, end } => {
                let line_count = self.editor.buffer.line_count();
                let start = start.min(line_count.saturating_sub(1));
                let end   = end.min(line_count.saturating_sub(1));
                // Delete from end down to start so indices stay valid
                let mut yanked = String::new();
                for l in (start..=end).rev() {
                    let s = self.editor.buffer.delete_line(l);
                    yanked = if yanked.is_empty() { s } else { format!("{}\n{}", s, yanked) };
                }
                self.editor.buffer.register = yanked;
                self.editor.buffer.register_linewise = true;
                self.editor.cursor_line = start.min(self.editor.buffer.line_count().saturating_sub(1));
                self.editor.clamp_cursor();
                self.editor.mode = Mode::Normal;
            }
            CommandAction::Undo => {
                if let Some(pos) = self.editor.buffer.undo() {
                    self.editor.set_cursor_from_char_idx(pos);
                }
                self.editor.mode = Mode::Normal;
            }
            CommandAction::SetTheme(name) => {
                self.renderer.set_theme(&name);
                // Keep config in sync so theme picker shows the correct selection.
                self.editor.config.theme.editor_theme = name.clone();
                // Persist to ~/.hirc so the theme survives restarts
                if let Err(e) = crate::config::loader::save_theme(&name) {
                    self.editor.set_msg(self.locale.messages.theme_save_failed.replace("{name}", &name).replace("{err}", &e.to_string()));
                } else {
                    self.editor.set_msg(self.locale.messages.theme_saved.replace("{name}", &name));
                }
                self.editor.mode = Mode::Normal;
            }
            CommandAction::OpenThemePicker => {
                let themes: Vec<&'static str> = CodePalette::available_themes().to_vec();
                let current = self.editor.config.theme.editor_theme.clone();
                let cursor = themes.iter().position(|t| *t == current).unwrap_or(0);
                self.theme_picker = Some(ThemePicker {
                    themes,
                    cursor,
                    original_theme: current,
                });
                self.editor.mode = Mode::Normal;
            }
            CommandAction::Preview => {
                let content = self.editor.buffer.rope.to_string();
                let file_path = self.editor.buffer.filepath().map(|p| p.to_path_buf());
                let msg = crate::ui::preview::open_preview(
                    &content,
                    file_path.as_deref(),
                    &self.locale,
                );
                self.editor.set_msg(msg);
                self.editor.mode = Mode::Normal;
            }
            CommandAction::Grep { pattern, is_regex } => {
                self.open_grep_panel(pattern, is_regex);
                self.editor.mode = Mode::Normal;
            }
            CommandAction::ToggleFileTree => {
                self.editor.filetree_visible = !self.editor.filetree_visible;
                if self.editor.filetree_visible && self.filetree.is_none() {
                    let root = self.editor.buffer.filepath()
                        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                        .or_else(|| std::env::current_dir().ok())
                        .unwrap_or_else(|| std::path::PathBuf::from("."));
                    self.filetree = crate::ui::filetree::FileTree::new(&root, self.editor.config.filetree.show_hidden).ok();
                }
                self.editor.mode = Mode::Normal;
            }
            CommandAction::ToggleTutorial => {
                self.tutorial_visible = !self.tutorial_visible;
                self.editor.mode = Mode::Normal;
            }
            CommandAction::ToggleMouse => {
                self.renderer.mouse_enabled = !self.renderer.mouse_enabled;
                let msg = if self.renderer.mouse_enabled {
                    "Mouse mode ON"
                } else {
                    "Mouse mode OFF — keyboard only"
                };
                self.editor.set_msg(msg.to_string());
                self.editor.mode = Mode::Normal;
            }
            CommandAction::AiEdit(instruction) => {
                self.editor.mode = Mode::Normal;
                self.start_ai_edit_session_command(instruction);
            }
            #[cfg(feature = "leetcode")]
            CommandAction::OpenLeetCode => {
                self.editor.mode = Mode::Normal;
                self.leetcode_panel = Some(LeetCodePanel::new());
            }
        }
        Ok(())
    }

    // ── AI agent-edit session ─────────────────────────────────────────────────

    /// Start an agent-edit session from the `:ai <instruction>` command (whole-file mode).
    fn start_ai_edit_session_command(&mut self, instruction: String) {
        self.ai_edit_session_id += 1;
        let session = AiEditSession::from_command(self.ai_edit_session_id, instruction.clone());
        self.ai_edit_session = Some(session);
        self.diff_panel = None;
        self.editor.set_msg(format!("AI editing… [Esc]cancel"));
        self.spawn_ai_edit_turn();
    }

    /// Start an agent-edit session from Visual `ga` (selection mode).
    fn start_ai_edit_session_selection(
        &mut self,
        instruction: String,
        start_line: usize,
        end_line: usize,
        selected_text: String,
    ) {
        self.ai_edit_session_id += 1;
        let session = AiEditSession::from_selection(
            self.ai_edit_session_id,
            instruction.clone(),
            start_line,
            end_line,
            selected_text,
        );
        self.ai_edit_session = Some(session);
        self.diff_panel = None;
        self.editor.set_msg(format!("AI editing selection… [Esc]cancel"));
        self.spawn_ai_edit_turn();
    }

    /// Spawn a background thread to get the next AI response in the Tool-Use Loop.
    fn spawn_ai_edit_turn(&mut self) {
        let session = match self.ai_edit_session.as_mut() {
            Some(s) => s,
            None => return,
        };

        if session.over_limit() {
            session.transition_to_confirm();
            self.diff_panel = Some(DiffPanel::new("Max tool turns reached."));
            return;
        }

        // Build messages for this turn
        let context = AiContext::from_cursor(
            &self.editor.buffer,
            self.editor.buffer.filepath().map(|p| p.to_str().unwrap_or("")).unwrap_or(""),
            self.editor.cursor_line,
            self.editor.cursor_col,
            self.editor.config.ai.context_lines,
            self.renderer.highlighter.filetype().name(),
        );
        let cfg = self.editor.config.ai.clone();

        use crate::ai::prompt::{PromptKind, build_messages};
        let kind = match &session.source {
            crate::ai::EditSource::VisualSelection { selected_text, .. } => {
                PromptKind::AgentEdit {
                    instruction: session.instruction.clone(),
                    selection: selected_text.clone(),
                }
            }
            crate::ai::EditSource::CommandLine => {
                PromptKind::AgentEdit {
                    instruction: session.instruction.clone(),
                    selection: String::new(),
                }
            }
        };

        // First turn: build fresh messages; subsequent turns: append tool result
        if session.messages.is_empty() {
            session.messages = build_messages(&kind, &context, &session.instruction, &self.locale);
        }

        let messages = session.messages.clone();
        let result_arc = Arc::clone(&self.ai_edit_result);

        std::thread::spawn(move || {
            let client = AiClient::new(&cfg);
            let res = client.chat(messages);
            let step = match res {
                Ok(text) => {
                    let thought = extract_thought(&text);
                    if let Some(tool) = parse_tool_call(&text) {
                        AiEditStepResult::ToolCall { tool, thought }
                    } else {
                        // No tool call — treat as Done with the text as summary
                        AiEditStepResult::Done { summary: thought }
                    }
                }
                Err(e) => AiEditStepResult::Error(e.to_string()),
            };
            *result_arc.lock().unwrap() = Some(step);
        });

        session.tool_turns += 1;
        self.ai_status = AiStatus::Requesting;
    }

    /// Poll the AI edit result channel and advance the session state machine.
    fn poll_ai_edit_result(&mut self) -> bool {
        let result = {
            let mut guard = self.ai_edit_result.lock().unwrap();
            guard.take()
        };
        let step = match result {
            Some(s) => s,
            None => return false,
        };

        self.ai_status = AiStatus::Idle;

        match step {
            AiEditStepResult::ToolCall { tool, thought } => {
                // Record thought
                if let Some(session) = self.ai_edit_session.as_mut() {
                    if !thought.is_empty() {
                        session.add_thought(thought.clone());
                    }
                    // Append assistant message
                    session.messages.push(crate::ai::prompt::Message {
                        role: "assistant".into(),
                        content: thought,
                    });
                }

                // Execute the tool (may mutate session.pending_diff)
                let tool_result = self.execute_ai_tool(tool);

                // Append tool result as user message
                if let Some(session) = self.ai_edit_session.as_mut() {
                    session.messages.push(crate::ai::prompt::Message {
                        role: "user".into(),
                        content: format!("Tool result:\n{}", tool_result.to_message()),
                    });
                }

                // Continue the loop
                self.spawn_ai_edit_turn();
            }
            AiEditStepResult::Done { summary } => {
                if let Some(session) = self.ai_edit_session.as_mut() {
                    session.transition_to_confirm();
                    let summary_clone = summary.clone();
                    self.diff_panel = Some(DiffPanel::new(summary_clone));
                    self.editor.set_msg(session.status_text());
                }
            }
            AiEditStepResult::Error(e) => {
                if let Some(session) = self.ai_edit_session.as_mut() {
                    session.transition_to_error(e.clone());
                }
                self.editor.set_msg(format!("AI edit error: {}", e));
                self.ai_edit_session = None;
                self.diff_panel = None;
            }
        }
        true
    }

    /// Execute a single AI tool call and return the result.
    /// Write operations are collected into `session.pending_diff`.
    fn execute_ai_tool(&mut self, tool: AiTool) -> ToolResult {
        let session = match self.ai_edit_session.as_mut() {
            Some(s) => s,
            None => return ToolResult::Error("No active session".into()),
        };

        match tool {
            AiTool::ReadBuffer { start, end } => {
                let total = self.editor.buffer.line_count();
                let end = end.unwrap_or(total.saturating_sub(1)).min(total.saturating_sub(1));
                let start = start.min(end);
                let mut lines = Vec::new();
                for l in start..=end {
                    lines.push((l, self.editor.buffer.line_str(l).to_string()));
                }
                ToolResult::Lines(lines)
            }

            AiTool::ReplaceRange { start, end, new_text } => {
                let total = self.editor.buffer.line_count();
                let end = end.min(total.saturating_sub(1));
                let start = start.min(end);
                // Collect old text for diff display
                let mut old_lines = Vec::new();
                for l in start..=end {
                    old_lines.push(self.editor.buffer.line_str(l).to_string());
                }
                let old_text = old_lines.join("\n");
                let hunk = DiffHunk::replace(start, end, old_text, new_text);
                let count = session.pending_diff.hunks.len() + 1;
                session.pending_diff.push(hunk);
                ToolResult::DiffQueued { hunk_count: count }
            }

            AiTool::InsertAfter { line, text } => {
                let hunk = DiffHunk::insert(line, text);
                let count = session.pending_diff.hunks.len() + 1;
                session.pending_diff.push(hunk);
                ToolResult::DiffQueued { hunk_count: count }
            }

            AiTool::DeleteRange { start, end } => {
                let total = self.editor.buffer.line_count();
                let end = end.min(total.saturating_sub(1));
                let start = start.min(end);
                let mut old_lines = Vec::new();
                for l in start..=end {
                    old_lines.push(self.editor.buffer.line_str(l).to_string());
                }
                let old_text = old_lines.join("\n");
                let hunk = DiffHunk::delete(start, end, old_text);
                let count = session.pending_diff.hunks.len() + 1;
                session.pending_diff.push(hunk);
                ToolResult::DiffQueued { hunk_count: count }
            }

            AiTool::Search { pattern } => {
                let mut matches = Vec::new();
                let total = self.editor.buffer.line_count();
                for l in 0..total {
                    let line = self.editor.buffer.line_str(l);
                    if line.contains(&pattern) {
                        matches.push((l, line.to_string()));
                    }
                }
                if matches.is_empty() {
                    ToolResult::Text(format!("No lines matching {:?}", pattern))
                } else {
                    ToolResult::Lines(matches)
                }
            }

            AiTool::GetOutline => {
                use crate::ai::OutlineItem;
                let mut items = Vec::new();
                let total = self.editor.buffer.line_count();
                for l in 0..total {
                    let line = self.editor.buffer.line_str(l);
                    let trimmed = line.trim_start();
                    if let Some(rest) = trimmed.strip_prefix("######") {
                        items.push(OutlineItem { level: 6, title: rest.trim().to_string(), line: l });
                    } else if let Some(rest) = trimmed.strip_prefix("#####") {
                        items.push(OutlineItem { level: 5, title: rest.trim().to_string(), line: l });
                    } else if let Some(rest) = trimmed.strip_prefix("####") {
                        items.push(OutlineItem { level: 4, title: rest.trim().to_string(), line: l });
                    } else if let Some(rest) = trimmed.strip_prefix("###") {
                        items.push(OutlineItem { level: 3, title: rest.trim().to_string(), line: l });
                    } else if let Some(rest) = trimmed.strip_prefix("##") {
                        items.push(OutlineItem { level: 2, title: rest.trim().to_string(), line: l });
                    } else if let Some(rest) = trimmed.strip_prefix('#') {
                        items.push(OutlineItem { level: 1, title: rest.trim().to_string(), line: l });
                    }
                }
                if items.is_empty() {
                    ToolResult::Text("No outline items found.".into())
                } else {
                    ToolResult::Outline(items)
                }
            }

            AiTool::AskUser { question } => {
                // For now, surface the question in the status bar and pause the loop.
                // A future enhancement could open an input prompt.
                self.editor.set_msg(format!("AI asks: {}", question));
                ToolResult::UserInput("(user input not yet supported — please use :ai to continue)".into())
            }

            AiTool::Done { summary } => {
                // The Done tool is handled by the caller (poll_ai_edit_result),
                // but if it arrives here via execute_ai_tool, treat it gracefully.
                session.transition_to_confirm();
                let summary_clone = summary.clone();
                self.diff_panel = Some(DiffPanel::new(summary_clone));
                ToolResult::Text(format!("Done: {}", summary))
            }
        }
    }

    /// Apply the pending diff to the buffer (called when user presses 'y' in DiffPanel).
    fn apply_ai_edit_diff(&mut self) {
        let session = match self.ai_edit_session.take() {
            Some(s) => s,
            None => return,
        };
        self.diff_panel = None;

        if session.pending_diff.is_empty() {
            self.editor.set_msg("AI: no changes to apply.");
            return;
        }

        self.editor.buffer.begin_group();
        // Apply hunks in reverse order so line numbers stay valid
        let mut hunks = session.pending_diff.hunks.clone();
        hunks.sort_by(|a, b| b.start_line.cmp(&a.start_line));

        for hunk in &hunks {
            match hunk.kind {
                HunkKind::Replace => {
                    let start = hunk.start_line;
                    let end   = hunk.end_line;
                    let char_start = self.editor.buffer.line_to_char(start);
                    let is_last = end + 1 >= self.editor.buffer.line_count();
                    let char_end = if is_last {
                        self.editor.buffer.len_chars()
                    } else {
                        self.editor.buffer.line_to_char(end + 1)
                    };
                    let delete_count = char_end.saturating_sub(char_start);
                    self.editor.buffer.delete(char_start, delete_count);
                    let to_insert = if is_last {
                        hunk.new_text.clone()
                    } else {
                        format!("{}\n", hunk.new_text)
                    };
                    self.editor.buffer.insert(char_start, &to_insert);
                }
                HunkKind::Insert => {
                    let after = hunk.start_line;
                    let line_end = self.editor.buffer.line_to_char(after)
                        + self.editor.buffer.line_len(after);
                    self.editor.buffer.insert(line_end, &format!("\n{}", hunk.new_text));
                }
                HunkKind::Delete => {
                    let start = hunk.start_line;
                    let end   = hunk.end_line;
                    let char_start = self.editor.buffer.line_to_char(start);
                    let is_last = end + 1 >= self.editor.buffer.line_count();
                    let char_end = if is_last {
                        self.editor.buffer.len_chars()
                    } else {
                        self.editor.buffer.line_to_char(end + 1)
                    };
                    let delete_count = char_end.saturating_sub(char_start);
                    self.editor.buffer.delete(char_start, delete_count);
                }
            }
        }

        self.editor.clamp_cursor();
        self.editor.scroll_to_cursor();
        self.editor.set_msg(format!("AI edits applied ({} hunk(s)).", hunks.len()));
    }

    /// Handle keyboard input while the DiffPanel overlay is visible.
    fn handle_diff_panel_key(&mut self, key: KeyEvent) -> Result<()> {
        let total_hunks = self.ai_edit_session
            .as_ref()
            .map(|s| s.pending_diff.hunks.len())
            .unwrap_or(0);

        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                self.apply_ai_edit_diff();
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.ai_edit_session = None;
                self.diff_panel = None;
                self.editor.set_msg("AI edit cancelled.");
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(dp) = self.diff_panel.as_mut() {
                    dp.next_hunk(total_hunks);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(dp) = self.diff_panel.as_mut() {
                    dp.prev_hunk(total_hunks);
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(dp) = self.diff_panel.as_mut() {
                    dp.scroll_down();
                }
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(dp) = self.diff_panel.as_mut() {
                    dp.scroll_up();
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ── Theme picker ──────────────────────────────────────────────────────────

    fn handle_theme_picker_key(&mut self, key: KeyEvent) {
        let picker = match self.theme_picker.as_mut() {
            Some(p) => p,
            None => return,
        };
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if picker.cursor + 1 < picker.themes.len() {
                    picker.cursor += 1;
                    // Live preview
                    let name = picker.themes[picker.cursor];
                    self.renderer.set_theme(name);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if picker.cursor > 0 {
                    picker.cursor -= 1;
                    // Live preview
                    let name = picker.themes[picker.cursor];
                    self.renderer.set_theme(name);
                }
            }
            KeyCode::Enter => {
                let name = picker.themes[picker.cursor];
                self.renderer.set_theme(name);
                // Persist to ~/.hirc so the theme survives restarts
                if let Err(e) = crate::config::loader::save_theme(name) {
                    self.editor.set_msg(self.locale.messages.theme_save_failed.replace("{name}", name).replace("{err}", &e.to_string()));
                } else {
                    self.editor.set_msg(self.locale.messages.theme_saved.replace("{name}", name));
                }
                self.theme_picker = None;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                // Restore original theme
                let original = picker.original_theme.clone();
                self.renderer.set_theme(&original);
                self.theme_picker = None;
            }
            _ => {}
        }
    }

    /// Execute a shell command via `:!{cmd}` and display the output in the message bar.
    fn run_shell_command(&mut self, cmd: &str) {
        use std::process::Command;
        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output();
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let combined = if stderr.is_empty() {
                    stdout.trim_end().to_string()
                } else if stdout.is_empty() {
                    stderr.trim_end().to_string()
                } else {
                    format!("{}\n{}", stdout.trim_end(), stderr.trim_end())
                };
                // Show first line in message bar; store full output for display
                let first_line = combined.lines().next().unwrap_or("(no output)").to_string();
                let status = if out.status.success() { "" } else { " [exit error]" };
                self.editor.set_msg(format!("!{}: {}{}", cmd, first_line, status));
                // If multi-line, store all lines as shell_output for overlay
                if combined.lines().count() > 1 {
                    self.editor.shell_output = Some(combined);
                } else {
                    self.editor.shell_output = None;
                }
            }
            Err(e) => {
                self.editor.set_msg(self.locale.messages.shell_error.replace("{err}", &e.to_string()));
                self.editor.shell_output = None;
            }
        }
    }

    // ── Substitution ──────────────────────────────────────────────────────────

    fn run_substitution(
        &mut self,
        range: crate::mode::command::SubRange,
        pattern: &str,
        replacement: &str,
        flags: &str,
    ) {
        use crate::mode::command::SubRange;
        let global = flags.contains('g');
        let ignore_case = flags.contains('i') || self.editor.config.general.ignore_case;

        let re_str = if ignore_case {
            format!("(?i){}", regex::escape(pattern))
        } else {
            regex::escape(pattern).to_string()
        };

        let re = match regex::Regex::new(&re_str) {
            Ok(r) => r,
            Err(e) => {
                self.editor.set_msg(format!("Bad pattern: {}", e));
                return;
            }
        };

        let total_lines = self.editor.buffer.line_count();
        let (start_line, end_line) = match range {
            SubRange::CurrentLine => (self.editor.cursor_line, self.editor.cursor_line),
            SubRange::WholeFile => (0, total_lines.saturating_sub(1)),
            SubRange::Lines(s, e) => (s, e.min(total_lines.saturating_sub(1))),
        };

        let mut count = 0usize;
        // Process in reverse to keep line indices stable
        for l in (start_line..=end_line).rev() {
            let line_text = self.editor.buffer.line_str(l);
            let new_line = if global {
                let repl: &str = replacement;
                let result = re.replace_all(&line_text, repl);
                if result == line_text { continue; }
                result.into_owned()
            } else {
                let repl: &str = replacement;
                let result = re.replace(&line_text, repl);
                if result == line_text { continue; }
                result.into_owned()
            };

            let start_char = self.editor.buffer.line_to_char(l);
            let old_len = line_text.chars().count();
            // Remove trailing newline from replacement check
            let end_char = start_char + old_len;
            self.editor.buffer.delete_range(start_char, end_char);
            self.editor.buffer.insert_str(start_char, &new_line);
            count += 1;
        }

        self.editor.set_msg(format!("替换 {} 处", count));
    }

    // ── File operations ───────────────────────────────────────────────────────

    fn open_file(&mut self, path: &Path) -> Result<()> {
        let buf = Buffer::from_file(path)?;
        let ft = path.extension()
            .and_then(|e| e.to_str())
            .map(FileType::from_ext)
            .unwrap_or(FileType::Plain);
        self.renderer.set_filetype(ft);
        // Full parse so the tree is ready before the first render.
        let source = buf.rope.to_string();
        self.renderer.ts_hl.full_parse(&source);
        self.editor.buffer = buf;
        self.editor.cursor_line = 0;
        self.editor.cursor_col = 0;
        self.editor.scroll_line = 0;
        self.editor.mode = Mode::Normal;
        self.filepath = Some(path.to_path_buf());
        // Refresh file tree if visible
        if self.editor.filetree_visible {
            if let Some(dir) = path.parent() {
                self.filetree = FileTree::new(dir, self.editor.config.filetree.show_hidden).ok();
            }
        }
        Ok(())
    }

    fn word_under_cursor(&self) -> String {
        let line = self.editor.buffer.line_str(self.editor.cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let col = self.editor.cursor_col.min(chars.len().saturating_sub(1));
        let is_w = |c: char| c.is_alphanumeric() || "/_.-".contains(c);
        let mut s = col;
        let mut e = col;
        if col < chars.len() {
            while s > 0 && is_w(chars[s-1]) { s -= 1; }
            while e < chars.len() && is_w(chars[e]) { e += 1; }
        }
        chars[s..e].iter().collect()
    }

    // ── File tree key handling ─────────────────────────────────────────────────

    fn handle_filetree_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            // Move cursor down
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ft) = &mut self.filetree {
                    ft.move_down();
                }
            }
            // Move cursor up
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(ft) = &mut self.filetree {
                    ft.move_up();
                }
            }
            // Enter / expand dir / open file
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                let selected = self.filetree.as_mut().and_then(|ft| ft.enter());
                if let Some(path) = selected {
                    // It was a file — open it and switch focus back to editor
                    self.open_file(&path)?;
                    self.set_focus(FocusZone::Editor);
                }
                // If it was a dir, ft.enter() already toggled expansion, nothing else needed
            }
            // Collapse dir / go to parent
            KeyCode::Char('h') | KeyCode::Left => {
                if let Some(ft) = &mut self.filetree {
                    ft.collapse_or_parent();
                }
            }
            // Switch focus back to editor
            KeyCode::Esc | KeyCode::Char('q') => {
                self.set_focus(FocusZone::Editor);
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cycle_focus();
            }
            // Tab — cycle focus
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.cycle_focus_reverse();
                } else {
                    self.cycle_focus();
                }
            }
            // Toggle hidden files
            KeyCode::Char('H') => {
                if let Some(ft) = &mut self.filetree {
                    ft.toggle_hidden();
                }
            }
            // Go to top / bottom  (gg / G style, single key for simplicity)
            KeyCode::Char('g') => {
                if let Some(ft) = &mut self.filetree {
                    ft.cursor = 0;
                }
            }
            KeyCode::Char('G') => {
                if let Some(ft) = &mut self.filetree {
                    let last = ft.nodes.len().saturating_sub(1);
                    ft.cursor = last;
                }
            }
            // a — new file
            KeyCode::Char('a') => {
                self.filetree_prompt = Some(FileTreePrompt::NewFile { input: String::new() });
            }
            // A — new directory
            KeyCode::Char('A') => {
                self.filetree_prompt = Some(FileTreePrompt::NewDir { input: String::new() });
            }
            // d — delete selected
            KeyCode::Char('d') => {
                if let Some(ft) = &self.filetree {
                    if let Some(path) = ft.selected_path() {
                        self.filetree_prompt = Some(FileTreePrompt::Delete {
                            path: path.to_path_buf(),
                            confirmed: false,
                        });
                    }
                }
            }
            // r — rename selected
            KeyCode::Char('r') => {
                if let Some(ft) = &self.filetree {
                    if let Some(path) = ft.selected_path() {
                        let name = path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();
                        self.filetree_prompt = Some(FileTreePrompt::Rename {
                            original: path.to_path_buf(),
                            input: name,
                        });
                    }
                }
            }
            // y — copy path to clipboard (via pbcopy / xclip)
            KeyCode::Char('y') => {
                if let Some(ft) = &self.filetree {
                    if let Some(path) = ft.selected_path() {
                        let path_str = path.to_string_lossy().to_string();
                        // Try pbcopy (macOS) then xclip (Linux)
                        let _ = std::process::Command::new("sh")
                            .arg("-c")
                            .arg(format!("echo -n '{}' | pbcopy 2>/dev/null || echo -n '{}' | xclip -selection clipboard 2>/dev/null", path_str, path_str))
                            .status();
                        self.editor.set_msg(format!("已复制路径: {}", path_str));
                    }
                }
            }
            // R — refresh
            KeyCode::Char('R') => {
                if let Some(ft) = &mut self.filetree {
                    ft.refresh();
                }
            }
            // / — search in file tree
            KeyCode::Char('/') => {
                if let Some(ft) = &mut self.filetree {
                    ft.start_search();
                }
                self.filetree_prompt = Some(FileTreePrompt::Search { input: String::new() });
            }
            _ => {}
        }
        Ok(())
    }

    // ── Chat panel key handling ──────────────────────────────────────────────

    fn handle_chat_key(&mut self, key: KeyEvent) -> Result<()> {
        // Chat panel is always in input mode when focused — no separate browse mode.
        // Ensure input is active (should already be set by set_focus).
        if !self.chat_input_active {
            self.chat_input_active = true;
        }
        self.handle_chat_input_key(key)
    }

    /// Handle keys while typing in the chat input line.
    fn handle_chat_input_key(&mut self, key: KeyEvent) -> Result<()> {
        let chars: Vec<char> = self.chat_input.chars().collect();
        let len = chars.len();
        match key.code {
            KeyCode::Esc => {
                // Exit chat — return focus to editor
                self.chat_input.clear();
                self.chat_input_cursor = 0;
                self.set_focus(FocusZone::Editor);
            }
            KeyCode::Enter => {
                // Submit the message, stay in input mode for follow-up
                let query = self.chat_input.trim().to_string();
                self.chat_input.clear();
                self.chat_input_cursor = 0;
                // Stay in input mode — no need to press i again
                if !query.is_empty() {
                    self.dispatch_ai_query_as_advisor(&query);
                }
            }
            KeyCode::Backspace => {
                if self.chat_input_cursor > 0 {
                    let mut c = chars.clone();
                    c.remove(self.chat_input_cursor - 1);
                    self.chat_input = c.into_iter().collect();
                    self.chat_input_cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if self.chat_input_cursor < len {
                    let mut c = chars.clone();
                    c.remove(self.chat_input_cursor);
                    self.chat_input = c.into_iter().collect();
                }
            }
            KeyCode::Left => {
                if self.chat_input_cursor > 0 {
                    self.chat_input_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.chat_input_cursor < len {
                    self.chat_input_cursor += 1;
                }
            }
            KeyCode::Home => {
                self.chat_input_cursor = 0;
            }
            KeyCode::End => {
                self.chat_input_cursor = len;
            }
            // Ctrl+a — move to start
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.chat_input_cursor = 0;
            }
            // Ctrl+e — move to end
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.chat_input_cursor = len;
            }
            // Ctrl+u — delete to start
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let remaining: String = chars[self.chat_input_cursor..].iter().collect();
                self.chat_input = remaining;
                self.chat_input_cursor = 0;
            }
            // Ctrl+k — delete to end
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let remaining: String = chars[..self.chat_input_cursor].iter().collect();
                self.chat_input = remaining;
            }
            // Ctrl+w — delete previous word
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.chat_input_cursor > 0 {
                    let mut pos = self.chat_input_cursor - 1;
                    while pos > 0 && chars[pos].is_whitespace() { pos -= 1; }
                    while pos > 0 && !chars[pos - 1].is_whitespace() { pos -= 1; }
                    let mut c = chars;
                    c.drain(pos..self.chat_input_cursor);
                    self.chat_input = c.into_iter().collect();
                    self.chat_input_cursor = pos;
                }
            }
            // Tab / Shift+Tab — cycle focus
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.cycle_focus_reverse();
                } else {
                    self.cycle_focus();
                }
            }
            // Ctrl+l — close chat panel
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.chat_visible = false;
                self.set_focus(FocusZone::Editor);
            }
            // PageUp / PageDown — scroll chat history
            KeyCode::PageUp => { self.chat_panel.scroll_up(10); }
            KeyCode::PageDown => { self.chat_panel.scroll_down(10); }
            // Ctrl+p / Ctrl+n — scroll chat history one line
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.chat_panel.scroll_up(1);
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.chat_panel.scroll_down(1);
            }
            // Ctrl+d — clear chat history
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.chat_panel.clear();
                self.editor.set_msg(self.locale.messages.chat_cleared.clone());
            }
            KeyCode::Char(c) => {
                let mut cv = chars;
                cv.insert(self.chat_input_cursor, c);
                self.chat_input = cv.into_iter().collect();
                self.chat_input_cursor += 1;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key input while a file-tree prompt is active.
    fn handle_filetree_prompt_key(&mut self, key: KeyEvent) -> Result<()> {
        use std::fs;

        // ── Search prompt has special real-time behavior ──
        let is_search = matches!(&self.filetree_prompt, Some(FileTreePrompt::Search { .. }));

        if is_search {
            match key.code {
                KeyCode::Esc => {
                    // Cancel search, restore full tree
                    if let Some(ft) = &mut self.filetree {
                        ft.cancel_search();
                    }
                    self.filetree_prompt = None;
                }
                KeyCode::Enter => {
                    // Confirm search: keep cursor position, exit search mode
                    if let Some(ft) = &mut self.filetree {
                        ft.end_search();
                    }
                    self.filetree_prompt = None;
                }
                KeyCode::Backspace => {
                    if let Some(FileTreePrompt::Search { input }) = &mut self.filetree_prompt {
                        input.pop();
                        let query = input.clone();
                        if let Some(ft) = &mut self.filetree {
                            ft.update_filter(&query);
                        }
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(FileTreePrompt::Search { input }) = &mut self.filetree_prompt {
                        input.push(c);
                        let query = input.clone();
                        if let Some(ft) = &mut self.filetree {
                            ft.update_filter(&query);
                        }
                    }
                }
                // Allow j/k navigation while searching
                KeyCode::Down => {
                    if let Some(ft) = &mut self.filetree { ft.move_down(); }
                }
                KeyCode::Up => {
                    if let Some(ft) = &mut self.filetree { ft.move_up(); }
                }
                _ => {}
            }
            return Ok(());
        }

        // ── Non-search prompts (NewFile, NewDir, Rename, Delete) ──
        match key.code {
            KeyCode::Esc => {
                self.filetree_prompt = None;
            }
            KeyCode::Enter => {
                let prompt = self.filetree_prompt.take();
                match prompt {
                    Some(FileTreePrompt::NewFile { input }) => {
                        let base = self.filetree_base_dir();
                        let new_path = base.join(input.trim());
                        match fs::File::create(&new_path) {
                            Ok(_) => {
                                if let Some(ft) = &mut self.filetree { ft.refresh(); }
                                self.editor.set_msg(format!("已创建: {}", new_path.display()));
                            }
                            Err(e) => self.editor.set_msg(format!("创建失败: {}", e)),
                        }
                    }
                    Some(FileTreePrompt::NewDir { input }) => {
                        let base = self.filetree_base_dir();
                        let new_path = base.join(input.trim());
                        match fs::create_dir_all(&new_path) {
                            Ok(_) => {
                                if let Some(ft) = &mut self.filetree { ft.refresh(); }
                                self.editor.set_msg(format!("已创建目录: {}", new_path.display()));
                            }
                            Err(e) => self.editor.set_msg(format!("创建失败: {}", e)),
                        }
                    }
                    Some(FileTreePrompt::Rename { original, input }) => {
                        let new_path = original.parent()
                            .map(|p| p.join(input.trim()))
                            .unwrap_or_else(|| PathBuf::from(input.trim()));
                        match fs::rename(&original, &new_path) {
                            Ok(_) => {
                                if let Some(ft) = &mut self.filetree { ft.refresh(); }
                                self.editor.set_msg(format!("已重命名为: {}", new_path.display()));
                            }
                            Err(e) => self.editor.set_msg(format!("重命名失败: {}", e)),
                        }
                    }
                    Some(FileTreePrompt::Delete { path, .. }) => {
                        // Enter without y/n — treat as cancel
                        self.editor.set_msg(format!("已取消删除: {}", path.display()));
                    }
                    Some(FileTreePrompt::Search { .. }) => { /* handled above */ }
                    None => {}
                }
            }
            // Delete confirm: y
            KeyCode::Char('y') => {
                if let Some(FileTreePrompt::Delete { path, .. }) = self.filetree_prompt.take() {
                    let result = if path.is_dir() {
                        std::fs::remove_dir_all(&path)
                    } else {
                        std::fs::remove_file(&path)
                    };
                    match result {
                        Ok(_) => {
                            if let Some(ft) = &mut self.filetree { ft.refresh(); }
                            self.editor.set_msg(format!("已删除: {}", path.display()));
                        }
                        Err(e) => self.editor.set_msg(format!("删除失败: {}", e)),
                    }
                } else if let Some(prompt) = &mut self.filetree_prompt {
                    if let Some(input) = prompt.input_mut() {
                        input.push('y');
                    }
                }
            }
            // Delete confirm: n
            KeyCode::Char('n') => {
                if let Some(FileTreePrompt::Delete { path, .. }) = &self.filetree_prompt {
                    self.editor.set_msg(format!("已取消删除: {}", path.display()));
                    self.filetree_prompt = None;
                } else if let Some(prompt) = &mut self.filetree_prompt {
                    if let Some(input) = prompt.input_mut() {
                        input.push('n');
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(prompt) = &mut self.filetree_prompt {
                    if let Some(input) = prompt.input_mut() {
                        input.pop();
                    }
                }
            }
            KeyCode::Char(c) => {
                if let Some(prompt) = &mut self.filetree_prompt {
                    if let Some(input) = prompt.input_mut() {
                        input.push(c);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ── Mouse handling ──────────────────────────────────────────────────────

    /// Compute the layout regions for mouse hit-testing.
    /// Returns (ft_width, edit_x, edit_w, chat_x, chat_width, edit_h).
    fn layout_regions(&self) -> (usize, usize, usize, usize, usize, usize) {
        let w = self.editor.term_width as usize;
        let h = self.editor.term_height as usize;
        let ft_width = if self.editor.filetree_visible {
            self.editor.config.filetree.width as usize
        } else { 0 };
        let chat_width = if self.chat_visible {
            (self.editor.config.chat.width as usize).min(w / 2)
        } else { 0 };
        let edit_x = ft_width + if ft_width > 0 { 1 } else { 0 };
        let chat_total = chat_width + if chat_width > 0 { 1 } else { 0 };
        let edit_w = w.saturating_sub(edit_x).saturating_sub(chat_total);
        let edit_h = h.saturating_sub(2);
        let chat_x = w.saturating_sub(chat_width);
        (ft_width, edit_x, edit_w, chat_x, chat_width, edit_h)
    }

    /// Determine which focus zone a screen coordinate falls in.
    fn zone_at(&self, col: u16, _row: u16) -> FocusZone {
        let (ft_width, _edit_x, _edit_w, chat_x, chat_width, _edit_h) = self.layout_regions();
        let x = col as usize;
        if chat_width > 0 && x >= chat_x {
            FocusZone::Chat
        } else if ft_width > 0 && x < ft_width {
            FocusZone::FileTree
        } else {
            FocusZone::Editor
        }
    }

    /// Handle mouse scroll: scroll the panel under the mouse pointer.
    fn handle_mouse_scroll(&mut self, col: u16, row: u16, up: bool) {
        let zone = self.zone_at(col, row);
        let amount = 3usize; // lines per scroll tick
        match zone {
            FocusZone::Editor => {
                if up {
                    self.editor.scroll_line = self.editor.scroll_line.saturating_sub(amount);
                } else {
                    let max = self.editor.buffer.line_count().saturating_sub(1);
                    self.editor.scroll_line = (self.editor.scroll_line + amount).min(max);
                }
                // Clamp cursor into the scroll_off safe zone so that the next
                // keypress won't cause scroll_to_cursor() to re-adjust the viewport.
                let (_, _, _, _, _, edit_h) = self.layout_regions();
                let off = self.editor.config.general.scroll_off;
                let buf_max = self.editor.buffer.line_count().saturating_sub(1);
                // Top safe boundary: scroll_line + scroll_off (but not past buf_max)
                let safe_top = (self.editor.scroll_line + off).min(buf_max);
                // Bottom safe boundary: last visible line - scroll_off (but not below safe_top)
                let last_visible = (self.editor.scroll_line + edit_h).saturating_sub(1).min(buf_max);
                let safe_bot = last_visible.saturating_sub(off).max(safe_top);
                self.editor.cursor_line = self.editor.cursor_line.clamp(safe_top, safe_bot);
            }
            FocusZone::Chat => {
                if up {
                    self.chat_panel.scroll_up(amount);
                } else {
                    self.chat_panel.scroll_down(amount);
                }
            }
            FocusZone::FileTree => {
                if let Some(ft) = &mut self.filetree {
                    if up {
                        ft.cursor = ft.cursor.saturating_sub(amount);
                    } else {
                        let max = ft.nodes.len().saturating_sub(1);
                        ft.cursor = (ft.cursor + amount).min(max);
                    }
                }
            }
        }
    }

    /// Handle mouse click: set focus to the clicked zone and position cursor.
    fn handle_mouse_click(&mut self, col: u16, row: u16) {
        let zone = self.zone_at(col, row);
        let (_ft_width, edit_x, _edit_w, chat_x, _chat_width, edit_h) = self.layout_regions();

        // Switch focus to the clicked zone
        self.set_focus(zone);

        match zone {
            FocusZone::Editor => {
                let r = row as usize;
                if r < edit_h {
                    let buf_line = self.editor.scroll_line + r;
                    if buf_line < self.editor.buffer.line_count() {
                        self.editor.cursor_line = buf_line;
                        // Convert display column to char index
                        let gutter = if self.editor.config.general.line_numbers {
                            self.editor.gutter_width()
                        } else { 0 };
                        let click_display_col = (col as usize).saturating_sub(edit_x + gutter);
                        let line = self.editor.buffer.line_str(buf_line);
                        let mut char_col = 0usize;
                        let mut display_acc = 0usize;
                        for ch in line.chars() {
                            let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                            if display_acc + w > click_display_col { break; }
                            display_acc += w;
                            char_col += 1;
                        }
                        self.editor.cursor_col = char_col;
                        self.editor.clamp_cursor();
                        // Exit any existing Visual mode so the next drag starts
                        // a fresh selection anchored at this click position.
                        if self.editor.mode.is_visual() {
                            self.editor.mode = Mode::Normal;
                        }
                    }
                }
            }
            FocusZone::FileTree => {
                let r = row as usize;
                if let Some(ft) = &mut self.filetree {
                    if r < ft.nodes.len() {
                        ft.cursor = r;
                    }
                }
            }
            FocusZone::Chat => {
                // Click in chat area — if clicking on the input line, activate input
                let input_row = {
                    let input_row_count = 1;
                    let usable_h = edit_h.saturating_sub(input_row_count);
                    let content_h = usable_h.saturating_sub(1);
                    (1 + content_h) as u16
                };
                if row == input_row {
                    self.chat_input_active = true;
                    // Position cursor based on click column within input
                    let prefix_w = 2usize; // "▶ " is 2 display columns
                    let click_in_input = (col as usize).saturating_sub(chat_x + prefix_w);
                    let chars: Vec<char> = self.chat_input.chars().collect();
                    let mut char_pos = 0usize;
                    let mut display_acc = 0usize;
                    for ch in &chars {
                        let w = unicode_width::UnicodeWidthChar::width(*ch).unwrap_or(0);
                        if display_acc + w > click_in_input { break; }
                        display_acc += w;
                        char_pos += 1;
                    }
                    self.chat_input_cursor = char_pos.min(chars.len());
                }
            }
        }
    }

    /// Handle mouse drag: in the editor area, start or extend Visual mode selection.
    fn handle_mouse_drag(&mut self, col: u16, row: u16) {
        let (_ft_width, edit_x, _edit_w, _chat_x, _chat_width, edit_h) = self.layout_regions();
        let x = col as usize;
        let r = row as usize;

        // Only handle drag in the editor area
        if x < edit_x || r >= edit_h { return; }

        let buf_line = self.editor.scroll_line + r;
        if buf_line >= self.editor.buffer.line_count() { return; }

        // Convert display column to char index
        let gutter = if self.editor.config.general.line_numbers {
            self.editor.gutter_width()
        } else { 0 };
        let click_display_col = x.saturating_sub(edit_x + gutter);
        let line = self.editor.buffer.line_str(buf_line);
        let mut char_col = 0usize;
        let mut display_acc = 0usize;
        for ch in line.chars() {
            let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if display_acc + w > click_display_col { break; }
            display_acc += w;
            char_col += 1;
        }

        // If not already in Visual mode, enter it with anchor at current cursor
        if !self.editor.mode.is_visual() {
            let anchor = self.editor.cursor_char_idx();
            self.editor.mode = Mode::Visual { kind: VisualKind::Char, anchor };
        }

        // Move cursor to drag position
        self.editor.cursor_line = buf_line;
        self.editor.cursor_col = char_col;
        self.editor.clamp_cursor();
    }

    /// Return the directory to use as base for new file/dir creation.
    fn filetree_base_dir(&self) -> PathBuf {
        if let Some(ft) = &self.filetree {
            if let Some(path) = ft.selected_path() {
                if path.is_dir() {
                    return path.to_path_buf();
                }
                if let Some(parent) = path.parent() {
                    return parent.to_path_buf();
                }
            }
            return ft.root.clone();
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    // ── Fuzzy File Picker ────────────────────────────────────────────────────

    fn open_file_picker(&mut self) {
        let root = if let Some(ft) = &self.filetree {
            ft.root.clone()
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };
        self.file_picker = Some(FilePicker::new(root));
    }

    fn handle_file_picker_key(&mut self, key: KeyEvent) -> Result<()> {
        let picker = match self.file_picker.as_mut() {
            Some(p) => p,
            None => return Ok(()),
        };

        match key.code {
            KeyCode::Esc => {
                self.file_picker = None;
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                picker.move_up();
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                picker.move_down();
            }
            KeyCode::Up => {
                picker.move_up();
            }
            KeyCode::Down => {
                picker.move_down();
            }
            KeyCode::Enter => {
                if let Some(path) = picker.selected_path() {
                    self.file_picker = None;
                    self.open_path(&path)?;
                }
            }
            KeyCode::Backspace => {
                picker.pop_char();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL)
                             && !key.modifiers.contains(KeyModifiers::ALT) => {
                picker.push_char(c);
            }
            _ => {}
        }
        Ok(())
    }

    // ── Global Grep Panel ────────────────────────────────────────────────────

    fn open_grep_panel(&mut self, pattern: String, is_regex: bool) {
        let root = if let Some(ft) = &self.filetree {
            ft.root.clone()
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };
        let mut panel = GrepPanel::new(root);
        panel.query = pattern;
        panel.is_regex = is_regex;
        // If a pattern was provided (e.g. from :grep), run the search immediately.
        if !panel.query.is_empty() {
            panel.run_search();
        }
        self.grep_panel = Some(panel);
    }

    fn handle_grep_panel_key(&mut self, key: KeyEvent) -> Result<()> {
        let panel = match self.grep_panel.as_mut() {
            Some(p) => p,
            None => return Ok(()),
        };

        match key.code {
            KeyCode::Esc => {
                self.grep_panel = None;
            }
            KeyCode::Enter => {
                if !panel.searched || panel.query.is_empty() {
                    // First Enter runs the search
                    panel.run_search();
                } else if let Some(m) = panel.selected() {
                    // Second Enter (or Enter on a result) jumps to the match
                    let path = m.path.clone();
                    let line_no = m.line_no.saturating_sub(1); // 0-based
                    let col = m.match_start;
                    self.grep_panel = None;
                    self.open_path(&path)?;
                    self.editor.cursor_line = line_no.min(self.editor.buffer.rope.len_lines().saturating_sub(1));
                    self.editor.cursor_col = col;
                    // Scroll so the target line is roughly centred in the viewport
                    let half = (self.editor.term_height as usize / 2).max(1);
                    self.editor.scroll_line = self.editor.cursor_line.saturating_sub(half);
                }
            }
            KeyCode::Up => { panel.move_up(); }
            KeyCode::Down => { panel.move_down(); }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                panel.move_up();
            }
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                panel.move_down();
            }
            KeyCode::Backspace => {
                panel.pop_char();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL)
                             && !key.modifiers.contains(KeyModifiers::ALT) => {
                panel.push_char(c);
            }
            _ => {}
        }
        Ok(())
    }

    /// Open a file path in the editor (reuse existing open_file logic).
    fn open_path(&mut self, path: &Path) -> Result<()> {
        if path.is_dir() {
            // Open as file tree root
            if let Ok(ft) = FileTree::new(path, self.editor.config.filetree.show_hidden) {
                self.filetree = Some(ft);
                self.editor.filetree_visible = true;
                self.focus = FocusZone::FileTree;
            }
            return Ok(());
        }
        match Buffer::from_file(path) {
            Ok(buf) => {
                let ft = path.extension()
                    .and_then(|e| e.to_str())
                    .map(FileType::from_ext)
                    .unwrap_or(FileType::Plain);
                self.renderer.set_filetype(ft);
                // Full parse so the tree is ready before the first render.
                let source = buf.rope.to_string();
                self.renderer.ts_hl.full_parse(&source);
                self.editor.buffer = buf;
                self.editor.cursor_line = 0;
                self.editor.cursor_col = 0;
                self.editor.scroll_line = 0;
                self.editor.mode = Mode::Normal;
                self.filepath = Some(path.to_path_buf());
                // Update file tree root to the new file's directory
                if let Some(parent) = path.parent() {
                    if let Ok(ft_new) = FileTree::new(parent, self.editor.config.filetree.show_hidden) {
                        self.filetree = Some(ft_new);
                    }
                }
                self.editor.set_msg(format!("Opened {}", path.display()));
            }
            Err(e) => {
                self.editor.set_msg(format!("Cannot open {}: {}", path.display(), e));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod intent_tests {
    use super::App;
    use crate::ai::prompt::PromptKind;

    fn kind_name(q: &str) -> &'static str {
        match App::infer_intent(q).0 {
            PromptKind::Plan        => "plan",
            PromptKind::Advisor     => "advisor",
            PromptKind::Complete    => "complete",
            PromptKind::Transform(_)=> "transform",
        }
    }

    // ── Explicit overrides ────────────────────────────────────────────────────
    #[test] fn explicit_plan_prefix()    { assert_eq!(kind_name("!删除所有注释"), "plan"); }
    #[test] fn explicit_advisor_prefix() { assert_eq!(kind_name("?这段代码是什么意思"), "advisor"); }

    // ── Action verbs → Plan ───────────────────────────────────────────────────
    #[test] fn chinese_action_replace()  { assert_eq!(kind_name("替换所有 TODO 为 FIXME"), "plan"); }
    #[test] fn chinese_action_refactor() { assert_eq!(kind_name("帮我重构这个函数"), "plan"); }
    #[test] fn chinese_action_delete()   { assert_eq!(kind_name("删除第 5 行"), "plan"); }
    #[test] fn english_action_fix()      { assert_eq!(kind_name("fix the null pointer bug"), "plan"); }
    #[test] fn english_action_rename()   { assert_eq!(kind_name("rename this variable to count"), "plan"); }

    // ── Question words → Advisor ──────────────────────────────────────────────
    #[test] fn chinese_question_how()    { assert_eq!(kind_name("怎么实现单例模式"), "advisor"); }
    #[test] fn chinese_question_what()   { assert_eq!(kind_name("这个函数是什么意思"), "advisor"); }
    #[test] fn english_question_how()    { assert_eq!(kind_name("how does this work"), "advisor"); }
    #[test] fn ends_with_question_mark() { assert_eq!(kind_name("这段代码有问题吗？"), "advisor"); }

    // ── Short query → Advisor (no longer Completion) ──────────────────────────
    #[test] fn short_query_advisor()     { assert_eq!(kind_name("fn main"), "advisor"); }
    #[test] fn short_query_code()        { assert_eq!(kind_name("impl Display"), "advisor"); }

    // ── Default → Advisor ─────────────────────────────────────────────────────
    #[test] fn long_ambiguous_advisor()  { assert_eq!(kind_name("这段代码的整体逻辑结构看起来比较清晰易读"), "advisor"); }
}
