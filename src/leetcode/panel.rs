//! LeetCode "古法时代" TUI Panel — retro phosphor-green terminal aesthetic.
//!
//! Design: DOS-era double-line borders (╔═╗║╚╝), amber/green phosphor colors,
//! ASCII art logo, scanline-style separators.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::models::*;
use super::api::{LeetCodeClient, Site};
use super::auth;
use super::cache;

/// The ASCII art logo for "古法时代" LeetCode mode.
const LOGO: &str = r#"
 ██╗     ███████╗███████╗████████╗ ██████╗ ██████╗ ██████╗ ███████╗
 ██║     ██╔════╝██╔════╝╚══██╔══╝██╔════╝██╔═══██╗██╔══██╗██╔════╝
 ██║     █████╗  █████╗     ██║   ██║     ██║   ██║██║  ██║█████╗  
 ██║     ██╔══╝  ██╔══╝     ██║   ██║     ██║   ██║██║  ██║██╔══╝  
 ███████╗███████╗███████╗   ██║   ╚██████╗╚██████╔╝██████╔╝███████╗
 ╚══════╝╚══════╝╚══════╝   ╚═╝    ╚═════╝ ╚═════╝ ╚═════╝ ╚══════╝

               ╔═══════════════════════════════════╗
               ║   古 法 时 代  · RETRO MODE       ║
               ╚═══════════════════════════════════╝
"#;

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
    /// Problem detail view.
    ProblemDetail,
    /// Login prompt.
    Login,
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

/// The main LeetCode panel state.
pub struct LeetCodePanel {
    pub view: LeetCodeView,
    pub problems: Vec<ProblemSummary>,
    pub filtered: Vec<usize>, // indices into `problems`
    pub cursor: usize,
    pub scroll_offset: usize,
    pub filter: ProblemFilter,
    pub detail: Option<ProblemDetail>,
    pub site: Site,
    pub logged_in: bool,
    pub status_msg: String,
    /// Splash screen countdown (frames until auto-transition).
    splash_ticks: u8,
    /// Login input buffer.
    pub login_input: String,
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

        Self {
            view: LeetCodeView::Splash,
            problems,
            filtered,
            cursor: 0,
            scroll_offset: 0,
            filter: ProblemFilter::default(),
            detail: None,
            site,
            logged_in,
            status_msg: String::from("Press any key to continue..."),
            splash_ticks: 0,
            login_input: String::new(),
        }
    }

    /// Handle a key event. Returns an action for the app to process.
    pub fn handle_key(&mut self, key: KeyEvent) -> LeetCodeAction {
        match self.view {
            LeetCodeView::Splash => self.handle_splash_key(key),
            LeetCodeView::ProblemList => self.handle_list_key(key),
            LeetCodeView::ProblemDetail => self.handle_detail_key(key),
            LeetCodeView::Login => self.handle_login_key(key),
        }
    }

    fn handle_splash_key(&mut self, key: KeyEvent) -> LeetCodeAction {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => LeetCodeAction::Close,
            _ => {
                // Any key → transition to problem list (or login if not logged in)
                if self.problems.is_empty() && !self.logged_in {
                    self.view = LeetCodeView::Login;
                    self.status_msg = String::from("Paste your LeetCode cookie (LEETCODE_SESSION=...;csrftoken=...):");
                } else if self.problems.is_empty() {
                    self.status_msg = String::from("Loading problems...");
                    self.fetch_problems();
                    self.view = LeetCodeView::ProblemList;
                } else {
                    self.view = LeetCodeView::ProblemList;
                    self.status_msg = String::from("j/k: navigate  Enter: open  /: search  q: quit  f: filter");
                }
                LeetCodeAction::Redraw
            }
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> LeetCodeAction {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) || key.code == KeyCode::Esc {
                    return LeetCodeAction::Close;
                }
                LeetCodeAction::Close
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
            KeyCode::Enter => {
                self.open_problem();
                LeetCodeAction::Redraw
            }
            KeyCode::Char('r') => {
                self.fetch_problems();
                LeetCodeAction::Redraw
            }
            KeyCode::Char('1') => {
                self.filter.difficulty = Some(Difficulty::Easy);
                self.apply_filter();
                LeetCodeAction::Redraw
            }
            KeyCode::Char('2') => {
                self.filter.difficulty = Some(Difficulty::Medium);
                self.apply_filter();
                LeetCodeAction::Redraw
            }
            KeyCode::Char('3') => {
                self.filter.difficulty = Some(Difficulty::Hard);
                self.apply_filter();
                LeetCodeAction::Redraw
            }
            KeyCode::Char('0') => {
                self.filter.difficulty = None;
                self.filter.status = None;
                self.filter.search.clear();
                self.apply_filter();
                LeetCodeAction::Redraw
            }
            _ => LeetCodeAction::None,
        }
    }

    fn handle_detail_key(&mut self, key: KeyEvent) -> LeetCodeAction {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
                self.view = LeetCodeView::ProblemList;
                self.detail = None;
                self.status_msg = String::from("j/k: navigate  Enter: open  /: search  q: quit  f: filter");
                LeetCodeAction::Redraw
            }
            _ => LeetCodeAction::None,
        }
    }

    fn handle_login_key(&mut self, key: KeyEvent) -> LeetCodeAction {
        match key.code {
            KeyCode::Esc => LeetCodeAction::Close,
            KeyCode::Enter => {
                // Try to parse the cookie
                if let Some(session) = auth::parse_cookie_string(&self.login_input, "user", "cn") {
                    let _ = auth::save_session(&session);
                    self.logged_in = true;
                    self.site = Site::CN;
                    self.login_input.clear();
                    self.status_msg = String::from("Login successful! Loading problems...");
                    self.fetch_problems();
                    self.view = LeetCodeView::ProblemList;
                } else {
                    self.status_msg = String::from("Invalid cookie format. Expected: LEETCODE_SESSION=...;csrftoken=...");
                }
                LeetCodeAction::Redraw
            }
            KeyCode::Char(c) => {
                self.login_input.push(c);
                LeetCodeAction::None
            }
            KeyCode::Backspace => {
                self.login_input.pop();
                LeetCodeAction::None
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

    /// Open the selected problem's detail view.
    fn open_problem(&mut self) {
        if let Some(&idx) = self.filtered.get(self.cursor) {
            let slug = self.problems[idx].title_slug.clone();
            let session = auth::load_session();
            let mut client = LeetCodeClient::new(self.site);
            if let Some(s) = session {
                client = client.with_session(s);
            }

            match client.fetch_problem_detail(&slug) {
                Ok(detail) => {
                    self.detail = Some(detail);
                    self.view = LeetCodeView::ProblemDetail;
                    self.status_msg = String::from("Esc/q: back to list");
                }
                Err(e) => {
                    self.status_msg = format!("Error loading problem: {}", e);
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

    /// Get the logo lines for splash rendering.
    pub fn logo_lines(&self) -> Vec<&str> {
        LOGO.lines().collect()
    }

    /// Get visible problem rows for the list view.
    pub fn visible_problems(&self, height: usize) -> Vec<&ProblemSummary> {
        // Adjust scroll to keep cursor visible
        let visible_height = height.saturating_sub(4); // header + footer + borders
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
        let visible_height = height.saturating_sub(4);
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
