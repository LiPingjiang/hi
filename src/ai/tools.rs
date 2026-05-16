//! AI 编辑工具集 — AI 在 Tool-Use Loop 中可以调用的操作。
//!
//! 设计原则：
//! - 读操作（ReadBuffer / Search / GetOutline）直接执行，结果返回给 AI
//! - 写操作（ReplaceRange / InsertAfter / DeleteRange）收集到 pending_diff，不立即修改 Buffer
//! - 所有写操作需要用户在 DiffPanel 中确认后才真正 apply

use serde::{Deserialize, Serialize};

// ── Tool call (AI → Editor) ───────────────────────────────────────────────────

/// AI 可以调用的编辑器工具。
/// 序列化格式：JSON，由 AI 在响应中输出。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tool", content = "args")]
pub enum AiTool {
    /// 读取 buffer 的指定行范围（0-based，end 为 -1 表示到文件末尾）
    #[serde(rename = "read_buffer")]
    ReadBuffer { start: usize, end: Option<usize> },

    /// 替换指定行范围的内容（0-based，inclusive）
    #[serde(rename = "replace_range")]
    ReplaceRange { start: usize, end: usize, new_text: String },

    /// 在指定行后插入新内容（0-based，插入到该行之后）
    #[serde(rename = "insert_after")]
    InsertAfter { line: usize, text: String },

    /// 删除指定行范围（0-based，inclusive）
    #[serde(rename = "delete_range")]
    DeleteRange { start: usize, end: usize },

    /// 搜索包含 pattern 的行，返回行号列表
    #[serde(rename = "search")]
    Search { pattern: String },

    /// 获取文档大纲（Markdown 标题层级）
    #[serde(rename = "get_outline")]
    GetOutline,

    /// 向用户提问（暂停等待输入）
    #[serde(rename = "ask_user")]
    AskUser { question: String },

    /// 任务完成，输出总结
    #[serde(rename = "done")]
    Done { summary: String },
}

// ── Tool result (Editor → AI) ─────────────────────────────────────────────────

/// 工具执行结果，注入回 AI 的 messages 中。
#[derive(Debug, Clone)]
pub enum ToolResult {
    /// 文本内容（read_buffer / search 结果）
    Text(String),
    /// 行列表（行号 + 内容）
    Lines(Vec<(usize, String)>),
    /// 文档大纲条目
    Outline(Vec<OutlineItem>),
    /// 用户输入（ask_user 的回答）
    UserInput(String),
    /// 写操作已收集到 pending_diff（不立即执行）
    DiffQueued { hunk_count: usize },
    /// 错误
    Error(String),
}

impl ToolResult {
    /// 序列化为注入 AI messages 的字符串。
    pub fn to_message(&self) -> String {
        match self {
            ToolResult::Text(s) => s.clone(),
            ToolResult::Lines(lines) => {
                lines.iter()
                    .map(|(n, l)| format!("{}: {}", n + 1, l))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            ToolResult::Outline(items) => {
                items.iter()
                    .map(|i| format!("{}{} (line {})", "  ".repeat(i.level.saturating_sub(1)), i.title, i.line + 1))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            ToolResult::UserInput(s) => format!("User answered: {}", s),
            ToolResult::DiffQueued { hunk_count } => {
                format!("OK: {} edit hunk(s) queued for user confirmation.", hunk_count)
            }
            ToolResult::Error(e) => format!("ERROR: {}", e),
        }
    }
}

/// 文档大纲条目（Markdown 标题）。
#[derive(Debug, Clone)]
pub struct OutlineItem {
    /// 标题级别（1-6）
    pub level: usize,
    /// 标题文本
    pub title: String,
    /// 所在行（0-based）
    pub line: usize,
}

// ── Diff types ────────────────────────────────────────────────────────────────

/// 一次 AI 编辑会话产生的所有 diff。
#[derive(Debug, Clone, Default)]
pub struct EditDiff {
    pub hunks: Vec<DiffHunk>,
}

impl EditDiff {
    pub fn is_empty(&self) -> bool {
        self.hunks.is_empty()
    }

    pub fn push(&mut self, hunk: DiffHunk) {
        self.hunks.push(hunk);
    }

    /// 按行号排序（渲染时从上到下展示）。
    pub fn sort(&mut self) {
        self.hunks.sort_by_key(|h| h.start_line);
    }
}

/// 单个 diff hunk：一段连续的行变更。
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// 变更类型
    pub kind: HunkKind,
    /// 起始行（0-based）
    pub start_line: usize,
    /// 结束行（0-based，inclusive；对于 InsertAfter 等于 start_line）
    pub end_line: usize,
    /// 原始文本（空字符串表示纯插入）
    pub old_text: String,
    /// 新文本（空字符串表示纯删除）
    pub new_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HunkKind {
    /// 替换（replace_range）
    Replace,
    /// 插入（insert_after）
    Insert,
    /// 删除（delete_range）
    Delete,
}

impl DiffHunk {
    pub fn replace(start: usize, end: usize, old_text: String, new_text: String) -> Self {
        Self { kind: HunkKind::Replace, start_line: start, end_line: end, old_text, new_text }
    }

    pub fn insert(after_line: usize, text: String) -> Self {
        Self { kind: HunkKind::Insert, start_line: after_line, end_line: after_line, old_text: String::new(), new_text: text }
    }

    pub fn delete(start: usize, end: usize, old_text: String) -> Self {
        Self { kind: HunkKind::Delete, start_line: start, end_line: end, old_text, new_text: String::new() }
    }

    /// 用于状态栏/面板的简短描述。
    pub fn summary(&self) -> String {
        match self.kind {
            HunkKind::Replace => format!("Replace lines {}-{}", self.start_line + 1, self.end_line + 1),
            HunkKind::Insert  => format!("Insert after line {}", self.start_line + 1),
            HunkKind::Delete  => format!("Delete lines {}-{}", self.start_line + 1, self.end_line + 1),
        }
    }
}
