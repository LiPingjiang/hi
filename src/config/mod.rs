pub mod loader;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub ai: AiConfig,
    pub theme: ThemeConfig,
    pub filetree: FileTreeConfig,
    pub chat: ChatConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub line_numbers: bool,
    pub tab_width: usize,
    pub expand_tab: bool,
    pub auto_indent: bool,
    pub ignore_case: bool,
    pub smart_case: bool,
    pub scroll_off: usize,
    /// Language tag for UI strings, e.g. "zh-CN", "en-US", "auto".
    /// "auto" (default) detects from the LANG / LC_ALL environment variable.
    pub language: String,
    /// Enable mouse reporting (scroll, click, drag).  Default: false.
    /// When false the editor operates in pure-keyboard Vim mode; mouse
    /// events trigger a "drop the mouse" reminder.  Use `:mouse` to toggle
    /// at runtime, or set `mouse = true` in ~/.hirc to start with mouse on.
    pub mouse: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub api_base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_secs: u64,
    pub yolo_mode: bool,
    pub context_lines: usize,
    pub debug: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    /// Legacy colorscheme name (kept for backward compat).
    pub colorscheme: String,
    /// Syntect theme name for the editor text area.
    /// Built-in options: "base16-ocean.dark", "Solarized (dark)",
    /// "base16-eighties.dark", "base16-mocha.dark", "InspiredGitHub".
    pub editor_theme: String,
    /// Chat panel Markdown theme name.
    /// Built-in options: "dark", "dracula", "tokyo-night".
    pub chat_theme: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FileTreeConfig {
    pub width: u16,
    pub show_hidden: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ChatConfig {
    /// Width of the chat panel in columns.
    pub width: u16,
    /// Maximum number of messages to keep in history.
    pub max_messages: usize,
    /// Number of recent conversation pairs to include in AI prompt context.
    pub context_pairs: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            ai: AiConfig::default(),
            theme: ThemeConfig::default(),
            filetree: FileTreeConfig::default(),
            chat: ChatConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            line_numbers: true,
            tab_width: 4,
            expand_tab: true,
            auto_indent: true,
            ignore_case: true,
            smart_case: true,
            scroll_off: 5,
            language: "auto".to_string(),
            mouse: false,
        }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            // Default to OpenAI; override in ~/.hirc for local endpoints:
            //   api_base_url = "http://localhost:11434/v1"  # Ollama
            //   api_base_url = "http://localhost:1234/v1"   # LM Studio
            api_base_url: "https://api.openai.com/v1".to_string(),
            // Leave empty for local endpoints that don't require authentication.
            api_key: String::new(),
            model: "gpt-4o".to_string(),
            timeout_secs: 30,
            yolo_mode: false,
            context_lines: 10,
            debug: false,
        }
    }
}


impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            colorscheme: "default".to_string(),
            editor_theme: "base16-ocean.dark".to_string(),
            chat_theme: "dark".to_string(),
        }
    }
}

impl Default for FileTreeConfig {
    fn default() -> Self {
        Self {
            width: 30,
            show_hidden: false,
        }
    }
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            width: 42,
            max_messages: 200,
            context_pairs: 5,
        }
    }
}
