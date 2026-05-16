//! Builds system + user prompt messages for different AI request kinds.
//!
//! Design: locale-aware system prompts are built from `Locale::ai` strings,
//! so the AI assistant speaks the user's language by default.

use crate::ai::context::AiContext;
use crate::locale::Locale;

#[derive(Debug, Clone)]
pub enum PromptKind {
    /// `?<query>` – free-form question / advice
    Advisor,
    /// `?!<intent>` – generate a concrete execution plan (list of edits)
    Plan,
    /// Ghost-text inline completion suggestion
    Complete,
    /// Refactor/transform the selected text
    Transform(String),
    /// Agent-edit mode: Tool-Use Loop with read/write tools.
    /// `instruction` is the user's high-level intent.
    /// `selection` is the pre-selected text (empty for whole-file mode).
    AgentEdit { instruction: String, selection: String },
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

pub fn build_messages(kind: &PromptKind, ctx: &AiContext, query: &str, locale: &Locale) -> Vec<Message> {
    build_messages_with_history(kind, ctx, query, &[], locale)
}

/// Build messages with conversation history injected between system and user.
/// `history` is a list of (user_content, assistant_content) pairs, oldest first.
pub fn build_messages_with_history(
    kind: &PromptKind,
    ctx: &AiContext,
    query: &str,
    history: &[(&str, &str)],
    locale: &Locale,
) -> Vec<Message> {
    let system = system_prompt(kind, ctx, locale);
    let user   = user_prompt(kind, ctx, query);
    let mut msgs = Vec::with_capacity(2 + history.len() * 2);
    msgs.push(Message { role: "system".into(), content: system });
    for (user_msg, asst_msg) in history {
        msgs.push(Message { role: "user".into(), content: user_msg.to_string() });
        msgs.push(Message { role: "assistant".into(), content: asst_msg.to_string() });
    }
    msgs.push(Message { role: "user".into(), content: user });
    msgs
}

// ── Per-scenario system prompts ───────────────────────────────────────────

fn system_prompt(kind: &PromptKind, ctx: &AiContext, locale: &Locale) -> String {
    let ai = &locale.ai;
    let file_info = format!(
        "Current file: `{}` ({}, {} lines total)",
        if ctx.filepath.is_empty() { "[unsaved]" } else { &ctx.filepath },
        ctx.language,
        ctx.total_lines,
    );

    match kind {
        PromptKind::Advisor => {
            let role = ai.role_advisor
                .replace("{file_info}", &file_info);
            format!("{}\n\n{}", ai.product_guide, role)
        }

        PromptKind::Complete => {
            ai.role_complete.replace("{file_info}", &file_info)
        }

        PromptKind::Plan => {
            let role = ai.role_plan
                .replace("{file_info}", &file_info);
            format!("{}\n\n{}", ai.product_guide, role)
        }

        PromptKind::Transform(instruction) => {
            ai.role_transform
                .replace("{file_info}", &file_info)
                .replace("{instruction}", instruction)
        }

        PromptKind::AgentEdit { .. } => {
            ai.role_agent_edit
                .replace("{file_info}", &file_info)
                .replace("{tool_spec}", TOOL_SPEC)
        }
    }
}

fn user_prompt(kind: &PromptKind, ctx: &AiContext, query: &str) -> String {
    let snippet_header = format!(
        "File: {}\nCursor at line {} col {}\nContext:\n```{}\n{}\n```",
        if ctx.filepath.is_empty() { "[unsaved]" } else { &ctx.filepath },
        ctx.cursor_line + 1,
        ctx.cursor_col + 1,
        ctx.language,
        ctx.snippet,
    );

    match kind {
        PromptKind::Advisor | PromptKind::Complete => {
            format!("{}\n\nQuestion: {}", snippet_header, query)
        }
        PromptKind::Plan => {
            format!("{}\n\nRequest: {}", snippet_header, query)
        }
        PromptKind::Transform(_) => {
            format!("{}\n\nTransform the selected text above.", snippet_header)
        }
        PromptKind::AgentEdit { instruction, selection } => {
            if selection.is_empty() {
                format!(
                    "{}\n\nInstruction: {}\n\nPlease use the tool API to read the document and make the requested changes.",
                    snippet_header, instruction
                )
            } else {
                format!(
                    "{}\n\nSelected text:\n```\n{}\n```\n\nInstruction: {}\n\nPlease use the tool API to apply the requested changes to the selected region.",
                    snippet_header, selection, instruction
                )
            }
        }
    }
}

// ── Tool specification injected into the AgentEdit system prompt ──────────────

/// JSON-schema-style description of all available tools, injected into the
/// AgentEdit system prompt so the model knows what it can call.
const TOOL_SPEC: &str = r#"## Available Tools

Respond with a JSON object on a single line starting with `TOOL:` to call a tool.

```
TOOL: {"tool": "<name>", "args": { ... }}
```

### read_buffer
Read lines from the document (0-based line numbers).
Args: `start` (int), `end` (int | null — null means end of file)
Returns: numbered lines

### replace_range
Replace lines start..=end (0-based, inclusive) with new_text.
Args: `start` (int), `end` (int), `new_text` (string, use \n for newlines)

### insert_after
Insert text after the given line (0-based).
Args: `line` (int), `text` (string)

### delete_range
Delete lines start..=end (0-based, inclusive).
Args: `start` (int), `end` (int)

### search
Find lines matching a literal pattern.
Args: `pattern` (string)
Returns: list of matching line numbers and content

### get_outline
Get the document outline (Markdown headings / code structure).
Args: (none)

### ask_user
Ask the user a clarifying question (pauses the loop).
Args: `question` (string)

### done
Signal that all edits are complete.
Args: `summary` (string — one-line description of what was changed)
"#;
