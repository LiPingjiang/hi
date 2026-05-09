//! Builds system + user prompt messages for different AI request kinds.

use crate::ai::context::AiContext;

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
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

pub fn build_messages(kind: &PromptKind, ctx: &AiContext, query: &str) -> Vec<Message> {
    let system = system_prompt(kind, ctx);
    let user   = user_prompt(kind, ctx, query);
    vec![
        Message { role: "system".into(), content: system },
        Message { role: "user".into(),   content: user   },
    ]
}

fn system_prompt(kind: &PromptKind, ctx: &AiContext) -> String {
    match kind {
        PromptKind::Advisor | PromptKind::Complete => {
            format!(
                "You are an expert {} programming assistant embedded in a terminal text editor called `hi`.\n\
                 The user is editing the file `{}` ({} total lines).\n\
                 Respond concisely. Prefer code over prose. Never include markdown fences unless asked.",
                ctx.language, ctx.filepath, ctx.total_lines
            )
        }
        PromptKind::Plan => {
            format!(
                "You are an expert {} programming assistant embedded in the `hi` editor.\n\
                 When asked to make changes, respond with a numbered list of atomic edit steps.\n\
                 Each step must be one of:\n\
                 - INSERT line <N>: <text>  (0-based line index)\n\
                 - DELETE line <N>\n\
                 - REPLACE line <N>: <new text>\n\
                 - REPLACE range <N>-<M>: <new text (multi-line ok, use \\n)>\n\
                 - MESSAGE: <advice with no edit>\n\
                 Output ONLY the numbered steps, no prose.",
                ctx.language
            )
        }
        PromptKind::Transform(instruction) => {
            format!(
                "You are a {} code transformation assistant in the `hi` editor.\n\
                 Task: {}\n\
                 Return ONLY the transformed code, preserving indentation. No markdown fences.",
                ctx.language, instruction
            )
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
    }
}
