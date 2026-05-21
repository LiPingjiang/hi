//! Local cache for LeetCode problem data.
//!
//! Stores problem lists and details in ~/.config/hi/leetcode_cache/
//! Also persists user solutions (code + language) per problem.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::models::ProblemSummary;

/// Get the cache directory: ~/.config/hi/leetcode_cache/
fn cache_dir() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hi")
        .join("leetcode_cache");
    config_dir
}

/// Load cached problem list.
pub fn load_problem_list() -> Option<Vec<ProblemSummary>> {
    let path = cache_dir().join("problems.json");
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save problem list to cache.
pub fn save_problem_list(problems: &[ProblemSummary]) -> Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("problems.json");
    let json = serde_json::to_string(problems)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Clear all cached data.
pub fn clear_cache() -> Result<()> {
    let dir = cache_dir();
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

// ── Solution persistence ─────────────────────────────────────────────────────

/// Persisted solution state for a single problem.
#[derive(Debug, Serialize, Deserialize)]
pub struct SavedSolution {
    /// The language slug (e.g. "python3", "cpp").
    pub lang_slug: String,
    /// The language index in the code_snippets array.
    pub lang_index: usize,
    /// The user's code.
    pub code: String,
    /// Cursor line position.
    pub cursor_line: usize,
    /// Cursor column position.
    pub cursor_col: usize,
}

/// Get the solutions directory: ~/.config/hi/leetcode_cache/solutions/
fn solutions_dir() -> PathBuf {
    cache_dir().join("solutions")
}

/// Load a saved solution for a problem by its title_slug.
pub fn load_solution(title_slug: &str) -> Option<SavedSolution> {
    let path = solutions_dir().join(format!("{}.json", title_slug));
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save the user's solution for a problem.
pub fn save_solution(title_slug: &str, solution: &SavedSolution) -> Result<()> {
    let dir = solutions_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", title_slug));
    let json = serde_json::to_string_pretty(solution)?;
    std::fs::write(&path, json)?;
    Ok(())
}
