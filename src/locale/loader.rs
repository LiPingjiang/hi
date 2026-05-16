//! Locale file loading with priority chain and fallback.
//!
//! Priority:
//!   1. `~/.config/hi/locales/{lang}.toml`  (user override)
//!   2. Bundled `zh-CN` / `en-US`           (compiled into binary)
//!   3. `Locale::default()`                 (hard-coded en-US, never panics)

use super::Locale;

/// Bundled locale files compiled into the binary at build time.
const LOCALE_ZH_CN: &str = include_str!("../../locales/zh-CN.toml");
const LOCALE_EN_US: &str = include_str!("../../locales/en-US.toml");

/// Load a locale by language tag.
pub fn load(lang: &str) -> Locale {
    // 1. Try user override file
    if let Some(user_locale) = load_user_file(lang) {
        return user_locale;
    }
    // 2. Try bundled locales
    match lang {
        "zh-CN" => parse_or_default(LOCALE_ZH_CN),
        "en-US" => parse_en_us(),
        _ => {
            // Unknown language: fall back to en-US
            parse_en_us()
        }
    }
}

/// Parse the bundled en-US locale (used as the canonical fallback).
pub fn parse_en_us() -> Locale {
    parse_or_default(LOCALE_EN_US)
}

fn load_user_file(lang: &str) -> Option<Locale> {
    let path = dirs::config_dir()?
        .join("hi")
        .join("locales")
        .join(format!("{}.toml", lang));
    let content = std::fs::read_to_string(&path).ok()?;
    // Parse the user file; merge missing keys from en-US defaults
    let overlay: Locale = toml::from_str(&content).ok()?;
    let base = parse_en_us();
    Some(merge(base, overlay))
}

fn parse_or_default(content: &str) -> Locale {
    toml::from_str(content).unwrap_or_default()
}

/// Merge `overlay` on top of `base`: any non-empty string in overlay wins.
/// This allows partial locale files (community translations) to work correctly —
/// untranslated keys fall back to the en-US base automatically.
fn merge(base: Locale, overlay: Locale) -> Locale {
    Locale {
        ui: merge_ui(base.ui, overlay.ui),
        messages: merge_messages(base.messages, overlay.messages),
        commands: merge_commands(base.commands, overlay.commands),
        ai: merge_ai(base.ai, overlay.ai),
    }
}

macro_rules! pick {
    ($base:expr, $overlay:expr) => {
        if $overlay.is_empty() { $base } else { $overlay }
    };
}

fn merge_ui(b: super::UiStrings, o: super::UiStrings) -> super::UiStrings {
    super::UiStrings {
        hint_normal:          pick!(b.hint_normal,          o.hint_normal),
        hint_normal_empty:    pick!(b.hint_normal_empty,    o.hint_normal_empty),
        hint_normal_comment:  pick!(b.hint_normal_comment,  o.hint_normal_comment),
        hint_normal_tag:      pick!(b.hint_normal_tag,      o.hint_normal_tag),
        hint_normal_url:      pick!(b.hint_normal_url,      o.hint_normal_url),
        hint_normal_number:   pick!(b.hint_normal_number,   o.hint_normal_number),
        hint_normal_string:   pick!(b.hint_normal_string,   o.hint_normal_string),
        hint_normal_word:     pick!(b.hint_normal_word,     o.hint_normal_word),
        hint_normal_macro:    pick!(b.hint_normal_macro,    o.hint_normal_macro),
        hint_normal_register: pick!(b.hint_normal_register, o.hint_normal_register),
        hint_normal_search:   pick!(b.hint_normal_search,   o.hint_normal_search),
        hint_insert:          pick!(b.hint_insert,          o.hint_insert),
        hint_visual:          pick!(b.hint_visual,          o.hint_visual),
        hint_command:         pick!(b.hint_command,         o.hint_command),
        hint_search:          pick!(b.hint_search,          o.hint_search),
        hint_ai:              pick!(b.hint_ai,              o.hint_ai),
        hint_filetree:        pick!(b.hint_filetree,        o.hint_filetree),
        hint_switch_zone:     pick!(b.hint_switch_zone,     o.hint_switch_zone),
        theme_picker_title:   pick!(b.theme_picker_title,   o.theme_picker_title),
    }
}

fn merge_messages(b: super::MessageStrings, o: super::MessageStrings) -> super::MessageStrings {
    super::MessageStrings {
        saved:                 pick!(b.saved,                 o.saved),
        save_failed:           pick!(b.save_failed,           o.save_failed),
        no_file_name:          pick!(b.no_file_name,          o.no_file_name),
        unsaved_changes:       pick!(b.unsaved_changes,       o.unsaved_changes),
        file_not_found:        pick!(b.file_not_found,        o.file_not_found),
        theme_saved:           pick!(b.theme_saved,           o.theme_saved),
        theme_save_failed:     pick!(b.theme_save_failed,     o.theme_save_failed),
        ai_thinking_plan:      pick!(b.ai_thinking_plan,      o.ai_thinking_plan),
        ai_thinking_advisor:   pick!(b.ai_thinking_advisor,   o.ai_thinking_advisor),
        ai_plan_steps_yolo:    pick!(b.ai_plan_steps_yolo,    o.ai_plan_steps_yolo),
        ai_plan_steps_confirm: pick!(b.ai_plan_steps_confirm, o.ai_plan_steps_confirm),
        ai_plan_applied:       pick!(b.ai_plan_applied,       o.ai_plan_applied),
        ai_plan_failed:        pick!(b.ai_plan_failed,        o.ai_plan_failed),
        ai_plan_cancelled:     pick!(b.ai_plan_cancelled,     o.ai_plan_cancelled),
        ai_error:              pick!(b.ai_error,              o.ai_error),
        macro_not_found:       pick!(b.macro_not_found,       o.macro_not_found),
        preview_opened:        pick!(b.preview_opened,        o.preview_opened),
        preview_not_markdown:  pick!(b.preview_not_markdown,  o.preview_not_markdown),
        preview_write_failed:  pick!(b.preview_write_failed,  o.preview_write_failed),
        preview_open_failed:   pick!(b.preview_open_failed,   o.preview_open_failed),
        shell_error:           pick!(b.shell_error,           o.shell_error),
        chat_cleared:          pick!(b.chat_cleared,          o.chat_cleared),
        mouse_hint:            pick!(b.mouse_hint,            o.mouse_hint),
    }
}

fn merge_commands(b: super::CommandStrings, o: super::CommandStrings) -> super::CommandStrings {
    super::CommandStrings {
        cmd_w:           pick!(b.cmd_w,           o.cmd_w),
        cmd_q:           pick!(b.cmd_q,           o.cmd_q),
        cmd_q_force:     pick!(b.cmd_q_force,     o.cmd_q_force),
        cmd_wq:          pick!(b.cmd_wq,          o.cmd_wq),
        cmd_x:           pick!(b.cmd_x,           o.cmd_x),
        cmd_e:           pick!(b.cmd_e,           o.cmd_e),
        cmd_e_reload:    pick!(b.cmd_e_reload,    o.cmd_e_reload),
        cmd_w_saveas:    pick!(b.cmd_w_saveas,    o.cmd_w_saveas),
        cmd_set_nu:      pick!(b.cmd_set_nu,      o.cmd_set_nu),
        cmd_set_nonu:    pick!(b.cmd_set_nonu,    o.cmd_set_nonu),
        cmd_set_tabstop: pick!(b.cmd_set_tabstop, o.cmd_set_tabstop),
        cmd_noh:         pick!(b.cmd_noh,         o.cmd_noh),
        cmd_theme:       pick!(b.cmd_theme,       o.cmd_theme),
        cmd_theme_name:  pick!(b.cmd_theme_name,  o.cmd_theme_name),
        cmd_u:           pick!(b.cmd_u,           o.cmd_u),
        cmd_d:           pick!(b.cmd_d,           o.cmd_d),
        cmd_s:           pick!(b.cmd_s,           o.cmd_s),
        cmd_percent_s:   pick!(b.cmd_percent_s,   o.cmd_percent_s),
        cmd_shell:       pick!(b.cmd_shell,       o.cmd_shell),
        cmd_preview:     pick!(b.cmd_preview,     o.cmd_preview),
        cmd_filetree:    pick!(b.cmd_filetree,    o.cmd_filetree),
        cmd_grep:        pick!(b.cmd_grep,        o.cmd_grep),
        cmd_tutorial:    pick!(b.cmd_tutorial,    o.cmd_tutorial),
        cmd_mouse:       pick!(b.cmd_mouse,       o.cmd_mouse),
    }
}

fn merge_ai(b: super::AiStrings, o: super::AiStrings) -> super::AiStrings {
    super::AiStrings {
        product_guide:   pick!(b.product_guide,   o.product_guide),
        role_advisor:    pick!(b.role_advisor,    o.role_advisor),
        role_plan:       pick!(b.role_plan,       o.role_plan),
        role_complete:   pick!(b.role_complete,   o.role_complete),
        role_transform:  pick!(b.role_transform,  o.role_transform),
        role_agent_edit: pick!(b.role_agent_edit, o.role_agent_edit),
    }
}
