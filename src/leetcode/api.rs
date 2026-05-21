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
        // leetcode.cn and leetcode.com use different GraphQL schemas
        let query = match self.site {
            Site::CN => r#"
                query problemsetQuestionList($categorySlug: String, $limit: Int, $skip: Int, $filters: QuestionListFilterInput) {
                    problemsetQuestionList(categorySlug: $categorySlug, limit: $limit, skip: $skip, filters: $filters) {
                        questions {
                            frontendQuestionId
                            title
                            titleCn
                            titleSlug
                            difficulty
                            status
                            acRate
                            paidOnly
                        }
                    }
                }
            "#,
            Site::Global => r#"
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
            "#,
        };

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
                    let id_str = q["frontendQuestionId"].as_str()?;
                    // For CN site, prefer Chinese title if available
                    let title = if self.site == Site::CN {
                        q["titleCn"].as_str()
                            .filter(|s| !s.is_empty())
                            .unwrap_or(q["title"].as_str().unwrap_or(""))
                            .to_string()
                    } else {
                        q["title"].as_str()?.to_string()
                    };
                    Some(ProblemSummary {
                        frontend_id: id_str.parse().ok()?,
                        title,
                        title_slug: q["titleSlug"].as_str()?.to_string(),
                        difficulty: match q["difficulty"].as_str()? {
                            "Easy" | "EASY" => Difficulty::Easy,
                            "Medium" | "MEDIUM" => Difficulty::Medium,
                            _ => Difficulty::Hard,
                        },
                        status: match q["status"].as_str() {
                            Some("ac") | Some("AC") => SolveStatus::Solved,
                            Some("notac") | Some("TRIED") => SolveStatus::Attempted,
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
        let query = match self.site {
            Site::CN => r#"
                query questionData($titleSlug: String!) {
                    question(titleSlug: $titleSlug) {
                        questionFrontendId
                        title
                        translatedTitle
                        titleSlug
                        difficulty
                        status
                        content
                        translatedContent
                        codeSnippets {
                            lang
                            langSlug
                            code
                        }
                        exampleTestcaseList
                    }
                }
            "#,
            Site::Global => r#"
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
            "#,
        };

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

        // For CN site, prefer translated (Chinese) content; fall back to English
        let content_html = if self.site == Site::CN {
            q["translatedContent"].as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(q["content"].as_str().unwrap_or(""))
                .to_string()
        } else {
            q["content"].as_str().unwrap_or("").to_string()
        };
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

        // For CN site, prefer translated (Chinese) title; fall back to English
        let title = if self.site == Site::CN {
            q["translatedTitle"].as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(q["title"].as_str().unwrap_or(""))
                .to_string()
        } else {
            q["title"].as_str().unwrap_or("").to_string()
        };

        let summary = ProblemSummary {
            frontend_id: q["questionFrontendId"].as_str().unwrap_or("0").parse().unwrap_or(0),
            title,
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

    /// Submit code for a problem. Returns submission ID.
    pub fn submit_code(&self, title_slug: &str, question_id: u32, code: &str, lang_slug: &str) -> Result<String> {
        let url = format!("{}/problems/{}/submit/", self.site.base_url(), title_slug);

        let body = json!({
            "lang": lang_slug,
            "question_id": question_id.to_string(),
            "typed_code": code,
        });

        let mut req = self.http.post(&url)
            .header("Content-Type", "application/json")
            .header("Referer", format!("{}/problems/{}/", self.site.base_url(), title_slug));

        if let Some(ref session) = self.session {
            req = req.header("Cookie", format!("LEETCODE_SESSION={};csrftoken={}", session.session_cookie, session.csrf_token));
            req = req.header("x-csrftoken", &session.csrf_token);
        }

        let resp = req.json(&body).send()?;
        let status = resp.status();
        let body_text = resp.text().unwrap_or_default();

        if !status.is_success() {
            return Err(anyhow::anyhow!("HTTP {} — {}", status.as_u16(),
                if body_text.len() > 200 { &body_text[..200] } else { &body_text }));
        }

        let data: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| anyhow::anyhow!("Invalid JSON response: {} (body: {})",
                e, if body_text.len() > 100 { &body_text[..100] } else { &body_text }))?;

        let submission_id = data["submission_id"]
            .as_u64()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(submission_id)
    }

    /// Check submission result. Returns (state, status_msg, runtime, memory).
    /// state: "PENDING", "SUCCESS", "FAILURE", etc.
    pub fn check_submission(&self, submission_id: &str) -> Result<SubmissionResult> {
        let url = format!("{}/submissions/detail/{}/check/", self.site.base_url(), submission_id);

        let mut req = self.http.get(&url)
            .header("Referer", self.site.base_url());

        if let Some(ref session) = self.session {
            req = req.header("Cookie", format!("LEETCODE_SESSION={};csrftoken={}", session.session_cookie, session.csrf_token));
        }

        let resp = req.send()?;
        let status = resp.status();
        let body_text = resp.text().unwrap_or_default();

        if !status.is_success() {
            return Err(anyhow::anyhow!("HTTP {} checking submission", status.as_u16()));
        }

        let data: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| anyhow::anyhow!("Invalid JSON from check: {}", e))?;

        let state = data["state"].as_str().unwrap_or("UNKNOWN").to_string();
        let status_msg = data["status_msg"].as_str().unwrap_or("").to_string();
        let runtime = data["status_runtime"].as_str().unwrap_or("").to_string();
        let memory = data["status_memory"].as_str().unwrap_or("").to_string();
        let total_correct = data["total_correct"].as_u64().unwrap_or(0) as u32;
        let total_testcases = data["total_testcases"].as_u64().unwrap_or(0) as u32;
        let compile_error = data["compile_error"].as_str().unwrap_or("").to_string();
        let full_compile_error = data["full_compile_error"].as_str().unwrap_or("").to_string();
        let runtime_error = data["runtime_error"].as_str().unwrap_or("").to_string();
        let full_runtime_error = data["full_runtime_error"].as_str().unwrap_or("").to_string();
        // last_testcase: submit check may use "last_testcase" or "input" or "input_formatted"
        let last_testcase = data["last_testcase"].as_str()
            .or_else(|| data["input_formatted"].as_str())
            .or_else(|| data["input"].as_str())
            .unwrap_or("").to_string();
        let expected_output = data["expected_output"].as_str().unwrap_or("").to_string();
        let code_output = data["code_output"].as_str()
            .or_else(|| data["code_output"].as_array().map(|_| ""))
            .unwrap_or("").to_string();
        let code_output = if code_output.is_empty() {
            // code_output can be an array of strings
            data["code_output"].as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("\n"))
                .unwrap_or_default()
        } else {
            code_output
        };
        let std_output = data["std_output"].as_str()
            .map(|s| s.to_string())
            .or_else(|| data["std_output_list"].as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("\n")))
            .unwrap_or_default();

        // Parse error line from compile error (e.g. "Line 5:" or "line 5")
        let error_line = {
            let err_text = if !full_compile_error.is_empty() { &full_compile_error } else { &compile_error };
            parse_error_line(err_text)
        };

        Ok(SubmissionResult {
            state,
            status_msg,
            runtime,
            memory,
            total_correct,
            total_testcases,
            compile_error,
            full_compile_error,
            runtime_error,
            full_runtime_error,
            last_testcase,
            expected_output,
            code_output,
            std_output,
            error_line,
        })
    }

    /// Run code against test cases (without submitting). Returns interpret ID.
    pub fn run_code(&self, title_slug: &str, question_id: u32, code: &str, lang_slug: &str, test_input: &str) -> Result<String> {
        let url = format!("{}/problems/{}/interpret_solution/", self.site.base_url(), title_slug);

        let body = json!({
            "lang": lang_slug,
            "question_id": question_id.to_string(),
            "typed_code": code,
            "data_input": test_input,
        });

        let mut req = self.http.post(&url)
            .header("Content-Type", "application/json")
            .header("Referer", format!("{}/problems/{}/", self.site.base_url(), title_slug));

        if let Some(ref session) = self.session {
            req = req.header("Cookie", format!("LEETCODE_SESSION={};csrftoken={}", session.session_cookie, session.csrf_token));
            req = req.header("x-csrftoken", &session.csrf_token);
        }

        let resp = req.json(&body).send()?;
        let status = resp.status();
        let body_text = resp.text().unwrap_or_default();

        if !status.is_success() {
            return Err(anyhow::anyhow!("HTTP {} — {}", status.as_u16(),
                if body_text.len() > 200 { &body_text[..200] } else { &body_text }));
        }

        let data: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| anyhow::anyhow!("Invalid JSON response: {}", e))?;

        let interpret_id = data["interpret_id"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        Ok(interpret_id)
    }

    /// Get the daily coding challenge.
    /// Fetch submission history for a problem (most recent N submissions).
    pub fn fetch_submissions(&self, title_slug: &str, limit: usize) -> Result<Vec<SubmissionEntry>> {
        let query = r#"
            query submissionList($questionSlug: String!, $limit: Int, $offset: Int) {
                questionSubmissionList(questionSlug: $questionSlug, limit: $limit, offset: $offset) {
                    submissions {
                        id
                        lang
                        langName
                        statusDisplay
                        runtime
                        memory
                        timestamp
                        code
                    }
                }
            }
        "#;

        let body = json!({
            "query": query,
            "variables": {
                "questionSlug": title_slug,
                "limit": limit,
                "offset": 0
            }
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
        let submissions = data["data"]["questionSubmissionList"]["submissions"]
            .as_array()
            .map(|arr| {
                arr.iter().filter_map(|s| {
                    let lang_name = s["langName"].as_str().unwrap_or("").to_string();
                    // Convert langName to lang_slug (lowercase, no spaces)
                    let lang_slug = s["lang"].as_str().unwrap_or("").to_string();
                    Some(SubmissionEntry {
                        id: s["id"].as_str().unwrap_or("").to_string(),
                        lang: lang_name,
                        lang_slug,
                        status_display: s["statusDisplay"].as_str().unwrap_or("").to_string(),
                        runtime: s["runtime"].as_str().unwrap_or("N/A").to_string(),
                        memory: s["memory"].as_str().unwrap_or("N/A").to_string(),
                        timestamp: s["timestamp"].as_str()
                            .and_then(|t| t.parse().ok())
                            .or_else(|| s["timestamp"].as_u64())
                            .unwrap_or(0),
                        code: s["code"].as_str().unwrap_or("").to_string(),
                    })
                }).collect()
            })
            .unwrap_or_default();

        Ok(submissions)
    }

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

/// Parse error line number from compile/runtime error messages.
/// Looks for patterns like "Line 5:", "line 5,", "Line 12 in", etc.
fn parse_error_line(error_msg: &str) -> usize {
    // Pattern: "Line N" (case insensitive)
    for line in error_msg.lines() {
        let lower = line.to_lowercase();
        if let Some(pos) = lower.find("line ") {
            let after = &line[pos + 5..];
            let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = num_str.parse::<usize>() {
                if n > 0 {
                    return n;
                }
            }
        }
    }
    0
}
