//! LeetCode Knowledge Base with inverted index for fast multi-dimensional queries.
//!
//! Data is embedded as gzip-compressed JSON at compile time and decompressed on first access.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;

/// Embedded compressed knowledge data
static KNOWLEDGE_GZ: &[u8] = include_bytes!("../../data/leetcode_knowledge/problems_v2.json.gz");

/// Global singleton for the knowledge base
static KNOWLEDGE: OnceLock<KnowledgeBase> = OnceLock::new();

/// A single problem entry from the knowledge base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeProblem {
    pub id: u32,
    pub title: String,
    pub slug: String,
    pub topics: Vec<String>,
    pub techniques: Vec<String>,
    pub patterns: Vec<String>,
    pub difficulty: u8,
    pub official_difficulty: String,
    pub key_insight: String,
    pub approach: String,
    pub time_complexity: String,
    pub space_complexity: String,
    pub code_python3: String,
}

/// Multi-dimensional filter for querying problems.
#[derive(Debug, Clone, Default)]
pub struct KnowledgeFilter {
    pub topics: Vec<String>,
    pub techniques: Vec<String>,
    pub patterns: Vec<String>,
    pub difficulty_min: Option<u8>,
    pub difficulty_max: Option<u8>,
    pub official_difficulty: Option<String>,
    pub search: String,
}

/// The knowledge base with inverted indices for fast lookup.
pub struct KnowledgeBase {
    /// All problems indexed by id
    problems: HashMap<u32, KnowledgeProblem>,
    /// Sorted list of all problem ids
    all_ids: Vec<u32>,

    // ── Inverted indices ──────────────────────────────────────────────────
    by_topic: HashMap<String, Vec<u32>>,
    by_technique: HashMap<String, Vec<u32>>,
    by_pattern: HashMap<String, Vec<u32>>,
    by_difficulty: HashMap<u8, Vec<u32>>,
    by_official_diff: HashMap<String, Vec<u32>>,

    // ── Metadata ─────────────────────────────────────────────────────────
    pub all_topics: Vec<String>,
    pub all_techniques: Vec<String>,
    pub all_patterns: Vec<String>,
}

impl KnowledgeBase {
    /// Load and index all problems from the embedded compressed data.
    fn load() -> Self {
        let mut decoder = flate2::read::GzDecoder::new(KNOWLEDGE_GZ);
        let mut json_str = String::new();
        decoder.read_to_string(&mut json_str).expect("Failed to decompress knowledge data");

        let problems_vec: Vec<KnowledgeProblem> =
            serde_json::from_str(&json_str).expect("Failed to parse knowledge JSON");

        let mut problems = HashMap::with_capacity(problems_vec.len());
        let mut all_ids = Vec::with_capacity(problems_vec.len());
        let mut by_topic: HashMap<String, Vec<u32>> = HashMap::new();
        let mut by_technique: HashMap<String, Vec<u32>> = HashMap::new();
        let mut by_pattern: HashMap<String, Vec<u32>> = HashMap::new();
        let mut by_difficulty: HashMap<u8, Vec<u32>> = HashMap::new();
        let mut by_official_diff: HashMap<String, Vec<u32>> = HashMap::new();

        let mut topic_set = std::collections::HashSet::new();
        let mut technique_set = std::collections::HashSet::new();
        let mut pattern_set = std::collections::HashSet::new();

        for p in problems_vec {
            let id = p.id;
            all_ids.push(id);

            for t in &p.topics {
                by_topic.entry(t.clone()).or_default().push(id);
                topic_set.insert(t.clone());
            }
            for t in &p.techniques {
                by_technique.entry(t.clone()).or_default().push(id);
                technique_set.insert(t.clone());
            }
            for t in &p.patterns {
                by_pattern.entry(t.clone()).or_default().push(id);
                pattern_set.insert(t.clone());
            }
            by_difficulty.entry(p.difficulty).or_default().push(id);
            by_official_diff.entry(p.official_difficulty.clone()).or_default().push(id);

            problems.insert(id, p);
        }

        all_ids.sort();

        let mut all_topics: Vec<String> = topic_set.into_iter().collect();
        all_topics.sort();
        let mut all_techniques: Vec<String> = technique_set.into_iter().collect();
        all_techniques.sort();
        let mut all_patterns: Vec<String> = pattern_set.into_iter().collect();
        all_patterns.sort();

        Self {
            problems,
            all_ids,
            by_topic,
            by_technique,
            by_pattern,
            by_difficulty,
            by_official_diff,
            all_topics,
            all_techniques,
            all_patterns,
        }
    }

    /// Get the global knowledge base instance (lazy-initialized).
    pub fn global() -> &'static Self {
        KNOWLEDGE.get_or_init(Self::load)
    }

    /// Get a problem by id.
    pub fn get(&self, id: u32) -> Option<&KnowledgeProblem> {
        self.problems.get(&id)
    }

    /// Total number of problems.
    pub fn len(&self) -> usize {
        self.problems.len()
    }

    /// Query problems matching the filter. Returns sorted ids.
    pub fn query(&self, filter: &KnowledgeFilter) -> Vec<u32> {
        let mut candidates: Option<Vec<u32>> = None;

        // Intersect by topics
        for topic in &filter.topics {
            if let Some(ids) = self.by_topic.get(topic) {
                candidates = Some(Self::intersect(candidates, ids));
            } else {
                return vec![];
            }
        }

        // Intersect by techniques
        for tech in &filter.techniques {
            if let Some(ids) = self.by_technique.get(tech) {
                candidates = Some(Self::intersect(candidates, ids));
            } else {
                return vec![];
            }
        }

        // Intersect by patterns
        for pat in &filter.patterns {
            if let Some(ids) = self.by_pattern.get(pat) {
                candidates = Some(Self::intersect(candidates, ids));
            } else {
                return vec![];
            }
        }

        // Intersect by official difficulty
        if let Some(ref od) = filter.official_difficulty {
            if let Some(ids) = self.by_official_diff.get(od) {
                candidates = Some(Self::intersect(candidates, ids));
            } else {
                return vec![];
            }
        }

        // Start with all ids if no index filter applied
        let mut result = candidates.unwrap_or_else(|| self.all_ids.clone());

        // Filter by difficulty range
        if filter.difficulty_min.is_some() || filter.difficulty_max.is_some() {
            let min = filter.difficulty_min.unwrap_or(1);
            let max = filter.difficulty_max.unwrap_or(10);
            result.retain(|id| {
                if let Some(p) = self.problems.get(id) {
                    p.difficulty >= min && p.difficulty <= max
                } else {
                    false
                }
            });
        }

        // Filter by search text
        if !filter.search.is_empty() {
            let q = filter.search.to_lowercase();
            result.retain(|id| {
                if let Some(p) = self.problems.get(id) {
                    p.title.to_lowercase().contains(&q)
                        || p.id.to_string().contains(&q)
                        || p.key_insight.contains(&q)
                } else {
                    false
                }
            });
        }

        result
    }

    /// Get statistics: count per topic.
    pub fn topic_stats(&self) -> Vec<(&str, usize)> {
        let mut stats: Vec<(&str, usize)> = self
            .by_topic
            .iter()
            .map(|(k, v)| (k.as_str(), v.len()))
            .collect();
        stats.sort_by(|a, b| b.1.cmp(&a.1));
        stats
    }

    /// Get statistics: count per technique.
    pub fn technique_stats(&self) -> Vec<(&str, usize)> {
        let mut stats: Vec<(&str, usize)> = self
            .by_technique
            .iter()
            .map(|(k, v)| (k.as_str(), v.len()))
            .collect();
        stats.sort_by(|a, b| b.1.cmp(&a.1));
        stats
    }

    /// Get statistics: count per difficulty level.
    pub fn difficulty_stats(&self) -> Vec<(u8, usize)> {
        let mut stats: Vec<(u8, usize)> = self
            .by_difficulty
            .iter()
            .map(|(k, v)| (*k, v.len()))
            .collect();
        stats.sort_by_key(|x| x.0);
        stats
    }

    /// Get statistics: count per pattern.
    pub fn pattern_stats(&self) -> Vec<(&str, usize)> {
        let mut stats: Vec<(&str, usize)> = self
            .by_pattern
            .iter()
            .map(|(k, v)| (k.as_str(), v.len()))
            .collect();
        stats.sort_by(|a, b| b.1.cmp(&a.1));
        stats
    }

    /// Get problem ids for a given topic.
    pub fn topic_ids(&self, topic: &str) -> Option<&[u32]> {
        self.by_topic.get(topic).map(|v| v.as_slice())
    }

    /// Get problem ids for a given technique.
    pub fn technique_ids(&self, technique: &str) -> Option<&[u32]> {
        self.by_technique.get(technique).map(|v| v.as_slice())
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn intersect(current: Option<Vec<u32>>, new_ids: &[u32]) -> Vec<u32> {
        match current {
            None => new_ids.to_vec(),
            Some(existing) => {
                // Both are sorted, use merge-intersect
                let new_set: std::collections::HashSet<u32> = new_ids.iter().copied().collect();
                existing.into_iter().filter(|id| new_set.contains(id)).collect()
            }
        }
    }
}
