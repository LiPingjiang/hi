//! LeetCode "古法时代" TUI Panel — retro phosphor-green terminal aesthetic.
//!
//! Design: DOS-era double-line borders (╔═╗║╚╝), amber/green phosphor colors,
//! ASCII art logo, scanline-style separators.

use std::io::Write as _IoWrite2;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ropey::Rope;

use crate::editor::Editor;
use crate::config::Config;
use crate::mode::Mode;
use crate::mode::normal::NormalAction;
use crate::mode::insert::InsertAction;
use crate::syntax::highlight::{FileType, CodePalette};
use crate::syntax::TsHighlighter;
use crate::ui::chatpanel::{ChatPanel, ChatRole};
use crate::ai::client::AiClient;
use crate::ai::prompt::Message;

macro_rules! lc_debug {
    ($($arg:tt)*) => {{
        if let Some(mut home) = dirs::home_dir() {
            home.push(".hi");
            let _ = std::fs::create_dir_all(&home);
            home.push("leetcode_debug.log");
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&home) {
                use std::time::SystemTime;
                let elapsed = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
                let secs = elapsed.as_secs() % 86400;
                let _ = writeln!(f, "[{:02}:{:02}:{:02}] {}", secs/3600, (secs%3600)/60, secs%60, format_args!($($arg)*));
            }
        }
    }};
}

use super::models::*;
use super::api::{LeetCodeClient, Site};
use super::auth;
use super::cache;
use super::knowledge_panel::KnowledgePanel as KnowledgeMapPanel;

/// Splash screen lines with per-line color zone markers.
/// 'L' = logo (bright fg), 'T' = title text (fg), 'B' = border (dim),
/// 'D' = decoration accent, 'C' = cool accent (blue/cyan), 'W' = warm accent
const SPLASH_LINES: &[(&str, char)] = &[
    ("", ' '),
    ("", ' '),
    ("██╗     ███████╗███████╗████████╗ ██████╗ ██████╗ ██████╗ ███████╗", 'L'),
    ("██║     ██╔════╝██╔════╝╚══██╔══╝██╔════╝██╔═══██╗██╔══██╗██╔════╝", 'L'),
    ("██║     █████╗  █████╗     ██║   ██║     ██║   ██║██║  ██║█████╗", 'L'),
    ("██║     ██╔══╝  ██╔══╝     ██║   ██║     ██║   ██║██║  ██║██╔══╝", 'L'),
    ("███████╗███████╗███████╗   ██║   ╚██████╗╚██████╔╝██████╔╝███████╗", 'L'),
    ("╚══════╝╚══════╝╚══════╝   ╚═╝    ╚═════╝ ╚═════╝ ╚═════╝ ╚══════╝", 'L'),
    ("", ' '),
    ("──────────────── 古 法 时 代 · RETRO MODE ────────────────", 'T'),
    ("", ' '),
    ("▁▂▃▄▅▆▇█▇▆▅▄▃▂▁    ◈    ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁    ◈    ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁", 'W'),
    ("─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─", 'D'),
    ("◇ · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · ◇", 'C'),
    ("", ' '),
    ("", ' '),
];

/// Splash color schemes — user can switch via `:color <name>`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplashColor {
    /// Classic green phosphor (default).
    Green,
    /// Amber/gold CRT.
    Amber,
    /// Cool cyan/blue.
    Cyan,
    /// Hot magenta/pink.
    Magenta,
    /// White/silver monochrome.
    Mono,
}

impl SplashColor {
    pub fn name(self) -> &'static str {
        match self {
            SplashColor::Green   => "green",
            SplashColor::Amber   => "amber",
            SplashColor::Cyan    => "cyan",
            SplashColor::Magenta => "magenta",
            SplashColor::Mono    => "mono",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "green"   => Some(SplashColor::Green),
            "amber"   => Some(SplashColor::Amber),
            "cyan"    => Some(SplashColor::Cyan),
            "magenta" => Some(SplashColor::Magenta),
            "mono"    => Some(SplashColor::Mono),
            _         => None,
        }
    }

    pub fn all() -> &'static [SplashColor] {
        &[SplashColor::Green, SplashColor::Amber, SplashColor::Cyan, SplashColor::Magenta, SplashColor::Mono]
    }

    /// Primary foreground RGB for this color scheme (bright).
    pub fn fg(self) -> (u8, u8, u8) {
        match self {
            SplashColor::Green   => (50, 255, 100),
            SplashColor::Amber   => (255, 200, 50),
            SplashColor::Cyan    => (80, 240, 255),
            SplashColor::Magenta => (255, 120, 220),
            SplashColor::Mono    => (230, 230, 230),
        }
    }

    /// Dim/border foreground RGB (medium brightness, not too dark).
    pub fn dim(self) -> (u8, u8, u8) {
        match self {
            SplashColor::Green   => (0, 180, 60),
            SplashColor::Amber   => (180, 120, 0),
            SplashColor::Cyan    => (0, 150, 180),
            SplashColor::Magenta => (180, 60, 140),
            SplashColor::Mono    => (140, 140, 140),
        }
    }

    /// Accent/decoration color RGB — subtle decoration elements.
    pub fn accent(self) -> (u8, u8, u8) {
        match self {
            SplashColor::Green   => (0, 140, 100),    // Teal/dark-cyan on green
            SplashColor::Amber   => (140, 100, 0),    // Dark gold on amber
            SplashColor::Cyan    => (0, 100, 140),    // Deep blue on cyan
            SplashColor::Magenta => (100, 0, 100),    // Deep purple on magenta
            SplashColor::Mono    => (80, 80, 100),    // Subtle blue-grey on mono
        }
    }

    /// Cool accent color RGB — blue/cyan tones for borders and structure.
    pub fn cool(self) -> (u8, u8, u8) {
        match self {
            SplashColor::Green   => (60, 160, 220),   // Sky blue
            SplashColor::Amber   => (100, 180, 255),  // Light blue
            SplashColor::Cyan    => (100, 200, 255),  // Bright cyan
            SplashColor::Magenta => (120, 140, 255),  // Periwinkle
            SplashColor::Mono    => (160, 170, 200),  // Cool grey
        }
    }

    /// Warm accent color RGB — for sparkline/wave highlights.
    pub fn warm(self) -> (u8, u8, u8) {
        match self {
            SplashColor::Green   => (100, 220, 180),  // Mint/aqua
            SplashColor::Amber   => (255, 220, 100),  // Warm yellow
            SplashColor::Cyan    => (180, 255, 200),  // Light mint
            SplashColor::Magenta => (255, 160, 220),  // Light pink
            SplashColor::Mono    => (200, 200, 220),  // Warm white
        }
    }
}

/// Available commands for completion in different views.
pub fn splash_commands() -> &'static [&'static str] {
    &["q", "quit", "color green", "color amber", "color cyan", "color magenta", "color mono", "color"]
}

pub fn list_commands() -> &'static [&'static str] {
    &["q", "quit"]
}

pub fn coding_commands() -> &'static [&'static str] {
    &[
        "q", "quit", "w", "write", "wq",
        "run", "submit", "sub",
        "lang", "lang!", "desc", "ai", "result", "res",
        "history", "hist",
        "theme catppuccin", "theme tokyo-night", "theme gruvbox",
        "theme night-owl", "theme ayu", "theme one-dark-pro",
        "theme monokai-pro", "theme dracula", "theme neon-minimalist",
        "theme glow-dark", "theme github-dark", "theme synthwave",
        "theme",
        "color green", "color amber", "color cyan", "color magenta", "color mono",
        "color",
    ]
}


/// Retro color palette (ANSI 256-color indices).
pub struct RetroColors;

impl RetroColors {
    /// Amber phosphor foreground.
    pub const AMBER: (u8, u8, u8) = (255, 176, 0);
    /// Green phosphor foreground.
    pub const GREEN: (u8, u8, u8) = (0, 255, 65);
    /// Dim green for borders.
    pub const DIM_GREEN: (u8, u8, u8) = (0, 128, 32);
    /// Dark background.
    pub const BG: (u8, u8, u8) = (8, 12, 8);
    /// Highlight bar.
    pub const HIGHLIGHT: (u8, u8, u8) = (0, 64, 16);
    /// Easy difficulty.
    pub const EASY: (u8, u8, u8) = (0, 200, 80);
    /// Medium difficulty.
    pub const MEDIUM: (u8, u8, u8) = (255, 176, 0);
    /// Hard difficulty.
    pub const HARD: (u8, u8, u8) = (255, 60, 60);
}

/// Sub-view within the LeetCode panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LeetCodeView {
    /// Splash screen with logo.
    Splash,
    /// Problem list browser.
    ProblemList,
    /// Coding workspace (editor + optional panels).
    Coding,
    /// Login prompt.
    Login,
    /// Knowledge map visualization.
    KnowledgeMap,
}

/// Which pane is focused in the coding workspace.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CodingFocus {
/// Code editor (main area).
Editor,
/// Problem description panel (left/top).
Description,
/// AI chat panel (right).
AiChat,
/// Result/error panel (bottom).
Result,
}

/// State for the coding workspace.
pub struct CodingState {
    /// The problem being worked on.
    pub detail: ProblemDetail,
    /// Currently selected language index (into detail.code_snippets).
    pub lang_index: usize,
    /// Embedded full-featured editor (Vim modes, undo, etc.)
    pub editor: Editor,
    /// Tree-sitter syntax highlighter for the code editor.
    pub ts_hl: TsHighlighter,
    /// Whether the description panel is visible.
    pub show_description: bool,
    /// Scroll offset for the description panel.
    pub desc_scroll: usize,
    /// Whether the AI chat panel is visible.
    pub show_ai: bool,
    /// Which pane is focused.
    pub focus: CodingFocus,
    /// Submission result (if any).
    pub result: Option<SubmissionResult>,
    /// Whether the error/result panel is visible.
    pub show_result_panel: bool,
    /// Scroll offset for the result panel.
    pub result_scroll: usize,
    /// Error line number to highlight in editor (0 = none).
    pub error_line: usize,
    /// Status message for the coding view.
    pub status_msg: String,
    /// Whether we're in language selection mode.
    pub selecting_lang: bool,
    /// Language selection cursor.
    pub lang_cursor: usize,
    /// Last time the solution was auto-saved (for periodic saving).
    pub last_save: std::time::Instant,
    /// Whether the code has been modified since last save.
    pub dirty: bool,
    /// Current syntax highlight theme name.
    pub theme_name: String,
    /// AI chat panel state (conversation history, scroll, etc.)
    pub ai_chat: ChatPanel,
    /// AI chat input buffer (what the user is typing).
    pub ai_input: String,
    /// Cursor position within ai_input (char index).
    pub ai_input_cursor: usize,
    /// Whether AI is currently processing a request.
    pub ai_pending: bool,
    /// Shared result slot for async AI response.
    pub ai_result: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    /// Code snapshot sent to AI last time (to detect changes).
    pub ai_last_code: String,
    /// Whether we're waiting for user to confirm lang switch (dirty buffer).
    pub lang_confirm_pending: bool,
    /// Submission history list (fetched from LeetCode API).
    pub submissions: Vec<SubmissionEntry>,
    /// Whether we're in history selection mode.
    pub selecting_history: bool,
    /// History selection cursor.
    pub history_cursor: usize,
    /// History scroll offset (for long lists).
    pub history_scroll: usize,
    /// Whether a submit is pending (deferred to next tick for UI feedback).
    pub submit_pending: bool,
    /// Spinner tick counter for submit animation.
    pub submit_tick: u8,
}

impl CodingState {
    /// Get the code text from the embedded editor's buffer.
    pub fn code(&self) -> String {
        self.editor.buffer.rope.to_string()
    }

    /// Set the code text in the embedded editor's buffer.
    pub fn set_code(&mut self, code: &str) {
        self.editor.buffer.rope = Rope::from_str(code);
        self.editor.buffer.modified = true;
        self.editor.clamp_cursor();
        self.dirty = true;
    }

    /// Map a LeetCode language slug to a `FileType` for syntax highlighting.
    pub fn filetype_from_lang_slug(slug: &str) -> FileType {
        match slug {
            "rust" => FileType::Rust,
            "python" | "python3" => FileType::Python,
            "java" => FileType::Java,
            "golang" | "go" => FileType::Go,
            "javascript" => FileType::JavaScript,
            "typescript" => FileType::TypeScript,
            "bash" | "shell" => FileType::Shell,
            _ => FileType::Plain,
        }
    }

    /// Get the current language's FileType.
    pub fn current_filetype(&self) -> FileType {
        let slug = self.detail.code_snippets.get(self.lang_index)
            .map(|s| s.lang_slug.as_str())
            .unwrap_or("");
        Self::filetype_from_lang_slug(slug)
    }
}

/// Which step of the login flow we're on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoginStep {
    /// Waiting for user to paste LEETCODE_SESSION value.
    Session,
    /// Waiting for user to paste csrftoken value.
    CsrfToken,
}

/// Action returned from the panel's key handler.
#[derive(Debug)]
pub enum LeetCodeAction {
    /// Nothing happened, stay in panel.
    None,
    /// Close the LeetCode panel, return to editor.
    Close,
    /// Redraw needed.
    Redraw,
}

/// Whether the panel is in search-input mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ListMode {
    /// Normal navigation.
    Normal,
    /// Typing a search query.
    Search,
    /// Typing a : command (e.g. :q).
    Command,
}

/// The main LeetCode panel state.
pub struct LeetCodePanel {
    pub view: LeetCodeView,
    pub problems: Vec<ProblemSummary>,
    pub filtered: Vec<usize>, // indices into `problems`
    pub cursor: usize,
    pub scroll_offset: usize,
    pub filter: ProblemFilter,
    /// Coding workspace state (active when view == Coding).
    pub coding: Option<CodingState>,
    pub site: Site,
    pub logged_in: bool,
    pub status_msg: String,
    /// Active splash color scheme.
    pub splash_color: SplashColor,
    /// Command completion candidates (shown as hint while typing).
    pub cmd_completions: Vec<String>,
    /// Login input buffer (current step's input).
    pub login_input: String,
    /// Which login step we're on.
    pub login_step: LoginStep,
    /// Saved LEETCODE_SESSION value from step 1.
    pub login_session_value: String,
    /// Current list interaction mode (normal vs search input).
    pub list_mode: ListMode,
    /// Search input buffer (shown in footer when in search mode).
    pub search_input: String,
    /// Number of items per page for pagination.
    pub page_size: usize,
    /// Knowledge map visualization panel.
    pub knowledge_panel: Option<KnowledgeMapPanel>,
    /// Whether splash screen is in command input mode.
    pub splash_cmd_active: bool,
    /// Command input buffer for splash screen.
    pub splash_cmd_input: String,
    /// Flag set by :q command to signal close.
    splash_cmd_close: bool,
}

impl LeetCodePanel {
    pub fn new() -> Self {
        // Try to load cached problems
        let problems = cache::load_problem_list().unwrap_or_default();
        let filtered: Vec<usize> = (0..problems.len()).collect();

        // Check for existing session
        let session = auth::load_session();
        let logged_in = session.is_some();
        let site = session
            .as_ref()
            .map(|s| if s.site == "cn" { Site::CN } else { Site::Global })
            .unwrap_or(Site::CN);

        let splash_color = Self::load_splash_color();

        Self {
            view: LeetCodeView::Splash,
            problems,
            filtered,
            cursor: 0,
            scroll_offset: 0,
            filter: ProblemFilter::default(),
            coding: None,
            site,
            logged_in,
            status_msg: String::from("Press any key to continue..."),
            splash_color,
            cmd_completions: Vec::new(),
            login_input: String::new(),
            login_step: LoginStep::Session,
            login_session_value: String::new(),
            list_mode: ListMode::Normal,
            search_input: String::new(),
            page_size: 20,
            knowledge_panel: None,
            splash_cmd_active: false,
            splash_cmd_input: String::new(),
            splash_cmd_close: false,
        }
    }

    /// Handle a key event. Returns an action for the app to process.
    pub fn handle_key(&mut self, key: KeyEvent) -> LeetCodeAction {
        match self.view {
            LeetCodeView::Splash => self.handle_splash_key(key),
            LeetCodeView::ProblemList => self.handle_list_key(key),
            LeetCodeView::Coding => self.handle_coding_key(key),
            LeetCodeView::Login => self.handle_login_key(key),
            LeetCodeView::KnowledgeMap => self.handle_knowledge_key(key),
        }
    }

    fn handle_splash_key(&mut self, key: KeyEvent) -> LeetCodeAction {
        // If in splash command input mode, handle typing
        if self.splash_cmd_active {
            return self.handle_splash_cmd_input(key);
        }

        match key.code {
            // ':' enters command mode on splash screen (for color / :q)
            KeyCode::Char(':') => {
                self.splash_cmd_active = true;
                self.splash_cmd_input.clear();
                self.update_cmd_completions(splash_commands());
                LeetCodeAction::Redraw
            }
            KeyCode::Esc => LeetCodeAction::None, // Esc does nothing on splash
            _ => {
                // Any other key → transition to problem list (or login if not logged in)
                if self.problems.is_empty() && !self.logged_in {
                    self.view = LeetCodeView::Login;
                    self.status_msg = String::from("Paste your LeetCode cookie (LEETCODE_SESSION=...;csrftoken=...):");
                } else if self.problems.is_empty() {
                    self.status_msg = String::from("Loading problems...");
                    self.fetch_problems();
                    self.view = LeetCodeView::ProblemList;
                } else {
                    self.view = LeetCodeView::ProblemList;
                    self.update_status_msg();
                }
                LeetCodeAction::Redraw
            }
        }
    }

    /// Handle command input on the splash screen.
    fn handle_splash_cmd_input(&mut self, key: KeyEvent) -> LeetCodeAction {
        match key.code {
            KeyCode::Esc => {
                self.splash_cmd_active = false;
                self.splash_cmd_input.clear();
                self.cmd_completions.clear();
                self.status_msg = String::from("Press any key to continue...");
                LeetCodeAction::Redraw
            }
            KeyCode::Enter => {
                let cmd = self.splash_cmd_input.trim().to_string();
                self.splash_cmd_active = false;
                self.splash_cmd_input.clear();
                self.cmd_completions.clear();
                self.execute_splash_cmd(&cmd);
                if self.splash_cmd_close {
                    self.splash_cmd_close = false;
                    return LeetCodeAction::Close;
                }
                LeetCodeAction::Redraw
            }
            // Tab: accept first completion
            KeyCode::Tab => {
                if let Some(first) = self.cmd_completions.first() {
                    self.splash_cmd_input = first.clone();
                    self.update_cmd_completions(splash_commands());
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Backspace => {
                self.splash_cmd_input.pop();
                self.update_cmd_completions(splash_commands());
                LeetCodeAction::Redraw
            }
            KeyCode::Char(c) => {
                self.splash_cmd_input.push(c);
                self.update_cmd_completions(splash_commands());
                LeetCodeAction::Redraw
            }
            _ => LeetCodeAction::None,
        }
    }

    /// Update command completions based on current input prefix.
    fn update_cmd_completions(&mut self, commands: &[&str]) {
        let input = self.splash_cmd_input.as_str();
        if input.is_empty() {
            self.cmd_completions = commands.iter().map(|s| s.to_string()).collect();
        } else {
            self.cmd_completions = commands.iter()
                .filter(|c| c.starts_with(input) && **c != input)
                .map(|s| s.to_string())
                .collect();
        }
    }

    /// Execute a command entered on the splash screen.
    /// Returns true if the command should close the panel.
    fn execute_splash_cmd(&mut self, cmd: &str) {
        let trimmed = cmd.trim();
        let available: Vec<&str> = SplashColor::all().iter().map(|c| c.name()).collect();

        // :q / :quit — close LeetCode panel
        if trimmed == "q" || trimmed == "quit" {
            self.splash_cmd_close = true;
            return;
        }

        // :color (no name) — list available colors
        if trimmed == "color" {
            self.status_msg = format!("Colors: {}  (current: {})", available.join(", "), self.splash_color.name());
            return;
        }

        // :color <name>
        if let Some(rest) = trimmed.strip_prefix("color ") {
            let color_name = rest.trim();
            if let Some(color) = SplashColor::from_name(color_name) {
                self.splash_color = color;
                self.save_splash_color();
                self.status_msg = format!("Color: {} ✓", color.name());
            } else {
                self.status_msg = format!("Unknown color. Available: {}", available.join(", "));
            }
            return;
        }

        self.status_msg = format!("Unknown command. Try :q | :color <{}>", available.join("/"));
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> LeetCodeAction {
        // If in search mode, handle search input first
        if self.list_mode == ListMode::Search {
            return self.handle_search_input(key);
        }
        // If in command mode, handle command input
        if self.list_mode == ListMode::Command {
            return self.handle_list_cmd_input(key);
        }

        match key.code {
            // ':' enters command mode (supports :q to close panel)
            KeyCode::Char(':') => {
                self.list_mode = ListMode::Command;
                self.search_input.clear();
                self.update_list_cmd_completions();
                LeetCodeAction::Redraw
            }
            KeyCode::Char('q') => {
                // 'q' returns to splash screen
                self.view = LeetCodeView::Splash;
                self.update_status_msg();
                LeetCodeAction::Redraw
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.filtered.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.filtered.len() - 1);
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                LeetCodeAction::Redraw
            }
            KeyCode::Char('G') => {
                if !self.filtered.is_empty() {
                    self.cursor = self.filtered.len() - 1;
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Char('g') => {
                self.cursor = 0;
                LeetCodeAction::Redraw
            }
            // Pagination: Ctrl+d = page down, Ctrl+u = page up
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if !self.filtered.is_empty() {
                    self.cursor = (self.cursor + self.page_size).min(self.filtered.len() - 1);
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = self.cursor.saturating_sub(self.page_size);
                LeetCodeAction::Redraw
            }
            KeyCode::PageDown => {
                if !self.filtered.is_empty() {
                    self.cursor = (self.cursor + self.page_size).min(self.filtered.len() - 1);
                }
                LeetCodeAction::Redraw
            }
            KeyCode::PageUp => {
                self.cursor = self.cursor.saturating_sub(self.page_size);
                LeetCodeAction::Redraw
            }
            KeyCode::Enter => {
                self.open_problem();
                LeetCodeAction::Redraw
            }
            KeyCode::Char('r') => {
                self.fetch_problems();
                LeetCodeAction::Redraw
            }
            // Search: / enters search mode
            KeyCode::Char('/') => {
                self.list_mode = ListMode::Search;
                self.search_input.clear();
                LeetCodeAction::Redraw
            }
            // Difficulty filter: 1/2/3
            KeyCode::Char('1') => {
                self.toggle_difficulty_filter(Difficulty::Easy);
                LeetCodeAction::Redraw
            }
            KeyCode::Char('2') => {
                self.toggle_difficulty_filter(Difficulty::Medium);
                LeetCodeAction::Redraw
            }
            KeyCode::Char('3') => {
                self.toggle_difficulty_filter(Difficulty::Hard);
                LeetCodeAction::Redraw
            }
            // Status filter: s cycles through status filters
            KeyCode::Char('s') => {
                self.cycle_status_filter();
                LeetCodeAction::Redraw
            }
            // Clear all filters
            KeyCode::Char('0') => {
                self.filter.difficulty = None;
                self.filter.status = None;
                self.filter.search.clear();
                self.search_input.clear();
                self.apply_filter();
                self.update_status_msg();
                LeetCodeAction::Redraw
            }
            // Knowledge map: m opens the knowledge visualization
            KeyCode::Char('m') => {
                self.knowledge_panel = Some(KnowledgeMapPanel::new());
                self.view = LeetCodeView::KnowledgeMap;
                LeetCodeAction::Redraw
            }
            _ => LeetCodeAction::None,
        }
    }

    /// Handle key events in the knowledge map view.
    fn handle_knowledge_key(&mut self, key: KeyEvent) -> LeetCodeAction {
        use super::knowledge_panel::PanelTab;

        match key.code {
            KeyCode::Char('q') => {
                // Return to problem list
                self.view = LeetCodeView::ProblemList;
                self.knowledge_panel = None;
                self.update_status_msg();
                LeetCodeAction::Redraw
            }
            // Tab switching: 1-4 or Tab key
            KeyCode::Tab => {
                if let Some(ref mut kp) = self.knowledge_panel {
                    kp.selected_tab = match kp.selected_tab {
                        PanelTab::Overview => PanelTab::Topics,
                        PanelTab::Topics => PanelTab::Techniques,
                        PanelTab::Techniques => PanelTab::Problems,
                        PanelTab::Problems => PanelTab::Overview,
                    };
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Char('1') => {
                if let Some(ref mut kp) = self.knowledge_panel {
                    kp.selected_tab = PanelTab::Overview;
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Char('2') => {
                if let Some(ref mut kp) = self.knowledge_panel {
                    kp.selected_tab = PanelTab::Topics;
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Char('3') => {
                if let Some(ref mut kp) = self.knowledge_panel {
                    kp.selected_tab = PanelTab::Techniques;
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Char('4') => {
                if let Some(ref mut kp) = self.knowledge_panel {
                    kp.selected_tab = PanelTab::Problems;
                }
                LeetCodeAction::Redraw
            }
            // Scroll in problems tab
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ref mut kp) = self.knowledge_panel {
                    if kp.selected_tab == PanelTab::Problems {
                        kp.scroll_offset = kp.scroll_offset.saturating_add(1)
                            .min(kp.filtered_ids.len().saturating_sub(1));
                    }
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(ref mut kp) = self.knowledge_panel {
                    if kp.selected_tab == PanelTab::Problems {
                        kp.scroll_offset = kp.scroll_offset.saturating_sub(1);
                    }
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(ref mut kp) = self.knowledge_panel {
                    if kp.selected_tab == PanelTab::Problems {
                        kp.scroll_offset = kp.scroll_offset.saturating_add(20)
                            .min(kp.filtered_ids.len().saturating_sub(1));
                    }
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(ref mut kp) = self.knowledge_panel {
                    if kp.selected_tab == PanelTab::Problems {
                        kp.scroll_offset = kp.scroll_offset.saturating_sub(20);
                    }
                }
                LeetCodeAction::Redraw
            }
            _ => LeetCodeAction::None,
        }
    }

    /// Handle key input while in search mode.
    fn handle_search_input(&mut self, key: KeyEvent) -> LeetCodeAction {
        match key.code {
            KeyCode::Esc => {
                // Cancel search, clear search text
                self.list_mode = ListMode::Normal;
                self.search_input.clear();
                self.filter.search.clear();
                self.apply_filter();
                self.update_status_msg();
                LeetCodeAction::Redraw
            }
            KeyCode::Enter => {
                // Confirm search
                self.list_mode = ListMode::Normal;
                self.filter.search = self.search_input.clone();
                self.apply_filter();
                self.update_status_msg();
                LeetCodeAction::Redraw
            }
            KeyCode::Char(c) => {
                self.search_input.push(c);
                // Live filter as user types
                self.filter.search = self.search_input.clone();
                self.apply_filter();
                LeetCodeAction::Redraw
            }
            KeyCode::Backspace => {
                self.search_input.pop();
                self.filter.search = self.search_input.clone();
                self.apply_filter();
                LeetCodeAction::Redraw
            }
            _ => LeetCodeAction::None,
        }
    }

    /// Handle key input while in command mode (list view).
    fn handle_list_cmd_input(&mut self, key: KeyEvent) -> LeetCodeAction {
        match key.code {
            KeyCode::Esc => {
                self.list_mode = ListMode::Normal;
                self.search_input.clear();
                self.cmd_completions.clear();
                self.update_status_msg();
                LeetCodeAction::Redraw
            }
            KeyCode::Enter => {
                let cmd = self.search_input.trim().to_string();
                self.list_mode = ListMode::Normal;
                self.search_input.clear();
                self.cmd_completions.clear();
                if cmd == "q" || cmd == "quit" {
                    return LeetCodeAction::Close;
                }
                self.status_msg = format!("Unknown command: :{}", cmd);
                LeetCodeAction::Redraw
            }
            KeyCode::Tab => {
                if let Some(first) = self.cmd_completions.first() {
                    self.search_input = first.clone();
                    self.update_list_cmd_completions();
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Backspace => {
                self.search_input.pop();
                self.update_list_cmd_completions();
                LeetCodeAction::Redraw
            }
            KeyCode::Char(c) => {
                self.search_input.push(c);
                self.update_list_cmd_completions();
                LeetCodeAction::Redraw
            }
            _ => LeetCodeAction::None,
        }
    }

/// Update completions for list command mode.
fn update_list_cmd_completions(&mut self) {
    let input = self.search_input.as_str();
    let commands = list_commands();
    if input.is_empty() {
        self.cmd_completions = commands.iter().map(|s| s.to_string()).collect();
    } else {
        self.cmd_completions = commands.iter()
            .filter(|c| c.starts_with(input) && **c != input)
            .map(|s| s.to_string())
            .collect();
    }
}

/// Update completions for coding command mode.
fn update_coding_cmd_completions(&mut self) {
    let input = if let Some(ref coding) = self.coding {
        if let Mode::Command(ref s) = coding.editor.mode {
            s.clone()
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    let commands = coding_commands();
    if input.is_empty() {
        self.cmd_completions = commands.iter().map(|s| s.to_string()).collect();
    } else {
        self.cmd_completions = commands.iter()
            .filter(|c| c.starts_with(input.as_str()) && **c != input.as_str())
            .map(|s| s.to_string())
            .collect();
    }
}

    /// Toggle difficulty filter (press same key again to clear).
    fn toggle_difficulty_filter(&mut self, d: Difficulty) {
        if self.filter.difficulty == Some(d) {
            self.filter.difficulty = None;
        } else {
            self.filter.difficulty = Some(d);
        }
        self.apply_filter();
        self.update_status_msg();
    }

    /// Cycle status filter: None → Solved → Attempted → NotStarted → None.
    fn cycle_status_filter(&mut self) {
        self.filter.status = match self.filter.status {
            None => Some(SolveStatus::Solved),
            Some(SolveStatus::Solved) => Some(SolveStatus::Attempted),
            Some(SolveStatus::Attempted) => Some(SolveStatus::NotStarted),
            Some(SolveStatus::NotStarted) => None,
        };
        self.apply_filter();
        self.update_status_msg();
    }

    /// Update the status message to reflect current filter state.
    fn update_status_msg(&mut self) {
        let mut parts = Vec::new();
        if let Some(d) = self.filter.difficulty {
            parts.push(format!("[{}]", d.label()));
        }
        if let Some(s) = self.filter.status {
            let label = match s {
                SolveStatus::Solved => "Solved",
                SolveStatus::Attempted => "Attempted",
                SolveStatus::NotStarted => "Todo",
            };
            parts.push(format!("[{}]", label));
        }
        if !self.filter.search.is_empty() {
            parts.push(format!("[search: {}]", self.filter.search));
        }
        let filter_info = if parts.is_empty() {
            String::new()
        } else {
            format!(" Filter: {} │", parts.join(" "))
        };
        self.status_msg = format!("{} {} of {} problems", filter_info, self.filtered.len(), self.problems.len());
    }

    /// Handle keys in the coding workspace.
    /// Uses the embedded Editor's full Vim mode system for consistent UX.
    fn handle_coding_key(&mut self, key: KeyEvent) -> LeetCodeAction {
        lc_debug!("handle_coding_key: code={:?} mods={:?}", key.code, key.modifiers);
        let coding = match self.coding.as_mut() {
            Some(c) => c,
            None => {
                lc_debug!("  coding is None! reverting to ProblemList");
                self.view = LeetCodeView::ProblemList;
                return LeetCodeAction::Redraw;
            }
        };

        // Lang switch confirmation — intercepts y/n/Esc
        if coding.lang_confirm_pending {
            return match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    coding.lang_confirm_pending = false;
                    coding.selecting_lang = true;
                    coding.lang_cursor = coding.lang_index;
                    coding.status_msg = String::from("Select language (j/k + Enter)...");
                    LeetCodeAction::Redraw
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    coding.lang_confirm_pending = false;
                    coding.status_msg = String::from("Cancelled.");
                    LeetCodeAction::Redraw
                }
                _ => LeetCodeAction::None,
            };
        }

        // History selection overlay — intercepts all keys
        if coding.selecting_history {
            return match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    let max = coding.submissions.len().saturating_sub(1);
                    coding.history_cursor = (coding.history_cursor + 1).min(max);
                    LeetCodeAction::Redraw
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    coding.history_cursor = coding.history_cursor.saturating_sub(1);
                    LeetCodeAction::Redraw
                }
                KeyCode::Enter => {
                    if let Some(entry) = coding.submissions.get(coding.history_cursor).cloned() {
                        // Restore the submission code into the editor
                        coding.editor.buffer.rope = Rope::from_str(&entry.code);
                        coding.editor.cursor_line = 0;
                        coding.editor.cursor_col = 0;
                        coding.editor.scroll_line = 0;
                        coding.editor.mode = Mode::Normal;
                        // Try to match the language
                        if let Some(idx) = coding.detail.code_snippets.iter()
                            .position(|s| s.lang_slug == entry.lang_slug)
                        {
                            coding.lang_index = idx;
                            let ft = CodingState::filetype_from_lang_slug(&entry.lang_slug);
                            coding.ts_hl.set_filetype(ft);
                        }
                        coding.ts_hl.force_full_parse();
                        coding.ts_hl.incremental_parse(&entry.code);
                        coding.dirty = true;
                        coding.selecting_history = false;
                        coding.status_msg = format!(
                            "Restored: {} │ {} │ {}",
                            entry.status_display, entry.lang, entry.runtime
                        );
                    }
                    LeetCodeAction::Redraw
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    coding.selecting_history = false;
                    coding.status_msg = String::new();
                    LeetCodeAction::Redraw
                }
                _ => LeetCodeAction::None,
            };
        }

        // Language selection overlay — intercepts all keys
        if coding.selecting_lang {
            lc_debug!("  selecting_lang=true, lang_cursor={}", coding.lang_cursor);
            return match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    let max = coding.detail.code_snippets.len().saturating_sub(1);
                    coding.lang_cursor = (coding.lang_cursor + 1).min(max);
                    LeetCodeAction::Redraw
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    coding.lang_cursor = coding.lang_cursor.saturating_sub(1);
                    LeetCodeAction::Redraw
                }
                KeyCode::Enter => {
                    coding.lang_index = coding.lang_cursor;
                    // Load the code template for the selected language
                    if let Some(snippet) = coding.detail.code_snippets.get(coding.lang_index) {
                        coding.editor.buffer.rope = Rope::from_str(&snippet.code);
                        coding.editor.cursor_line = 0;
                        coding.editor.cursor_col = 0;
                        coding.editor.scroll_line = 0;
                        coding.editor.mode = Mode::Normal;
                        // Update syntax highlighter for the new language
                        let ft = CodingState::filetype_from_lang_slug(&snippet.lang_slug);
                        coding.ts_hl.set_filetype(ft);
                        coding.ts_hl.force_full_parse();
                        coding.ts_hl.incremental_parse(&snippet.code);
                    }
                    coding.selecting_lang = false;
                    coding.dirty = true;
                    coding.status_msg = format!(
                        "Lang: {} ✓",
                        coding.detail.code_snippets.get(coding.lang_index)
                            .map(|s| s.lang.as_str()).unwrap_or("?")
                    );
                    LeetCodeAction::Redraw
                }
                KeyCode::Esc => {
                    coding.selecting_lang = false;
                    LeetCodeAction::Redraw
                }
                _ => LeetCodeAction::None,
            };
        }

        // ── Global Ctrl shortcuts (work in any mode/focus) ──
        match key.code {
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // In editor Normal mode, Ctrl-D is page-down; otherwise toggle desc
                if coding.focus != CodingFocus::Editor || !coding.editor.mode.is_normal() {
                    coding.show_description = !coding.show_description;
                    return LeetCodeAction::Redraw;
                }
                // Fall through to editor handling for Ctrl-D page-down
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                coding.show_ai = !coding.show_ai;
                if coding.show_ai {
                    coding.focus = CodingFocus::AiChat;
                } else {
                    coding.focus = CodingFocus::Editor;
                }
                return LeetCodeAction::Redraw;
            }
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                coding.selecting_lang = true;
                coding.lang_cursor = coding.lang_index;
                return LeetCodeAction::Redraw;
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.start_submit();
                return LeetCodeAction::Redraw;
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.run_code();
                return LeetCodeAction::Redraw;
            }
            // Tab cycles focus between panels
            KeyCode::Tab if coding.editor.mode.is_normal() => {
                coding.focus = match coding.focus {
                    CodingFocus::Editor => {
                        if coding.show_description { CodingFocus::Description }
                        else if coding.show_ai { CodingFocus::AiChat }
                        else if coding.show_result_panel { CodingFocus::Result }
                        else { CodingFocus::Editor }
                    }
                    CodingFocus::Description => {
                        if coding.show_ai { CodingFocus::AiChat }
                        else if coding.show_result_panel { CodingFocus::Result }
                        else { CodingFocus::Editor }
                    }
                    CodingFocus::AiChat => {
                        if coding.show_result_panel { CodingFocus::Result }
                        else { CodingFocus::Editor }
                    }
                    CodingFocus::Result => CodingFocus::Editor,
                };
                return LeetCodeAction::Redraw;
            }
            _ => {}
        }

        // ── Focus-specific handling ──
        match coding.focus {
            CodingFocus::Editor => {
                self.handle_editor_key(key)
            }
            CodingFocus::Description => {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if let Some(c) = self.coding.as_mut() {
                            c.desc_scroll += 1;
                        }
                        LeetCodeAction::Redraw
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if let Some(c) = self.coding.as_mut() {
                            c.desc_scroll = c.desc_scroll.saturating_sub(1);
                        }
                        LeetCodeAction::Redraw
                    }
                    _ => LeetCodeAction::None,
                }
            }
            CodingFocus::Result => {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if let Some(c) = self.coding.as_mut() {
                            c.result_scroll += 1;
                        }
                        LeetCodeAction::Redraw
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if let Some(c) = self.coding.as_mut() {
                            c.result_scroll = c.result_scroll.saturating_sub(1);
                        }
                        LeetCodeAction::Redraw
                    }
                    KeyCode::Esc => {
                        if let Some(c) = self.coding.as_mut() {
                            c.focus = CodingFocus::Editor;
                        }
                        LeetCodeAction::Redraw
                    }
                    KeyCode::Char('q') => {
                        if let Some(c) = self.coding.as_mut() {
                            c.show_result_panel = false;
                            c.focus = CodingFocus::Editor;
                        }
                        LeetCodeAction::Redraw
                    }
                    _ => LeetCodeAction::None,
                }
            }
            CodingFocus::AiChat => {
                self.handle_ai_chat_key(key)
            }
        }
    }

    /// Handle key input in the code editor area — delegates to the embedded Editor's
    /// full Vim mode system (Normal/Insert/Visual/Command/Search).
    fn handle_editor_key(&mut self, key: KeyEvent) -> LeetCodeAction {
        let coding = match self.coding.as_mut() {
            Some(c) => c,
            None => return LeetCodeAction::None,
        };

        let mode = coding.editor.mode.clone();
        match mode {
            Mode::Normal => {
                let action = coding.editor.handle_normal_key(key);
                match action {
                    NormalAction::None => LeetCodeAction::Redraw,
                    NormalAction::EnterInsert { col_offset } => {
                        // Enter insert mode (i, a, A, I, s, etc.)
                        // Set mode to Insert BEFORE clamping so cursor can be at line_len
                        coding.editor.mode = Mode::Insert;
                        if col_offset > 0 {
                            let line_len = coding.editor.buffer.line_len(coding.editor.cursor_line);
                            coding.editor.cursor_col = (coding.editor.cursor_col + col_offset as usize).min(line_len);
                        }
                        coding.editor.begin_insert_session(col_offset, false, false);
                        LeetCodeAction::Redraw
                    }
                    NormalAction::EnterInsertNewline { above } => {
                        // o/O — open new line
                        coding.editor.begin_insert_session(0, above, !above);
                        if above {
                            let char_idx = coding.editor.buffer.pos_to_char(coding.editor.cursor_line, 0);
                            let indent = coding.editor.buffer.indent_of_line(coding.editor.cursor_line);
                            coding.editor.buffer.insert_newline(char_idx, "");
                            // Insert indent on the new line (which is current line)
                            if !indent.is_empty() {
                                let new_char_idx = coding.editor.buffer.pos_to_char(coding.editor.cursor_line, 0);
                                coding.editor.buffer.insert_str(new_char_idx, &indent);
                            }
                            coding.editor.cursor_col = coding.editor.buffer.indent_of_line(coding.editor.cursor_line).len();
                        } else {
                            let line_len = coding.editor.buffer.line_len(coding.editor.cursor_line);
                            let char_idx = coding.editor.buffer.pos_to_char(coding.editor.cursor_line, line_len);
                            let indent = coding.editor.buffer.indent_of_line(coding.editor.cursor_line);
                            coding.editor.buffer.insert_newline(char_idx, &indent);
                            coding.editor.cursor_line += 1;
                            coding.editor.cursor_col = indent.len();
                        }
                        coding.editor.mode = Mode::Insert;
                        coding.dirty = true;
                        LeetCodeAction::Redraw
                    }
                    NormalAction::EnterVisual { kind } => {
                        let anchor = coding.editor.cursor_char_idx();
                        coding.editor.mode = Mode::Visual { kind, anchor };
                        LeetCodeAction::Redraw
                    }
                    NormalAction::EnterCommand => {
                        coding.editor.mode = Mode::Command(String::new());
                        self.cmd_completions = coding_commands().iter().map(|s| s.to_string()).collect();
                        LeetCodeAction::Redraw
                    }
                    NormalAction::EnterSearch => {
                        coding.editor.mode = Mode::Search(String::new());
                        LeetCodeAction::Redraw
                    }
                    NormalAction::Quit { .. } => {
                        // q or :q — exit coding view, back to list
                        self.save_current_solution();
                        self.view = LeetCodeView::ProblemList;
                        self.coding = None;
                        self.update_status_msg();
                        LeetCodeAction::Redraw
                    }
                    // ? key: open AI panel and focus it
                    NormalAction::EnterAi => {
                        coding.show_ai = true;
                        coding.focus = CodingFocus::AiChat;
                        LeetCodeAction::Redraw
                    }
                    // Ignore actions not applicable in LeetCode context
                    NormalAction::OpenFileAtCursor
                    | NormalAction::ToggleFileTree
                    | NormalAction::ToggleChatPanel
                    | NormalAction::ToggleTutorial
                    | NormalAction::SwitchFocus
                    | NormalAction::ToggleHiddenFiles => LeetCodeAction::None,
                    NormalAction::ExecuteCommand(cmd) => {
                        self.handle_lc_command(&cmd)
                    }
                    NormalAction::AiAction(_) => LeetCodeAction::None,
                    NormalAction::DotRepeat => {
                        // Replay last action
                        if let Some(ref action) = coding.editor.last_action.clone() {
                            match action {
                                crate::editor::RepeatAction::Insert { enter_col_offset, newline_above, newline_below, text } => {
                                    if *newline_below {
                                        let line_len = coding.editor.buffer.line_len(coding.editor.cursor_line);
                                        let char_idx = coding.editor.buffer.pos_to_char(coding.editor.cursor_line, line_len);
                                        let indent = coding.editor.buffer.indent_of_line(coding.editor.cursor_line);
                                        coding.editor.buffer.insert_newline(char_idx, &indent);
                                        coding.editor.cursor_line += 1;
                                        coding.editor.cursor_col = indent.len();
                                    } else if *newline_above {
                                        let char_idx = coding.editor.buffer.pos_to_char(coding.editor.cursor_line, 0);
                                        coding.editor.buffer.insert_newline(char_idx, "");
                                        coding.editor.cursor_col = 0;
                                    } else {
                                        coding.editor.cursor_col = (coding.editor.cursor_col as i32 + enter_col_offset).max(0) as usize;
                                    }
                                    // Insert the recorded text
                                    for ch in text.chars() {
                                        let char_idx = coding.editor.buffer.pos_to_char(coding.editor.cursor_line, coding.editor.cursor_col);
                                        coding.editor.buffer.insert_char(char_idx, ch);
                                        coding.editor.cursor_col += 1;
                                    }
                                    coding.dirty = true;
                                }
                                _ => {}
                            }
                        }
                        LeetCodeAction::Redraw
                    }
                    NormalAction::PlayMacro(_) => LeetCodeAction::None,
                }
            }
            Mode::Insert => {
                match coding.editor.handle_insert_key(key) {
                    InsertAction::ExitToNormal => {
                        coding.editor.mode = Mode::Normal;
                    }
                    InsertAction::None => {
                        // Text was modified
                        coding.dirty = true;
                    }
                }
                coding.editor.scroll_to_cursor();
                LeetCodeAction::Redraw
            }
            Mode::Visual { .. } => {
                // Handle visual mode keys through normal handler
                // (visual mode uses the same handler in our editor)
                let action = coding.editor.handle_normal_key(key);
                match action {
                    NormalAction::None => {}
                    _ => {
                        // Most visual actions result in mode change or edit
                        coding.dirty = true;
                    }
                }
                LeetCodeAction::Redraw
            }
            Mode::Command(ref _cmd) => {
                // Command mode: handle typing and execution with tab completion
                match key.code {
                    KeyCode::Esc => {
                        coding.editor.mode = Mode::Normal;
                        self.cmd_completions.clear();
                    }
                    KeyCode::Enter => {
                        let cmd = if let Mode::Command(ref s) = coding.editor.mode {
                            s.clone()
                        } else {
                            String::new()
                        };
                        coding.editor.mode = Mode::Normal;
                        self.cmd_completions.clear();
                        return self.handle_lc_command(&cmd);
                    }
                    KeyCode::Tab => {
                        // Accept first completion
                        if let Some(first) = self.cmd_completions.first().cloned() {
                            if let Mode::Command(ref mut s) = coding.editor.mode {
                                *s = first;
                            }
                        }
                        self.update_cmd_completions(coding_commands());
                    }
                    KeyCode::Backspace => {
                        if let Mode::Command(ref mut s) = coding.editor.mode {
                            if s.is_empty() {
                                coding.editor.mode = Mode::Normal;
                                self.cmd_completions.clear();
                            } else {
                                s.pop();
                            }
                        }
                        self.update_coding_cmd_completions();
                    }
                    KeyCode::Char(c) => {
                        if let Mode::Command(ref mut s) = coding.editor.mode {
                            s.push(c);
                        }
                        self.update_coding_cmd_completions();
                    }
                    _ => {}
                }
                LeetCodeAction::Redraw
            }
            Mode::Search(ref _pat) => {
                // Search mode: handle typing
                match key.code {
                    KeyCode::Esc => {
                        coding.editor.mode = Mode::Normal;
                    }
                    KeyCode::Enter => {
                        if let Mode::Search(ref s) = coding.editor.mode {
                            let pat = s.clone();
                            let ic = coding.editor.config.general.ignore_case;
                            coding.editor.mode = Mode::Normal;
                            coding.editor.run_search(&pat, ic);
                        } else {
                            coding.editor.mode = Mode::Normal;
                        }
                    }
                    KeyCode::Backspace => {
                        if let Mode::Search(ref mut s) = coding.editor.mode {
                            if s.is_empty() {
                                coding.editor.mode = Mode::Normal;
                            } else {
                                s.pop();
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        if let Mode::Search(ref mut s) = coding.editor.mode {
                            s.push(c);
                        }
                    }
                    _ => {}
                }
                LeetCodeAction::Redraw
            }
            Mode::Ai(_) => {
                // AI mode not used in LeetCode context
                coding.editor.mode = Mode::Normal;
                LeetCodeAction::Redraw
            }
        }
    }

/// Handle a :command in the LeetCode coding view.
fn handle_lc_command(&mut self, cmd: &str) -> LeetCodeAction {
    let trimmed = cmd.trim();
    // ── :color <name> ──────────────────────────────────────────────
    if let Some(rest) = trimmed.strip_prefix("color ") {
        let name = rest.trim();
        if let Some(color) = SplashColor::from_name(name) {
            self.splash_color = color;
            self.save_splash_color();
            let msg = format!("Color: {} ✓", color.name());
            if let Some(ref mut coding) = self.coding {
                coding.status_msg = msg.clone();
            } else {
                self.status_msg = msg;
            }
        } else {
            let available: Vec<&str> = SplashColor::all()
                .iter()
                .map(|c| c.name())
                .collect();
            let msg = format!("Unknown color '{}'. Available: {}", name, available.join(", "));
            if let Some(ref mut coding) = self.coding {
                coding.status_msg = msg.clone();
            } else {
                self.status_msg = msg;
            }
        }
        return LeetCodeAction::Redraw;
    }
    if trimmed == "color" {
        let available: Vec<&str> = SplashColor::all().iter().map(|c| c.name()).collect();
        let msg = format!("Colors: {}  (current: {})", available.join(", "), self.splash_color.name());
        if let Some(ref mut coding) = self.coding {
            coding.status_msg = msg.clone();
        } else {
            self.status_msg = msg;
        }
        return LeetCodeAction::Redraw;
    }
    // ── :theme <name> ──────────────────────────────────────────────
    if let Some(rest) = trimmed.strip_prefix("theme ") {
        let name = rest.trim();
        if let Some(palette) = CodePalette::by_name(name) {
            if let Some(ref mut coding) = self.coding {
                coding.ts_hl.set_palette(palette);
                coding.theme_name = name.to_string();
                let code = coding.editor.buffer.rope.to_string();
                coding.ts_hl.force_full_parse();
                coding.ts_hl.incremental_parse(&code);
                coding.status_msg = format!("Theme: {} ✓", name);
            }
        } else {
            let available = CodePalette::available_themes().join(", ");
            if let Some(ref mut coding) = self.coding {
                coding.status_msg = format!("Unknown theme '{}'. Available: {}", name, available);
            }
        }
        return LeetCodeAction::Redraw;
    }
    if trimmed == "theme" {
        let available = CodePalette::available_themes().join(", ");
        let current = self.coding.as_ref().map(|c| c.theme_name.as_str()).unwrap_or("catppuccin");
        let msg = format!("Themes: {}  (current: {})", available, current);
        if let Some(ref mut coding) = self.coding {
            coding.status_msg = msg;
        }
        return LeetCodeAction::Redraw;
    }

    match trimmed {
        "q" | "quit" => {
            self.save_current_solution();
            self.view = LeetCodeView::ProblemList;
            self.coding = None;
            self.update_status_msg();
            LeetCodeAction::Redraw
        }
        "w" | "write" => {
            self.save_current_solution();
            if let Some(ref mut coding) = self.coding {
                coding.status_msg = String::from("Solution saved.");
            }
            LeetCodeAction::Redraw
        }
        "wq" => {
            self.save_current_solution();
            self.view = LeetCodeView::ProblemList;
            self.coding = None;
            self.update_status_msg();
            LeetCodeAction::Redraw
        }
        "run" => {
            self.run_code();
            LeetCodeAction::Redraw
        }
        "submit" | "sub" => {
            self.start_submit();
            LeetCodeAction::Redraw
        }
        "lang" => {
            if let Some(ref mut coding) = self.coding {
                if coding.dirty {
                    // Code has been modified — ask for confirmation
                    coding.lang_confirm_pending = true;
                    coding.status_msg = String::from("Buffer modified! Switch lang will discard changes. [y]es / [n]o");
                } else {
                    coding.selecting_lang = true;
                    coding.lang_cursor = coding.lang_index;
                }
            }
            LeetCodeAction::Redraw
        }
        "desc" => {
            if let Some(ref mut coding) = self.coding {
                coding.show_description = !coding.show_description;
            }
            LeetCodeAction::Redraw
        }
        "ai" => {
            if let Some(ref mut coding) = self.coding {
                coding.show_ai = !coding.show_ai;
                if coding.show_ai {
                    coding.focus = CodingFocus::AiChat;
                } else {
                    coding.focus = CodingFocus::Editor;
                }
            }
            LeetCodeAction::Redraw
        }
        "lang!" => {
            // Force lang switch without confirmation
            if let Some(ref mut coding) = self.coding {
                coding.selecting_lang = true;
                coding.lang_cursor = coding.lang_index;
                coding.lang_confirm_pending = false;
            }
            LeetCodeAction::Redraw
        }
        "result" | "res" => {
            if let Some(ref mut coding) = self.coding {
                if coding.result.is_some() {
                    coding.show_result_panel = !coding.show_result_panel;
                    coding.result_scroll = 0;
                } else {
                    coding.status_msg = String::from("No result yet. Use :run or :submit first.");
                }
            }
            LeetCodeAction::Redraw
        }
        "history" | "hist" => {
            // Fetch submission history and open selection overlay
            let slug = self.coding.as_ref()
                .map(|c| c.detail.summary.title_slug.clone());
            if let Some(slug) = slug {
                if let Some(ref mut coding) = self.coding {
                    coding.status_msg = String::from("Fetching history...");
                }
                let session = auth::load_session();
                let mut client = LeetCodeClient::new(self.site);
                if let Some(s) = session {
                    client = client.with_session(s);
                }
                match client.fetch_submissions(&slug, 20) {
                    Ok(subs) => {
                        if let Some(ref mut coding) = self.coding {
                            if subs.is_empty() {
                                coding.status_msg = String::from("No submission history found.");
                            } else {
                                coding.submissions = subs;
                                coding.selecting_history = true;
                                coding.history_cursor = 0;
                                coding.history_scroll = 0;
                                coding.status_msg = String::from("Select a submission (j/k + Enter)...");
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(ref mut coding) = self.coding {
                            coding.status_msg = format!("Failed to fetch history: {}", e);
                        }
                    }
                }
            }
            LeetCodeAction::Redraw
        }
        _ => {
            if let Some(ref mut coding) = self.coding {
                coding.status_msg = format!("Unknown command: :{}", trimmed);
            }
            LeetCodeAction::Redraw
        }
    }
}

    /// Handle key input in the AI chat panel.
    fn handle_ai_chat_key(&mut self, key: KeyEvent) -> LeetCodeAction {
        let coding = match self.coding.as_mut() {
            Some(c) => c,
            None => return LeetCodeAction::None,
        };

        match key.code {
            KeyCode::Esc => {
                coding.focus = CodingFocus::Editor;
                LeetCodeAction::Redraw
            }
            KeyCode::Enter => {
                if coding.ai_input.trim().is_empty() {
                    return LeetCodeAction::None;
                }
                let query = coding.ai_input.clone();
                coding.ai_input.clear();
                coding.ai_input_cursor = 0;
                coding.ai_chat.push_user(&query);
                // Send to AI
                let _ = coding; // release borrow
                self.send_ai_message();
                LeetCodeAction::Redraw
            }
            KeyCode::Char(c) => {
                // ai_input_cursor is a char index; convert to byte offset for insert
                let byte_pos = coding.ai_input.char_indices()
                    .nth(coding.ai_input_cursor)
                    .map(|(i, _)| i)
                    .unwrap_or(coding.ai_input.len());
                coding.ai_input.insert(byte_pos, c);
                coding.ai_input_cursor += 1;
                LeetCodeAction::Redraw
            }
            KeyCode::Backspace => {
                if coding.ai_input_cursor > 0 {
                    coding.ai_input_cursor -= 1;
                    // Convert char index to byte offset for remove
                    let byte_pos = coding.ai_input.char_indices()
                        .nth(coding.ai_input_cursor)
                        .map(|(i, _)| i)
                        .unwrap_or(coding.ai_input.len());
                    coding.ai_input.remove(byte_pos);
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Left => {
                coding.ai_input_cursor = coding.ai_input_cursor.saturating_sub(1);
                LeetCodeAction::Redraw
            }
            KeyCode::Right => {
                if coding.ai_input_cursor < coding.ai_input.chars().count() {
                    coding.ai_input_cursor += 1;
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Up => {
                coding.ai_chat.scroll_up(1);
                LeetCodeAction::Redraw
            }
            KeyCode::Down => {
                coding.ai_chat.scroll_down(1);
                LeetCodeAction::Redraw
            }
            _ => LeetCodeAction::None,
        }
    }

    /// Send the user's message to the AI with full LeetCode context.
    fn send_ai_message(&mut self) {
        let coding = match self.coding.as_mut() {
            Some(c) => c,
            None => return,
        };

        if coding.ai_pending {
            coding.ai_chat.push_system("AI is still processing...");
            return;
        }

        // Build context: code + problem + knowledge + error
        let user_code = coding.code();
        let code_changed = coding.ai_last_code != user_code;
        coding.ai_last_code = user_code.clone();
        let problem_desc = coding.detail.content_text.clone();
        let problem_title = format!("#{} {}", coding.detail.summary.frontend_id, coding.detail.summary.title);
        let _problem_slug = coding.detail.summary.title_slug.clone();

        // Gather knowledge map context if available
        let knowledge_ctx = {
            use super::ai_context::LeetCodeAiContext;
            if let Some(ctx) = LeetCodeAiContext::from_problem(
                coding.detail.summary.frontend_id,
                &user_code,
                super::ai_context::LeetCodeAiMode::Debug,
            ) {
                format!(
                    "\nKnowledge Map:\n- Topics: {}\n- Techniques: {}\n- Key Insight: {}\n- Approach: {}\n- Complexity: Time {} | Space {}\n- Reference:\n```python\n{}\n```",
                    ctx.topics.join(", "),
                    ctx.techniques.join(", "),
                    ctx.key_insight,
                    ctx.approach,
                    ctx.time_complexity,
                    ctx.space_complexity,
                    ctx.reference_code,
                )
            } else {
                String::new()
            }
        };

        // Gather recent error info
        let error_ctx = if let Some(ref res) = coding.result {
            let mut parts = Vec::new();
            if !res.compile_error.is_empty() {
                parts.push(format!("Compile Error: {}", if !res.full_compile_error.is_empty() { &res.full_compile_error } else { &res.compile_error }));
            }
            if !res.runtime_error.is_empty() {
                parts.push(format!("Runtime Error: {}", if !res.full_runtime_error.is_empty() { &res.full_runtime_error } else { &res.runtime_error }));
            }
            if !res.last_testcase.is_empty() {
                parts.push(format!("Failed testcase: {} | Expected: {} | Got: {}", res.last_testcase, res.expected_output, res.code_output));
            }
            if parts.is_empty() {
                String::new()
            } else {
                format!("\nLatest Error:\n{}", parts.join("\n"))
            }
        } else {
            String::new()
        };

        // Build system prompt
        let code_note = if code_changed { " [CODE UPDATED since last message]" } else { "" };
        let system_prompt = format!(
            "You are an AI coding assistant helping with LeetCode problems. \
             Be concise and helpful. Respond in the same language as the user's question. \
             Use the context below.\n\n\
             Problem: {}\n\n\
             Description:\n{}\n\n\
             User's Current Code ({}){}:\n```\n{}\n```{}{}\n",
            problem_title,
            problem_desc,
            coding.detail.code_snippets.get(coding.lang_index).map(|s| s.lang.as_str()).unwrap_or("?"),
            code_note,
            user_code,
            knowledge_ctx,
            error_ctx,
        );

        // Gather conversation history
        let history = coding.ai_chat.recent_history(5);
        let user_query = coding.ai_chat.messages.last()
            .filter(|m| m.role == ChatRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // Build messages
        let mut messages: Vec<Message> = Vec::new();
        messages.push(Message { role: "system".into(), content: system_prompt });
        for (user_msg, asst_msg) in &history {
            // Skip the last user message (we'll add it separately)
            if *user_msg == user_query.as_str() { continue; }
            messages.push(Message { role: "user".into(), content: user_msg.to_string() });
            messages.push(Message { role: "assistant".into(), content: asst_msg.to_string() });
        }
        messages.push(Message { role: "user".into(), content: user_query.clone() });

        // Load AI config and spawn thread
        let cfg = crate::config::loader::load().ai;
        let result_arc = std::sync::Arc::clone(&coding.ai_result);
        coding.ai_pending = true;
        coding.status_msg = String::from("AI thinking...");

        std::thread::spawn(move || {
            let client = AiClient::new(&cfg);
            let res = client.chat(messages);
            let response = match res {
                Ok(text) => text,
                Err(e) => format!("Error: {}", e),
            };
            *result_arc.lock().unwrap() = Some(response);
        });
    }

    /// Poll for AI response (called from the main event loop tick).
    pub fn poll_ai_response(&mut self) {
        let coding = match self.coding.as_mut() {
            Some(c) => c,
            None => return,
        };

        if !coding.ai_pending {
            return;
        }

        let response = {
            let mut guard = coding.ai_result.lock().unwrap();
            guard.take()
        };

        if let Some(text) = response {
            coding.ai_pending = false;
            if text.starts_with("Error: ") {
                coding.ai_chat.push_system(&text);
                coding.status_msg = String::from("AI error. See chat panel.");
            } else {
                coding.ai_chat.push_assistant(&text);
                coding.status_msg = String::from("AI responded.");
            }
        }
    }

    fn handle_login_key(&mut self, key: KeyEvent) -> LeetCodeAction {
        match key.code {
            KeyCode::Esc => {
                // Reset login state on cancel
                self.login_input.clear();
                self.login_session_value.clear();
                self.login_step = LoginStep::Session;
                LeetCodeAction::Close
            }
            KeyCode::Enter => {
                let input = self.login_input.trim().to_string();
                if input.is_empty() {
                    return LeetCodeAction::None;
                }
                match self.login_step {
                    LoginStep::Session => {
                        // Strip "LEETCODE_SESSION=" prefix if user pasted it
                        let value = input.strip_prefix("LEETCODE_SESSION=")
                            .unwrap_or(&input)
                            .to_string();
                        self.login_session_value = value;
                        self.login_input.clear();
                        self.login_step = LoginStep::CsrfToken;
                    }
                    LoginStep::CsrfToken => {
                        // Strip "csrftoken=" prefix if user pasted it
                        let csrf = input.strip_prefix("csrftoken=")
                            .unwrap_or(&input)
                            .to_string();
                        // Build session from the two values
                        let session = auth::Session {
                            session_cookie: self.login_session_value.clone(),
                            csrf_token: csrf,
                            username: "user".to_string(),
                            site: "cn".to_string(),
                        };
                        let _ = auth::save_session(&session);
                        self.logged_in = true;
                        self.site = Site::CN;
                        self.login_input.clear();
                        self.login_session_value.clear();
                        self.login_step = LoginStep::Session;
                        self.status_msg = String::from("Login successful! Loading problems...");
                        self.fetch_problems();
                        self.view = LeetCodeView::ProblemList;
                    }
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Char(c) => {
                self.login_input.push(c);
                LeetCodeAction::Redraw
            }
            KeyCode::Backspace => {
                self.login_input.pop();
                LeetCodeAction::Redraw
            }
            _ => LeetCodeAction::None,
        }
    }

    /// Fetch problems from the API (blocking for now — will be async later).
    fn fetch_problems(&mut self) {
        let session = auth::load_session();
        let mut client = LeetCodeClient::new(self.site);
        if let Some(s) = session {
            client = client.with_session(s);
        }

        match client.fetch_problem_list(0, 100) {
            Ok(problems) => {
                let _ = cache::save_problem_list(&problems);
                self.problems = problems;
                self.apply_filter();
                self.status_msg = format!("{} problems loaded", self.problems.len());
            }
            Err(e) => {
                self.status_msg = format!("Error: {}", e);
            }
        }
    }

    /// Save the current solution to local cache.
    pub fn save_current_solution(&mut self) {
        if let Some(ref mut coding) = self.coding {
            if !coding.dirty {
                return;
            }
            let slug = &coding.detail.summary.title_slug;
            let lang_slug = coding.detail.code_snippets.get(coding.lang_index)
                .map(|s| s.lang_slug.clone())
                .unwrap_or_default();
            let solution = cache::SavedSolution {
                lang_slug,
                lang_index: coding.lang_index,
                code: coding.code(),
                cursor_line: coding.editor.cursor_line,
                cursor_col: coding.editor.cursor_col,
            };
            let _ = cache::save_solution(slug, &solution);
            coding.dirty = false;
            coding.last_save = std::time::Instant::now();
        }
    }

    /// Auto-save if dirty and enough time has passed (every 5 seconds).
    pub fn auto_save(&mut self) {
        let should_save = self.coding.as_ref()
            .map(|c| c.dirty && c.last_save.elapsed() >= std::time::Duration::from_secs(5))
            .unwrap_or(false);
        if should_save {
            self.save_current_solution();
        }
    }

    /// Open the selected problem into the coding workspace.
    fn open_problem(&mut self) {
        lc_debug!("open_problem: cursor={}, filtered_len={}", self.cursor, self.filtered.len());
        if let Some(&idx) = self.filtered.get(self.cursor) {
            let slug = self.problems[idx].title_slug.clone();
            lc_debug!("  fetching detail for slug={}", slug);
            let session = auth::load_session();
            let mut client = LeetCodeClient::new(self.site);
            if let Some(s) = session {
                client = client.with_session(s);
            }

            match client.fetch_problem_detail(&slug) {
                Ok(detail) => {
                    lc_debug!("  detail OK: snippets={}, content_len={}", detail.code_snippets.len(), detail.content_text.len());

                    // Try to restore saved solution
                    let saved = cache::load_solution(&slug);
                    let (lang_index, code, cursor_line, cursor_col, selecting_lang) = if let Some(ref sol) = saved {
                        // Find the matching language index
                        let idx = detail.code_snippets.iter()
                            .position(|s| s.lang_slug == sol.lang_slug)
                            .unwrap_or(sol.lang_index.min(detail.code_snippets.len().saturating_sub(1)));
                        lc_debug!("  restored saved solution: lang={}, code_len={}", sol.lang_slug, sol.code.len());
                        (idx, sol.code.clone(), sol.cursor_line, sol.cursor_col, false)
                    } else {
                        // No saved solution — start with first language, show lang picker
                        let has_snippets = !detail.code_snippets.is_empty();
                        let initial_code = detail.code_snippets.first()
                            .map(|s| s.code.clone())
                            .unwrap_or_default();
                        (0, initial_code, 0, 0, has_snippets)
                    };

                    let status_msg = if selecting_lang {
                        String::from("Select language (j/k + Enter)...")
                    } else {
                        format!(
                            "Lang: {}",
                            detail.code_snippets.get(lang_index)
                                .map(|s| s.lang.as_str()).unwrap_or("?")
                        )
                    };

                    // Create an embedded Editor with the code loaded into its buffer
                    let mut lc_editor = Editor::new(Config::default(), 80, 24);
                    lc_editor.buffer.rope = Rope::from_str(&code);
                    lc_editor.cursor_line = cursor_line;
                    lc_editor.cursor_col = cursor_col;
                    lc_editor.clamp_cursor();
                    // Start in Normal mode (Vim-style)
                    lc_editor.mode = Mode::Normal;

                    // Create syntax highlighter for the selected language
                    let lang_slug = detail.code_snippets.get(lang_index)
                        .map(|s| s.lang_slug.as_str())
                        .unwrap_or("");
                    let ft = CodingState::filetype_from_lang_slug(lang_slug);
let palette = CodePalette::by_name("catppuccin")
                .unwrap_or_else(CodePalette::catppuccin_mocha);
                    let mut ts_hl = TsHighlighter::new(ft, palette);
                    // Do initial full parse so highlighting is ready
                    ts_hl.force_full_parse();
                    ts_hl.incremental_parse(&code);

                    self.coding = Some(CodingState {
                        detail,
                        lang_index,
                        editor: lc_editor,
                        ts_hl,
                        show_description: true,
                        desc_scroll: 0,
                        show_ai: false,
                        focus: CodingFocus::Editor,
                        result: None,
                        show_result_panel: false,
                        result_scroll: 0,
                        error_line: 0,
                        status_msg,
                        selecting_lang,
                        lang_cursor: lang_index,
                        last_save: std::time::Instant::now(),
                        dirty: false,
                        theme_name: String::from("catppuccin"),
                        ai_chat: ChatPanel::new(40, 50),
                        ai_input: String::new(),
                        ai_input_cursor: 0,
                        ai_pending: false,
                        ai_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
                        ai_last_code: String::new(),
                        lang_confirm_pending: false,
                        submissions: Vec::new(),
                        selecting_history: false,
                        history_cursor: 0,
                        history_scroll: 0,
                        submit_pending: false,
                        submit_tick: 0,
                    });
                    self.view = LeetCodeView::Coding;
                    lc_debug!("  -> view set to Coding, selecting_lang={}", selecting_lang);
                }
                Err(e) => {
                    lc_debug!("  detail ERROR: {}", e);
                    self.status_msg = format!("Error loading problem: {}", e);
                }
            }
        }
    }

    /// Initiate a submit — sets the pending flag and shows immediate feedback.
    /// The actual network call is deferred to `poll_submit()` on the next tick.
    fn start_submit(&mut self) {
        if let Some(ref mut coding) = self.coding {
            if coding.submit_pending {
                return; // already pending
            }
            coding.submit_pending = true;
            coding.submit_tick = 0;
            coding.status_msg = String::from("⟳ Submitting...");
        }
    }

    /// Called every tick (~100ms) from the event loop. If a submit is pending,
    /// executes the actual blocking submit. Returns true if a redraw is needed.
    pub fn poll_submit(&mut self) -> bool {
        let pending = self.coding.as_ref().map_or(false, |c| c.submit_pending);
        if !pending {
            return false;
        }
        // On the first tick, just show the animation (the UI has already been
        // redrawn with "Submitting..." from start_submit). On the second tick,
        // actually perform the blocking submit.
        let tick = self.coding.as_ref().unwrap().submit_tick;
        if tick == 0 {
            // First tick: advance tick counter, let the UI render the message
            if let Some(ref mut coding) = self.coding {
                coding.submit_tick = 1;
            }
            return true; // redraw with the spinner
        }
        // Second tick onwards: execute the blocking submit
        self.submit_code();
        if let Some(ref mut coding) = self.coding {
            coding.submit_pending = false;
            coding.submit_tick = 0;
        }
        true
    }

    /// Submit the current code to LeetCode.
    fn submit_code(&mut self) {
        let (slug, question_id, code, lang_slug) = {
            let coding = match self.coding.as_ref() {
                Some(c) => c,
                None => return,
            };
            let lang_slug = coding.detail.code_snippets.get(coding.lang_index)
                .map(|s| s.lang_slug.clone())
                .unwrap_or_default();
            (coding.detail.summary.title_slug.clone(), coding.detail.summary.frontend_id, coding.code(), lang_slug)
        };

        let session = auth::load_session();
        let mut client = LeetCodeClient::new(self.site);
        if let Some(s) = session {
            client = client.with_session(s);
        }

        match client.submit_code(&slug, question_id, &code, &lang_slug) {
            Ok(submission_id) => {
                // Poll for result with retry loop
                if let Some(coding) = self.coding.as_mut() {
                    coding.status_msg = String::from("Submitted! Checking result...");
                }
                // Wait a bit before first check
                std::thread::sleep(std::time::Duration::from_secs(2));

                let mut final_result: Option<SubmissionResult> = None;
                let mut last_err: Option<String> = None;
                for attempt in 0..15 {
                    match client.check_submission(&submission_id) {
                        Ok(result) => {
                            if result.state == "PENDING" || result.state == "STARTED" {
                                // Still processing, update status and retry
                                if let Some(coding) = self.coding.as_mut() {
                                    coding.status_msg = format!("Judging... (attempt {}/15)", attempt + 1);
                                }
                                std::thread::sleep(std::time::Duration::from_secs(1));
                                continue;
                            }
                            // Got final result
                            final_result = Some(result);
                            break;
                        }
                        Err(e) => {
                            last_err = Some(format!("{}", e));
                            // Network error, retry a few times
                            if attempt < 5 {
                                std::thread::sleep(std::time::Duration::from_secs(1));
                                continue;
                            }
                            break;
                        }
                    }
                }

                if let Some(result) = final_result {
                    if let Some(coding) = self.coding.as_mut() {
                        let msg = if result.status_msg == "Accepted" {
                            coding.error_line = 0;
                            coding.show_result_panel = false;
                            format!("✓ Accepted │ {} │ {} │ {}/{} tests",
                                result.runtime, result.memory,
                                result.total_correct, result.total_testcases)
                        } else {
                            coding.error_line = result.error_line;
                            coding.show_result_panel = true;
                            coding.result_scroll = 0;
                            format!("✗ {} │ {}/{} tests │ :result to view details",
                                result.status_msg,
                                result.total_correct, result.total_testcases)
                        };
                        coding.status_msg = msg;
                        coding.result = Some(result);
                    }
                } else {
                    if let Some(coding) = self.coding.as_mut() {
                        let err_msg = last_err.unwrap_or_else(|| "Timed out waiting for result".to_string());
                        coding.status_msg = format!("Check failed: {}", err_msg);
                        coding.show_result_panel = false;
                    }
                }
            }
            Err(e) => {
                if let Some(coding) = self.coding.as_mut() {
                    let err_msg = format!("{}", e);
                    // Create a synthetic result to display in the panel
                    coding.result = Some(SubmissionResult {
                        state: String::from("ERROR"),
                        status_msg: String::from("Submit Failed"),
                        compile_error: String::new(),
                        runtime_error: String::new(),
                        full_compile_error: err_msg.clone(),
                        full_runtime_error: String::new(),
                        last_testcase: String::new(),
                        expected_output: String::new(),
                        code_output: String::new(),
                        std_output: String::new(),
                        error_line: 0,
                        runtime: String::new(),
                        memory: String::new(),
                        total_correct: 0,
                        total_testcases: 0,
                    });
                    coding.show_result_panel = true;
                    coding.result_scroll = 0;
                    coding.error_line = 0;
                    coding.status_msg = format!("✗ Submit failed │ :result to view details");
                }
            }
        }
    }

    /// Run code against test cases.
    fn run_code(&mut self) {
        let (slug, question_id, code, lang_slug, test_input) = {
            let coding = match self.coding.as_ref() {
                Some(c) => c,
                None => return,
            };
            let lang_slug = coding.detail.code_snippets.get(coding.lang_index)
                .map(|s| s.lang_slug.clone())
                .unwrap_or_default();
            (
                coding.detail.summary.title_slug.clone(),
                coding.detail.summary.frontend_id,
                coding.code(),
                lang_slug,
                coding.detail.test_cases.clone(),
            )
        };

        let session = auth::load_session();
        let mut client = LeetCodeClient::new(self.site);
        if let Some(s) = session {
            client = client.with_session(s);
        }

        match client.run_code(&slug, question_id, &code, &lang_slug, &test_input) {
            Ok(interpret_id) => {
                if let Some(coding) = self.coding.as_mut() {
                    coding.status_msg = String::from("Running tests...");
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
                match client.check_submission(&interpret_id) {
                    Ok(result) => {
                        if let Some(coding) = self.coding.as_mut() {
                            let has_error = !result.compile_error.is_empty() || !result.runtime_error.is_empty();
                            let msg = if !has_error && result.status_msg == "Accepted" {
                                coding.error_line = 0;
                                coding.show_result_panel = false;
                                format!("✓ Run OK │ {}/{} passed", result.total_correct, result.total_testcases)
                            } else if !result.compile_error.is_empty() {
                                coding.error_line = result.error_line;
                                coding.show_result_panel = true;
                                coding.result_scroll = 0;
                                format!("✗ Compile Error │ :result to view details")
                            } else if !result.runtime_error.is_empty() {
                                coding.error_line = result.error_line;
                                coding.show_result_panel = true;
                                coding.result_scroll = 0;
                                format!("✗ Runtime Error │ :result to view details")
                            } else {
                                coding.error_line = 0;
                                coding.show_result_panel = true;
                                coding.result_scroll = 0;
                                format!("✗ {} │ {}/{} passed │ :result to view details",
                                    result.status_msg, result.total_correct, result.total_testcases)
                            };
                            coding.status_msg = msg;
                            coding.result = Some(result);
                        }
                    }
                    Err(e) => {
                        if let Some(coding) = self.coding.as_mut() {
                            let err_msg = format!("{}", e);
                            coding.result = Some(SubmissionResult {
                                state: String::from("ERROR"),
                                status_msg: String::from("Check Failed"),
                                compile_error: String::new(),
                                runtime_error: String::new(),
                                full_compile_error: err_msg.clone(),
                                full_runtime_error: String::new(),
                                last_testcase: String::new(),
                                expected_output: String::new(),
                                code_output: String::new(),
                                std_output: String::new(),
                                error_line: 0,
                                runtime: String::new(),
                                memory: String::new(),
                                total_correct: 0,
                                total_testcases: 0,
                            });
                            coding.show_result_panel = true;
                            coding.result_scroll = 0;
                            coding.status_msg = format!("✗ Check failed │ :result to view details");
                        }
                    }
                }
            }
            Err(e) => {
                if let Some(coding) = self.coding.as_mut() {
                    let err_msg = format!("{}", e);
                    coding.result = Some(SubmissionResult {
                        state: String::from("ERROR"),
                        status_msg: String::from("Run Failed"),
                        compile_error: String::new(),
                        runtime_error: String::new(),
                        full_compile_error: err_msg.clone(),
                        full_runtime_error: String::new(),
                        last_testcase: String::new(),
                        expected_output: String::new(),
                        code_output: String::new(),
                        std_output: String::new(),
                        error_line: 0,
                        runtime: String::new(),
                        memory: String::new(),
                        total_correct: 0,
                        total_testcases: 0,
                    });
                    coding.show_result_panel = true;
                    coding.result_scroll = 0;
                    coding.error_line = 0;
                    coding.status_msg = format!("✗ Run failed │ :result to view details");
                }
            }
        }
    }

    /// Apply the current filter to the problem list.
    fn apply_filter(&mut self) {
        self.filtered = self.problems
            .iter()
            .enumerate()
            .filter(|(_, p)| self.filter.matches(p))
            .map(|(i, _)| i)
            .collect();
        self.cursor = 0;
        self.scroll_offset = 0;
    }

    // ── Rendering helpers (used by the renderer) ──────────────────────────────

    /// Get the splash lines with their color zone tags.
    pub fn splash_lines(&self) -> &'static [(&'static str, char)] {
        SPLASH_LINES
    }

    /// No-op (surf animation removed). Kept for API compat.
    pub fn tick_surf(&mut self) {}

    /// Load the saved splash color from `~/.hi/leetcode/splash_color`.
    fn load_splash_color() -> SplashColor {
        let path = dirs::home_dir()
            .map(|h| h.join(".hi").join("leetcode").join("splash_color"));
        path.as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| SplashColor::from_name(s.trim()))
            .unwrap_or(SplashColor::Green)
    }

    /// Persist the current splash color to `~/.hi/leetcode/splash_color`.
    fn save_splash_color(&self) {
        let path = dirs::home_dir()
            .map(|h| h.join(".hi").join("leetcode").join("splash_color"));
        if let Some(ref p) = path {
            let _ = std::fs::create_dir_all(p.parent().unwrap());
            let _ = std::fs::write(p, self.splash_color.name());
        }
    }

    /// Get visible problem rows for the list view.
    pub fn visible_problems(&self, height: usize) -> Vec<&ProblemSummary> {
        // Adjust scroll to keep cursor visible
        // header(1) + col_header(1) + separator(1) + footer_sep(1) + footer(1) = 5
        let visible_height = height.saturating_sub(5);
        let start = if self.cursor >= self.scroll_offset + visible_height {
            self.cursor - visible_height + 1
        } else if self.cursor < self.scroll_offset {
            self.cursor
        } else {
            self.scroll_offset
        };

        self.filtered[start..]
            .iter()
            .take(visible_height)
            .filter_map(|&i| self.problems.get(i))
            .collect()
    }

    /// Get the currently selected problem index within the visible window.
    pub fn cursor_in_view(&self, height: usize) -> usize {
        let visible_height = height.saturating_sub(5);
        let start = if self.cursor >= self.scroll_offset + visible_height {
            self.cursor - visible_height + 1
        } else if self.cursor < self.scroll_offset {
            self.cursor
        } else {
            self.scroll_offset
        };
        self.cursor - start
    }
}
