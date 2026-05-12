//! Right-side AI chat panel — stores conversation history and renders it.

use unicode_width::UnicodeWidthChar;
use crate::ui::mdrender::{MdRenderer, MdLine, StyledSpan};

/// A single message in the chat history.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    /// Pre-wrapped lines for rendering (populated by `wrap_lines`).
    wrapped: Vec<String>,
    /// The panel width used when `wrapped` was computed.
    wrapped_at: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatRole {
    User,
    Assistant,
    System,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: ChatRole::User, content: content.into(), wrapped: Vec::new(), wrapped_at: 0 }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: ChatRole::Assistant, content: content.into(), wrapped: Vec::new(), wrapped_at: 0 }
    }
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: ChatRole::System, content: content.into(), wrapped: Vec::new(), wrapped_at: 0 }
    }

    /// Return display-width-aware wrapped lines for a given panel content width.
    /// Caches the result; re-wraps only when width changes.
    pub fn wrap_lines(&mut self, width: usize) -> &[String] {
        if self.wrapped_at == width && !self.wrapped.is_empty() {
            return &self.wrapped;
        }
        self.wrapped = wrap_text(&self.content, width);
        self.wrapped_at = width;
        &self.wrapped
    }
}

/// The chat panel state.
pub struct ChatPanel {
    /// Full conversation history.
    pub messages: Vec<ChatMessage>,
    /// Scroll offset (number of rendered lines scrolled up from bottom).
    pub scroll: usize,
    /// Maximum number of messages to keep (older ones are dropped).
    pub max_messages: usize,
    /// Panel width in columns (set from config, may be overridden).
    pub width: usize,
}

impl ChatPanel {
    pub fn new(width: usize, max_messages: usize) -> Self {
        Self {
            messages: Vec::new(),
            scroll: 0,
            max_messages,
            width,
        }
    }

    /// Append a user message and reset scroll to bottom.
    pub fn push_user(&mut self, content: &str) {
        self.messages.push(ChatMessage::user(content));
        self.trim();
        self.scroll = 0;
    }

    /// Append an assistant message and reset scroll to bottom.
    pub fn push_assistant(&mut self, content: &str) {
        self.messages.push(ChatMessage::assistant(content));
        self.trim();
        self.scroll = 0;
    }

    /// Append a system/error message.
    pub fn push_system(&mut self, content: &str) {
        self.messages.push(ChatMessage::system(content));
        self.trim();
        self.scroll = 0;
    }

    /// Scroll up by `n` lines.
    pub fn scroll_up(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_add(n);
    }

    /// Scroll down by `n` lines (towards latest).
    pub fn scroll_down(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
    }

    /// Clear all messages.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.scroll = 0;
    }

    /// Collect all rendered lines (role label + wrapped content) for the panel.
    /// Returns a flat list of `(ChatRole, String)` pairs for rendering.
    pub fn render_lines(&mut self, content_width: usize) -> Vec<(ChatRole, String)> {
        let mut lines: Vec<(ChatRole, String)> = Vec::new();
        for msg in &mut self.messages {
            let role = msg.role.clone();
            // Role label line
            let label = match &role {
                ChatRole::User      => "▶ You",
                ChatRole::Assistant  => "◀ AI",
                ChatRole::System     => "● System",
            };
            lines.push((role.clone(), label.to_string()));
            // Content lines
            let wrapped = msg.wrap_lines(content_width);
            for wl in wrapped {
                lines.push((role.clone(), wl.clone()));
            }
            // Blank separator
            lines.push((role.clone(), String::new()));
        }
        lines
    }

    /// Render all messages into styled lines using the Markdown renderer for
    /// Assistant messages. Returns `(ChatRole, MdLine)` pairs.
    pub fn render_lines_styled(
        &mut self,
        content_width: usize,
        md_renderer: &MdRenderer,
    ) -> Vec<(ChatRole, MdLine)> {
        use crossterm::style::Color;
        let mut lines: Vec<(ChatRole, MdLine)> = Vec::new();

        for msg in &mut self.messages {
            let role = msg.role.clone();

            // Role label line with distinctive styling
            let (label, label_fg) = match &role {
                ChatRole::User      => ("▶ You", Color::Cyan),
                ChatRole::Assistant  => ("◀ AI", Color::Green),
                ChatRole::System     => ("● System", Color::Yellow),
            };
            let mut label_line = MdLine::new();
            let mut label_span = StyledSpan::styled(label, Some(label_fg), None);
            label_span.bold = true;
            label_line.push(label_span);
            lines.push((role.clone(), label_line));

            // Content lines
            match &role {
                ChatRole::Assistant => {
                    // Render Markdown for AI responses
                    let md_lines = md_renderer.render(&msg.content, content_width);
                    for ml in md_lines {
                        lines.push((role.clone(), ml));
                    }
                }
                _ => {
                    // Plain text for User and System messages
                    let wrapped = msg.wrap_lines(content_width);
                    let fg = match &role {
                        ChatRole::User   => Color::Cyan,
                        ChatRole::System => Color::Yellow,
                        _ => Color::White,
                    };
                    for wl in wrapped {
                        let mut line = MdLine::new();
                        line.push(StyledSpan::styled(wl.clone(), Some(fg), None));
                        lines.push((role.clone(), line));
                    }
                }
            }

            // Blank separator
            lines.push((role.clone(), MdLine::empty()));
        }
        lines
    }

    fn trim(&mut self) {
        while self.messages.len() > self.max_messages {
            self.messages.remove(0);
        }
    }

    /// Return the recent N message pairs as (role, content) for prompt injection.
    /// Returns at most `n` user+assistant pairs (2*n messages).
    pub fn recent_history(&self, n: usize) -> Vec<(&str, &str)> {
        let mut pairs: Vec<(&str, &str)> = Vec::new();
        // Walk backwards collecting user/assistant pairs
        let mut iter = self.messages.iter().rev().peekable();
        let mut collected = 0;
        let mut pending_assistant: Option<&str> = None;
        while let Some(msg) = iter.next() {
            match msg.role {
                ChatRole::Assistant => {
                    pending_assistant = Some(&msg.content);
                }
                ChatRole::User => {
                    if let Some(asst) = pending_assistant.take() {
                        pairs.push((&msg.content, asst));
                        collected += 1;
                        if collected >= n { break; }
                    }
                }
                ChatRole::System => {}
            }
        }
        pairs.reverse();
        pairs
    }
}

/// Word-wrap text to fit within `max_width` display columns.
/// Respects CJK double-width characters.
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let max_width = max_width.max(4); // minimum sane width
    let mut result = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            result.push(String::new());
            continue;
        }
        let mut current = String::new();
        let mut current_width = 0usize;
        for ch in line.chars() {
            let ch_w = UnicodeWidthChar::width(ch).unwrap_or(0);
            if current_width + ch_w > max_width && current_width > 0 {
                result.push(current);
                current = String::new();
                current_width = 0;
            }
            current.push(ch);
            current_width += ch_w;
        }
        if !current.is_empty() || result.is_empty() {
            result.push(current);
        }
    }
    if result.is_empty() {
        result.push(String::new());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_ascii() {
        let lines = wrap_text("hello world, this is a test", 10);
        assert!(lines.iter().all(|l| display_width(l) <= 10));
        assert!(lines.len() >= 2);
    }

    #[test]
    fn wrap_cjk() {
        // Each CJK char is 2 columns wide
        let lines = wrap_text("你好世界测试文本", 8);
        // 8 cols = 4 CJK chars per line
        assert!(lines.iter().all(|l| display_width(l) <= 8));
        assert_eq!(lines.len(), 2); // 8 chars * 2 = 16 cols, 16/8 = 2 lines
    }

    #[test]
    fn wrap_empty() {
        let lines = wrap_text("", 20);
        assert_eq!(lines, vec![""]);
    }

    #[test]
    fn chat_panel_history() {
        let mut panel = ChatPanel::new(40, 100);
        panel.push_user("hello");
        panel.push_assistant("hi there");
        panel.push_user("how are you");
        panel.push_assistant("I'm fine");

        let history = panel.recent_history(1);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].0, "how are you");
        assert_eq!(history[0].1, "I'm fine");

        let history2 = panel.recent_history(5);
        assert_eq!(history2.len(), 2);
    }

    fn display_width(s: &str) -> usize {
        s.chars().map(|c| UnicodeWidthChar::width(c).unwrap_or(0)).sum()
    }
}
