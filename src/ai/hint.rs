//! Manages the hint / ghost-text state produced by AI responses.

/// The kind of AI response displayed to the user.
#[derive(Debug, Clone, PartialEq)]
pub enum HintKind {
    /// Free-form answer shown in the hint/status bar.
    Advisor(String),
    /// Execution plan that the user must confirm before applying.
    Plan(Vec<String>),
    /// Inline completion (ghost text shown at cursor position).
    Completion(String),
    /// Error message from the AI subsystem.
    Error(String),
}

impl HintKind {
    /// Whether this hint represents a plan that needs user confirmation.
    pub fn is_plan(&self) -> bool {
        matches!(self, HintKind::Plan(_))
    }

    /// Returns a one-line summary for the status bar.
    pub fn summary(&self) -> String {
        match self {
            HintKind::Advisor(s) => s.lines().next().unwrap_or("").to_string(),
            HintKind::Plan(steps) => format!("计划 {} 步 — [y]确认  [n]取消", steps.len()),
            HintKind::Completion(s) => s.lines().next().unwrap_or("").to_string(),
            HintKind::Error(e) => format!("AI error: {}", e),
        }
    }

    /// Full content, primarily for the plan overlay or multi-line display.
    pub fn lines(&self) -> Vec<String> {
        match self {
            HintKind::Advisor(s) => s.lines().map(String::from).collect(),
            HintKind::Plan(steps) => steps.clone(),
            HintKind::Completion(s) => s.lines().map(String::from).collect(),
            HintKind::Error(e) => vec![format!("Error: {}", e)],
        }
    }
}
