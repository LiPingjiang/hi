//! Application glue: event loop, mode dispatch, AI integration.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use crate::ai::{AiClient, AiContext, HintKind};
use crate::ai::parser::{parse_plan, apply_steps};
use crate::ai::prompt::{build_messages, PromptKind};
use crate::buffer::Buffer;
use crate::config::Config;
use crate::editor::Editor;
use crate::mode::{Mode, VisualKind};
use crate::mode::command::CommandAction;
use crate::mode::insert::InsertAction;
use crate::mode::normal::NormalAction;
use crate::mode::visual::VisualAction;
use crate::mode::ai::{handle_ai_input_key, AiInputAction};
use crate::syntax::highlight::FileType;
use crate::ui::filetree::FileTree;
use crate::ui::ghost::GhostText;
use crate::ui::renderer::Renderer;

/// Pending file-tree prompt (new file / new dir / rename / delete confirm).
#[derive(Debug, Clone)]
pub enum FileTreePrompt {
    NewFile  { input: String },
    NewDir   { input: String },
    Rename   { original: std::path::PathBuf, input: String },
    Delete   { path: std::path::PathBuf, confirmed: bool },
}

impl FileTreePrompt {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NewFile  { .. } => "新建文件: ",
            Self::NewDir   { .. } => "新建目录: ",
            Self::Rename   { .. } => "重命名: ",
            Self::Delete   { .. } => "删除? [y/n]: ",
        }
    }
    pub fn input_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::NewFile  { input } | Self::NewDir { input } | Self::Rename { input, .. } => Some(input),
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
    editor: Editor,
    renderer: Renderer,
    filetree: Option<FileTree>,
    ghost: GhostText,

    // AI async state
    ai_result: Arc<Mutex<Option<HintKind>>>,
    ai_pending: bool,
    ai_query_msg: Option<String>,       // single-line result shown in hint bar
    plan_lines: Option<Vec<String>>,    // plan overlay content

    should_quit: bool,
    filepath: Option<PathBuf>,

    // incsearch: cursor position when '/' was pressed (for Esc restore)
    search_saved_pos: Option<(usize, usize)>,

    // Visual Block insert: when Some, on Esc from Insert we replicate typed text to all lines
    block_insert: Option<BlockInsertState>,

    // File tree prompt (new file / rename / delete confirm)
    filetree_prompt: Option<FileTreePrompt>,
}

impl App {
    /// Create a new App, optionally loading a file.
    pub fn new(config: Config, filepath: Option<&Path>, width: u16, height: u16) -> Result<Self> {
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

        let renderer = Renderer::new(ft);
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
            editor,
            renderer,
            filetree,
            ghost: GhostText::default(),
            ai_result: Arc::new(Mutex::new(None)),
            ai_pending: false,
            ai_query_msg: None,
            plan_lines: None,
            should_quit: false,
            filepath: filepath.map(PathBuf::from),
            search_saved_pos: None,
            block_insert: None,
            filetree_prompt: None,
        })
    }

    /// Main run loop.
    pub fn run(&mut self) -> Result<()> {
        self.renderer.init()?;

        loop {
            // Render
            self.renderer.render(
                &self.editor,
                &self.filetree,
                &self.ghost,
                &self.ai_query_msg,
                &self.plan_lines,
                &self.filetree_prompt,
            )?;

            // Clear one-shot status message after render
            self.editor.status_msg = None;

            // Poll AI result
            self.poll_ai_result();

            if self.should_quit {
                break;
            }

            // Wait for input (100ms timeout so AI poll runs regularly)
            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key) => {
                        // If shell output overlay is visible, any key dismisses it
                        if self.editor.shell_output.is_some() {
                            self.editor.shell_output = None;
                        } else if self.filetree_prompt.is_some() {
                            self.handle_filetree_prompt_key(key)?;
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
                        match me.kind {
                            MouseEventKind::ScrollDown => {
                                // Scroll down = content moves up = cursor moves down
                                if self.editor.filetree_focus {
                                    if let Some(ft) = &mut self.filetree {
                                        ft.move_down();
                                    }
                                } else {
                                    self.editor.move_down(3);
                                    self.editor.scroll_to_cursor();
                                }
                            }
                            MouseEventKind::ScrollUp => {
                                // Scroll up = content moves down = cursor moves up
                                if self.editor.filetree_focus {
                                    if let Some(ft) = &mut self.filetree {
                                        ft.move_up();
                                    }
                                } else {
                                    self.editor.move_up(3);
                                    self.editor.scroll_to_cursor();
                                }
                            }
                            MouseEventKind::Down(MouseButton::Left) => {
                                // Click in file tree area
                                let ft_width = self.editor.config.filetree.width as u16;
                                if self.editor.filetree_visible && me.column < ft_width {
                                    self.editor.filetree_focus = true;
                                    if let Some(ft) = &mut self.filetree {
                                        ft.cursor = me.row as usize;
                                    }
                                } else {
                                    self.editor.filetree_focus = false;
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        self.renderer.cleanup()?;
        Ok(())
    }

    // ── AI polling ────────────────────────────────────────────────────────────

    fn poll_ai_result(&mut self) {
        if !self.ai_pending { return; }
        let result = {
            let mut guard = self.ai_result.lock().unwrap();
            guard.take()
        };
        if let Some(hint) = result {
            self.ai_pending = false;
            self.apply_ai_hint(hint);
        }
    }

    fn apply_ai_hint(&mut self, hint: HintKind) {
        match &hint {
            HintKind::Advisor(text) => {
                // Show in hint bar (first line) and set ghost explanation
                self.ai_query_msg = Some(text.clone());
                self.ghost.explanation = text.lines().next().unwrap_or("").to_string();
            }
            HintKind::Plan(steps) => {
                self.plan_lines = Some(steps.clone());
                if self.editor.config.ai.yolo_mode {
                    // yolo_mode: skip confirmation, execute immediately
                    self.editor.set_msg(format!("AI 自动执行 {} 步 (yolo)", steps.len()));
                    self.apply_plan();
                } else {
                    self.editor.set_msg(format!("AI 计划 {} 步 — [y]确认  [n]取消", steps.len()));
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
                self.editor.set_msg(format!("AI error: {}", e));
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

        // Action verbs → Plan
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

        // Question words → Advisor
        let advisor_triggers = [
            "怎么", "如何", "什么是", "什么意思", "什么", "为什么", "解释", "说明", "介绍",
            "what ", "how ", "why ", "explain", "describe", "what's", "whats",
        ];
        if advisor_triggers.iter().any(|t| lower.contains(t)) || q.ends_with('?') || q.ends_with('？') {
            return (PromptKind::Advisor, q.to_string());
        }

        // Short query with no clear signal → inline Completion
        if q.chars().count() <= 15 {
            return (PromptKind::Complete, q.to_string());
        }

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
        };
        self.editor.set_msg(format!("AI 思考中… [{}]  [Esc]取消", intent_label));

        let messages = build_messages(&kind, &context, &real_query);
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
    }

    fn apply_plan(&mut self) {
        if let Some(plan_steps) = self.plan_lines.take() {
            // Re-parse from canonical step format
            let raw = plan_steps.join("\n");
            let steps = parse_plan(&raw);
            self.editor.buffer.begin_group();
            if let Err(e) = apply_steps(&mut self.editor.buffer, &steps) {
                self.editor.set_msg(format!("计划执行失败: {}", e));
            } else {
                self.editor.set_msg(format!("计划已应用 {} 步", steps.len()));
            }
            self.editor.clamp_cursor();
            self.editor.scroll_to_cursor();
            self.editor.mode = Mode::Normal;
        }
    }

    // ── Key dispatch ──────────────────────────────────────────────────────────

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // When the file tree has focus, all keys go to the tree handler
        if self.editor.filetree_visible && self.editor.filetree_focus {
            return self.handle_filetree_key(key);
        }
        match self.editor.mode.clone() {
            Mode::Normal => self.handle_normal(key)?,
            Mode::Insert => self.handle_insert(key),
            Mode::Visual { kind, anchor } => self.handle_visual(key, kind, anchor),
            Mode::Command(mut input) => {
                let action = self.editor.handle_command_key(key, &mut input);
                // Re-sync mode (input may have changed)
                if self.editor.mode.is_command() {
                    self.editor.mode = Mode::Command(input);
                }
                self.execute_command_action(action)?;
            }
            Mode::Search(mut pattern) => self.handle_search(key, &mut pattern),
            Mode::Ai(mut input) => self.handle_ai_mode(key, &mut input),
        }
        Ok(())
    }

    fn handle_normal(&mut self, key: KeyEvent) -> Result<()> {
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
                    self.editor.set_msg("计划已取消".to_string());
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
                    self.editor.set_msg("未保存的修改！用 :q! 强制退出或 :wq 保存退出".to_string());
                }
            }
            NormalAction::OpenFileAtCursor => {
                let word = self.word_under_cursor();
                let path = PathBuf::from(&word);
                if path.exists() {
                    self.open_file(&path)?;
                } else {
                    self.editor.set_msg(format!("File not found: {}", word));
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
            NormalAction::SwitchFocus => {
                self.editor.filetree_focus = !self.editor.filetree_focus;
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

    fn play_macro(&mut self, reg: char) {
        use crossterm::event::KeyEvent;
        let keys = match self.editor.macros.get(&reg) {
            Some(k) => k.iter().map(|mk| KeyEvent::new(mk.code, mk.modifiers)).collect::<Vec<_>>(),
            None => {
                self.editor.set_msg(format!("宏 @{} 不存在", reg));
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
                self.dispatch_ai_query(&query);
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
            AiInputAction::ConfirmPlan => {
                self.apply_plan();
            }
            AiInputAction::CancelPlan => {
                self.plan_lines = None;
                self.ai_query_msg = None;
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
                    self.editor.set_msg("未保存的修改！用 :q! 强制退出或 :wq 保存退出".to_string());
                    self.editor.mode = Mode::Normal;
                }
            }
            CommandAction::SaveAndQuit => {
                if let Err(e) = self.editor.buffer.save() {
                    self.editor.set_msg(format!("保存失败: {}", e));
                    self.editor.mode = Mode::Normal;
                } else {
                    self.should_quit = true;
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
        }
        Ok(())
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
                self.editor.set_msg(format!("shell error: {}", e));
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
                    self.editor.filetree_focus = false;
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
                self.editor.filetree_focus = false;
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.editor.filetree_focus = false;
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
            _ => {}
        }
        Ok(())
    }

    /// Handle key input while a file-tree prompt is active.
    fn handle_filetree_prompt_key(&mut self, key: KeyEvent) -> Result<()> {
        use std::fs;
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

    // ── Short query → Completion ──────────────────────────────────────────────
    #[test] fn short_query_completion()  { assert_eq!(kind_name("fn main"), "complete"); }
    #[test] fn short_query_code()        { assert_eq!(kind_name("impl Display"), "complete"); }

    // ── Default → Advisor ─────────────────────────────────────────────────────
    #[test] fn long_ambiguous_advisor()  { assert_eq!(kind_name("这段代码的整体逻辑结构看起来比较清晰易读"), "advisor"); }
}
