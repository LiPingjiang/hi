//! Builds the snippet of buffer text sent to the AI model as context.

use crate::buffer::Buffer;

/// A captured slice of the buffer surrounding the cursor, plus metadata.
#[derive(Debug, Clone)]
pub struct AiContext {
    /// File path (empty string if unsaved)
    pub filepath: String,
    /// The complete text of the selection or surrounding context
    pub snippet: String,
    /// Line number of the cursor (0-based)
    pub cursor_line: usize,
    /// Column of the cursor (0-based)
    pub cursor_col: usize,
    /// Total lines in the file
    pub total_lines: usize,
    /// Detected language/file type label
    pub language: String,
}

impl AiContext {
    /// Build context around the cursor using `context_lines` lines before and after.
    pub fn from_cursor(
        buffer: &Buffer,
        filepath: &str,
        cursor_line: usize,
        cursor_col: usize,
        context_lines: usize,
        language: &str,
    ) -> Self {
        let total = buffer.line_count();
        let start = cursor_line.saturating_sub(context_lines);
        let end = (cursor_line + context_lines + 1).min(total);

        let mut lines = Vec::new();
        for i in start..end {
            lines.push(buffer.line_str(i));
        }
        let snippet = lines.join("\n");

        Self {
            filepath: filepath.to_string(),
            snippet,
            cursor_line,
            cursor_col,
            total_lines: total,
            language: language.to_string(),
        }
    }

    /// Build context from a visual selection.
    pub fn from_selection(
        buffer: &Buffer,
        filepath: &str,
        start_line: usize,
        end_line: usize,
        language: &str,
    ) -> Self {
        let total = buffer.line_count();
        let mut lines = Vec::new();
        for i in start_line..=end_line.min(total.saturating_sub(1)) {
            lines.push(buffer.line_str(i));
        }
        let snippet = lines.join("\n");
        Self {
            filepath: filepath.to_string(),
            snippet,
            cursor_line: start_line,
            cursor_col: 0,
            total_lines: total,
            language: language.to_string(),
        }
    }
}
