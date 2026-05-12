//! Builds system + user prompt messages for different AI request kinds.
//!
//! Design: a shared product knowledge base (`PRODUCT_GUIDE`) is combined with
//! per-scenario role instructions so the AI always knows what `hi` is and how
//! it works, while receiving task-specific directives.

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
    build_messages_with_history(kind, ctx, query, &[])
}

/// Build messages with conversation history injected between system and user.
/// `history` is a list of (user_content, assistant_content) pairs, oldest first.
pub fn build_messages_with_history(
    kind: &PromptKind,
    ctx: &AiContext,
    query: &str,
    history: &[(&str, &str)],
) -> Vec<Message> {
    let system = system_prompt(kind, ctx);
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

// ── Product knowledge base ────────────────────────────────────────────────

/// Comprehensive guide to the `hi` editor, shared across all prompt scenarios.
/// This ensures the AI always knows the product identity, features, and keybindings.
const PRODUCT_GUIDE: &str = "\
# hi — Terminal Text Editor

You are the built-in AI assistant of `hi`, a Vim-style terminal text editor written in Rust.
Your name is `hi assistant`. You are NOT OpenClaw, ChatGPT, Claude, or any other AI product.
When asked who you are, say you are the AI assistant embedded in the `hi` editor.

## Modes

hi has 6 modes. The current mode is shown in the status bar.

- NORMAL: default mode for navigation and commands
- INSERT: text input mode
- VISUAL / V-LINE / V-BLOCK: selection modes (char / line / block)
- COMMAND: Ex commands (`:` prefix)
- SEARCH: incremental search (`/` prefix)
- AI: AI query input (`?` prefix in editor, or type directly in chat panel)

## Focus Zones

The UI has three focus zones, cycled with Tab or Ctrl+w:
- Editor (main text area)
- FileTree (left sidebar, toggle with Ctrl+t)
- Chat (right AI panel, toggle with Ctrl+l)

## Normal Mode Keybindings

Movement:
  h/j/k/l or arrow keys — left/down/up/right
  w/b/e — word forward / back / end
  W/B/E — WORD forward / back / end (whitespace-delimited)
  0/^/$ — line start / first non-blank / line end
  gg/G — file top / bottom
  {/} — paragraph back / forward
  % — matching bracket
  H/M/L — screen top / middle / bottom
  f/F/t/T + char — find char forward/backward (till variant)
  ;/, — repeat / reverse last f/F/t/T
  n/N — next / previous search match
  */# — search word under cursor forward / backward
  Ctrl+d/u — half page down / up
  Ctrl+f/b — full page down / up
  Ctrl+o/Ctrl+i — jump back / forward in jump list

Editing:
  x/X — delete char at / before cursor
  dd — delete line (with count: 3dd)
  yy — yank line
  cc — change line
  D/C — delete / change to end of line
  p/P — paste after / before
  J — join lines
  u / Ctrl+r — undo / redo
  ~ — toggle case
  >>/<<  — indent / dedent line
  . — repeat last edit
  r + char — replace char under cursor
  s/S — substitute char / line (delete then insert)

Operators + motions/text-objects:
  d/y/c + w/e/b/$/0/^/j/k/G — operator + motion
  d/y/c + i/a + w/W/p/s/t/\"/'/`/(/)/[/]/{/}/</>  — text objects
  gU/gu + motion — uppercase / lowercase

Marks & registers:
  m{a-z} — set mark; `{a-z} — jump to mark; '{a-z} — jump to mark line
  \"{a-z} — select named register; then d/y/p to use it

Macros:
  q{a-z} — start recording; q — stop; @{a-z} — play macro

Mode transitions:
  i/a/I/A — enter Insert (at cursor / after / line start / line end)
  o/O — open line below / above and enter Insert
  v/V/Ctrl+v — enter Visual char / line / block
  : — enter Command mode
  / — enter Search mode
  ? — enter AI query mode

Other:
  Ctrl+t — toggle file tree
  Ctrl+l — toggle chat panel
  Tab / Ctrl+w — cycle focus zone
  gd — go to definition (file-local)
  gf — open file path under cursor
  zh — toggle hidden files in file tree

## Insert Mode Keybindings

  Esc — exit to Normal
  Enter — new line (with auto-indent)
  Backspace/Delete — delete char
  Tab — insert tab/spaces
  Ctrl+w — delete previous word
  Ctrl+u — delete to line start
  Arrow keys — move cursor

## Visual Mode Keybindings

  Movement keys — extend selection
  d/x — delete selection
  y — yank selection
  c — change selection
  > / < — indent / dedent selected lines
  ~ — toggle case of selection
  o — swap cursor and anchor
  I — block insert (V-BLOCK mode)
  ? — send selection to AI
  Esc — exit to Normal

## Command Mode (: prefix)

  :w / :w {file} — save / save as
  :q / :q! — quit / force quit
  :wq / :x — save and quit
  :e {file} — open file
  :e! — reload current file
  :{n} — go to line n
  :set nu / :set nonu — toggle line numbers
  :set tabstop=N — set tab width
  :s/pat/rep/flags — substitute (current line)
  :%s/pat/rep/g — substitute (whole file)
  :noh — clear search highlight
  :d / :{n}d / :{n,m}d — delete lines
  :u — undo
  :!{cmd} — run shell command

## Search Mode (/ prefix)

  Type pattern, Enter to search, Esc to cancel.
  n/N in Normal mode to navigate matches.

## AI Features

Users interact with AI in two ways:
1. Editor AI mode: press ? in Normal mode, type query, Enter to submit
   - Prefix ?! for edit plans (AI returns numbered edit steps)
   - In Visual mode, ? sends selected text as context
2. Chat panel: focus chat (Tab/Ctrl+w), press i or Enter to type, Enter to send
   - Multi-turn conversation with history
   - Scroll with j/k, Ctrl+u/d, g/G; clear with D

AI capabilities:
- Advisor: answer questions about code, explain, suggest improvements
- Plan: generate step-by-step edit instructions (INSERT/DELETE/REPLACE)
- Complete: inline ghost-text completion at cursor (Tab to accept)
- Transform: refactor/rewrite selected code

## File Tree (left sidebar)

  j/k — navigate
  Enter/l — open file or expand directory
  h — collapse directory or go to parent
  a — new file; A — new directory
  d — delete (with confirmation)
  r — rename
  y — copy path to clipboard
  R — refresh
  H — toggle hidden files
  g/G — jump to top / bottom
  Esc/q — return focus to editor

## Chat Panel (right sidebar)

Browse mode:
  j/k — scroll down / up
  Ctrl+u/d — scroll 10 lines
  g/G — scroll to top / bottom
  i / Enter — start typing a message
  D — clear conversation history
  Esc/q — return focus to editor
  Ctrl+l — close chat panel

Input mode:
  Type message, Enter to send
  Esc — cancel input, return to browse mode

## Configuration (~/.hirc, TOML format)

[general] — line_numbers, tab_width, expand_tab, auto_indent, ignore_case, smart_case, scroll_off
[ai] — api_base_url, api_key, model, timeout_secs, yolo_mode, context_lines, debug
[theme] — colorscheme
[filetree] — width, show_hidden
[chat] — width, max_messages, context_pairs
";

// ── Per-scenario system prompts ───────────────────────────────────────────

fn system_prompt(kind: &PromptKind, ctx: &AiContext) -> String {
    let file_info = format!(
        "Current file: `{}` ({}, {} lines total)",
        if ctx.filepath.is_empty() { "[unsaved]" } else { &ctx.filepath },
        ctx.language,
        ctx.total_lines,
    );

    let role_instruction = match kind {
        PromptKind::Advisor => format!(
            "{}\n\n## Your Role: Advisor\n\n\
             {}\n\n\
             You are answering the user's question about their code or the editor.\n\
             Be concise and precise. Prefer code examples over lengthy prose.\n\
             When the user asks about editor usage, refer to the keybindings above.\n\
             Never include markdown fences unless the user asks for formatted output.\n\
             Respond in the same language the user uses (Chinese if they write Chinese, English if English).",
            PRODUCT_GUIDE, file_info,
        ),

        PromptKind::Complete => format!(
            "You are the inline completion engine of the `hi` terminal text editor.\n\n\
             {}\n\n\
             Generate a short, natural code continuation at the cursor position.\n\
             Output ONLY the completion text — no explanation, no markdown fences, no prefix.\n\
             Match the existing code style, indentation, and naming conventions.\n\
             If unsure, output nothing rather than guessing wrong.",
            file_info,
        ),

        PromptKind::Plan => format!(
            "{}\n\n## Your Role: Edit Planner\n\n\
             {}\n\n\
             When asked to make changes, respond with a numbered list of atomic edit steps.\n\
             Each step must be one of:\n\
             - INSERT line <N>: <text>  (0-based line index)\n\
             - DELETE line <N>\n\
             - REPLACE line <N>: <new text>\n\
             - REPLACE range <N>-<M>: <new text (multi-line ok, use \\n)>\n\
             - MESSAGE: <advice with no edit>\n\
             Output ONLY the numbered steps, no prose.",
            PRODUCT_GUIDE, file_info,
        ),

        PromptKind::Transform(instruction) => format!(
            "You are the code transformation engine of the `hi` terminal text editor.\n\n\
             {}\n\n\
             Task: {}\n\
             Return ONLY the transformed code, preserving indentation.\n\
             No markdown fences, no explanation, no surrounding text.",
            file_info, instruction,
        ),
    };

    role_instruction
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
