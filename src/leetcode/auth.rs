//! LeetCode session/authentication management.
//!
//! Authentication is done via Cookie paste (most reliable cross-platform method).
//! Session is stored in ~/.config/hi/leetcode_session.json.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A LeetCode session (extracted from browser cookies).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub csrf_token: String,
    pub session_cookie: String,
    pub username: String,
    pub site: String, // "cn" or "global"
}

/// Get the session file path: ~/.config/hi/leetcode_session.json
fn session_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hi");
    config_dir.join("leetcode_session.json")
}

/// Load a saved session from disk.
pub fn load_session() -> Option<Session> {
    let path = session_path();
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save session to disk.
pub fn save_session(session: &Session) -> Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(session)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Clear saved session.
pub fn clear_session() -> Result<()> {
    let path = session_path();
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// Parse a raw cookie string (from browser) into a Session.
/// Expected format: "LEETCODE_SESSION=xxx; csrftoken=yyy"
/// or just the values separated by semicolons.
pub fn parse_cookie_string(raw: &str, username: &str, site: &str) -> Option<Session> {
    let mut session_cookie = String::new();
    let mut csrf_token = String::new();

    for part in raw.split(';') {
        let part = part.trim();
        if let Some(val) = part.strip_prefix("LEETCODE_SESSION=") {
            session_cookie = val.trim().to_string();
        } else if let Some(val) = part.strip_prefix("csrftoken=") {
            csrf_token = val.trim().to_string();
        }
    }

    if session_cookie.is_empty() || csrf_token.is_empty() {
        return None;
    }

    Some(Session {
        csrf_token,
        session_cookie,
        username: username.to_string(),
        site: site.to_string(),
    })
}
