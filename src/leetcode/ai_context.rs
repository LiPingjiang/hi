//! LeetCode AI context integration.
//!
//! Provides enriched context to the AI assistant when the user is working on
//! a LeetCode problem, enabling hint/explain/debug modes.

use super::knowledge::{KnowledgeBase, KnowledgeProblem};

/// Mode of AI assistance for LeetCode problems.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LeetCodeAiMode {
    /// Give a subtle hint without revealing the full solution
    Hint,
    /// Explain the approach and algorithm in detail
    Explain,
    /// Help debug the user's current code
    Debug,
    /// Compare multiple approaches
    Compare,
}

/// Enriched AI context for LeetCode problems.
#[derive(Debug, Clone)]
pub struct LeetCodeAiContext {
    pub problem_id: u32,
    pub problem_title: String,
    pub problem_slug: String,
    pub topics: Vec<String>,
    pub techniques: Vec<String>,
    pub difficulty: u8,
    pub official_difficulty: String,
    pub key_insight: String,
    pub approach: String,
    pub time_complexity: String,
    pub space_complexity: String,
    pub reference_code: String,
    pub mode: LeetCodeAiMode,
    pub user_code: String,
}

impl LeetCodeAiContext {
    /// Try to build a LeetCode AI context from a problem id and user's current code.
    pub fn from_problem(id: u32, user_code: &str, mode: LeetCodeAiMode) -> Option<Self> {
        let kb = KnowledgeBase::global();
        let p = kb.get(id)?;

        Some(Self {
            problem_id: p.id,
            problem_title: p.title.clone(),
            problem_slug: p.slug.clone(),
            topics: p.topics.clone(),
            techniques: p.techniques.clone(),
            difficulty: p.difficulty,
            official_difficulty: p.official_difficulty.clone(),
            key_insight: p.key_insight.clone(),
            approach: p.approach.clone(),
            time_complexity: p.time_complexity.clone(),
            space_complexity: p.space_complexity.clone(),
            reference_code: p.code_python3.clone(),
            mode,
            user_code: user_code.to_string(),
        })
    }

    /// Try to detect a LeetCode problem from a file path or buffer content.
    /// Looks for patterns like "# 1. Two Sum" or "leetcode/1-two-sum.py" etc.
    pub fn detect_problem_id(filepath: &str, content: &str) -> Option<u32> {
        // Try from filename: "1-two-sum.py", "0001.py", "problem_1.rs"
        if let Some(id) = Self::id_from_filename(filepath) {
            return Some(id);
        }

        // Try from content: "# 1. Two Sum" or "Problem 1:" or "LeetCode #1"
        if let Some(id) = Self::id_from_content(content) {
            return Some(id);
        }

        None
    }

    /// Generate the system prompt addition for the AI based on mode.
    pub fn to_system_prompt(&self) -> String {
        match self.mode {
            LeetCodeAiMode::Hint => self.hint_prompt(),
            LeetCodeAiMode::Explain => self.explain_prompt(),
            LeetCodeAiMode::Debug => self.debug_prompt(),
            LeetCodeAiMode::Compare => self.compare_prompt(),
        }
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn hint_prompt(&self) -> String {
        format!(
            "[LeetCode Context]\n\
             Problem: #{} {} ({})\n\
             Topics: {}\n\
             Techniques: {}\n\
             \n\
             Instructions: The user is working on this problem. Give a SUBTLE HINT \
             without revealing the full solution. Guide them toward the key insight: \
             \"{}\"\n\
             Do NOT show complete code. Ask leading questions instead.",
            self.problem_id,
            self.problem_title,
            self.official_difficulty,
            self.topics.join(", "),
            self.techniques.join(", "),
            self.key_insight,
        )
    }

    fn explain_prompt(&self) -> String {
        format!(
            "[LeetCode Context]\n\
             Problem: #{} {} ({})\n\
             Topics: {}\n\
             Techniques: {}\n\
             Difficulty: {}/10\n\
             \n\
             Key Insight: {}\n\
             Approach: {}\n\
             Time: {} | Space: {}\n\
             \n\
             Reference Solution:\n```python\n{}\n```\n\
             \n\
             Instructions: Explain this problem's solution thoroughly. Cover:\n\
             1. Why this approach works\n\
             2. Step-by-step walkthrough\n\
             3. Time/space complexity analysis\n\
             4. Common pitfalls",
            self.problem_id,
            self.problem_title,
            self.official_difficulty,
            self.topics.join(", "),
            self.techniques.join(", "),
            self.difficulty,
            self.key_insight,
            self.approach,
            self.time_complexity,
            self.space_complexity,
            self.reference_code,
        )
    }

    fn debug_prompt(&self) -> String {
        format!(
            "[LeetCode Context]\n\
             Problem: #{} {} ({})\n\
             Topics: {}\n\
             Techniques: {}\n\
             \n\
             Key Insight: {}\n\
             Expected Approach: {}\n\
             Expected Complexity: Time {} | Space {}\n\
             \n\
             User's Code:\n```\n{}\n```\n\
             \n\
             Reference Solution:\n```python\n{}\n```\n\
             \n\
             Instructions: Help debug the user's code. Compare with the reference \
             approach, identify bugs or inefficiencies, and suggest fixes. \
             Be specific about what's wrong and why.",
            self.problem_id,
            self.problem_title,
            self.official_difficulty,
            self.topics.join(", "),
            self.techniques.join(", "),
            self.key_insight,
            self.approach,
            self.time_complexity,
            self.space_complexity,
            self.user_code,
            self.reference_code,
        )
    }

    fn compare_prompt(&self) -> String {
        format!(
            "[LeetCode Context]\n\
             Problem: #{} {} ({})\n\
             Topics: {}\n\
             Techniques: {}\n\
             \n\
             Instructions: Compare different approaches to solve this problem.\n\
             Known techniques for this problem: {}\n\
             Discuss trade-offs between approaches in terms of:\n\
             - Time complexity\n\
             - Space complexity\n\
             - Code simplicity\n\
             - Edge case handling",
            self.problem_id,
            self.problem_title,
            self.official_difficulty,
            self.topics.join(", "),
            self.techniques.join(", "),
            self.techniques.join(", "),
        )
    }

    fn id_from_filename(filepath: &str) -> Option<u32> {
        let filename = filepath.rsplit('/').next().unwrap_or(filepath);
        let stem = filename.split('.').next().unwrap_or(filename);

        // Try patterns: "1-two-sum", "0001", "problem_1", "lc1", "1_two_sum"
        // Extract leading digits
        let digits: String = stem.chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect();

        if !digits.is_empty() {
            if let Ok(id) = digits.parse::<u32>() {
                if id >= 1 && id <= 3000 {
                    return Some(id);
                }
            }
        }
        None
    }

    fn id_from_content(content: &str) -> Option<u32> {
        // Check first 10 lines for problem id patterns
        for line in content.lines().take(10) {
            let trimmed = line.trim();

            // "# 1. Two Sum" or "## Problem 1"
            if let Some(rest) = trimmed.strip_prefix('#') {
                let rest = rest.trim_start_matches('#').trim();
                let digits: String = rest.chars()
                    .skip_while(|c| !c.is_ascii_digit())
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(id) = digits.parse::<u32>() {
                    if id >= 1 && id <= 3000 {
                        return Some(id);
                    }
                }
            }

            // "LeetCode #1" or "LC #1" or "Problem 1:"
            let lower = trimmed.to_lowercase();
            if lower.starts_with("leetcode") || lower.starts_with("lc") || lower.starts_with("problem") {
                let digits: String = trimmed.chars()
                    .skip_while(|c| !c.is_ascii_digit())
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(id) = digits.parse::<u32>() {
                    if id >= 1 && id <= 3000 {
                        return Some(id);
                    }
                }
            }
        }
        None
    }
}

/// Related problems suggestion based on current problem.
pub fn suggest_related(problem_id: u32, limit: usize) -> Vec<&'static KnowledgeProblem> {
    let kb = KnowledgeBase::global();
    let current = match kb.get(problem_id) {
        Some(p) => p,
        None => return vec![],
    };

    // Score other problems by shared topics/techniques
    let mut scores: Vec<(u32, usize)> = Vec::new();
    for &id in &kb.query(&super::knowledge::KnowledgeFilter::default()) {
        if id == problem_id {
            continue;
        }
        if let Some(other) = kb.get(id) {
            let mut score = 0;
            for t in &current.topics {
                if other.topics.contains(t) {
                    score += 1;
                }
            }
            for t in &current.techniques {
                if other.techniques.contains(t) {
                    score += 2; // Techniques are more specific
                }
            }
            if score > 0 {
                scores.push((id, score));
            }
        }
    }

    scores.sort_by(|a, b| b.1.cmp(&a.1));
    scores.iter()
        .take(limit)
        .filter_map(|(id, _)| kb.get(*id))
        .collect()
}
