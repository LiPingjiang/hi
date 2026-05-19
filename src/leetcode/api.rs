//! LeetCode API client — GraphQL + REST interactions.
//!
//! Supports both leetcode.com and leetcode.cn.

use anyhow::Result;
use serde_json::json;

use super::models::*;
use super::auth::Session;

/// Which LeetCode site to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Site {
    /// leetcode.com (international)
    Global,
    /// leetcode.cn (China)
    CN,
}

impl Site {
    pub fn base_url(&self) -> &'static str {
        match self {
            Site::Global => "https://leetcode.com",
            Site::CN => "https://leetcode.cn",
        }
    }

    pub fn graphql_url(&self) -> String {
        format!("{}/graphql", self.base_url())
    }
}

/// LeetCode API client.
pub struct LeetCodeClient {
    http: reqwest::blocking::Client,
    pub site: Site,
    pub session: Option<Session>,
}

impl LeetCodeClient {
    pub fn new(site: Site) -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self { http, site, session: None }
    }

    pub fn with_session(mut self, session: Session) -> Self {
        self.session = Some(session);
        self
    }

    pub fn is_logged_in(&self) -> bool {
        self.session.is_some()
    }

    /// Fetch the problem list via GraphQL.
    pub fn fetch_problem_list(&self, skip: usize, limit: usize) -> Result<Vec<ProblemSummary>> {
        let query = r#"
            query problemsetQuestionList($categorySlug: String, $limit: Int, $skip: Int, $filters: QuestionListFilterInput) {
                problemsetQuestionList: questionList(categorySlug: $categorySlug, limit: $limit, skip: $skip, filters: $filters) {
                    questions: data {
                        frontendQuestionId: questionFrontendId
                        title
                        titleSlug
                        difficulty
                        status
                        acRate
                        paidOnly: isPaidOnly
                    }
                }
            }
        "#;

        let variables = json!({
            "categorySlug": "all-code-essentials",
            "skip": skip,
            "limit": limit,
            "filters": {}
        });

        let body = json!({
            "query": query,
            "variables": variables
        });

        let mut req = self.http.post(self.site.graphql_url())
            .header("Content-Type", "application/json")
            .header("Referer", self.site.base_url());

        if let Some(ref session) = self.session {
            req = req.header("Cookie", format!("LEETCODE_SESSION={};csrftoken={}", session.session_cookie, session.csrf_token));
            req = req.header("x-csrftoken", &session.csrf_token);
        }

        let resp = req.json(&body).send()?;
        let data: serde_json::Value = resp.json()?;

        let questions = data["data"]["problemsetQuestionList"]["questions"]
            .as_array()
            .map(|arr| {
                arr.iter().filter_map(|q| {
                    Some(ProblemSummary {
                        frontend_id: q["frontendQuestionId"].as_str()?.parse().ok()?,
                        title: q["title"].as_str()?.to_string(),
                        title_slug: q["titleSlug"].as_str()?.to_string(),
                        difficulty: match q["difficulty"].as_str()? {
                            "Easy" => Difficulty::Easy,
                            "Medium" => Difficulty::Medium,
                            _ => Difficulty::Hard,
                        },
                        status: match q["status"].as_str() {
                            Some("ac") => SolveStatus::Solved,
                            Some("notac") => SolveStatus::Attempted,
                            _ => SolveStatus::NotStarted,
                        },
                        acceptance: q["acRate"].as_f64().unwrap_or(0.0) as f32,
                        paid_only: q["paidOnly"].as_bool().unwrap_or(false),
                    })
                }).collect()
            })
            .unwrap_or_default();

        Ok(questions)
    }

    /// Fetch full problem detail.
    pub fn fetch_problem_detail(&self, title_slug: &str) -> Result<ProblemDetail> {
        let query = r#"
            query questionData($titleSlug: String!) {
                question(titleSlug: $titleSlug) {
                    questionFrontendId
                    title
                    titleSlug
                    difficulty
                    status
                    content
                    codeSnippets {
                        lang
                        langSlug
                        code
                    }
                    exampleTestcaseList
                }
            }
        "#;

        let body = json!({
            "query": query,
            "variables": { "titleSlug": title_slug }
        });

        let mut req = self.http.post(self.site.graphql_url())
            .header("Content-Type", "application/json")
            .header("Referer", self.site.base_url());

        if let Some(ref session) = self.session {
            req = req.header("Cookie", format!("LEETCODE_SESSION={};csrftoken={}", session.session_cookie, session.csrf_token));
            req = req.header("x-csrftoken", &session.csrf_token);
        }

        let resp = req.json(&body).send()?;
        let data: serde_json::Value = resp.json()?;
        let q = &data["data"]["question"];

        let content_html = q["content"].as_str().unwrap_or("").to_string();
        // Simple HTML → plain text: strip tags
        let content_text = html_to_text(&content_html);

        let code_snippets: Vec<CodeSnippet> = q["codeSnippets"]
            .as_array()
            .map(|arr| {
                arr.iter().filter_map(|s| {
                    Some(CodeSnippet {
                        lang: s["lang"].as_str()?.to_string(),
                        lang_slug: s["langSlug"].as_str()?.to_string(),
                        code: s["code"].as_str()?.to_string(),
                    })
                }).collect()
            })
            .unwrap_or_default();

        let test_cases = q["exampleTestcaseList"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        let summary = ProblemSummary {
            frontend_id: q["questionFrontendId"].as_str().unwrap_or("0").parse().unwrap_or(0),
            title: q["title"].as_str().unwrap_or("").to_string(),
            title_slug: q["titleSlug"].as_str().unwrap_or("").to_string(),
            difficulty: match q["difficulty"].as_str() {
                Some("Easy") => Difficulty::Easy,
                Some("Medium") => Difficulty::Medium,
                _ => Difficulty::Hard,
            },
            status: match q["status"].as_str() {
                Some("ac") => SolveStatus::Solved,
                Some("notac") => SolveStatus::Attempted,
                _ => SolveStatus::NotStarted,
            },
            acceptance: 0.0,
            paid_only: false,
        };

        Ok(ProblemDetail {
            summary,
            content_text,
            code_snippets,
            test_cases,
        })
    }

    /// Get the daily coding challenge.
    pub fn daily_question(&self) -> Result<ProblemSummary> {
        let query = r#"
            query questionOfToday {
                activeDailyCodingChallengeQuestion {
                    question {
                        frontendQuestionId: questionFrontendId
                        title
                        titleSlug
                        difficulty
                        status
                        acRate
                    }
                }
            }
        "#;

        let body = json!({ "query": query, "variables": {} });

        let mut req = self.http.post(self.site.graphql_url())
            .header("Content-Type", "application/json")
            .header("Referer", self.site.base_url());

        if let Some(ref session) = self.session {
            req = req.header("Cookie", format!("LEETCODE_SESSION={};csrftoken={}", session.session_cookie, session.csrf_token));
            req = req.header("x-csrftoken", &session.csrf_token);
        }

        let resp = req.json(&body).send()?;
        let data: serde_json::Value = resp.json()?;
        let q = &data["data"]["activeDailyCodingChallengeQuestion"]["question"];

        Ok(ProblemSummary {
            frontend_id: q["frontendQuestionId"].as_str().unwrap_or("0").parse().unwrap_or(0),
            title: q["title"].as_str().unwrap_or("").to_string(),
            title_slug: q["titleSlug"].as_str().unwrap_or("").to_string(),
            difficulty: match q["difficulty"].as_str() {
                Some("Easy") => Difficulty::Easy,
                Some("Medium") => Difficulty::Medium,
                _ => Difficulty::Hard,
            },
            status: match q["status"].as_str() {
                Some("ac") => SolveStatus::Solved,
                Some("notac") => SolveStatus::Attempted,
                _ => SolveStatus::NotStarted,
            },
            acceptance: q["acRate"].as_f64().unwrap_or(0.0) as f32,
            paid_only: false,
        })
    }
}

/// Simple HTML to plain text converter (strips tags, decodes basic entities).
fn html_to_text(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut chars = html.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '<' => {
                // Check for <br>, <p>, <li> → insert newline
                let tag_start: String = chars.clone().take(4).collect();
                if tag_start.starts_with("br") || tag_start.starts_with("/p") || tag_start.starts_with("/li") {
                    result.push('\n');
                }
                if tag_start.starts_with("li") {
                    result.push_str("\n  • ");
                }
                in_tag = true;
            }
            '>' => { in_tag = false; }
            '&' if !in_tag => {
                // Decode common HTML entities
                let entity: String = chars.clone().take(10).take_while(|&c| c != ';').collect();
                let decoded = match entity.as_str() {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "nbsp" => " ",
                    "#39" => "'",
                    "le" => "≤",
                    "ge" => "≥",
                    _ => {
                        result.push('&');
                        continue;
                    }
                };
                result.push_str(decoded);
                // Skip past the entity
                for _ in 0..=entity.len() {
                    chars.next();
                }
            }
            _ if !in_tag => { result.push(c); }
            _ => {}
        }
    }

    // Clean up excessive blank lines
    let mut cleaned = String::new();
    let mut blank_count = 0;
    for line in result.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                cleaned.push('\n');
            }
        } else {
            blank_count = 0;
            cleaned.push_str(line);
            cleaned.push('\n');
        }
    }

    cleaned.trim().to_string()
}
