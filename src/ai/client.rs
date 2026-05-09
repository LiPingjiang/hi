//! HTTP client for the OpenAI-compatible chat completions API.
//! Uses `reqwest` in blocking mode so we can call from the synchronous
//! event loop (wrapped in `tokio::task::spawn_blocking` from `app.rs`).
//!
//! Local endpoints (Ollama, LM Studio, etc.) typically don't require an API
//! key — leave `api_key` empty and the Authorization header is omitted.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

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

        let resp = req.send().context("HTTP request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            anyhow::bail!("API error {}: {}", status, text);
        }

        let parsed: ChatResponse = resp.json().context("Failed to parse API response")?;

        parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .context("API returned empty choices")
    }
}
