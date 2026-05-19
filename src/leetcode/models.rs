//! Data models for LeetCode problems, submissions, and filters.

use serde::{Deserialize, Serialize};

/// Problem difficulty level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl Difficulty {
    pub fn label(&self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Medium => "Medium",
            Difficulty::Hard => "Hard",
        }
    }

    pub fn short(&self) -> &'static str {
        match self {
            Difficulty::Easy => "E",
            Difficulty::Medium => "M",
            Difficulty::Hard => "H",
        }
    }
}

/// Whether the user has solved a problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolveStatus {
    Solved,
    Attempted,
    NotStarted,
}

impl SolveStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            SolveStatus::Solved => "✓",
            SolveStatus::Attempted => "○",
            SolveStatus::NotStarted => " ",
        }
    }
}

/// Lightweight problem info for the list view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemSummary {
    pub frontend_id: u32,
    pub title: String,
    pub title_slug: String,
    pub difficulty: Difficulty,
    pub status: SolveStatus,
    pub acceptance: f32,
    pub paid_only: bool,
}

/// Full problem detail (fetched on demand).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemDetail {
    pub summary: ProblemSummary,
    /// Plain-text description (converted from HTML).
    pub content_text: String,
    /// Code snippets keyed by language slug.
    pub code_snippets: Vec<CodeSnippet>,
    /// Example test cases.
    pub test_cases: String,
}

/// A code template for a specific language.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSnippet {
    pub lang: String,
    pub lang_slug: String,
    pub code: String,
}

/// Filter criteria for the problem list.
#[derive(Debug, Clone, Default)]
pub struct ProblemFilter {
    pub difficulty: Option<Difficulty>,
    pub status: Option<SolveStatus>,
    pub search: String,
}

impl ProblemFilter {
    pub fn matches(&self, p: &ProblemSummary) -> bool {
        if let Some(d) = self.difficulty {
            if p.difficulty != d {
                return false;
            }
        }
        if let Some(s) = self.status {
            if p.status != s {
                return false;
            }
        }
        if !self.search.is_empty() {
            let q = self.search.to_lowercase();
            let title_match = p.title.to_lowercase().contains(&q);
            let id_match = p.frontend_id.to_string().contains(&q);
            if !title_match && !id_match {
                return false;
            }
        }
        true
    }
}
