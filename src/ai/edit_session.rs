//! AI 编辑会话状态机。
//!
//! 一次 `AiEditSession` 对应用户发起的一次 AI 编辑请求（`:ai <指令>` 或 Visual + `ga`）。
//! 会话在 App 层持有，驱动 Tool-Use Loop 的异步执行。
//!
//! # 生命周期
//!
//! ```text
//! 用户触发
//!   → Thinking（AI 调用工具，读取/收集 diff）
//!   → AwaitingConfirm（展示 DiffPanel，等待 y/n）
//!   → Applying（apply_diff 写入 Buffer）
//!   → Done / Error
//! ```

use crate::ai::tools::{AiTool, EditDiff};
use crate::ai::prompt::Message;

// ── Session phase ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum EditPhase {
    /// AI 正在思考 / 调用工具（异步进行中）
    Thinking,
    /// 等待用户在 DiffPanel 中确认（y/n）
    AwaitingConfirm,
    /// 用户已确认，正在 apply diff
    Applying,
    /// 完成
    Done { summary: String },
    /// 出错
    Error(String),
}

// ── Session ───────────────────────────────────────────────────────────────────

/// 一次 AI 编辑会话的完整状态。
pub struct AiEditSession {
    /// 用户的原始指令
    pub instruction: String,

    /// 触发来源
    pub source: EditSource,

    /// 当前执行阶段
    pub phase: EditPhase,

    /// AI 的思考过程摘要（用于 DiffPanel 底部展示）
    pub thoughts: Vec<String>,

    /// 已收集的 diff（写操作暂存区）
    pub pending_diff: EditDiff,

    /// Tool-Use Loop 的消息历史（system + user + assistant + tool 轮次）
    pub messages: Vec<Message>,

    /// 已执行的工具调用轮次数（防止无限循环）
    pub tool_turns: usize,

    /// 最大工具调用轮次
    pub max_tool_turns: usize,

    /// 会话 ID（用于 undo 断点标记）
    pub id: u64,
}

/// 编辑请求的触发来源。
#[derive(Debug, Clone)]
pub enum EditSource {
    /// Visual 模式选区 + `ga`
    VisualSelection {
        start_line: usize,
        end_line: usize,
        selected_text: String,
    },
    /// `:ai <instruction>` 命令（全文模式）
    CommandLine,
}

impl AiEditSession {
    /// 创建新会话（Visual 选区模式）。
    pub fn from_selection(
        id: u64,
        instruction: String,
        start_line: usize,
        end_line: usize,
        selected_text: String,
    ) -> Self {
        Self {
            instruction,
            source: EditSource::VisualSelection { start_line, end_line, selected_text },
            phase: EditPhase::Thinking,
            thoughts: Vec::new(),
            pending_diff: EditDiff::default(),
            messages: Vec::new(),
            tool_turns: 0,
            max_tool_turns: 8,
            id,
        }
    }

    /// 创建新会话（命令行模式）。
    pub fn from_command(id: u64, instruction: String) -> Self {
        Self {
            instruction,
            source: EditSource::CommandLine,
            phase: EditPhase::Thinking,
            thoughts: Vec::new(),
            pending_diff: EditDiff::default(),
            messages: Vec::new(),
            tool_turns: 0,
            max_tool_turns: 8,
            id,
        }
    }

    /// 是否正在等待用户确认。
    pub fn is_awaiting_confirm(&self) -> bool {
        self.phase == EditPhase::AwaitingConfirm
    }

    /// 是否已完成（Done 或 Error）。
    pub fn is_terminal(&self) -> bool {
        matches!(self.phase, EditPhase::Done { .. } | EditPhase::Error(_))
    }

    /// 记录一条 AI 思考摘要。
    pub fn add_thought(&mut self, thought: impl Into<String>) {
        self.thoughts.push(thought.into());
    }

    /// 推进到 AwaitingConfirm 阶段（AI 已完成工具调用，diff 已收集完毕）。
    pub fn transition_to_confirm(&mut self) {
        self.pending_diff.sort();
        self.phase = EditPhase::AwaitingConfirm;
    }

    /// 推进到 Done 阶段。
    pub fn transition_to_done(&mut self, summary: String) {
        self.phase = EditPhase::Done { summary };
    }

    /// 推进到 Error 阶段。
    pub fn transition_to_error(&mut self, err: String) {
        self.phase = EditPhase::Error(err);
    }

    /// 是否超过最大工具调用轮次。
    pub fn over_limit(&self) -> bool {
        self.tool_turns >= self.max_tool_turns
    }

    /// 选区范围（仅 VisualSelection 模式有效）。
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        match &self.source {
            EditSource::VisualSelection { start_line, end_line, .. } => {
                Some((*start_line, *end_line))
            }
            EditSource::CommandLine => None,
        }
    }

    /// 状态栏显示的简短状态文字。
    pub fn status_text(&self) -> String {
        match &self.phase {
            EditPhase::Thinking => {
                format!("AI editing… turn {}/{}  [Esc]cancel", self.tool_turns, self.max_tool_turns)
            }
            EditPhase::AwaitingConfirm => {
                let n = self.pending_diff.hunks.len();
                if n == 0 {
                    "AI: no changes needed  [Esc]close".into()
                } else {
                    format!("AI diff: {} hunk(s) — [y]apply  [n]cancel  [j/k]scroll", n)
                }
            }
            EditPhase::Applying => "Applying AI edits…".into(),
            EditPhase::Done { summary } => format!("AI done: {}", summary),
            EditPhase::Error(e) => format!("AI error: {}", e),
        }
    }
}

// ── Async result type ─────────────────────────────────────────────────────────

/// AI Tool-Use Loop 的单步结果，由 spawn_blocking 线程返回给主线程。
#[derive(Debug)]
pub enum AiEditStepResult {
    /// AI 调用了一个工具，需要主线程执行并返回结果
    ToolCall {
        tool: AiTool,
        /// AI 的原始响应文本（含思考过程）
        thought: String,
    },
    /// AI 完成了所有工具调用，diff 已收集完毕
    Done {
        summary: String,
    },
    /// AI 出错
    Error(String),
}
