//! DiffPanel — 展示 AI 编辑会话产生的 diff，等待用户 y/n 确认。
//!
//! 渲染为全宽覆盖层（overlay），显示在编辑器上方。
//! 布局：
//!   ┌─ AI Edit Diff ─────────────────────────────────────────────────────┐
//!   │ Hunk 1/3: Replace lines 5-7                                        │
//!   │ - old line 5                                                        │
//!   │ + new line 5                                                        │
//!   │ ...                                                                 │
//!   │ [y] apply  [n] cancel  [j/k] scroll hunks  [Esc] cancel            │
//!   └────────────────────────────────────────────────────────────────────┘

use crate::ai::tools::{EditDiff, DiffHunk, HunkKind};

// ── State ─────────────────────────────────────────────────────────────────────

/// UI state for the DiffPanel overlay.
pub struct DiffPanel {
    /// Which hunk is currently focused (0-based).
    pub cursor: usize,
    /// Vertical scroll offset within the current hunk's content.
    pub scroll: usize,
    /// AI's summary / thought text shown at the top.
    pub summary: String,
}

impl DiffPanel {
    pub fn new(summary: impl Into<String>) -> Self {
        Self { cursor: 0, scroll: 0, summary: summary.into() }
    }

    /// Move focus to the next hunk.
    pub fn next_hunk(&mut self, total: usize) {
        if total > 0 {
            self.cursor = (self.cursor + 1).min(total - 1);
            self.scroll = 0;
        }
    }

    /// Move focus to the previous hunk.
    pub fn prev_hunk(&mut self, total: usize) {
        if total > 0 {
            self.cursor = self.cursor.saturating_sub(1);
            self.scroll = 0;
        }
    }

    /// Scroll down within the current hunk view.
    pub fn scroll_down(&mut self) {
        self.scroll += 1;
    }

    /// Scroll up within the current hunk view.
    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }
}

// ── Rendering helpers (pure data, no crossterm here) ─────────────────────────
// The actual crossterm drawing is done in ui/renderer.rs::render_diff_panel.

/// A single display line in the diff panel.
#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiffLineKind {
    /// Context / header line (grey)
    Header,
    /// Removed line (red, prefixed with '-')
    Removed,
    /// Added line (green, prefixed with '+')
    Added,
    /// Hint / footer line
    Hint,
}

/// Build the display lines for a single hunk.
pub fn hunk_display_lines(hunk: &DiffHunk, hunk_idx: usize, total: usize) -> Vec<DiffLine> {
    let mut lines = Vec::new();

    // Header
    lines.push(DiffLine {
        kind: DiffLineKind::Header,
        text: format!(
            "── Hunk {}/{}: {} ──",
            hunk_idx + 1,
            total,
            hunk.summary()
        ),
    });

    match hunk.kind {
        HunkKind::Replace | HunkKind::Delete => {
            for (i, l) in hunk.old_text.lines().enumerate() {
                lines.push(DiffLine {
                    kind: DiffLineKind::Removed,
                    text: format!("{:>4} - {}", hunk.start_line + 1 + i, l),
                });
            }
        }
        HunkKind::Insert => {}
    }

    match hunk.kind {
        HunkKind::Replace | HunkKind::Insert => {
            for (i, l) in hunk.new_text.lines().enumerate() {
                lines.push(DiffLine {
                    kind: DiffLineKind::Added,
                    text: format!("{:>4} + {}", hunk.start_line + 1 + i, l),
                });
            }
        }
        HunkKind::Delete => {}
    }

    lines
}

/// Build all display lines for the full diff (all hunks).
pub fn all_display_lines(diff: &EditDiff) -> Vec<DiffLine> {
    let total = diff.hunks.len();
    let mut out = Vec::new();
    for (i, hunk) in diff.hunks.iter().enumerate() {
        out.extend(hunk_display_lines(hunk, i, total));
        // Blank separator between hunks
        out.push(DiffLine { kind: DiffLineKind::Header, text: String::new() });
    }
    out
}
