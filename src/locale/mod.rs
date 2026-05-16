//! Locale system: load user-visible strings from TOML files.
//!
//! # Priority chain
//!
//! 1. `~/.config/hi/locales/{lang}.toml`  — user override (highest priority)
//! 2. Bundled `zh-CN` / `en-US`           — compiled into binary via `include_str!`
//! 3. `Default` impl                       — hard-coded en-US fallback (never panics)
//!
//! # Language detection
//!
//! When `language = "auto"` (the default), the language tag is derived from the
//! `LANG` / `LC_ALL` / `LC_MESSAGES` environment variable:
//!
//! ```text
//! LANG=zh_CN.UTF-8  →  "zh-CN"
//! LANG=en_US.UTF-8  →  "en-US"
//! LANG=ru_RU.UTF-8  →  "ru-RU"  (community locale, falls back to en-US if not found)
//! ```
//!
//! # Community translations
//!
//! Drop a `{lang}.toml` file into `~/.config/hi/locales/` and set
//! `language = "{lang}"` in `~/.hirc`.  Untranslated keys fall back to en-US
//! automatically, so partial translations work fine.

pub mod loader;

use serde::Deserialize;

// ── Top-level struct ──────────────────────────────────────────────────────────

/// All user-visible strings, grouped by subsystem.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Locale {
    pub ui:       UiStrings,
    pub messages: MessageStrings,
    pub commands: CommandStrings,
    pub ai:       AiStrings,
}

// ── Sub-structs ───────────────────────────────────────────────────────────────

/// Hint bar and overlay UI strings.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UiStrings {
    // Normal mode — context-sensitive hints
    pub hint_normal:          String,
    pub hint_normal_empty:    String,
    pub hint_normal_comment:  String,
    pub hint_normal_tag:      String,
    pub hint_normal_url:      String,
    pub hint_normal_number:   String,
    pub hint_normal_string:   String,
    pub hint_normal_word:     String,
    /// Contains `{reg}` placeholder for the macro register letter.
    pub hint_normal_macro:    String,
    pub hint_normal_register: String,
    pub hint_normal_search:   String,
    // Other modes
    pub hint_insert:          String,
    pub hint_visual:          String,
    pub hint_command:         String,
    pub hint_search:          String,
    pub hint_ai:              String,
    pub hint_filetree:        String,
    /// Appended to any hint when multiple panels are visible simultaneously.
    pub hint_switch_zone:     String,
    // Overlays
    pub theme_picker_title:   String,
}

/// Status bar messages and transient notifications.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MessageStrings {
    pub saved:                 String,
    /// `{err}` — error description
    pub save_failed:           String,
    /// Shown when :w / :wq is used but the buffer has no associated file path.
    pub no_file_name:          String,
    pub unsaved_changes:       String,
    /// `{path}` — file path
    pub file_not_found:        String,
    /// `{name}` — theme name
    pub theme_saved:           String,
    /// `{name}`, `{err}`
    pub theme_save_failed:     String,
    pub ai_thinking_plan:      String,
    pub ai_thinking_advisor:   String,
    /// `{n}` — step count
    pub ai_plan_steps_yolo:    String,
    /// `{n}`
    pub ai_plan_steps_confirm: String,
    /// `{n}`
    pub ai_plan_applied:       String,
    /// `{err}`
    pub ai_plan_failed:        String,
    pub ai_plan_cancelled:     String,
    /// `{err}`
    pub ai_error:              String,
    /// `{reg}` — register letter
    pub macro_not_found:       String,
    /// `{path}` — temp file path
    pub preview_opened:        String,
    pub preview_not_markdown:  String,
    /// `{err}`
    pub preview_write_failed:  String,
    /// `{err}`
    pub preview_open_failed:   String,
    /// `{err}`
    pub shell_error:           String,
    pub chat_cleared:          String,
    pub mouse_hint:            String,
}

/// Command-mode completion descriptions (one per `:command`).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CommandStrings {
    pub cmd_w:           String,
    pub cmd_q:           String,
    pub cmd_q_force:     String,
    pub cmd_wq:          String,
    pub cmd_x:           String,
    pub cmd_e:           String,
    pub cmd_e_reload:    String,
    pub cmd_w_saveas:    String,
    pub cmd_set_nu:      String,
    pub cmd_set_nonu:    String,
    pub cmd_set_tabstop: String,
    pub cmd_noh:         String,
    pub cmd_theme:       String,
    pub cmd_theme_name:  String,
    pub cmd_u:           String,
    pub cmd_d:           String,
    pub cmd_s:           String,
    pub cmd_percent_s:   String,
    pub cmd_shell:       String,
    pub cmd_preview:     String,
    pub cmd_filetree:    String,
    pub cmd_grep:        String,
    pub cmd_tutorial:    String,
    pub cmd_mouse:       String,
}

/// AI system prompt strings.
///
/// `product_guide` is injected into every system prompt.
/// The `role_*` strings are appended per prompt kind and may contain
/// `{file_info}` and `{instruction}` placeholders.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AiStrings {
    pub product_guide:  String,
    pub role_advisor:   String,
    pub role_plan:      String,
    pub role_complete:  String,
    pub role_transform: String,
}

// ── Default impls (hard-coded en-US fallback) ─────────────────────────────────

impl Default for Locale {
    fn default() -> Self {
        // Use hard-coded field defaults — do NOT call loader here.
        // loader::parse_or_default() calls Locale::default() on parse failure,
        // so calling the loader from here would cause infinite recursion.
        Self {
            ui:       UiStrings::default(),
            messages: MessageStrings::default(),
            commands: CommandStrings::default(),
            ai:       AiStrings::default(),
        }
    }
}

impl Default for UiStrings {
    fn default() -> Self {
        Self {
            hint_normal:          "[i]insert  [v]select  [dd]del line  [yy]yank  [p]paste  [.]repeat  [u]undo  [?]AI  [Ctrl+l]chat".into(),
            hint_normal_empty:    "[i]insert here  [o]new line below  [O]new line above  [dd]delete  [?]AI".into(),
            hint_normal_comment:  "[yy]yank line  [dd]delete line  [A]append  [I]insert at start  [?]AI".into(),
            hint_normal_tag:      "[cit]change tag  [dit]delete tag  [vat]select tag  [?]AI".into(),
            hint_normal_url:      "[gf]open file  [yiw]yank path  [ciw]replace path  [?]AI".into(),
            hint_normal_number:   "[Ctrl+a]increment  [Ctrl+x]decrement  [ciw]change  [?]AI".into(),
            hint_normal_string:   "[ci\"]change string  [di\"]delete  [yi\"]yank  [?]AI".into(),
            hint_normal_word:     "[ciw]change word  [diw]delete  [yiw]yank  [*]search  [?]AI".into(),
            hint_normal_macro:    "● Recording macro @{reg}  [q]stop".into(),
            hint_normal_register: "[a-z]select register".into(),
            hint_normal_search:   "Search active  [n]next  [N]prev  [:noh]clear".into(),
            hint_insert:          "Typing...  [Esc]normal mode  [Ctrl+w]del word".into(),
            hint_visual:          "[y]yank  [d]delete  [c]change  [>]indent  [?]AI  [Esc]exit".into(),
            hint_command:         ":w save  :q quit  :wq save+quit  :filetree toggle tree  :tut tutorial  [Esc]cancel".into(),
            hint_search:          "Type pattern, Enter to search  [Esc]cancel".into(),
            hint_ai:              "Describe your intent, Enter to send  [Esc]cancel".into(),
            hint_filetree:        "[j/k]navigate  [l/Enter]open  [h]collapse  [Ctrl+w/Esc]back".into(),
            hint_switch_zone:     "[Ctrl+w]switch panel".into(),
            theme_picker_title:   "Theme  j/k Enter Esc".into(),
        }
    }
}

impl Default for MessageStrings {
    fn default() -> Self {
        Self {
            saved:                 "Saved".into(),
            save_failed:           "Save failed: {err}".into(),
            no_file_name:          "No file name \u{2014} use :w <filename> to save".into(),
            unsaved_changes:       "Unsaved changes! Use :q! to force quit or :wq to save".into(),
            file_not_found:        "File not found: {path}".into(),
            theme_saved:           "Theme: {name} (saved)".into(),
            theme_save_failed:     "Theme: {name} (save failed: {err})".into(),
            ai_thinking_plan:      "AI thinking… [plan mode]  [Esc]cancel".into(),
            ai_thinking_advisor:   "AI thinking… [advisor mode]  [Esc]cancel".into(),
            ai_plan_steps_yolo:    "AI auto-applying {n} steps (yolo)".into(),
            ai_plan_steps_confirm: "AI plan: {n} steps — [y]confirm  [n]cancel".into(),
            ai_plan_applied:       "Plan applied: {n} steps".into(),
            ai_plan_failed:        "Plan failed: {err}".into(),
            ai_plan_cancelled:     "Plan cancelled".into(),
            ai_error:              "AI error: {err}".into(),
            macro_not_found:       "Macro @{reg} not found".into(),
            preview_opened:        "Preview opened: {path}".into(),
            preview_not_markdown:  "Preview only supports Markdown files (.md)".into(),
            preview_write_failed:  "Failed to write preview: {err}".into(),
            preview_open_failed:   "Failed to open browser: {err}".into(),
            shell_error:           "Shell error: {err}".into(),
            chat_cleared:          "Chat history cleared".into(),
            mouse_hint:            "Drop the mouse! Use :mouse to enter mouse mode".into(),
        }
    }
}

impl Default for CommandStrings {
    fn default() -> Self {
        Self {
            cmd_w:           "Save file".into(),
            cmd_q:           "Quit".into(),
            cmd_q_force:     "Force quit (discard changes)".into(),
            cmd_wq:          "Save and quit".into(),
            cmd_x:           "Save and quit".into(),
            cmd_e:           "Open file".into(),
            cmd_e_reload:    "Reload current file".into(),
            cmd_w_saveas:    "Save as…".into(),
            cmd_set_nu:      "Show line numbers".into(),
            cmd_set_nonu:    "Hide line numbers".into(),
            cmd_set_tabstop: "Set tab width".into(),
            cmd_noh:         "Clear search highlight".into(),
            cmd_theme:       "Open theme picker".into(),
            cmd_theme_name:  "Switch theme by name".into(),
            cmd_u:           "Undo".into(),
            cmd_d:           "Delete current line".into(),
            cmd_s:           "Substitute (current line) s/pat/rep/".into(),
            cmd_percent_s:   "Substitute all  %s/pat/rep/g".into(),
            cmd_shell:       "Run shell command".into(),
            cmd_preview:     "Preview Markdown in browser".into(),
            cmd_filetree:    "Toggle file tree sidebar".into(),
            cmd_grep:        "Search in project".into(),
            cmd_tutorial:    "Toggle tutorial board".into(),
            cmd_mouse:       "Toggle mouse mode".into(),
        }
    }
}

impl Default for AiStrings {
    fn default() -> Self {
        Self {
            product_guide:  "# hi — Terminal Text Editor\n\nYou are the built-in AI assistant of `hi`.".into(),
            role_advisor:   "## Your Role: Advisor\n\n{file_info}\n\nAnswer the user's question concisely.".into(),
            role_plan:      "## Your Role: Edit Planner\n\n{file_info}\n\nOutput numbered edit steps only.".into(),
            role_complete:  "You are the inline completion engine.\n\n{file_info}\n\nOutput ONLY the completion text.".into(),
            role_transform: "You are the code transformation engine.\n\n{file_info}\n\nTask: {instruction}\nReturn ONLY the transformed code.".into(),
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

impl Locale {
    /// Load locale for the given language tag (e.g. `"zh-CN"`, `"en-US"`).
    /// Falls back to en-US if the requested language is not available.
    pub fn load(lang: &str) -> Self {
        loader::load(lang)
    }

    /// Auto-detect language from environment variables, then load.
    pub fn auto() -> Self {
        let lang = detect_language_from_env();
        loader::load(&lang)
    }
}

/// Detect language tag from `LANG` / `LC_ALL` / `LC_MESSAGES`.
/// Returns `"zh-CN"`, `"en-US"`, etc.  Defaults to `"en-US"` if unrecognised.
pub fn detect_language_from_env() -> String {
    let raw = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .unwrap_or_default();
    detect_language_from_env_with(&raw)
}

/// Pure-function version for testing — takes the raw `LANG` string directly.
pub fn detect_language_from_env_with(raw: &str) -> String {
    // raw is like "zh_CN.UTF-8" or "en_US.UTF-8"
    let base = raw.split('.').next().unwrap_or("").replace('_', "-");
    match base.as_str() {
        s if s.starts_with("zh") => "zh-CN".to_string(),
        s if s.starts_with("en") || s.is_empty() => "en-US".to_string(),
        other => other.to_string(), // pass through for community locales
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_en_loads() {
        let locale = Locale::load("en-US");
        assert_eq!(locale.messages.saved, "Saved");
        assert!(!locale.ui.hint_normal.is_empty());
        assert!(!locale.ai.product_guide.is_empty());
    }

    #[test]
    fn test_locale_zh_loads() {
        let locale = Locale::load("zh-CN");
        assert_eq!(locale.messages.saved, "已保存");
        assert!(locale.ui.hint_normal.contains("插入"));
    }

    #[test]
    fn test_locale_fallback_to_en() {
        // Unknown language tag falls back to en-US
        let locale = Locale::load("xx-XX");
        assert_eq!(locale.messages.saved, "Saved");
    }

    #[test]
    fn test_detect_from_lang_env() {
        assert_eq!(detect_language_from_env_with("zh_CN.UTF-8"), "zh-CN");
        assert_eq!(detect_language_from_env_with("zh_TW.UTF-8"), "zh-CN");
        assert_eq!(detect_language_from_env_with("en_US.UTF-8"), "en-US");
        assert_eq!(detect_language_from_env_with("ru_RU.UTF-8"), "ru-RU");
        assert_eq!(detect_language_from_env_with(""),             "en-US");
    }

    #[test]
    fn test_message_placeholder_format() {
        let locale = Locale::load("en-US");
        let msg = locale.messages.theme_saved.replace("{name}", "dracula");
        assert_eq!(msg, "Theme: dracula (saved)");

        let locale_zh = Locale::load("zh-CN");
        let msg_zh = locale_zh.messages.theme_saved.replace("{name}", "dracula");
        assert!(msg_zh.contains("dracula"));
        assert!(msg_zh.contains("已保存"));
    }

    #[test]
    fn test_ai_prompts_language_separation() {
        let en = Locale::load("en-US");
        let zh = Locale::load("zh-CN");
        assert!(en.ai.product_guide.contains("terminal text editor"));
        assert!(!en.ai.product_guide.contains("终端文本编辑器"));
        assert!(zh.ai.product_guide.contains("终端文本编辑器"));
        assert!(!zh.ai.product_guide.contains("terminal text editor"));
    }
}
