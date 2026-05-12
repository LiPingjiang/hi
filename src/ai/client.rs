//! HTTP client for the OpenAI-compatible chat completions API.
//! Uses `reqwest` in blocking mode so we can call from the synchronous
//! event loop (wrapped in `tokio::task::spawn_blocking` from `app.rs`).
//!
//! Local endpoints (Ollama, LM Studio, etc.) typically don't require an API
//! key — leave `api_key` empty and the Authorization header is omitted.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::Duration;

use crate::ai::log as ai_log;
use crate::ai::prompt::Message;
use crate::config::AiConfig;

// ── Request / Response wire types ────────────────────────────────────────────

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f64,
    max_tokens: u32,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
}

// ── Client ───────────────────────────────────────────────────────────────────

pub struct AiClient {
    base_url: String,
    api_key: String,
    model: String,
    timeout: Duration,
}

impl AiClient {
    pub fn new(cfg: &AiConfig) -> Self {
        Self {
            base_url: cfg.api_base_url.trim_end_matches('/').to_string(),
            api_key: cfg.api_key.clone(),
            model: cfg.model.clone(),
            timeout: Duration::from_secs(cfg.timeout_secs),
        }
    }

    /// Send a chat-completions request and return the assistant content.
    /// This is a blocking call — wrap with `spawn_blocking` on async contexts.
    ///
    /// An empty `api_key` is allowed: local endpoints (Ollama, LM Studio, etc.)
    /// don't require authentication, so we simply omit the Authorization header.
    pub fn chat(&self, messages: Vec<Message>) -> Result<String> {
        let chat_messages: Vec<ChatMessage> = messages
            .into_iter()
            .map(|m| ChatMessage { role: m.role, content: m.content })
            .collect();

        let body = ChatRequest {
            model: self.model.clone(),
            messages: chat_messages,
            temperature: 0.2,
            max_tokens: 2048,
            stream: false,
        };

        let url = format!("{}/chat/completions", self.base_url);

        // Debug: log request details
        ai_log::log(&format!("→ POST {} (model={}, timeout={}s)", url, self.model, self.timeout.as_secs()));
        ai_log::log(&format!("  auth: {}", if self.api_key.is_empty() { "none" } else { "bearer ***" }));
        if let Ok(json) = serde_json::to_string_pretty(&body) {
            ai_log::log_block("Request Body", &json);
        }

        let client = reqwest::blocking::Client::builder()
            .timeout(self.timeout)
            .build()
            .context("Failed to build HTTP client")?;

        // Only attach Authorization header when a key is provided.
        let req = client.post(&url).json(&body);
        let req = if self.api_key.is_empty() {
            req
        } else {
            req.bearer_auth(&self.api_key)
        };

        let resp = match req.send() {
            Ok(r) => r,
            Err(e) => {
                ai_log::log(&format!("✗ HTTP send failed: {}", e));
                if let Some(source) = e.source() {
                    ai_log::log(&format!("  cause: {}", source));
                }
                return Err(anyhow::anyhow!(e).context("HTTP request failed"));
            }
        };

        let status = resp.status();
        ai_log::log(&format!("← HTTP {} {}", status.as_u16(), status.canonical_reason().unwrap_or("")));

        if !status.is_success() {
            let text = resp.text().unwrap_or_default();
            ai_log::log_block(&format!("Error Response ({})", status.as_u16()), &text);
            anyhow::bail!("API error {}: {}", status, text);
        }

        let raw_text = resp.text().context("Failed to read response body")?;
        ai_log::log_block("Response Body", &raw_text);

        let parsed: ChatResponse = serde_json::from_str(&raw_text)
            .context("Failed to parse API response")?;

        let content = parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .context("API returned empty choices")?;

        ai_log::log(&format!("✓ AI response: {} chars", content.len()));
        Ok(content)
    }
}
