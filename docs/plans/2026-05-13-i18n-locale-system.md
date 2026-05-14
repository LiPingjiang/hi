# i18n Locale System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use super-assistant:subagent-driven-development to implement this plan task-by-task.

**Goal:** Extract all user-visible strings from the codebase into external TOML locale files, with `zh-CN` and `en-US` bundled in the binary, auto-detection from the system locale, and a fallback chain so community-contributed translations work even when incomplete.

**Architecture:** A `Locale` struct is loaded once at startup and passed into `App` alongside `Config`. All user-visible strings (hint bar, status messages, command descriptions, AI system prompts) are read from `locale.key` instead of being hardcoded. Two canonical locale files (`zh-CN.toml` and `en-US.toml`) are embedded via `include_str!` so the binary works with zero external files. Users can drop additional `.toml` files into `~/.config/hi/locales/` to add or override any language.

**Tech Stack:** Rust, `serde` + `toml` (already in Cargo.toml), `std::env::var("LANG")` for auto-detection, `include_str!` for bundled locales.

**Execution Config:**
```yaml
confirm_after_each_task: false
skip_spec_review: false
skip_quality_review: false
parallel_tasks: 1
```

---

## Overview of string categories

| Category | Location | Count (approx) |
|---|---|---|
| Hint bar strings | `src/ui/statusbar.rs` | ~15 strings |
| Status messages | `src/app.rs` (set_msg calls) | ~20 strings |
| Command descriptions | `src/mode/cmd_completion.rs` | ~20 strings |
| AI system prompts | `src/ai/prompt.rs` | 4 prompts (PRODUCT_GUIDE + 4 roles) |
| Theme picker title | `src/ui/renderer.rs` | 1 string |
| Preview messages | `src/ui/preview.rs` | 3 strings |

---

## Task 1: Define the `Locale` struct and TOML schema

**Files:**
- Create: `src/locale/mod.rs`
- Create: `src/locale/loader.rs`
- Modify: `src/lib.rs` (add `pub mod locale;`)

**Step 1: Write the failing test**

```rust
// src/locale/mod.rs (partial, for test)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_en_loads() {
        let locale = Locale::load("en-US");
        assert_eq!(locale.messages.saved, "Saved");
        assert!(!locale.ui.hint_normal.is_empty());
    }

    #[test]
    fn test_locale_zh_loads() {
        let locale = Locale::load("zh-CN");
        assert!(!locale.messages.saved.is_empty());
    }

    #[test]
    fn test_locale_fallback_to_en() {
        // Unknown language falls back to en-US
        let locale = Locale::load("xx-XX");
        assert_eq!(locale.messages.saved, "Saved");
    }

    #[test]
    fn test_detect_from_lang_env() {
        // LANG=zh_CN.UTF-8 -> "zh-CN"
        assert_eq!(detect_language_from_env_with("zh_CN.UTF-8"), "zh-CN");
        assert_eq!(detect_language_from_env_with("en_US.UTF-8"), "en-US");
        assert_eq!(detect_language_from_env_with("ru_RU.UTF-8"), "ru-RU");
        assert_eq!(detect_language_from_env_with(""), "en-US");
    }
}
```

**Step 2: Run test to verify it fails**

```bash
cd /Users/lipingjiang/Codes/hi && cargo test locale 2>&1 | head -20
```
Expected: FAIL — `locale` module not found.

**Step 3: Implement `src/locale/mod.rs`**

```rust
//! Locale system: load user-visible strings from TOML files.
//!
//! Priority order:
//!   1. ~/.config/hi/locales/{lang}.toml  (user override)
//!   2. Bundled en-US / zh-CN (compiled into binary via include_str!)
//!   3. Hard-coded en-US defaults (final fallback, never panics)

pub mod loader;

use serde::Deserialize;

/// All user-visible strings, grouped by subsystem.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Locale {
    pub ui: UiStrings,
    pub messages: MessageStrings,
    pub commands: CommandStrings,
    pub ai: AiStrings,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UiStrings {
    // Hint bar
    pub hint_normal:        String,
    pub hint_normal_empty:  String,
    pub hint_normal_comment:String,
    pub hint_normal_tag:    String,
    pub hint_normal_url:    String,
    pub hint_normal_number: String,
    pub hint_normal_string: String,
    pub hint_normal_word:   String,
    pub hint_normal_macro:  String,
    pub hint_normal_register: String,
    pub hint_normal_search: String,
    pub hint_insert:        String,
    pub hint_visual:        String,
    pub hint_command:       String,
    pub hint_search:        String,
    pub hint_ai:            String,
    pub hint_filetree:      String,
    // Theme picker
    pub theme_picker_title: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MessageStrings {
    pub saved:                  String,
    pub save_failed:            String,
    pub unsaved_changes:        String,
    pub file_not_found:         String,
    pub theme_saved:            String,   // "{name} (saved)"
    pub theme_save_failed:      String,   // "{name} (save failed: {err})"
    pub ai_thinking_plan:       String,   // "AI 思考中… [计划模式]"
    pub ai_thinking_advisor:    String,
    pub ai_plan_steps_yolo:     String,   // "AI 自动执行 {n} 步 (yolo)"
    pub ai_plan_steps_confirm:  String,   // "AI 计划 {n} 步 — [y]确认 [n]取消"
    pub ai_plan_applied:        String,   // "计划已应用 {n} 步"
    pub ai_plan_failed:         String,
    pub ai_plan_cancelled:      String,
    pub ai_error:               String,
    pub macro_not_found:        String,
    pub preview_opened:         String,
    pub preview_not_markdown:   String,
    pub preview_write_failed:   String,
    pub preview_open_failed:    String,
    pub shell_error:            String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CommandStrings {
    pub cmd_w:          String,
    pub cmd_q:          String,
    pub cmd_q_force:    String,
    pub cmd_wq:         String,
    pub cmd_x:          String,
    pub cmd_e:          String,
    pub cmd_e_reload:   String,
    pub cmd_w_saveas:   String,
    pub cmd_set_nu:     String,
    pub cmd_set_nonu:   String,
    pub cmd_set_tabstop:String,
    pub cmd_noh:        String,
    pub cmd_theme:      String,
    pub cmd_theme_name: String,
    pub cmd_u:          String,
    pub cmd_d:          String,
    pub cmd_s:          String,
    pub cmd_percent_s:  String,
    pub cmd_shell:      String,
    pub cmd_preview:    String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AiStrings {
    /// Full PRODUCT_GUIDE injected into every system prompt.
    pub product_guide:          String,
    /// Role instruction appended for Advisor mode.
    pub role_advisor:           String,
    /// Role instruction appended for Plan mode.
    pub role_plan:              String,
    /// Role instruction for Complete mode (replaces product_guide).
    pub role_complete:          String,
    /// Role instruction for Transform mode.
    pub role_transform:         String,
}

impl Locale {
    /// Load locale for the given language tag (e.g. "zh-CN", "en-US").
    /// Falls back to en-US if the requested language is not available.
    pub fn load(lang: &str) -> Self {
        loader::load(lang)
    }

    /// Auto-detect language from environment, then load.
    pub fn auto() -> Self {
        let lang = detect_language_from_env();
        loader::load(&lang)
    }
}

/// Detect language tag from LANG / LC_ALL / LC_MESSAGES environment variables.
/// Returns "zh-CN", "en-US", etc. Defaults to "en-US" if unrecognised.
pub fn detect_language_from_env() -> String {
    let raw = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .unwrap_or_default();
    detect_language_from_env_with(&raw)
}

/// Pure function version for testing (takes the raw LANG string directly).
pub fn detect_language_from_env_with(raw: &str) -> String {
    // raw is like "zh_CN.UTF-8" or "en_US.UTF-8"
    let base = raw.split('.').next().unwrap_or("").replace('_', "-");
    match base.as_str() {
        "zh-CN" | "zh-TW" | "zh-HK" => "zh-CN".to_string(),
        s if s.starts_with("zh") => "zh-CN".to_string(),
        s if s.starts_with("en") => "en-US".to_string(),
        "" => "en-US".to_string(),
        other => other.to_string(),  // pass through for community locales
    }
}
```

**Step 4: Implement `src/locale/loader.rs`**

```rust
//! Locale file loading with priority chain and fallback.

use super::Locale;

/// Bundled locale files compiled into the binary.
const LOCALE_ZH_CN: &str = include_str!("../../locales/zh-CN.toml");
const LOCALE_EN_US: &str = include_str!("../../locales/en-US.toml");

/// Load a locale by language tag.
/// Priority: user file > bundled > en-US fallback.
pub fn load(lang: &str) -> Locale {
    // 1. Try user override: ~/.config/hi/locales/{lang}.toml
    if let Some(user_locale) = load_user_file(lang) {
        return user_locale;
    }
    // 2. Try bundled locales
    match lang {
        "zh-CN" => parse_or_default(LOCALE_ZH_CN),
        "en-US" => parse_or_default(LOCALE_EN_US),
        _ => {
            // Unknown language: fall back to en-US
            parse_or_default(LOCALE_EN_US)
        }
    }
}

fn load_user_file(lang: &str) -> Option<Locale> {
    let path = dirs::config_dir()?
        .join("hi")
        .join("locales")
        .join(format!("{}.toml", lang));
    let content = std::fs::read_to_string(&path).ok()?;
    // Merge with en-US defaults so partial translations work
    let base: Locale = parse_or_default(LOCALE_EN_US);
    let overlay: Locale = toml::from_str(&content).ok()?;
    Some(merge(base, overlay))
}

fn parse_or_default(content: &str) -> Locale {
    toml::from_str(content).unwrap_or_default()
}

/// Merge `overlay` on top of `base`: any non-empty string in overlay wins.
/// This allows partial locale files (community translations) to work correctly.
fn merge(base: Locale, overlay: Locale) -> Locale {
    use std::mem;
    macro_rules! merge_str {
        ($b:expr, $o:expr) => {
            if $o.is_empty() { $b } else { $o }
        };
    }
    // We use a field-by-field merge. This is verbose but explicit and zero-cost.
    Locale {
        ui: crate::locale::UiStrings {
            hint_normal:         merge_str!(base.ui.hint_normal,         overlay.ui.hint_normal),
            hint_normal_empty:   merge_str!(base.ui.hint_normal_empty,   overlay.ui.hint_normal_empty),
            hint_normal_comment: merge_str!(base.ui.hint_normal_comment, overlay.ui.hint_normal_comment),
            hint_normal_tag:     merge_str!(base.ui.hint_normal_tag,     overlay.ui.hint_normal_tag),
            hint_normal_url:     merge_str!(base.ui.hint_normal_url,     overlay.ui.hint_normal_url),
            hint_normal_number:  merge_str!(base.ui.hint_normal_number,  overlay.ui.hint_normal_number),
            hint_normal_string:  merge_str!(base.ui.hint_normal_string,  overlay.ui.hint_normal_string),
            hint_normal_word:    merge_str!(base.ui.hint_normal_word,    overlay.ui.hint_normal_word),
            hint_normal_macro:   merge_str!(base.ui.hint_normal_macro,   overlay.ui.hint_normal_macro),
            hint_normal_register:merge_str!(base.ui.hint_normal_register,overlay.ui.hint_normal_register),
            hint_normal_search:  merge_str!(base.ui.hint_normal_search,  overlay.ui.hint_normal_search),
            hint_insert:         merge_str!(base.ui.hint_insert,         overlay.ui.hint_insert),
            hint_visual:         merge_str!(base.ui.hint_visual,         overlay.ui.hint_visual),
            hint_command:        merge_str!(base.ui.hint_command,        overlay.ui.hint_command),
            hint_search:         merge_str!(base.ui.hint_search,         overlay.ui.hint_search),
            hint_ai:             merge_str!(base.ui.hint_ai,             overlay.ui.hint_ai),
            hint_filetree:       merge_str!(base.ui.hint_filetree,       overlay.ui.hint_filetree),
            theme_picker_title:  merge_str!(base.ui.theme_picker_title,  overlay.ui.theme_picker_title),
        },
        messages: crate::locale::MessageStrings {
            saved:               merge_str!(base.messages.saved,               overlay.messages.saved),
            save_failed:         merge_str!(base.messages.save_failed,         overlay.messages.save_failed),
            unsaved_changes:     merge_str!(base.messages.unsaved_changes,     overlay.messages.unsaved_changes),
            file_not_found:      merge_str!(base.messages.file_not_found,      overlay.messages.file_not_found),
            theme_saved:         merge_str!(base.messages.theme_saved,         overlay.messages.theme_saved),
            theme_save_failed:   merge_str!(base.messages.theme_save_failed,   overlay.messages.theme_save_failed),
            ai_thinking_plan:    merge_str!(base.messages.ai_thinking_plan,    overlay.messages.ai_thinking_plan),
            ai_thinking_advisor: merge_str!(base.messages.ai_thinking_advisor, overlay.messages.ai_thinking_advisor),
            ai_plan_steps_yolo:  merge_str!(base.messages.ai_plan_steps_yolo,  overlay.messages.ai_plan_steps_yolo),
            ai_plan_steps_confirm:merge_str!(base.messages.ai_plan_steps_confirm,overlay.messages.ai_plan_steps_confirm),
            ai_plan_applied:     merge_str!(base.messages.ai_plan_applied,     overlay.messages.ai_plan_applied),
            ai_plan_failed:      merge_str!(base.messages.ai_plan_failed,      overlay.messages.ai_plan_failed),
            ai_plan_cancelled:   merge_str!(base.messages.ai_plan_cancelled,   overlay.messages.ai_plan_cancelled),
            ai_error:            merge_str!(base.messages.ai_error,            overlay.messages.ai_error),
            macro_not_found:     merge_str!(base.messages.macro_not_found,     overlay.messages.macro_not_found),
            preview_opened:      merge_str!(base.messages.preview_opened,      overlay.messages.preview_opened),
            preview_not_markdown:merge_str!(base.messages.preview_not_markdown,overlay.messages.preview_not_markdown),
            preview_write_failed:merge_str!(base.messages.preview_write_failed,overlay.messages.preview_write_failed),
            preview_open_failed: merge_str!(base.messages.preview_open_failed, overlay.messages.preview_open_failed),
            shell_error:         merge_str!(base.messages.shell_error,         overlay.messages.shell_error),
        },
        commands: crate::locale::CommandStrings {
            cmd_w:           merge_str!(base.commands.cmd_w,           overlay.commands.cmd_w),
            cmd_q:           merge_str!(base.commands.cmd_q,           overlay.commands.cmd_q),
            cmd_q_force:     merge_str!(base.commands.cmd_q_force,     overlay.commands.cmd_q_force),
            cmd_wq:          merge_str!(base.commands.cmd_wq,          overlay.commands.cmd_wq),
            cmd_x:           merge_str!(base.commands.cmd_x,           overlay.commands.cmd_x),
            cmd_e:           merge_str!(base.commands.cmd_e,           overlay.commands.cmd_e),
            cmd_e_reload:    merge_str!(base.commands.cmd_e_reload,    overlay.commands.cmd_e_reload),
            cmd_w_saveas:    merge_str!(base.commands.cmd_w_saveas,    overlay.commands.cmd_w_saveas),
            cmd_set_nu:      merge_str!(base.commands.cmd_set_nu,      overlay.commands.cmd_set_nu),
            cmd_set_nonu:    merge_str!(base.commands.cmd_set_nonu,    overlay.commands.cmd_set_nonu),
            cmd_set_tabstop: merge_str!(base.commands.cmd_set_tabstop, overlay.commands.cmd_set_tabstop),
            cmd_noh:         merge_str!(base.commands.cmd_noh,         overlay.commands.cmd_noh),
            cmd_theme:       merge_str!(base.commands.cmd_theme,       overlay.commands.cmd_theme),
            cmd_theme_name:  merge_str!(base.commands.cmd_theme_name,  overlay.commands.cmd_theme_name),
            cmd_u:           merge_str!(base.commands.cmd_u,           overlay.commands.cmd_u),
            cmd_d:           merge_str!(base.commands.cmd_d,           overlay.commands.cmd_d),
            cmd_s:           merge_str!(base.commands.cmd_s,           overlay.commands.cmd_s),
            cmd_percent_s:   merge_str!(base.commands.cmd_percent_s,   overlay.commands.cmd_percent_s),
            cmd_shell:       merge_str!(base.commands.cmd_shell,       overlay.commands.cmd_shell),
            cmd_preview:     merge_str!(base.commands.cmd_preview,     overlay.commands.cmd_preview),
        },
        ai: crate::locale::AiStrings {
            product_guide:   merge_str!(base.ai.product_guide,   overlay.ai.product_guide),
            role_advisor:    merge_str!(base.ai.role_advisor,    overlay.ai.role_advisor),
            role_plan:       merge_str!(base.ai.role_plan,       overlay.ai.role_plan),
            role_complete:   merge_str!(base.ai.role_complete,   overlay.ai.role_complete),
            role_transform:  merge_str!(base.ai.role_transform,  overlay.ai.role_transform),
        },
    }
}
```

**Step 5: Wire into `src/lib.rs`**

Add `pub mod locale;` after the existing module declarations.

**Step 6: Run test to verify it passes**

```bash
cd /Users/lipingjiang/Codes/hi && cargo test locale::tests 2>&1 | tail -15
```
Expected: PASS (after locale files are created in Task 2).

**Step 7: Commit**

```bash
git add src/locale/ src/lib.rs
git commit -m "feat(locale): add Locale struct, loader, and env-based language detection"
```

---

## Task 2: Write the two canonical locale files

**Files:**
- Create: `locales/zh-CN.toml`
- Create: `locales/en-US.toml`

These files are the source of truth. They must be complete — every key must be present. The `include_str!` in `loader.rs` references these paths relative to the crate root.

**Step 1: Create `locales/zh-CN.toml`**

```toml
# locales/zh-CN.toml
# Official Simplified Chinese locale for hi editor.
# Maintained by the hi core team.

[ui]
hint_normal          = "[i]插入  [v]选择  [dd]删行  [yy]复制行  [p]粘贴  [.]重复  [u]撤销  [?]AI  [Ctrl+l]对话面板"
hint_normal_empty    = "[i]在此插入  [o]下方新行  [O]上方新行  [dd]删除空行  [?]AI"
hint_normal_comment  = "[gcc]切换注释  [yy]复制注释  [dd]删除注释  [A]行尾追加  [?]AI"
hint_normal_tag      = "[cit]修改tag内容  [dit]删除tag内容  [vat]选中含tag  [ci\"]修改属性值  [?]AI"
hint_normal_url      = "[gf]打开文件  [yiw]复制路径  [ciw]替换路径  [?]AI"
hint_normal_number   = "[Ctrl+a]数字+1  [Ctrl+x]数字-1  [ciw]修改数字  [yiw]复制数字  [?]AI"
hint_normal_string   = "[ci\"]修改字符串内容  [di\"]删除内容  [yi\"]复制内容  [va\"]选中含引号  [?]AI"
hint_normal_word     = "[ciw]修改单词  [diw]删除单词  [yiw]复制单词  [*]搜索此词  [?]AI"
hint_normal_macro    = "● 录制宏 @{reg}  [q]停止录制  操作将被记录"
hint_normal_register = "[a-z]选择寄存器  \"ayy→复制到a  \"ap→从a粘贴  \"+y→系统剪贴板"
hint_normal_search   = "搜索高亮中  [n]下一个  [N]上一个  [/]新搜索  [:noh]清除高亮"
hint_insert          = "正在输入...  [Esc]返回Normal  [Ctrl+w]删词  [Ctrl+u]删至行首"
hint_visual          = "[y]复制  [d]删除  [c]替换  [>]缩进  [<]反缩进  [?]AI操作选区  [Esc]退出"
hint_command         = ":w保存  :q退出  :wq保存退出  :%s/查找/替换/g  [Esc]取消"
hint_search          = "输入搜索词，Enter确认  n/N跳转  [Esc]取消"
hint_ai              = "描述你的意图，按Enter发送  [Tab]确认建议  [Esc]取消  示例：把所有ERROR替换为WARN"
hint_filetree        = "[j/k]上下移动  [l/Enter]打开/展开  [h]折叠  [g/G]顶/底  [H]显隐文件  [Ctrl+w/Esc]返回编辑"
theme_picker_title   = "选择主题 j/k Enter Esc"

[messages]
saved                = "已保存"
save_failed          = "保存失败: {err}"
unsaved_changes      = "未保存的修改！用 :q! 强制退出或 :wq 保存退出"
file_not_found       = "文件未找到: {path}"
theme_saved          = "主题: {name} (已保存)"
theme_save_failed    = "主题: {name} (保存失败: {err})"
ai_thinking_plan     = "AI 思考中… [计划模式]  [Esc]取消"
ai_thinking_advisor  = "AI 思考中… [顾问模式]  [Esc]取消"
ai_plan_steps_yolo   = "AI 自动执行 {n} 步 (yolo)"
ai_plan_steps_confirm= "AI 计划 {n} 步 — [y]确认  [n]取消"
ai_plan_applied      = "计划已应用 {n} 步"
ai_plan_failed       = "计划执行失败: {err}"
ai_plan_cancelled    = "计划已取消"
ai_error             = "AI 错误: {err}"
macro_not_found      = "宏 @{reg} 不存在"
preview_opened       = "预览已打开: {path}"
preview_not_markdown = "预览仅支持 Markdown 文件 (.md)"
preview_write_failed = "预览文件写入失败: {err}"
preview_open_failed  = "浏览器打开失败: {err}"
shell_error          = "Shell 错误: {err}"

[commands]
cmd_w           = "保存文件"
cmd_q           = "退出"
cmd_q_force     = "强制退出（不保存）"
cmd_wq          = "保存并退出"
cmd_x           = "保存并退出"
cmd_e           = "打开文件"
cmd_e_reload    = "重新加载当前文件"
cmd_w_saveas    = "另存为…"
cmd_set_nu      = "显示行号"
cmd_set_nonu    = "隐藏行号"
cmd_set_tabstop = "设置 Tab 宽度"
cmd_noh         = "清除搜索高亮"
cmd_theme       = "打开主题选择器"
cmd_theme_name  = "切换主题（输入名称）"
cmd_u           = "撤销"
cmd_d           = "删除当前行"
cmd_s           = "替换（当前行）s/pat/rep/"
cmd_percent_s   = "全文替换 %s/pat/rep/g"
cmd_shell       = "执行 Shell 命令"
cmd_preview     = "浏览器预览 Markdown"

[ai]
product_guide = """
# hi — 终端文本编辑器

你是 `hi` 的内置 AI 助手，`hi` 是一款用 Rust 编写的 Vim 风格终端文本编辑器。
你的名字是 `hi assistant`。你不是 OpenClaw、ChatGPT、Claude 或任何其他 AI 产品。
当被问及你是谁时，请说你是嵌入在 `hi` 编辑器中的 AI 助手。

## 模式

hi 有 6 种模式，当前模式显示在状态栏中：
- NORMAL：默认的导航和命令模式
- INSERT：文本输入模式
- VISUAL / V-LINE / V-BLOCK：选择模式（字符 / 行 / 块）
- COMMAND：Ex 命令（`:` 前缀）
- SEARCH：增量搜索（`/` 前缀）
- AI：AI 查询输入（在编辑器中按 `?`，或直接在对话面板中输入）

## 焦点区域

UI 有三个焦点区域，用 Tab 或 Ctrl+w 循环切换：
- 编辑器（主文本区域）
- 文件树（左侧边栏，用 Ctrl+t 切换）
- 对话（右侧 AI 面板，用 Ctrl+l 切换）

## Normal 模式快捷键

移动：
  h/j/k/l 或方向键 — 左/下/上/右
  w/b/e — 单词前进 / 后退 / 末尾
  0/^/$ — 行首 / 第一个非空字符 / 行尾
  gg/G — 文件顶部 / 底部
  Ctrl+d/u — 半页下 / 上

编辑：
  x — 删除光标处字符
  dd — 删除行
  yy — 复制行
  p/P — 在后 / 前粘贴
  u / Ctrl+r — 撤销 / 重做
  . — 重复上次编辑

模式切换：
  i/a/I/A — 进入插入模式
  v/V/Ctrl+v — 进入可视模式
  : — 进入命令模式
  / — 进入搜索模式
  ? — 进入 AI 查询模式

## 命令模式（: 前缀）

  :w — 保存  :q — 退出  :wq — 保存退出
  :%s/查找/替换/g — 全文替换
  :theme — 打开主题选择器
  :preview — 在浏览器中预览 Markdown

## AI 功能

用户通过两种方式与 AI 交互：
1. 编辑器 AI 模式：在 Normal 模式按 ?，输入查询，Enter 发送
   - 前缀 ?! 用于编辑计划（AI 返回编号的编辑步骤）
   - 在 Visual 模式下，? 将选中文本作为上下文发送
2. 对话面板：聚焦对话（Tab/Ctrl+w），按 i 或 Enter 输入，Enter 发送

## 配置（~/.hirc，TOML 格式）

[general] — line_numbers, tab_width
[ai] — api_base_url, api_key, model, yolo_mode
[theme] — editor_theme, chat_theme
[general] — language = "auto"  # auto | zh-CN | en-US | ru-RU
"""

role_advisor = """
## 你的角色：顾问

{file_info}

你正在回答用户关于其代码或编辑器的问题。
请简洁精确，优先使用代码示例而非冗长的文字说明。
当用户询问编辑器用法时，请参考上方的快捷键说明。
除非用户要求格式化输出，否则不要使用 Markdown 代码围栏。
用用户使用的语言回复（用户用中文则用中文，用英文则用英文）。
"""

role_plan = """
## 你的角色：编辑计划生成器

{file_info}

当被要求进行修改时，请用编号列表回复原子编辑步骤。
每个步骤必须是以下之一：
- INSERT line <N>: <text>  （0-based 行索引）
- DELETE line <N>
- REPLACE line <N>: <new text>
- REPLACE range <N>-<M>: <new text（多行用 \\n）>
- MESSAGE: <无需编辑的建议>
只输出编号步骤，不要有其他文字。
"""

role_complete = """
你是 `hi` 终端文本编辑器的内联补全引擎。

{file_info}

在光标位置生成简短、自然的代码续写。
只输出补全文本——不要解释，不要 Markdown 围栏，不要前缀。
匹配现有代码风格、缩进和命名规范。
如果不确定，宁可不输出也不要猜错。
"""

role_transform = """
你是 `hi` 终端文本编辑器的代码转换引擎。

{file_info}

任务：{instruction}
只返回转换后的代码，保持缩进。
不要 Markdown 围栏，不要解释，不要多余文字。
"""
```

**Step 2: Create `locales/en-US.toml`**

```toml
# locales/en-US.toml
# Official English (US) locale for hi editor.
# Maintained by the hi core team.
# This file is the reference for community translations.

[ui]
hint_normal          = "[i]insert  [v]select  [dd]del line  [yy]yank  [p]paste  [.]repeat  [u]undo  [?]AI  [Ctrl+l]chat"
hint_normal_empty    = "[i]insert here  [o]new line below  [O]new line above  [dd]delete  [?]AI"
hint_normal_comment  = "[gcc]toggle comment  [yy]yank  [dd]delete  [A]append  [?]AI"
hint_normal_tag      = "[cit]change tag  [dit]delete tag  [vat]select tag  [ci\"]change attr  [?]AI"
hint_normal_url      = "[gf]open file  [yiw]yank path  [ciw]replace path  [?]AI"
hint_normal_number   = "[Ctrl+a]increment  [Ctrl+x]decrement  [ciw]change  [yiw]yank  [?]AI"
hint_normal_string   = "[ci\"]change string  [di\"]delete  [yi\"]yank  [va\"]select with quotes  [?]AI"
hint_normal_word     = "[ciw]change word  [diw]delete  [yiw]yank  [*]search  [?]AI"
hint_normal_macro    = "● Recording macro @{reg}  [q]stop  all actions will be recorded"
hint_normal_register = "[a-z]select register  \"ayy→yank to a  \"ap→paste from a  \"+y→clipboard"
hint_normal_search   = "Search active  [n]next  [N]prev  [/]new search  [:noh]clear"
hint_insert          = "Typing...  [Esc]normal mode  [Ctrl+w]del word  [Ctrl+u]del to line start"
hint_visual          = "[y]yank  [d]delete  [c]change  [>]indent  [<]dedent  [?]AI  [Esc]exit"
hint_command         = ":w save  :q quit  :wq save+quit  :%s/find/replace/g  [Esc]cancel"
hint_search          = "Type pattern, Enter to search  n/N navigate  [Esc]cancel"
hint_ai              = "Describe your intent, Enter to send  [Tab]accept suggestion  [Esc]cancel"
hint_filetree        = "[j/k]navigate  [l/Enter]open  [h]collapse  [g/G]top/bottom  [H]hidden  [Ctrl+w/Esc]back"
theme_picker_title   = "Theme  j/k Enter Esc"

[messages]
saved                = "Saved"
save_failed          = "Save failed: {err}"
unsaved_changes      = "Unsaved changes! Use :q! to force quit or :wq to save and quit"
file_not_found       = "File not found: {path}"
theme_saved          = "Theme: {name} (saved)"
theme_save_failed    = "Theme: {name} (save failed: {err})"
ai_thinking_plan     = "AI thinking… [plan mode]  [Esc]cancel"
ai_thinking_advisor  = "AI thinking… [advisor mode]  [Esc]cancel"
ai_plan_steps_yolo   = "AI auto-applying {n} steps (yolo)"
ai_plan_steps_confirm= "AI plan: {n} steps — [y]confirm  [n]cancel"
ai_plan_applied      = "Plan applied: {n} steps"
ai_plan_failed       = "Plan failed: {err}"
ai_plan_cancelled    = "Plan cancelled"
ai_error             = "AI error: {err}"
macro_not_found      = "Macro @{reg} not found"
preview_opened       = "Preview opened: {path}"
preview_not_markdown = "Preview only supports Markdown files (.md)"
preview_write_failed = "Failed to write preview: {err}"
preview_open_failed  = "Failed to open browser: {err}"
shell_error          = "Shell error: {err}"

[commands]
cmd_w           = "Save file"
cmd_q           = "Quit"
cmd_q_force     = "Force quit (discard changes)"
cmd_wq          = "Save and quit"
cmd_x           = "Save and quit"
cmd_e           = "Open file"
cmd_e_reload    = "Reload current file"
cmd_w_saveas    = "Save as…"
cmd_set_nu      = "Show line numbers"
cmd_set_nonu    = "Hide line numbers"
cmd_set_tabstop = "Set tab width"
cmd_noh         = "Clear search highlight"
cmd_theme       = "Open theme picker"
cmd_theme_name  = "Switch theme by name"
cmd_u           = "Undo"
cmd_d           = "Delete current line"
cmd_s           = "Substitute (current line) s/pat/rep/"
cmd_percent_s   = "Substitute all  %s/pat/rep/g"
cmd_shell       = "Run shell command"
cmd_preview     = "Preview Markdown in browser"

[ai]
product_guide = """
# hi — Terminal Text Editor

You are the built-in AI assistant of `hi`, a Vim-style terminal text editor written in Rust.
Your name is `hi assistant`. You are NOT OpenClaw, ChatGPT, Claude, or any other AI product.
When asked who you are, say you are the AI assistant embedded in the `hi` editor.

## Modes

hi has 6 modes. The current mode is shown in the status bar.
- NORMAL: default mode for navigation and commands
- INSERT: text input mode
- VISUAL / V-LINE / V-BLOCK: selection modes (char / line / block)
- COMMAND: Ex commands (`:` prefix)
- SEARCH: incremental search (`/` prefix)
- AI: AI query input (`?` prefix in editor, or type directly in chat panel)

## Focus Zones

The UI has three focus zones, cycled with Tab or Ctrl+w:
- Editor (main text area)
- FileTree (left sidebar, toggle with Ctrl+t)
- Chat (right AI panel, toggle with Ctrl+l)

## Normal Mode Keybindings

Movement: h/j/k/l, w/b/e, 0/^/$, gg/G, Ctrl+d/u
Editing: x, dd, yy, p/P, u/Ctrl+r, .
Mode transitions: i/a/I/A (insert), v/V/Ctrl+v (visual), : (command), / (search), ? (AI)

## Command Mode (: prefix)

  :w save  :q quit  :wq save+quit
  :%s/find/replace/g — substitute all
  :theme — open theme picker
  :preview — preview Markdown in browser

## AI Features

1. Editor AI mode: press ? in Normal mode, type query, Enter to send
   - Prefix ?! for edit plans
   - In Visual mode, ? sends selected text as context
2. Chat panel: focus chat (Tab/Ctrl+w), press i or Enter to type

## Configuration (~/.hirc, TOML format)

[general] — line_numbers, tab_width
[ai] — api_base_url, api_key, model, yolo_mode
[theme] — editor_theme, chat_theme
[general] — language = "auto"  # auto | zh-CN | en-US | ru-RU
"""

role_advisor = """
## Your Role: Advisor

{file_info}

You are answering the user's question about their code or the editor.
Be concise and precise. Prefer code examples over lengthy prose.
When the user asks about editor usage, refer to the keybindings above.
Never include markdown fences unless the user asks for formatted output.
Respond in the same language the user uses (Chinese if they write Chinese, English if English).
"""

role_plan = """
## Your Role: Edit Planner

{file_info}

When asked to make changes, respond with a numbered list of atomic edit steps.
Each step must be one of:
- INSERT line <N>: <text>  (0-based line index)
- DELETE line <N>
- REPLACE line <N>: <new text>
- REPLACE range <N>-<M>: <new text (multi-line ok, use \\n)>
- MESSAGE: <advice with no edit>
Output ONLY the numbered steps, no prose.
"""

role_complete = """
You are the inline completion engine of the `hi` terminal text editor.

{file_info}

Generate a short, natural code continuation at the cursor position.
Output ONLY the completion text — no explanation, no markdown fences, no prefix.
Match the existing code style, indentation, and naming conventions.
If unsure, output nothing rather than guessing wrong.
"""

role_transform = """
You are the code transformation engine of the `hi` terminal text editor.

{file_info}

Task: {instruction}
Return ONLY the transformed code, preserving indentation.
No markdown fences, no explanation, no surrounding text.
"""
```

**Step 3: Run test to verify it passes**

```bash
cd /Users/lipingjiang/Codes/hi && cargo test locale 2>&1 | tail -15
```
Expected: all 4 locale tests PASS.

**Step 4: Commit**

```bash
git add locales/
git commit -m "feat(locale): add zh-CN and en-US canonical locale files"
```

---

## Task 3: Wire `Locale` into `Config` and `App`

**Files:**
- Modify: `src/config/mod.rs` — add `language` field to `GeneralConfig`
- Modify: `src/config/loader.rs` — load `Locale` alongside `Config`
- Modify: `src/app.rs` — add `locale: Arc<Locale>` field, pass to subsystems
- Modify: `src/main.rs` — load locale at startup

**Step 1: Write the failing test**

```rust
// In src/config/mod.rs tests
#[test]
fn test_language_default_is_auto() {
    let cfg = Config::default();
    assert_eq!(cfg.general.language, "auto");
}
```

**Step 2: Run test to verify it fails**

```bash
cd /Users/lipingjiang/Codes/hi && cargo test test_language_default 2>&1 | tail -10
```

**Step 3: Add `language` to `GeneralConfig`**

In `src/config/mod.rs`, add to `GeneralConfig`:
```rust
pub language: String,   // "auto" | "zh-CN" | "en-US" | ...
```

In `Default for GeneralConfig`:
```rust
language: "auto".to_string(),
```

**Step 4: Load locale in `src/main.rs`**

```rust
// After loading config:
let lang = if config.general.language == "auto" {
    crate::locale::detect_language_from_env()
} else {
    config.general.language.clone()
};
let locale = std::sync::Arc::new(crate::locale::Locale::load(&lang));
```

Pass `locale.clone()` into `App::new(...)`.

**Step 5: Add `locale` field to `App`**

In `src/app.rs`:
```rust
pub locale: std::sync::Arc<crate::locale::Locale>,
```

Update `App::new(...)` signature and constructor accordingly.

**Step 6: Run test to verify it passes**

```bash
cd /Users/lipingjiang/Codes/hi && cargo test test_language_default 2>&1 | tail -10
```

**Step 7: Commit**

```bash
git add src/config/mod.rs src/config/loader.rs src/app.rs src/main.rs
git commit -m "feat(locale): wire Locale into Config and App, add language config key"
```

---

## Task 4: Replace hardcoded strings in `statusbar.rs`

**Files:**
- Modify: `src/ui/statusbar.rs`
- Modify: `src/editor/mod.rs` — add `locale` field to `Editor` (or pass as param)

**Strategy:** Pass `&Locale` as a parameter to `hint_line()` and `info_line()`. This avoids storing a reference in `Editor` and keeps the borrow checker happy.

**Step 1: Write the failing test**

```rust
// src/ui/statusbar.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::Locale;

    #[test]
    fn test_hint_insert_uses_locale() {
        let editor = Editor::default();
        let locale = Locale::load("en-US");
        // hint_line now takes &Locale
        let hint = editor.hint_line(&locale);
        assert!(hint.contains("Esc"), "en-US hint should mention Esc");
    }

    #[test]
    fn test_hint_insert_zh() {
        let editor = Editor::default();
        let locale = Locale::load("zh-CN");
        let hint = editor.hint_line(&locale);
        assert!(hint.contains("Esc"), "zh-CN hint should also mention Esc");
    }
}
```

**Step 2: Run test to verify it fails**

```bash
cd /Users/lipingjiang/Codes/hi && cargo test test_hint_insert 2>&1 | tail -10
```

**Step 3: Refactor `hint_line` to accept `&Locale`**

Change signature:
```rust
pub fn hint_line(&self, locale: &crate::locale::Locale) -> String
```

Replace every hardcoded string with the corresponding `locale.ui.*` field. For the macro recording hint which needs `{reg}` substitution:
```rust
locale.ui.hint_normal_macro.replace("{reg}", &reg.to_string())
```

**Step 4: Update all callers of `hint_line` in `src/ui/renderer.rs`**

Pass `&app.locale` (or `&locale`) wherever `hint_line()` is called.

**Step 5: Run test to verify it passes**

```bash
cd /Users/lipingjiang/Codes/hi && cargo test test_hint_insert 2>&1 | tail -10
```

**Step 6: Commit**

```bash
git add src/ui/statusbar.rs src/ui/renderer.rs
git commit -m "feat(locale): replace hardcoded hint bar strings with locale keys"
```

---

## Task 5: Replace hardcoded strings in `app.rs` (status messages)

**Files:**
- Modify: `src/app.rs`

**Step 1: Write the failing test**

```rust
// In src/app.rs tests (or a new integration test)
#[test]
fn test_locale_message_format() {
    let locale = crate::locale::Locale::load("en-US");
    let msg = locale.messages.theme_saved.replace("{name}", "dracula");
    assert_eq!(msg, "Theme: dracula (saved)");

    let locale_zh = crate::locale::Locale::load("zh-CN");
    let msg_zh = locale_zh.messages.theme_saved.replace("{name}", "dracula");
    assert!(msg_zh.contains("dracula"));
    assert!(msg_zh.contains("已保存"));
}
```

**Step 2: Run test to verify it fails**

```bash
cd /Users/lipingjiang/Codes/hi && cargo test test_locale_message_format 2>&1 | tail -10
```

**Step 3: Replace all `set_msg` hardcoded strings in `app.rs`**

Pattern: replace each `format!("...")` or `"...".to_string()` with a locale lookup + substitution. Examples:

```rust
// Before:
self.editor.set_msg(format!("Theme: {} (saved)", name));
// After:
self.editor.set_msg(self.locale.messages.theme_saved.replace("{name}", &name));

// Before:
self.editor.set_msg("计划已取消".to_string());
// After:
self.editor.set_msg(self.locale.messages.ai_plan_cancelled.clone());

// Before:
self.editor.set_msg(format!("AI 计划 {} 步 — [y]确认  [n]取消", steps.len()));
// After:
self.editor.set_msg(
    self.locale.messages.ai_plan_steps_confirm
        .replace("{n}", &steps.len().to_string())
);
```

Full list of replacements (all `set_msg` calls in `app.rs`):

| Old string | Locale key | Substitutions |
|---|---|---|
| `"AI 自动执行 {} 步 (yolo)"` | `ai_plan_steps_yolo` | `{n}` |
| `"AI 计划 {} 步 — [y]确认  [n]取消"` | `ai_plan_steps_confirm` | `{n}` |
| `"AI error: {}"` | `ai_error` | `{err}` |
| `"AI 思考中… [{}]  [Esc]取消"` | `ai_thinking_plan` / `ai_thinking_advisor` | — |
| `"计划执行失败: {}"` | `ai_plan_failed` | `{err}` |
| `"计划已应用 {} 步"` | `ai_plan_applied` | `{n}` |
| `"计划已取消"` | `ai_plan_cancelled` | — |
| `"未保存的修改！..."` | `unsaved_changes` | — |
| `"File not found: {}"` | `file_not_found` | `{path}` |
| `"宏 @{} 不存在"` | `macro_not_found` | `{reg}` |
| `"Theme: {} (saved)"` | `theme_saved` | `{name}` |
| `"Theme: {} (save failed: {})"` | `theme_save_failed` | `{name}`, `{err}` |
| `"保存失败: {}"` | `save_failed` | `{err}` |

**Step 4: Run test to verify it passes**

```bash
cd /Users/lipingjiang/Codes/hi && cargo test test_locale_message_format 2>&1 | tail -10
```

**Step 5: Compile check**

```bash
cd /Users/lipingjiang/Codes/hi && cargo build 2>&1 | grep -E "^error" | head -20
```

**Step 6: Commit**

```bash
git add src/app.rs
git commit -m "feat(locale): replace hardcoded status messages in app.rs with locale keys"
```

---

## Task 6: Replace hardcoded strings in `cmd_completion.rs` and `preview.rs`

**Files:**
- Modify: `src/mode/cmd_completion.rs`
- Modify: `src/ui/preview.rs`

**Strategy for `cmd_completion.rs`:** The `CMD_REGISTRY` is a `const` array of `&'static str`, which can't hold runtime locale strings. Solution: change `desc` from `&'static str` to `String` and build the registry dynamically from `&Locale` at construction time. `CmdCompletionState` gets a `locale: Arc<Locale>` and rebuilds the registry on `update()`.

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::Locale;
    use std::sync::Arc;

    #[test]
    fn test_cmd_registry_uses_locale_en() {
        let locale = Arc::new(Locale::load("en-US"));
        let state = CmdCompletionState::new_with_locale(locale);
        let candidates = state.all_commands();
        let w_cmd = candidates.iter().find(|c| c.trigger == "w").unwrap();
        assert_eq!(w_cmd.desc, "Save file");
    }

    #[test]
    fn test_cmd_registry_uses_locale_zh() {
        let locale = Arc::new(Locale::load("zh-CN"));
        let state = CmdCompletionState::new_with_locale(locale);
        let candidates = state.all_commands();
        let w_cmd = candidates.iter().find(|c| c.trigger == "w").unwrap();
        assert_eq!(w_cmd.desc, "保存文件");
    }
}
```

**Step 2: Run test to verify it fails**

```bash
cd /Users/lipingjiang/Codes/hi && cargo test test_cmd_registry 2>&1 | tail -10
```

**Step 3: Refactor `CmdEntry` and `CmdCompletionState`**

```rust
// Change CmdEntry.desc to owned String
pub struct CmdEntry {
    pub trigger: &'static str,
    pub desc: String,   // was &'static str
    pub has_arg: bool,
}

// Add locale-aware constructor
impl CmdCompletionState {
    pub fn new_with_locale(locale: std::sync::Arc<crate::locale::Locale>) -> Self {
        Self {
            items: Vec::new(),
            selected: None,
            locale,
        }
    }
}

// Build registry dynamically
fn build_registry(locale: &crate::locale::Locale) -> Vec<CmdEntry> {
    vec![
        CmdEntry { trigger: "w",            desc: locale.commands.cmd_w.clone(),           has_arg: false },
        CmdEntry { trigger: "q",            desc: locale.commands.cmd_q.clone(),           has_arg: false },
        // ... all entries
    ]
}
```

**Step 4: Replace `preview.rs` messages**

`open_preview` currently returns hardcoded strings. Change its signature to accept `&Locale`:

```rust
pub fn open_preview(buffer_content: &str, file_path: Option<&Path>, locale: &crate::locale::Locale) -> String
```

Replace:
```rust
// Before:
return "Preview only supports Markdown files (.md)".to_string();
// After:
return locale.messages.preview_not_markdown.clone();

// Before:
format!("Preview opened: {}", tmp_path.display())
// After:
locale.messages.preview_opened.replace("{path}", &tmp_path.display().to_string())
```

Update the caller in `app.rs` to pass `&self.locale`.

**Step 5: Run test to verify it passes**

```bash
cd /Users/lipingjiang/Codes/hi && cargo test test_cmd_registry 2>&1 | tail -10
```

**Step 6: Commit**

```bash
git add src/mode/cmd_completion.rs src/ui/preview.rs src/app.rs
git commit -m "feat(locale): localize command descriptions and preview messages"
```

---

## Task 7: Replace AI system prompts in `prompt.rs`

**Files:**
- Modify: `src/ai/prompt.rs`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::Locale;
    use crate::ai::context::AiContext;

    fn dummy_ctx() -> AiContext {
        AiContext {
            filepath: "test.rs".to_string(),
            language: "rust".to_string(),
            total_lines: 10,
            cursor_line: 0,
            cursor_col: 0,
            snippet: "fn main() {}".to_string(),
        }
    }

    #[test]
    fn test_system_prompt_en_contains_english() {
        let locale = Locale::load("en-US");
        let ctx = dummy_ctx();
        let msgs = build_messages_with_locale(&PromptKind::Advisor, &ctx, "hello", &[], &locale);
        let system = &msgs[0].content;
        assert!(system.contains("terminal text editor"), "en-US prompt should be in English");
        assert!(!system.contains("终端文本编辑器"), "en-US prompt should not contain Chinese");
    }

    #[test]
    fn test_system_prompt_zh_contains_chinese() {
        let locale = Locale::load("zh-CN");
        let ctx = dummy_ctx();
        let msgs = build_messages_with_locale(&PromptKind::Advisor, &ctx, "你好", &[], &locale);
        let system = &msgs[0].content;
        assert!(system.contains("终端文本编辑器"), "zh-CN prompt should be in Chinese");
    }
}
```

**Step 2: Run test to verify it fails**

```bash
cd /Users/lipingjiang/Codes/hi && cargo test test_system_prompt 2>&1 | tail -10
```

**Step 3: Refactor `prompt.rs`**

Add a new public function that accepts `&Locale`:

```rust
pub fn build_messages_with_locale(
    kind: &PromptKind,
    ctx: &AiContext,
    query: &str,
    history: &[(&str, &str)],
    locale: &crate::locale::Locale,
) -> Vec<Message> {
    let system = system_prompt_from_locale(kind, ctx, locale);
    let user   = user_prompt(kind, ctx, query);
    let mut msgs = Vec::with_capacity(2 + history.len() * 2);
    msgs.push(Message { role: "system".into(), content: system });
    for (user_msg, asst_msg) in history {
        msgs.push(Message { role: "user".into(),      content: user_msg.to_string() });
        msgs.push(Message { role: "assistant".into(), content: asst_msg.to_string() });
    }
    msgs.push(Message { role: "user".into(), content: user });
    msgs
}

fn system_prompt_from_locale(kind: &PromptKind, ctx: &AiContext, locale: &crate::locale::Locale) -> String {
    let file_info = format!(
        "Current file: `{}` ({}, {} lines total)",
        if ctx.filepath.is_empty() { "[unsaved]" } else { &ctx.filepath },
        ctx.language,
        ctx.total_lines,
    );
    let file_info_placeholder = "{file_info}";

    match kind {
        PromptKind::Advisor => format!(
            "{}\n\n{}",
            locale.ai.product_guide,
            locale.ai.role_advisor.replace(file_info_placeholder, &file_info),
        ),
        PromptKind::Plan => format!(
            "{}\n\n{}",
            locale.ai.product_guide,
            locale.ai.role_plan.replace(file_info_placeholder, &file_info),
        ),
        PromptKind::Complete => locale.ai.role_complete.replace(file_info_placeholder, &file_info),
        PromptKind::Transform(instruction) => locale.ai.role_transform
            .replace(file_info_placeholder, &file_info)
            .replace("{instruction}", instruction),
    }
}
```

Keep the old `build_messages` / `build_messages_with_history` as thin wrappers that load the default locale, so existing call sites don't break immediately.

**Step 4: Update callers in `app.rs`** to pass `&self.locale` to `build_messages_with_locale`.

**Step 5: Run test to verify it passes**

```bash
cd /Users/lipingjiang/Codes/hi && cargo test test_system_prompt 2>&1 | tail -10
```

**Step 6: Commit**

```bash
git add src/ai/prompt.rs src/app.rs
git commit -m "feat(locale): localize AI system prompts — zh-CN and en-US native prompts"
```

---

## Task 8: Add `language` to `~/.hirc` docs + update README

**Files:**
- Modify: `README.md`
- Create: `locales/CONTRIBUTING.md` — guide for community translators

**Step 1: Update README configuration section**

Add to the Configuration section:

```toml
[general]
language = "auto"   # auto | zh-CN | en-US | ru-RU | ja-JP | ...
                    # "auto" reads LANG / LC_ALL from your environment
```

Add a new "Internationalization" section explaining:
- How to switch language
- Where to put custom locale files (`~/.config/hi/locales/`)
- How to contribute a new language (link to `locales/CONTRIBUTING.md`)

**Step 2: Create `locales/CONTRIBUTING.md`**

```markdown
# Contributing a New Locale

hi ships with zh-CN and en-US built in. Adding a new language takes ~30 minutes
and requires no Rust knowledge.

## Steps

1. Copy `en-US.toml` to `{lang-tag}.toml` (e.g. `ru-RU.toml`, `ja-JP.toml`).
2. Translate every string value. Keys must stay in English — only values change.
3. For the `[ai]` section, translate the prompts into your language.
   The AI will respond in your language when the system prompt is in your language.
4. Test locally:
   - Place the file in `~/.config/hi/locales/{lang-tag}.toml`
   - Set `language = "{lang-tag}"` in `~/.hirc`
   - Launch hi and verify all UI strings appear correctly
5. Open a PR adding your file to `locales/`.

## Partial translations

Untranslated keys fall back to en-US automatically, so partial translations work fine.
You don't need to translate everything before submitting.

## Placeholder syntax

Some strings contain `{name}`, `{n}`, `{err}`, `{path}`, `{reg}` placeholders.
Keep these exactly as-is — they are replaced at runtime with actual values.

## Language tag format

Use IETF BCP 47 tags: `zh-CN`, `en-US`, `ru-RU`, `ja-JP`, `de-DE`, `fr-FR`, etc.
The tag must match the prefix of your system's `LANG` environment variable
(e.g. `LANG=ru_RU.UTF-8` → tag `ru-RU`).
```

**Step 3: Commit**

```bash
git add README.md locales/CONTRIBUTING.md
git commit -m "docs(locale): add i18n section to README and community translation guide"
```

---

## Task 9: Full compile, test, and install

**Step 1: Run all tests**

```bash
cd /Users/lipingjiang/Codes/hi && cargo test 2>&1 | tail -20
```
Expected: all tests pass.

**Step 2: Build release**

```bash
cd /Users/lipingjiang/Codes/hi && cargo build --release 2>&1 | grep -E "^error|Finished"
```
Expected: `Finished release profile`.

**Step 3: Install and smoke-test**

```bash
cargo install --path /Users/lipingjiang/Codes/hi
```

Then manually verify:
- Launch `hi README.md` — hint bar should be in Chinese (system locale is zh-CN)
- Run `:theme` — picker title should be in Chinese
- Run `:preview` — success message should be in Chinese
- Set `language = "en-US"` in `~/.hirc`, relaunch — all strings should be in English
- Run `?` and ask a question — AI system prompt should be in the configured language

**Step 4: Final commit**

```bash
cd /Users/lipingjiang/Codes/hi && git add -A && git commit -m "feat(locale): complete i18n system — zh-CN and en-US native support, community locale files"
```

**Step 5: Push**

```bash
cd /Users/lipingjiang/Codes/hi && git push
```
