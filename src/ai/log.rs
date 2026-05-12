//! Debug logging for AI subsystem.
//!
//! When `debug = true` in `[ai]` config (or `--debug` CLI flag),
//! all AI-related events are appended to `~/.hi/ai.log`.
//! The log file is auto-rotated when it exceeds 2 MB.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag — set once at startup from config.
static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

const MAX_LOG_SIZE: u64 = 2 * 1024 * 1024; // 2 MB

/// Call once at startup to enable/disable debug logging.
pub fn init(enabled: bool) {
    DEBUG_ENABLED.store(enabled, Ordering::Relaxed);
    if enabled {
        // Ensure ~/.hi/ directory exists
        if let Some(dir) = log_dir() {
            let _ = fs::create_dir_all(&dir);
        }
        log("=== hi debug logging started ===");
    }
}

/// Returns true if debug logging is enabled.
pub fn is_enabled() -> bool {
    DEBUG_ENABLED.load(Ordering::Relaxed)
}

/// Log a message to ~/.hi/ai.log (no-op if debug is disabled).
pub fn log(msg: &str) {
    if !is_enabled() { return; }
    let Some(path) = log_path() else { return };

    // Auto-rotate if file is too large
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_LOG_SIZE {
            let backup = path.with_extension("log.old");
            let _ = fs::rename(&path, &backup);
        }
    }

    let timestamp = chrono_now();
    let line = format!("[{}] {}\n", timestamp, msg);

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path);

    if let Ok(mut f) = file {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Log a multi-line block with a header (e.g. request body, response body).
pub fn log_block(header: &str, body: &str) {
    if !is_enabled() { return; }
    let separator = "─".repeat(60);
    log(&format!("┌{}", separator));
    log(&format!("│ {}", header));
    log(&format!("├{}", separator));
    for line in body.lines() {
        log(&format!("│ {}", line));
    }
    log(&format!("└{}", separator));
}

fn log_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".hi"))
}

fn log_path() -> Option<PathBuf> {
    log_dir().map(|d| d.join("ai.log"))
}

/// Simple timestamp without pulling in the `chrono` crate.
fn chrono_now() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Convert to rough human-readable: HH:MM:SS (UTC)
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    let ms = dur.subsec_millis();
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, ms)
}
