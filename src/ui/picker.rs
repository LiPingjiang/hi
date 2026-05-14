//! Fuzzy file picker overlay — triggered by Ctrl+P.
//!
//! Walks the directory tree from the editor's working root, collects all
//! file paths, and lets the user type a fuzzy query to narrow the list.
//! Pressing Enter opens the selected file; Esc dismisses without action.
//!
//! Fuzzy matching: every character in the query must appear in the candidate
//! path in order (subsequence match), scored by consecutive-run length so
//! that "src/app" ranks above "src/ai/prompt" when the query is "app".

use std::path::{Path, PathBuf};
use std::fs;

// ── Fuzzy scoring ────────────────────────────────────────────────────────────

/// Returns `Some(score)` if `query` is a subsequence of `text`, else `None`.
/// Higher score = better match.  Consecutive matching characters are rewarded.
pub fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let q: Vec<char> = query.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();

    let mut qi = 0;
    let mut score: i32 = 0;
    let mut consecutive: i32 = 0;
    let mut last_match: Option<usize> = None;

    for (ti, &tc) in t.iter().enumerate() {
        if qi < q.len() && tc == q[qi] {
            // Reward consecutive matches
            if last_match == Some(ti.wrapping_sub(1)) {
                consecutive += 1;
                score += 10 + consecutive * 5;
            } else {
                consecutive = 0;
                score += 1;
            }
            // Reward matches at word boundaries (after '/', '-', '_', '.')
            if ti == 0 || matches!(t[ti - 1], '/' | '-' | '_' | '.') {
                score += 8;
            }
            last_match = Some(ti);
            qi += 1;
        }
    }

    if qi == q.len() { Some(score) } else { None }
}

// ── File collection ──────────────────────────────────────────────────────────

const MAX_FILES: usize = 5_000;

/// Collect all files under `root`, returning paths relative to `root`.
/// Skips hidden directories (`.git`, `.svn`, `node_modules`, `target`, etc.).
pub fn collect_files(root: &Path) -> Vec<PathBuf> {
    let mut results = Vec::with_capacity(512);
    collect_recursive(root, root, &mut results);
    results
}

fn collect_recursive(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    if out.len() >= MAX_FILES {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        if out.len() >= MAX_FILES {
            break;
        }
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden dirs and common noise dirs
        if name_str.starts_with('.') {
            continue;
        }
        if matches!(name_str.as_ref(), "node_modules" | "target" | "dist" | "build" | "__pycache__") {
            continue;
        }

        if path.is_dir() {
            collect_recursive(root, &path, out);
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_path_buf());
        }
    }
}

// ── Picker state ─────────────────────────────────────────────────────────────

pub struct FilePicker {
    /// All files under the working root.
    pub all_files: Vec<PathBuf>,
    /// Current query string.
    pub query: String,
    /// Filtered + scored results (path index into all_files, score).
    pub matches: Vec<(usize, i32)>,
    /// Cursor position within `matches`.
    pub cursor: usize,
    /// Root directory used for display and opening.
    pub root: PathBuf,
}

impl FilePicker {
    pub fn new(root: PathBuf) -> Self {
        let all_files = collect_files(&root);
        let matches: Vec<(usize, i32)> = (0..all_files.len()).map(|i| (i, 0)).collect();
        Self {
            all_files,
            query: String::new(),
            matches,
            cursor: 0,
            root,
        }
    }

    /// Recompute `matches` from the current `query`.
    pub fn update_matches(&mut self) {
        let q = &self.query;
        let mut scored: Vec<(usize, i32)> = self.all_files
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let display = p.to_string_lossy();
                fuzzy_score(q, &display).map(|s| (i, s))
            })
            .collect();

        // Sort: higher score first; ties broken by path length (shorter = better)
        scored.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| self.all_files[a.0].as_os_str().len()
                    .cmp(&self.all_files[b.0].as_os_str().len()))
        });

        self.matches = scored;
        self.cursor = 0;
    }

    /// Push a character to the query and refresh matches.
    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
        self.update_matches();
    }

    /// Delete the last character from the query and refresh matches.
    pub fn pop_char(&mut self) {
        self.query.pop();
        self.update_matches();
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.matches.len() {
            self.cursor += 1;
        }
    }

    /// Return the absolute path of the currently selected file, if any.
    pub fn selected_path(&self) -> Option<PathBuf> {
        self.matches.get(self.cursor).map(|(i, _)| self.root.join(&self.all_files[*i]))
    }

    /// Visible window of results (up to `max_rows` items, centred on cursor).
    pub fn visible_window(&self, max_rows: usize) -> (usize, &[(usize, i32)]) {
        let total = self.matches.len();
        if total == 0 {
            return (0, &[]);
        }
        let start = if self.cursor >= max_rows.saturating_sub(1) {
            self.cursor.saturating_sub(max_rows / 2)
        } else {
            0
        };
        let end = (start + max_rows).min(total);
        (start, &self.matches[start..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_score_subsequence() {
        assert!(fuzzy_score("app", "src/app.rs").is_some());
        assert!(fuzzy_score("xyz", "src/app.rs").is_none());
    }

    #[test]
    fn test_fuzzy_score_consecutive_bonus() {
        let s1 = fuzzy_score("app", "src/app.rs").unwrap();
        let s2 = fuzzy_score("app", "a_p_p.rs").unwrap();
        assert!(s1 > s2, "consecutive match should score higher");
    }

    #[test]
    fn test_empty_query_matches_all() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
    }
}
