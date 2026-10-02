//! HTTP client for the OpenAI-compatible chat completions API.
//! Uses `reqwest` in blocking mode so we can call from the synchronous
//! event loop (wrapped in `tokio::task::spawn_blocking` from `app.rs`).
//!
//! Local endpoints (Ollama, LM Studio, etc.) typically don't require an API
//! key — leave `api_key` empty and the Authorization header is omitted.
//!
//! Supports fallback providers: if the primary endpoint fails, the client
//! will try each configured fallback in order until one succeeds.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::Duration;

use crate::ai::log as ai_log;
use crate::ai::prompt::Message;
use crate::config::AiConfig;

// ── Request / Response wire types ────────────────────────────────────────────

#[derive(Serialize, Clone)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f64,
    max_tokens: u32,
    stream: bool,
}

#[derive(Serialize, Deserialize, Clone)]
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

// ── Provider endpoint descriptor ─────────────────────────────────────────────

struct ProviderEndpoint {
    base_url: String,
    api_key: String,
    model: String,
    timeout: Duration,
    label: String,
}

// ── Client ───────────────────────────────────────────────────────────────────

pub struct AiClient {
    providers: Vec<ProviderEndpoint>,
}

impl AiClient {
    pub fn new(cfg: &AiConfig) -> Self {
        let mut providers = Vec::new();

        // Primary provider
        providers.push(ProviderEndpoint {
            base_url: cfg.api_base_url.trim_end_matches('/').to_string(),
            api_key: cfg.api_key.clone(),
            model: cfg.model.clone(),
            timeout: Duration::from_secs(cfg.timeout_secs),
            label: "primary".to_string(),
        });

        // Fallback providers
        for (i, fb) in cfg.fallback.iter().enumerate() {
            providers.push(ProviderEndpoint {
                base_url: fb.api_base_url.trim_end_matches('/').to_string(),
                api_key: fb.api_key.clone(),
                model: fb.model.clone(),
                timeout: Duration::from_secs(fb.timeout_secs),
                label: format!("fallback[{}]", i),
            });
        }

        Self { providers }
    }

    /// Send a chat-completions request and return the assistant content.
    /// Tries the primary provider first, then each fallback in order.
    /// This is a blocking call — wrap with `spawn_blocking` on async contexts.
    pub fn chat(&self, messages: Vec<Message>) -> Result<String> {
        let chat_messages: Vec<ChatMessage> = messages
            .into_iter()
            .map(|m| ChatMessage { role: m.role, content: m.content })
            .collect();

        let mut last_error: Option<anyhow::Error> = None;

        for provider in &self.providers {
            let body = ChatRequest {
                model: provider.model.clone(),
                messages: chat_messages.clone(),
                temperature: 0.2,
                max_tokens: 2048,
                stream: false,
            };

            match self.try_provider(provider, &body) {
                Ok(content) => return Ok(content),
                Err(e) => {
                    ai_log::log(&format!(
                        "⚠ Provider [{}] failed: {}, trying next...",
                        provider.label, e
                    ));
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("No providers configured")))
    }

    /// Attempt a single request against one provider endpoint.
    fn try_provider(&self, provider: &ProviderEndpoint, body: &ChatRequest) -> Result<String> {
        let url = format!("{}/chat/completions", provider.base_url);

        ai_log::log(&format!(
            "→ POST {} [{}] (model={}, timeout={}s)",
            url, provider.label, provider.model, provider.timeout.as_secs()
        ));
        ai_log::log(&format!(
            "  auth: {}",
            if provider.api_key.is_empty() { "none" } else { "bearer ***" }
        ));
        if let Ok(json) = serde_json::to_string_pretty(body) {
            ai_log::log_block("Request Body", &json);
        }

        let client = reqwest::blocking::Client::builder()
            .timeout(provider.timeout)
            .build()
            .context("Failed to build HTTP client")?;

        let req = client.post(&url).json(body);
        let req = if provider.api_key.is_empty() {
            req
        } else {
            req.bearer_auth(&provider.api_key)
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
        ai_log::log(&format!(
            "← HTTP {} {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or("")
        ));

        if !status.is_success() {
            let text = resp.text().unwrap_or_default();
            ai_log::log_block(&format!("Error Response ({})", status.as_u16()), &text);
            anyhow::bail!("API error {}: {}", status, text);
        }

        let raw_text = resp.text().context("Failed to read response body")?;
        ai_log::log_block("Response Body", &raw_text);

        let parsed: ChatResponse =
            serde_json::from_str(&raw_text).context("Failed to parse API response")?;

        let content = parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .context("API returned empty choices")?;

        ai_log::log(&format!(
            "✓ AI response [{}]: {} chars",
            provider.label,
            content.len()
        ));
        Ok(content)
    }
}
