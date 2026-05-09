pub mod loader;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub ai: AiConfig,
    pub theme: ThemeConfig,
    pub filetree: FileTreeConfig,
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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub colorscheme: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FileTreeConfig {
    pub width: u16,
    pub show_hidden: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            ai: AiConfig::default(),
            theme: ThemeConfig::default(),
            filetree: FileTreeConfig::default(),
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
        }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            api_base_url: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            model: "gpt-4o".to_string(),
            timeout_secs: 30,
            yolo_mode: false,
            context_lines: 10,
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            colorscheme: "default".to_string(),
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
