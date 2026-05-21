//! Knowledge Map visualization panel with colorful charts and animations.
//!
//! Renders topic/technique/difficulty distributions as colored bar charts
//! in the terminal, with optional user progress overlay.

use crossterm::style::{Color, SetForegroundColor, ResetColor, Attribute, SetAttribute};
use std::fmt::Write as FmtWrite;
use unicode_width::UnicodeWidthStr;

use super::knowledge::{KnowledgeBase, KnowledgeFilter};

/// Pad a string with spaces to reach a target display width (CJK-aware).
fn pad_to_width(s: &str, target_width: usize) -> String {
    let dw = UnicodeWidthStr::width(s);
    if dw >= target_width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(target_width - dw))
    }
}

/// Color palette for the visualization
const TOPIC_COLORS: &[Color] = &[
    Color::Rgb { r: 255, g: 107, b: 107 },  // Red
    Color::Rgb { r: 255, g: 159, b: 67 },   // Orange
    Color::Rgb { r: 255, g: 234, b: 167 },  // Yellow
    Color::Rgb { r: 85, g: 239, b: 196 },   // Green
    Color::Rgb { r: 116, g: 185, b: 255 },  // Blue
    Color::Rgb { r: 162, g: 155, b: 254 },  // Purple
    Color::Rgb { r: 253, g: 121, b: 168 },  // Pink
    Color::Rgb { r: 0, g: 206, b: 209 },    // Cyan
    Color::Rgb { r: 255, g: 165, b: 0 },    // Gold
    Color::Rgb { r: 144, g: 238, b: 144 },  // LightGreen
    Color::Rgb { r: 255, g: 182, b: 193 },  // LightPink
    Color::Rgb { r: 173, g: 216, b: 230 },  // LightBlue
];

const TECHNIQUE_COLORS: &[Color] = &[
    Color::Rgb { r: 46, g: 204, b: 113 },   // Emerald
    Color::Rgb { r: 52, g: 152, b: 219 },   // Peter River
    Color::Rgb { r: 155, g: 89, b: 182 },   // Amethyst
    Color::Rgb { r: 231, g: 76, b: 60 },    // Alizarin
    Color::Rgb { r: 241, g: 196, b: 15 },   // Sun Flower
    Color::Rgb { r: 26, g: 188, b: 156 },   // Turquoise
    Color::Rgb { r: 230, g: 126, b: 34 },   // Carrot
    Color::Rgb { r: 52, g: 73, b: 94 },     // Wet Asphalt
    Color::Rgb { r: 22, g: 160, b: 133 },   // Green Sea
    Color::Rgb { r: 192, g: 57, b: 43 },    // Pomegranate
    Color::Rgb { r: 142, g: 68, b: 173 },   // Wisteria
    Color::Rgb { r: 41, g: 128, b: 185 },   // Belize Hole
    Color::Rgb { r: 39, g: 174, b: 96 },    // Nephritis
    Color::Rgb { r: 211, g: 84, b: 0 },     // Pumpkin
    Color::Rgb { r: 127, g: 140, b: 141 },  // Asbestos
];

/// Difficulty gradient from green (easy) to red (hard)
fn difficulty_color(level: u8) -> Color {
    match level {
        1 => Color::Rgb { r: 46, g: 204, b: 113 },
        2 => Color::Rgb { r: 85, g: 239, b: 196 },
        3 => Color::Rgb { r: 116, g: 185, b: 255 },
        4 => Color::Rgb { r: 162, g: 155, b: 254 },
        5 => Color::Rgb { r: 241, g: 196, b: 15 },
        6 => Color::Rgb { r: 255, g: 159, b: 67 },
        7 => Color::Rgb { r: 255, g: 107, b: 107 },
        8 => Color::Rgb { r: 231, g: 76, b: 60 },
        9 => Color::Rgb { r: 192, g: 57, b: 43 },
        10 => Color::Rgb { r: 142, g: 68, b: 173 },
        _ => Color::White,
    }
}

/// Map difficulty level (1-10) to Chinese/English label
fn difficulty_label(level: u8) -> &'static str {
    match level {
        1 => "入门 Beginner",
        2 => "简单 Easy",
        3 => "简单+ Easy+",
        4 => "中等 Medium",
        5 => "中等+ Medium+",
        6 => "中难 Med-Hard",
        7 => "困难 Hard",
        8 => "困难+ Hard+",
        9 => "地狱 Expert",
        10 => "噩梦 Insane",
        _ => "未知 Unknown",
    }
}

/// Block characters for smooth bar rendering
const BAR_CHARS: &[char] = &[' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

/// Render a smooth bar of given width (supports fractional blocks)
fn render_bar(width: f64, max_width: usize, color: Color) -> String {
    let full_blocks = width as usize;
    let remainder = ((width - full_blocks as f64) * 8.0) as usize;
    let mut bar = String::new();

    write!(bar, "{}", SetForegroundColor(color)).ok();

    for _ in 0..full_blocks.min(max_width) {
        bar.push('█');
    }
    if full_blocks < max_width && remainder > 0 {
        bar.push(BAR_CHARS[remainder]);
    }

    write!(bar, "{}", ResetColor).ok();
    bar
}

/// Sparkline characters for mini inline charts
const SPARK_CHARS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Render a sparkline from values
fn render_sparkline(values: &[usize], color: Color) -> String {
    if values.is_empty() {
        return String::new();
    }
    let max = *values.iter().max().unwrap_or(&1) as f64;
    let mut s = String::new();
    write!(s, "{}", SetForegroundColor(color)).ok();
    for &v in values {
        let idx = ((v as f64 / max) * 7.0) as usize;
        s.push(SPARK_CHARS[idx.min(7)]);
    }
    write!(s, "{}", ResetColor).ok();
    s
}

/// User progress data (cached)
#[derive(Debug, Clone, Default)]
pub struct UserProgress {
    pub solved_ids: Vec<u32>,
    pub attempted_ids: Vec<u32>,
    pub last_synced_ts: u64,
}

impl UserProgress {
    pub fn solved_count(&self) -> usize {
        self.solved_ids.len()
    }

    pub fn is_solved(&self, id: u32) -> bool {
        self.solved_ids.binary_search(&id).is_ok()
    }

    /// Count solved problems matching a filter dimension
    pub fn solved_in(&self, ids: &[u32]) -> usize {
        ids.iter().filter(|id| self.is_solved(**id)).count()
    }
}

/// The knowledge map panel state
pub struct KnowledgePanel {
    pub filter: KnowledgeFilter,
    pub filtered_ids: Vec<u32>,
    pub scroll_offset: usize,
    pub selected_tab: PanelTab,
    pub user_progress: Option<UserProgress>,
    pub animation_frame: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PanelTab {
    Overview,
    Topics,
    Techniques,
    Problems,
}

impl KnowledgePanel {
    pub fn new() -> Self {
        let kb = KnowledgeBase::global();
        Self {
            filter: KnowledgeFilter::default(),
            filtered_ids: kb.query(&KnowledgeFilter::default()),
            scroll_offset: 0,
            selected_tab: PanelTab::Overview,
            user_progress: None,
            animation_frame: 0,
        }
    }

    /// Advance animation frame (call on each render tick)
    pub fn tick(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1);
    }

    /// Apply current filter and update results
    pub fn apply_filter(&mut self) {
        let kb = KnowledgeBase::global();
        self.filtered_ids = kb.query(&self.filter);
        self.scroll_offset = 0;
    }

    /// Render the overview tab content into a string buffer.
    /// Returns lines of pre-formatted terminal output.
    pub fn render_overview(&self, width: usize) -> Vec<String> {
        let kb = KnowledgeBase::global();
        let mut lines = Vec::new();
        let bar_max = width.saturating_sub(30).min(40);

        // ── Header with animation ────────────────────────────────────────
        let header_color = TOPIC_COLORS[(self.animation_frame as usize / 4) % TOPIC_COLORS.len()];
        lines.push(format!(
            "{}{}  ◆ LeetCode Knowledge Map ◆  {} problems{}{}",
            SetForegroundColor(header_color),
            SetAttribute(Attribute::Bold),
            kb.len(),
            SetAttribute(Attribute::Reset),
            ResetColor,
        ));
        lines.push(String::new());

        // ── Topic Distribution ───────────────────────────────────────────
        lines.push(format!(
            "{}{}▌ 题型分布 (Topics){}{}",
            SetForegroundColor(Color::Cyan),
            SetAttribute(Attribute::Bold),
            SetAttribute(Attribute::Reset),
            ResetColor,
        ));

        let topic_stats = kb.topic_stats();
        let topic_max = topic_stats.first().map(|x| x.1).unwrap_or(1);
        let topic_label_w = 8; // display width for topic labels
        for (i, (name, count)) in topic_stats.iter().take(8).enumerate() {
            let bar_width = (*count as f64 / topic_max as f64) * bar_max as f64;
            let color = TOPIC_COLORS[i % TOPIC_COLORS.len()];
            let bar = render_bar(bar_width, bar_max, color);

            let user_info = if let Some(ref up) = self.user_progress {
                let solved = up.solved_in(kb.topic_ids(*name).unwrap_or(&[]));
                format!(" {}{}/{}{}",
                    SetForegroundColor(Color::DarkGrey),
                    solved, count,
                    ResetColor)
            } else {
                String::new()
            };

            let padded_name = pad_to_width(name, topic_label_w);
            lines.push(format!(
                "  {} {} {:>4}{}",
                padded_name, bar, count, user_info
            ));
        }
        lines.push(String::new());

        // ── Technique Distribution ───────────────────────────────────────
        lines.push(format!(
            "{}{}▌ 解法分布 (Techniques){}{}",
            SetForegroundColor(Color::Magenta),
            SetAttribute(Attribute::Bold),
            SetAttribute(Attribute::Reset),
            ResetColor,
        ));

        let tech_stats = kb.technique_stats();
        let tech_max = tech_stats.first().map(|x| x.1).unwrap_or(1);
        let tech_label_w = 10; // display width for technique labels
        for (i, (name, count)) in tech_stats.iter().take(10).enumerate() {
            let bar_width = (*count as f64 / tech_max as f64) * bar_max as f64;
            let color = TECHNIQUE_COLORS[i % TECHNIQUE_COLORS.len()];
            let bar = render_bar(bar_width, bar_max, color);

            let user_info = if let Some(ref up) = self.user_progress {
                let solved = up.solved_in(kb.technique_ids(*name).unwrap_or(&[]));
                format!(" {}{}/{}{}",
                    SetForegroundColor(Color::DarkGrey),
                    solved, count,
                    ResetColor)
            } else {
                String::new()
            };

            let padded_name = pad_to_width(name, tech_label_w);
            lines.push(format!(
                "  {} {} {:>4}{}",
                padded_name, bar, count, user_info
            ));
        }
        lines.push(String::new());

        // ── Difficulty Distribution (sparkline + bars) ───────────────────
        lines.push(format!(
            "{}{}▌ 难度分布 (Difficulty){}{}",
            SetForegroundColor(Color::Yellow),
            SetAttribute(Attribute::Bold),
            SetAttribute(Attribute::Reset),
            ResetColor,
        ));

        let diff_stats = kb.difficulty_stats();
        let diff_max = diff_stats.iter().map(|x| x.1).max().unwrap_or(1);
        let diff_label_w = 16; // display width for difficulty labels (e.g. "中等+ Medium+")
        for (level, count) in &diff_stats {
            let bar_width = (*count as f64 / diff_max as f64) * bar_max as f64;
            let color = difficulty_color(*level);
            let bar = render_bar(bar_width, bar_max, color);
            let label = difficulty_label(*level);
            let padded_label = pad_to_width(label, diff_label_w);
            lines.push(format!(
                "  {}{}{}  {} {:>4}",
                SetForegroundColor(color),
                padded_label,
                ResetColor,
                bar,
                count,
            ));
        }

        // Sparkline summary
        let spark_values: Vec<usize> = (1..=10)
            .map(|d| diff_stats.iter().find(|(l, _)| *l == d).map(|(_, c)| *c).unwrap_or(0))
            .collect();
        let sparkline = render_sparkline(&spark_values, Color::Rgb { r: 255, g: 215, b: 0 });
        lines.push(format!("  难度曲线: {}", sparkline));
        lines.push(String::new());

        // ── User Progress (if available) ─────────────────────────────────
        if let Some(ref up) = self.user_progress {
            lines.push(format!(
                "{}{}▌ 我的进度{}{}",
                SetForegroundColor(Color::Green),
                SetAttribute(Attribute::Bold),
                SetAttribute(Attribute::Reset),
                ResetColor,
            ));

            let total = kb.len();
            let solved = up.solved_count();
            let pct = (solved as f64 / total as f64) * 100.0;
            let progress_bar = render_bar(
                (pct / 100.0) * bar_max as f64,
                bar_max,
                Color::Rgb { r: 46, g: 204, b: 113 },
            );

            lines.push(format!(
                "  已解决: {}{}/{}  ({:.1}%){}",
                SetForegroundColor(Color::Green),
                solved, total, pct,
                ResetColor,
            ));
            lines.push(format!("  {}", progress_bar));
            lines.push(String::new());

            // Weakness analysis
            lines.push(format!(
                "  {}薄弱项:{}",
                SetForegroundColor(Color::Red),
                ResetColor,
            ));
            let mut weakness: Vec<(&str, usize, usize)> = tech_stats.iter()
                .map(|(name, total)| {
                    let solved = up.solved_in(
                        kb.technique_ids(*name).unwrap_or(&[])
                    );
                    (*name, solved, *total)
                })
                .filter(|(_, _, total)| *total > 20)
                .collect();
            weakness.sort_by(|a, b| {
                let ra = a.1 as f64 / a.2 as f64;
                let rb = b.1 as f64 / b.2 as f64;
                ra.partial_cmp(&rb).unwrap()
            });
            for (name, solved, total) in weakness.iter().take(3) {
                lines.push(format!(
                    "    {} {}/{} ({:.0}%)",
                    name, solved, total,
                    (*solved as f64 / *total as f64) * 100.0
                ));
            }
            lines.push(String::new());
        }

        // ── Footer ──────────────────────────────────────────────────────
        lines.push(format!(
            "{}  [Tab] 切换面板  [/] 搜索  [f] 过滤  [q] 返回{}",
            SetForegroundColor(Color::DarkGrey),
            ResetColor,
        ));

        lines
    }

    /// Render the filter bar at the top of the panel.
    pub fn render_filter_bar(&self, width: usize) -> String {
        let mut s = String::new();
        write!(s, "{}{}╔", SetForegroundColor(Color::DarkGrey), SetAttribute(Attribute::Dim)).ok();
        for _ in 0..width.saturating_sub(2) {
            s.push('═');
        }
        write!(s, "╗{}{}", SetAttribute(Attribute::Reset), ResetColor).ok();
        s.push('\n');

        // Filter content
        write!(s, "{}║{}", SetForegroundColor(Color::DarkGrey), ResetColor).ok();
        let filter_text = if self.filter.search.is_empty() && self.filter.topics.is_empty() && self.filter.techniques.is_empty() {
            format!(
                " {}全部 {} 题{}",
                SetForegroundColor(Color::White),
                self.filtered_ids.len(),
                ResetColor,
            )
        } else {
            let mut parts = Vec::new();
            if !self.filter.search.is_empty() {
                parts.push(format!("🔍{}", self.filter.search));
            }
            for t in &self.filter.topics {
                parts.push(format!(
                    "{}[{}]{}",
                    SetForegroundColor(Color::Cyan), t, ResetColor
                ));
            }
            for t in &self.filter.techniques {
                parts.push(format!(
                    "{}[{}]{}",
                    SetForegroundColor(Color::Magenta), t, ResetColor
                ));
            }
            if let Some(min) = self.filter.difficulty_min {
                let max = self.filter.difficulty_max.unwrap_or(10);
                parts.push(format!(
                    "{}难度{}-{}{}",
                    SetForegroundColor(Color::Yellow), min, max, ResetColor
                ));
            }
            format!(" {} → {} 题", parts.join(" "), self.filtered_ids.len())
        };
        write!(s, "{}", filter_text).ok();

        // Pad to width
        write!(s, "{}║{}", SetForegroundColor(Color::DarkGrey), ResetColor).ok();
        s.push('\n');

        write!(s, "{}╚", SetForegroundColor(Color::DarkGrey)).ok();
        for _ in 0..width.saturating_sub(2) {
            s.push('═');
        }
        write!(s, "╝{}", ResetColor).ok();

        s
    }

    /// Render the tab bar showing available panels with current panel name.
    pub fn render_tab_bar(&self, width: usize) -> String {
        let tabs = [
            (PanelTab::Overview, "1:概览", "◆ 知识图谱 · 概览"),
            (PanelTab::Topics, "2:题型", "◆ 知识图谱 · 题型分布"),
            (PanelTab::Techniques, "3:解法", "◆ 知识图谱 · 解法分布"),
            (PanelTab::Problems, "4:题目", "◆ 知识图谱 · 题目列表"),
        ];

        let mut s = String::new();
        // Panel title
        let current_title = tabs.iter()
            .find(|(t, _, _)| *t == self.selected_tab)
            .map(|(_, _, title)| *title)
            .unwrap_or("◆ 知识图谱");
        write!(s, " {}{}{}{} │ ",
            SetForegroundColor(Color::White),
            SetAttribute(Attribute::Bold),
            current_title,
            SetAttribute(Attribute::Reset),
        ).ok();

        for (tab, label, _) in &tabs {
            if *tab == self.selected_tab {
                write!(
                    s, "{}{}[{}]{}{}  ",
                    SetForegroundColor(Color::White),
                    SetAttribute(Attribute::Bold),
                    label,
                    SetAttribute(Attribute::Reset),
                    ResetColor,
                ).ok();
            } else {
                write!(
                    s, "{} {} {}  ",
                    SetForegroundColor(Color::DarkGrey),
                    label,
                    ResetColor,
                ).ok();
            }
        }
        // Pad remaining width with a subtle line
        let used_approx = current_title.len() + 5 + tabs.iter().map(|(_, l, _)| l.len() + 4).sum::<usize>();
        if width > used_approx {
            write!(s, "{}", SetForegroundColor(Color::DarkGrey)).ok();
            for _ in 0..width.saturating_sub(used_approx) {
                s.push('─');
            }
            write!(s, "{}", ResetColor).ok();
        }
        s
    }

    /// Render the Topics tab: detailed topic distribution.
    pub fn render_topics(&self, width: usize) -> Vec<String> {
        let kb = KnowledgeBase::global();
        let mut lines = Vec::new();
        let bar_max = width.saturating_sub(30).min(40);

        lines.push(format!(
            "{}{}  ◆ 题型分布 Topics Distribution{}{}",
            SetForegroundColor(Color::Cyan),
            SetAttribute(Attribute::Bold),
            SetAttribute(Attribute::Reset),
            ResetColor,
        ));
        lines.push(String::new());

        let topic_stats = kb.topic_stats();
        let topic_max = topic_stats.first().map(|x| x.1).unwrap_or(1);
        let topic_label_w = 12; // display width for topic labels
        for (i, (name, count)) in topic_stats.iter().enumerate() {
            let bar_width = (*count as f64 / topic_max as f64) * bar_max as f64;
            let color = TOPIC_COLORS[i % TOPIC_COLORS.len()];
            let bar = render_bar(bar_width, bar_max, color);

            let pct = (*count as f64 / kb.len() as f64) * 100.0;
            let user_info = if let Some(ref up) = self.user_progress {
                let solved = up.solved_in(kb.topic_ids(*name).unwrap_or(&[]));
                format!(" {}{}/{}{}",
                    SetForegroundColor(Color::DarkGrey),
                    solved, count,
                    ResetColor)
            } else {
                String::new()
            };

            let padded_name = pad_to_width(name, topic_label_w);
            lines.push(format!(
                "  {}{}{} {} {:>4} ({:.1}%){}",
                SetForegroundColor(color),
                padded_name,
                ResetColor,
                bar, count, pct, user_info
            ));
        }
        lines.push(String::new());
        lines.push(format!(
            "{}  共 {} 个题型分类{}",
            SetForegroundColor(Color::DarkGrey),
            topic_stats.len(),
            ResetColor,
        ));

        lines
    }

    /// Render the Techniques tab: detailed technique distribution.
    pub fn render_techniques(&self, width: usize) -> Vec<String> {
        let kb = KnowledgeBase::global();
        let mut lines = Vec::new();
        let bar_max = width.saturating_sub(30).min(40);

        lines.push(format!(
            "{}{}  ◆ 解法分布 Techniques Distribution{}{}",
            SetForegroundColor(Color::Magenta),
            SetAttribute(Attribute::Bold),
            SetAttribute(Attribute::Reset),
            ResetColor,
        ));
        lines.push(String::new());

        let tech_stats = kb.technique_stats();
        let tech_max = tech_stats.first().map(|x| x.1).unwrap_or(1);
        let tech_label_w = 12; // display width for technique labels
        for (i, (name, count)) in tech_stats.iter().enumerate() {
            let bar_width = (*count as f64 / tech_max as f64) * bar_max as f64;
            let color = TECHNIQUE_COLORS[i % TECHNIQUE_COLORS.len()];
            let bar = render_bar(bar_width, bar_max, color);

            let pct = (*count as f64 / kb.len() as f64) * 100.0;
            let user_info = if let Some(ref up) = self.user_progress {
                let solved = up.solved_in(kb.technique_ids(*name).unwrap_or(&[]));
                format!(" {}{}/{}{}",
                    SetForegroundColor(Color::DarkGrey),
                    solved, count,
                    ResetColor)
            } else {
                String::new()
            };

            let padded_name = pad_to_width(name, tech_label_w);
            lines.push(format!(
                "  {}{}{} {} {:>4} ({:.1}%){}",
                SetForegroundColor(color),
                padded_name,
                ResetColor,
                bar, count, pct, user_info
            ));
        }
        lines.push(String::new());
        lines.push(format!(
            "{}  共 {} 种解法分类{}",
            SetForegroundColor(Color::DarkGrey),
            tech_stats.len(),
            ResetColor,
        ));

        lines
    }

    /// Render a compact problem list for the Problems tab.
    pub fn render_problem_list(&self, width: usize, height: usize) -> Vec<String> {
        let kb = KnowledgeBase::global();
        let mut lines = Vec::new();
        let visible = self.filtered_ids.iter()
            .skip(self.scroll_offset)
            .take(height);

        for &id in visible {
            if let Some(p) = kb.get(id) {
                let diff_color = difficulty_color(p.difficulty);
                let solved_marker = if let Some(ref up) = self.user_progress {
                    if up.is_solved(id) {
                        format!("{}✓{}", SetForegroundColor(Color::Green), ResetColor)
                    } else {
                        " ".to_string()
                    }
                } else {
                    " ".to_string()
                };

                let title_width = width.saturating_sub(20);
                let title: String = p.title.chars().take(title_width).collect();

                lines.push(format!(
                    " {} {}{:>4}{} {}{:<tw$}{} {}{}{}",
                    solved_marker,
                    SetForegroundColor(Color::DarkGrey),
                    id,
                    ResetColor,
                    SetForegroundColor(Color::White),
                    title,
                    ResetColor,
                    SetForegroundColor(diff_color),
                    "●".repeat(p.difficulty as usize),
                    ResetColor,
                    tw = title_width,
                ));
            }
        }

        // Scroll indicator
        if self.filtered_ids.len() > height {
            let pct = (self.scroll_offset as f64 / (self.filtered_ids.len() - height) as f64) * 100.0;
            lines.push(format!(
                "{}  ─── {}/{} ({:.0}%) ───{}",
                SetForegroundColor(Color::DarkGrey),
                self.scroll_offset + 1,
                self.filtered_ids.len(),
                pct,
                ResetColor,
            ));
        }

        lines
    }
}
