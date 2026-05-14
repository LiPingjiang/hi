//! Render-performance logging for development diagnostics.
//!
//! Writes per-frame timing data to `~/.hi/perf.log` when enabled via
//! `HI_PERF=1` environment variable.  Designed to be zero-cost when
//! disabled (all functions early-return on a global `AtomicBool`).
//!
//! ## Log format
//!
//! Each frame produces one structured log line:
//!
//! ```text
//! [HH:MM:SS.mmm] FRAME #42 total=3.2ms | preparse=0.8ms(lines=200) | lines=1.5ms(rows=50) | overlays=0.1ms | flush=0.3ms | buf=4096B | scroll=150
//! ```
//!
//! Slow frames (>8ms) are tagged with `⚠ SLOW`.
//! Very slow frames (>16ms) are tagged with `🔴 JANK`.
//!
//! Event-loop metrics are logged separately:
//!
//! ```text
//! [HH:MM:SS.mmm] EVENT drain=5 coalesced_scroll=4 dispatch=0.2ms
//! ```

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

// ── Global state ────────────────────────────────────────────────────────────

static ENABLED: AtomicBool = AtomicBool::new(false);
static FRAME_COUNTER: AtomicU64 = AtomicU64::new(0);

const MAX_LOG_SIZE: u64 = 5 * 1024 * 1024; // 5 MB — larger than ai.log since perf is verbose
const SLOW_FRAME_MS: f64 = 8.0;
const JANK_FRAME_MS: f64 = 16.0;

// ── Initialization ──────────────────────────────────────────────────────────

/// Call once at startup.  Reads `HI_PERF` env var.
pub fn init() {
    let enabled = std::env::var("HI_PERF")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    ENABLED.store(enabled, Ordering::Relaxed);
    if enabled {
        if let Some(dir) = log_dir() {
            let _ = fs::create_dir_all(&dir);
        }
        write_line("═══════════════════════════════════════════════════════════");
        write_line("  hi perf logging started  (HI_PERF=1)");
        write_line("═══════════════════════════════════════════════════════════");
    }
}

#[inline]
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

// ── Frame timing builder ────────────────────────────────────────────────────

/// Accumulates timing data for a single render frame.
/// Created by `FrameTimer::start()`, consumed by `FrameTimer::finish()`.
pub struct FrameTimer {
    frame_start: Instant,
    frame_id: u64,
    // Phase timings (set via setters)
    preparse_ms: f64,
    preparse_lines: usize,
    lines_ms: f64,
    lines_rows: usize,
    overlays_ms: f64,
    flush_ms: f64,
    flush_bytes: usize,
    scroll_line: usize,
    viewport_h: usize,
    total_lines: usize,
}

impl FrameTimer {
    /// Begin timing a new frame.  Returns `None` if perf logging is disabled.
    #[inline]
    pub fn start() -> Option<Self> {
        if !is_enabled() { return None; }
        let id = FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
        Some(Self {
            frame_start: Instant::now(),
            frame_id: id,
            preparse_ms: 0.0,
            preparse_lines: 0,
            lines_ms: 0.0,
            lines_rows: 0,
            overlays_ms: 0.0,
            flush_ms: 0.0,
            flush_bytes: 0,
            scroll_line: 0,
            viewport_h: 0,
            total_lines: 0,
        })
    }

    /// Record syntect pre-parse phase.
    pub fn set_preparse(&mut self, elapsed: std::time::Duration, lines: usize) {
        self.preparse_ms = elapsed.as_secs_f64() * 1000.0;
        self.preparse_lines = lines;
    }

    /// Record main line-rendering phase.
    pub fn set_lines(&mut self, elapsed: std::time::Duration, rows: usize) {
        self.lines_ms = elapsed.as_secs_f64() * 1000.0;
        self.lines_rows = rows;
    }

    /// Record overlay rendering (filetree separator, chat panel, popups).
    pub fn set_overlays(&mut self, elapsed: std::time::Duration) {
        self.overlays_ms = elapsed.as_secs_f64() * 1000.0;
    }

    /// Record flush phase.
    pub fn set_flush(&mut self, elapsed: std::time::Duration, bytes: usize) {
        self.flush_ms = elapsed.as_secs_f64() * 1000.0;
        self.flush_bytes = bytes;
    }

    /// Record viewport context for correlation.
    pub fn set_viewport(&mut self, scroll_line: usize, viewport_h: usize, total_lines: usize) {
        self.scroll_line = scroll_line;
        self.viewport_h = viewport_h;
        self.total_lines = total_lines;
    }

    /// Finish timing and write the log line.
    pub fn finish(self) {
        let total_ms = self.frame_start.elapsed().as_secs_f64() * 1000.0;

        let tag = if total_ms >= JANK_FRAME_MS {
            " 🔴 JANK"
        } else if total_ms >= SLOW_FRAME_MS {
            " ⚠ SLOW"
        } else {
            ""
        };

        let msg = format!(
            "FRAME #{} total={:.1}ms{} | preparse={:.1}ms(lines={}) | lines={:.1}ms(rows={}) | overlays={:.1}ms | flush={:.1}ms({}B) | scroll={} viewport={} total_lines={}",
            self.frame_id,
            total_ms,
            tag,
            self.preparse_ms,
            self.preparse_lines,
            self.lines_ms,
            self.lines_rows,
            self.overlays_ms,
            self.flush_ms,
            self.flush_bytes,
            self.scroll_line,
            self.viewport_h,
            self.total_lines,
        );
        write_line(&msg);
    }
}

// ── Event loop logging ──────────────────────────────────────────────────────

/// Log event-loop drain metrics after processing a batch of events.
pub fn log_event_batch(drain_count: usize, scroll_events: usize, dispatch_elapsed: std::time::Duration) {
    if !is_enabled() { return; }
    let dispatch_ms = dispatch_elapsed.as_secs_f64() * 1000.0;
    write_line(&format!(
        "EVENT drain={} coalesced_scroll={} dispatch={:.2}ms",
        drain_count, scroll_events, dispatch_ms,
    ));
}

/// Log a one-off diagnostic message (for ad-hoc instrumentation).
pub fn log_note(msg: &str) {
    if !is_enabled() { return; }
    write_line(&format!("NOTE  {}", msg));
}

// ── File I/O ────────────────────────────────────────────────────────────────

fn write_line(msg: &str) {
    let Some(path) = log_path() else { return };

    // Auto-rotate
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_LOG_SIZE {
            let backup = path.with_extension("log.old");
            let _ = fs::rename(&path, &backup);
        }
    }

    let ts = timestamp();
    let line = format!("[{}] {}\n", ts, msg);

    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = f.write_all(line.as_bytes());
    }
}

fn log_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".hi"))
}

fn log_path() -> Option<PathBuf> {
    log_dir().map(|d| d.join("perf.log"))
}

fn timestamp() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    let ms = dur.subsec_millis();
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, ms)
}
