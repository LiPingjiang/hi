//! Local cache for LeetCode problem data.
//!
//! Stores problem lists and details in ~/.config/hi/leetcode_cache/

use anyhow::Result;
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
