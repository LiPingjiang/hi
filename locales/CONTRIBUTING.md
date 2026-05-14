# Contributing a Translation to `hi`

Thank you for helping make `hi` accessible in your language!

## How it works

`hi` loads UI strings from TOML locale files at startup.

**Priority chain (highest → lowest):**

1. `~/.config/hi/locales/{lang}.toml` — your personal override
2. Bundled `zh-CN` / `en-US` — compiled into the binary
3. Hard-coded en-US defaults — never panics, always works

Partial translations are fully supported: any key you omit falls back to the
en-US value automatically.

## Quick start

1. Copy `en-US.toml` to a new file named after your BCP-47 language tag:

   ```
   cp locales/en-US.toml locales/ru-RU.toml
   ```

2. Translate the values (keep the keys unchanged):

   ```toml
   [messages]
   saved = "Сохранено"
   save_failed = "Ошибка сохранения: {err}"
   ```

3. Test it locally by placing the file in `~/.config/hi/locales/` and setting:

   ```toml
   # ~/.hirc
   [general]
   language = "ru-RU"
   ```

4. Open a PR with your `locales/{lang}.toml` file.

## File structure

```
locales/
  en-US.toml   ← reference file (all keys, English values)
  zh-CN.toml   ← Simplified Chinese (bundled)
  ru-RU.toml   ← example community locale (not bundled)
```

## Sections

| Section      | Contents |
|---|---|
| `[ui]`       | Hint bar strings shown at the bottom of the screen |
| `[messages]` | Status bar messages and transient notifications |
| `[commands]` | Command-mode (`:`) completion descriptions |
| `[ai]`       | AI system prompts — the AI will respond in the locale's language |

## Placeholder syntax

Some strings contain `{placeholder}` tokens that are replaced at runtime:

| Placeholder | Meaning |
|---|---|
| `{err}`  | Error message string |
| `{path}` | File path |
| `{name}` | Theme or file name |
| `{n}`    | A count (number of steps, etc.) |
| `{reg}`  | Macro register letter (a–z) |
| `{file_info}` | AI context: current file name, language, line count |
| `{instruction}` | AI transform task description |

Keep all placeholders intact in your translation.

## AI prompts

The `[ai]` section contains the system prompts sent to the language model.
Translating these makes the AI respond in your language by default.

`product_guide` is a multi-line string (use `"""..."""` in TOML).
`role_*` strings are appended per request kind and may contain `{file_info}`
or `{instruction}` placeholders.

## Bundling a locale

To bundle a locale into the binary (so it works without a user file), add it
to `src/locale/loader.rs`:

```rust
const LOCALE_RU_RU: &str = include_str!("../../locales/ru-RU.toml");

pub fn load(lang: &str) -> Locale {
    match lang {
        "zh-CN" => parse_or_default(LOCALE_ZH_CN),
        "en-US" => parse_en_us(),
        "ru-RU" => parse_or_default(LOCALE_RU_RU),   // ← add this
        _ => parse_en_us(),
    }
}
```

Open a PR and we'll review and merge it!
