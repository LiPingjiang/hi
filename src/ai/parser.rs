//! Parses the model's plan-mode output into structured `EditStep`s.
//!
//! Expected format (from the system prompt):
//!   1. INSERT line 5: some text to insert
//!   2. DELETE line 3
//!   3. REPLACE line 7: new content here
//!   4. REPLACE range 10-12: first line\nsecond line
//!   5. MESSAGE: some advisory text

use anyhow::Result;

#[derive(Debug, Clone)]
pub enum EditStep {
    /// Insert `text` before `line` (0-based).
    Insert { line: usize, text: String },
    /// Delete `line` (0-based).
    Delete { line: usize },
    /// Replace `line` with `text` (0-based).
    Replace { line: usize, text: String },
    /// Replace the range [start, end] inclusive (0-based) with `text`.
    ReplaceRange { start: usize, end: usize, text: String },
    /// No edit, just a message for the status bar.
    Message(String),
}

/// Parse the raw LLM output into a list of `EditStep`s.
/// Lines that don't match any known pattern are silently skipped.
pub fn parse_plan(raw: &str) -> Vec<EditStep> {
    let mut steps = Vec::new();
    for line in raw.lines() {
        // Strip leading "N. " or "- " prefixes
        let trimmed = strip_list_prefix(line.trim());
        if trimmed.is_empty() {
            continue;
        }

        if let Some(step) = try_parse_step(trimmed) {
            steps.push(step);
        }
    }
    steps
}

fn try_parse_step(s: &str) -> Option<EditStep> {
    let upper = s.to_ascii_uppercase();

    if upper.starts_with("INSERT LINE ") {
        // INSERT line N: text
        let rest = &s["INSERT line ".len()..];
        let (n, text) = split_colon(rest)?;
        return Some(EditStep::Insert { line: parse_usize(n)?, text: unescape(text) });
    }

    if upper.starts_with("DELETE LINE ") {
        let rest = &s["DELETE line ".len()..];
        let n = rest.trim();
        return Some(EditStep::Delete { line: parse_usize(n)? });
    }

    if upper.starts_with("REPLACE LINE ") {
        let rest = &s["REPLACE line ".len()..];
        let (n, text) = split_colon(rest)?;
        return Some(EditStep::Replace { line: parse_usize(n)?, text: unescape(text) });
    }

    if upper.starts_with("REPLACE RANGE ") {
        let rest = &s["REPLACE range ".len()..];
        let (range_part, text) = split_colon(rest)?;
        let dash_pos = range_part.find('-')?;
        let start = parse_usize(range_part[..dash_pos].trim())?;
        let end   = parse_usize(range_part[dash_pos+1..].trim())?;
        return Some(EditStep::ReplaceRange { start, end, text: unescape(text) });
    }

    if upper.starts_with("MESSAGE:") {
        let msg = s["MESSAGE:".len()..].trim().to_string();
        return Some(EditStep::Message(msg));
    }

    // Fallback: treat as advisory message if none of the above match
    None
}

fn strip_list_prefix(s: &str) -> &str {
    // "1. ", "2. ", "- ", "* "
    let s = s.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start_matches(['.', ')'].as_slice()).trim_start();
    let s = s.trim_start_matches(['-', '*'].as_slice()).trim_start();
    s
}

fn split_colon(s: &str) -> Option<(&str, &str)> {
    let pos = s.find(':')?;
    Some((s[..pos].trim(), s[pos+1..].trim()))
}

fn parse_usize(s: &str) -> Option<usize> {
    s.trim().parse().ok()
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\t", "\t")
}

/// Apply a list of EditSteps to a buffer.
/// Returns lines (0-based) that were modified so caller can refresh highlighting.
pub fn apply_steps(
    buffer: &mut crate::buffer::Buffer,
    steps: &[EditStep],
) -> Result<Vec<usize>> {
    let mut changed = Vec::new();
    // Apply in reverse order so line numbers stay consistent
    let mut sorted = steps.to_vec();
    sorted.sort_by(|a, b| {
        let la = step_line(a).unwrap_or(0);
        let lb = step_line(b).unwrap_or(0);
        lb.cmp(&la) // descending
    });

    for step in &sorted {
        match step {
            EditStep::Insert { line, text } => {
                let char_idx = buffer.pos_to_char(*line, 0);
                let to_insert = format!("{}\n", text);
                buffer.insert(char_idx, &to_insert);
                changed.push(*line);
            }
            EditStep::Delete { line } => {
                let start = buffer.pos_to_char(*line, 0);
                let next_line_start = buffer.pos_to_char(line + 1, 0);
                if next_line_start > start {
                    buffer.delete(start, next_line_start - start);
                }
                changed.push(*line);
            }
            EditStep::Replace { line, text } => {
                // Use line_to_char boundaries so we never double-count the
                // newline or rely on a mutated line_count after the delete.
                let line_start = buffer.line_to_char(*line);
                let is_last    = *line + 1 >= buffer.line_count();
                let line_end   = if is_last {
                    buffer.len_chars()
                } else {
                    buffer.line_to_char(*line + 1) // includes the trailing \n
                };
                let delete_count = line_end.saturating_sub(line_start);
                buffer.delete(line_start, delete_count);
                // Preserve "has newline" semantics: non-last lines get a newline,
                // last line only gets one if the original line had one.
                let to_insert = if is_last {
                    text.clone()
                } else {
                    format!("{}\n", text)
                };
                buffer.insert(line_start, &to_insert);
                changed.push(*line);
            }
            EditStep::ReplaceRange { start, end, text } => {
                let char_start = buffer.pos_to_char(*start, 0);
                let last_line_text = buffer.line_str(*end);
                let last_chars = last_line_text.chars().count();
                let char_end_excl = buffer.pos_to_char(*end, last_chars);
                let delete_count = char_end_excl.saturating_sub(char_start);
                buffer.delete(char_start, delete_count);
                buffer.insert(char_start, text);
                for l in *start..=*end { changed.push(l); }
            }
            EditStep::Message(_) => {}
        }
    }
    changed.sort_unstable();
    changed.dedup();
    Ok(changed)
}

fn step_line(step: &EditStep) -> Option<usize> {
    match step {
        EditStep::Insert { line, .. } => Some(*line),
        EditStep::Delete { line }     => Some(*line),
        EditStep::Replace { line, .. } => Some(*line),
        EditStep::ReplaceRange { start, .. } => Some(*start),
        EditStep::Message(_) => None,
    }
}

// ── Agent-edit tool call parser ───────────────────────────────────────────────

use crate::ai::tools::AiTool;

/// Parse a single AI tool call from the model's raw response text.
///
/// The model is instructed to emit tool calls as:
/// ```text
/// TOOL: {"tool": "read_buffer", "args": {"start": 0, "end": 10}}
/// ```
///
/// Returns `Some(AiTool)` if a valid `TOOL:` line is found, `None` otherwise.
/// Only the **first** `TOOL:` line in `raw` is parsed.
pub fn parse_tool_call(raw: &str) -> Option<AiTool> {
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(json_str) = trimmed.strip_prefix("TOOL:") {
            let json_str = json_str.trim();
            if let Ok(tool) = serde_json::from_str::<AiTool>(json_str) {
                return Some(tool);
            }
            // Fallback: try wrapping in the tagged enum format if the model
            // emitted a flat object like {"tool":"read_buffer","start":0,"end":5}
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                if let Some(tool_name) = v.get("tool").and_then(|t| t.as_str()) {
                    // Re-wrap into {"tool": name, "args": rest}
                    let args = v.clone();
                    let wrapped = serde_json::json!({
                        "tool": tool_name,
                        "args": args
                    });
                    if let Ok(tool) = serde_json::from_value::<AiTool>(wrapped) {
                        return Some(tool);
                    }
                }
            }
        }
    }
    None
}

/// Extract the "thought" text from a model response — everything before the
/// first `TOOL:` line (or the whole response if no tool call is present).
pub fn extract_thought(raw: &str) -> String {
    let mut lines = Vec::new();
    for line in raw.lines() {
        if line.trim().starts_with("TOOL:") {
            break;
        }
        lines.push(line);
    }
    lines.join("\n").trim().to_string()
}
