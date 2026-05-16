//! Tutorial board — a right-side panel that teaches the three core Vim philosophies.
//!
//! The board is intentionally minimal: three rules, no fluff.
//! It occupies the same right-side slot as the chat panel but with lower priority
//! (chat always takes the rightmost position when both are visible).

use crossterm::style::Color;
use unicode_width::UnicodeWidthChar;

/// A single styled line in the tutorial board.
pub struct TutorialLine {
    pub spans: Vec<TutSpan>,
}

pub struct TutSpan {
    pub text: String,
    pub fg: Option<Color>,
    pub bold: bool,
}

impl TutSpan {
    pub fn plain(text: impl Into<String>) -> Self {
        Self { text: text.into(), fg: None, bold: false }
    }
    pub fn colored(text: impl Into<String>, fg: Color) -> Self {
        Self { text: text.into(), fg: Some(fg), bold: false }
    }
    pub fn bold(text: impl Into<String>, fg: Color) -> Self {
        Self { text: text.into(), fg: Some(fg), bold: true }
    }
}

impl TutorialLine {
    pub fn new(spans: Vec<TutSpan>) -> Self { Self { spans } }
    pub fn empty() -> Self { Self { spans: vec![TutSpan::plain("")] } }
}

/// The tutorial board state.
pub struct TutorialBoard {
    pub scroll: usize,
    pub width: usize,
}

impl TutorialBoard {
    pub fn new(width: usize) -> Self {
        Self { scroll: 0, width }
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_add(n);
    }

    pub fn scroll_down(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
    }
}

// ── Content generation ────────────────────────────────────────────────────────

/// Generate the tutorial content lines for the given language.
/// `lang` should be "zh-CN" or "en-US" (or anything else → English).
pub fn tutorial_content(lang: &str) -> Vec<TutorialLine> {
    if lang.starts_with("zh") {
        tutorial_zh()
    } else {
        tutorial_en()
    }
}

fn tutorial_zh() -> Vec<TutorialLine> {
    let title_color = Color::Cyan;
    let accent = Color::Yellow;
    let example = Color::Green;
    let dim = Color::DarkGrey;

    vec![
        // ── Header ──
        TutorialLine::new(vec![
            TutSpan::bold("━━ 三条军规 ━━", title_color),
        ]),
        TutorialLine::empty(),

        // ── Rule 1: Drop the mouse ──
        TutorialLine::new(vec![
            TutSpan::bold("① 扔掉鼠标", accent),
        ]),
        TutorialLine::empty(),
        TutorialLine::new(vec![
            TutSpan::plain("  手不离键盘。"),
        ]),
        TutorialLine::new(vec![
            TutSpan::colored("  h", example),
            TutSpan::plain("←  "),
            TutSpan::colored("j", example),
            TutSpan::plain("↓  "),
            TutSpan::colored("k", example),
            TutSpan::plain("↑  "),
            TutSpan::colored("l", example),
            TutSpan::plain("→"),
        ]),
        TutorialLine::new(vec![
            TutSpan::colored("  w", example),
            TutSpan::plain(" 下个词  "),
            TutSpan::colored("b", example),
            TutSpan::plain(" 上个词  "),
            TutSpan::colored("gg", example),
            TutSpan::plain("/"),
            TutSpan::colored("G", example),
            TutSpan::plain(" 顶/底"),
        ]),
        TutorialLine::empty(),

        // ── Rule 2: Verb + Modifier + Noun ──
        TutorialLine::new(vec![
            TutSpan::bold("② 动词 + 范围 + 名词", accent),
        ]),
        TutorialLine::empty(),
        TutorialLine::new(vec![
            TutSpan::plain("  像说话一样组合命令："),
        ]),
        TutorialLine::empty(),
        TutorialLine::new(vec![
            TutSpan::plain("  动词  "),
            TutSpan::colored("d", example),
            TutSpan::plain("删 "),
            TutSpan::colored("c", example),
            TutSpan::plain("改 "),
            TutSpan::colored("y", example),
            TutSpan::plain("复制"),
        ]),
        TutorialLine::new(vec![
            TutSpan::plain("  范围  "),
            TutSpan::colored("i", example),
            TutSpan::plain("里面 "),
            TutSpan::colored("a", example),
            TutSpan::plain("包围 "),
            TutSpan::colored("t", example),
            TutSpan::plain("直到 "),
            TutSpan::colored("f", example),
            TutSpan::plain("找到"),
        ]),
        TutorialLine::new(vec![
            TutSpan::plain("  名词  "),
            TutSpan::colored("w", example),
            TutSpan::plain("词 "),
            TutSpan::colored("s", example),
            TutSpan::plain("句 "),
            TutSpan::colored("p", example),
            TutSpan::plain("段 "),
            TutSpan::colored("t", example),
            TutSpan::plain("标签 "),
            TutSpan::colored("\"", example),
            TutSpan::plain("引号内"),
        ]),
        TutorialLine::empty(),
        TutorialLine::new(vec![
            TutSpan::colored("  diw", example),
            TutSpan::plain(" → "),
            TutSpan::colored("d", dim),
            TutSpan::plain("elete "),
            TutSpan::colored("i", dim),
            TutSpan::plain("nside "),
            TutSpan::colored("w", dim),
            TutSpan::plain("ord  删除单词"),
        ]),
        TutorialLine::new(vec![
            TutSpan::colored("  ci\"", example),
            TutSpan::plain(" → "),
            TutSpan::colored("c", dim),
            TutSpan::plain("hange "),
            TutSpan::colored("i", dim),
            TutSpan::plain("nside "),
            TutSpan::colored("\"", dim),
            TutSpan::plain("  改引号内容"),
        ]),
        TutorialLine::new(vec![
            TutSpan::colored("  yap", example),
            TutSpan::plain(" → "),
            TutSpan::colored("y", dim),
            TutSpan::plain("ank "),
            TutSpan::colored("a", dim),
            TutSpan::plain("round "),
            TutSpan::colored("p", dim),
            TutSpan::plain("aragraph  复制段落"),
        ]),
        TutorialLine::empty(),

        // ── Rule 3: Modes are breathing ──
        TutorialLine::new(vec![
            TutSpan::bold("③ 模式就是呼吸", accent),
        ]),
        TutorialLine::empty(),
        TutorialLine::new(vec![
            TutSpan::colored("  Normal", example),
            TutSpan::plain(" = 思考（默认）"),
        ]),
        TutorialLine::new(vec![
            TutSpan::colored("  Insert", example),
            TutSpan::plain(" = 输入（"),
            TutSpan::colored("i", example),
            TutSpan::plain(" 进入）"),
        ]),
        TutorialLine::new(vec![
            TutSpan::colored("  Esc", example),
            TutSpan::plain("    = 呼气（回到思考）"),
        ]),
        TutorialLine::empty(),
        TutorialLine::new(vec![
            TutSpan::plain("  节奏：写几个字 → "),
            TutSpan::colored("Esc", example),
            TutSpan::plain(" → 移动 → "),
            TutSpan::colored("i", example),
            TutSpan::plain(" → 再写"),
        ]),
        TutorialLine::empty(),

        // ── Footer ──
        TutorialLine::new(vec![
            TutSpan::colored("  ─────────────────────", dim),
        ]),
        TutorialLine::new(vec![
            TutSpan::colored("  Ctrl+h", example),
            TutSpan::plain(" 关闭此面板"),
        ]),
    ]
}

fn tutorial_en() -> Vec<TutorialLine> {
    let title_color = Color::Cyan;
    let accent = Color::Yellow;
    let example = Color::Green;
    let dim = Color::DarkGrey;

    vec![
        // ── Header ──
        TutorialLine::new(vec![
            TutSpan::bold("━━ Three Rules ━━", title_color),
        ]),
        TutorialLine::empty(),

        // ── Rule 1 ──
        TutorialLine::new(vec![
            TutSpan::bold("① Drop the Mouse", accent),
        ]),
        TutorialLine::empty(),
        TutorialLine::new(vec![
            TutSpan::plain("  Hands stay on keyboard."),
        ]),
        TutorialLine::new(vec![
            TutSpan::colored("  h", example),
            TutSpan::plain("←  "),
            TutSpan::colored("j", example),
            TutSpan::plain("↓  "),
            TutSpan::colored("k", example),
            TutSpan::plain("↑  "),
            TutSpan::colored("l", example),
            TutSpan::plain("→"),
        ]),
        TutorialLine::new(vec![
            TutSpan::colored("  w", example),
            TutSpan::plain(" next word  "),
            TutSpan::colored("b", example),
            TutSpan::plain(" prev word  "),
            TutSpan::colored("gg", example),
            TutSpan::plain("/"),
            TutSpan::colored("G", example),
            TutSpan::plain(" top/bottom"),
        ]),
        TutorialLine::empty(),

        // ── Rule 2 ──
        TutorialLine::new(vec![
            TutSpan::bold("② Verb + Scope + Noun", accent),
        ]),
        TutorialLine::empty(),
        TutorialLine::new(vec![
            TutSpan::plain("  Compose commands like sentences:"),
        ]),
        TutorialLine::empty(),
        TutorialLine::new(vec![
            TutSpan::plain("  Verb  "),
            TutSpan::colored("d", example),
            TutSpan::plain("elete "),
            TutSpan::colored("c", example),
            TutSpan::plain("hange "),
            TutSpan::colored("y", example),
            TutSpan::plain("ank"),
        ]),
        TutorialLine::new(vec![
            TutSpan::plain("  Scope "),
            TutSpan::colored("i", example),
            TutSpan::plain("nside "),
            TutSpan::colored("a", example),
            TutSpan::plain("round "),
            TutSpan::colored("t", example),
            TutSpan::plain("ill "),
            TutSpan::colored("f", example),
            TutSpan::plain("ind"),
        ]),
        TutorialLine::new(vec![
            TutSpan::plain("  Noun  "),
            TutSpan::colored("w", example),
            TutSpan::plain("ord "),
            TutSpan::colored("s", example),
            TutSpan::plain("entence "),
            TutSpan::colored("p", example),
            TutSpan::plain("aragraph "),
            TutSpan::colored("\"", example),
            TutSpan::plain("quotes"),
        ]),
        TutorialLine::empty(),
        TutorialLine::new(vec![
            TutSpan::colored("  diw", example),
            TutSpan::plain(" → "),
            TutSpan::colored("d", dim),
            TutSpan::plain("elete "),
            TutSpan::colored("i", dim),
            TutSpan::plain("nside "),
            TutSpan::colored("w", dim),
            TutSpan::plain("ord"),
        ]),
        TutorialLine::new(vec![
            TutSpan::colored("  ci\"", example),
            TutSpan::plain(" → "),
            TutSpan::colored("c", dim),
            TutSpan::plain("hange "),
            TutSpan::colored("i", dim),
            TutSpan::plain("nside "),
            TutSpan::colored("\"", dim),
            TutSpan::plain("quotes"),
        ]),
        TutorialLine::new(vec![
            TutSpan::colored("  yap", example),
            TutSpan::plain(" → "),
            TutSpan::colored("y", dim),
            TutSpan::plain("ank "),
            TutSpan::colored("a", dim),
            TutSpan::plain("round "),
            TutSpan::colored("p", dim),
            TutSpan::plain("aragraph"),
        ]),
        TutorialLine::empty(),

        // ── Rule 3 ──
        TutorialLine::new(vec![
            TutSpan::bold("③ Modes = Breathing", accent),
        ]),
        TutorialLine::empty(),
        TutorialLine::new(vec![
            TutSpan::colored("  Normal", example),
            TutSpan::plain(" = think (default)"),
        ]),
        TutorialLine::new(vec![
            TutSpan::colored("  Insert", example),
            TutSpan::plain(" = type (press "),
            TutSpan::colored("i", example),
            TutSpan::plain(")"),
        ]),
        TutorialLine::new(vec![
            TutSpan::colored("  Esc", example),
            TutSpan::plain("    = exhale (back to think)"),
        ]),
        TutorialLine::empty(),
        TutorialLine::new(vec![
            TutSpan::plain("  Rhythm: type → "),
            TutSpan::colored("Esc", example),
            TutSpan::plain(" → move → "),
            TutSpan::colored("i", example),
            TutSpan::plain(" → type"),
        ]),
        TutorialLine::empty(),

        // ── Footer ──
        TutorialLine::new(vec![
            TutSpan::colored("  ─────────────────────", dim),
        ]),
        TutorialLine::new(vec![
            TutSpan::colored("  Ctrl+h", example),
            TutSpan::plain(" close this panel"),
        ]),
    ]
}

/// Calculate display width of a string (CJK-aware).
pub fn display_width(s: &str) -> usize {
    s.chars().map(|c| UnicodeWidthChar::width(c).unwrap_or(0)).sum()
}
