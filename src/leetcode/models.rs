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

/// Result of a code submission or test run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionResult {
    /// "PENDING", "SUCCESS", or other states.
    pub state: String,
    /// Human-readable status (e.g. "Accepted", "Wrong Answer").
    pub status_msg: String,
    /// Runtime (e.g. "4 ms").
    pub runtime: String,
    /// Memory usage (e.g. "6.2 MB").
    pub memory: String,
    /// Number of test cases passed.
    pub total_correct: u32,
    /// Total number of test cases.
    pub total_testcases: u32,
    /// Compile error message (if any).
    pub compile_error: String,
    /// Full compile error (multiline).
    pub full_compile_error: String,
    /// Runtime error message (if any).
    pub runtime_error: String,
    /// Full runtime error (multiline).
    pub full_runtime_error: String,
    /// Last test case input that failed.
    pub last_testcase: String,
    /// Expected output for the failed test case.
    pub expected_output: String,
    /// Actual code output.
    pub code_output: String,
    /// Stdout output from the code.
    pub std_output: String,
    /// Error line number (parsed from compile error, 0 = unknown).
    pub error_line: usize,
}

/// A past submission entry (from submission history).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionEntry {
    /// Submission ID.
    pub id: String,
    /// Language display name (e.g. "Python3", "Rust").
    pub lang: String,
    /// Language slug (e.g. "python3", "rust").
    pub lang_slug: String,
    /// Status (e.g. "Accepted", "Wrong Answer").
    pub status_display: String,
    /// Runtime (e.g. "4 ms").
    pub runtime: String,
    /// Memory (e.g. "6.2 MB").
    pub memory: String,
    /// Timestamp (unix seconds).
    pub timestamp: u64,
    /// The submitted code.
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
