# Configuration

## Config File

`hi` reads configuration from `~/.hirc` (TOML format).

```toml
# ~/.hirc

[general]
line_numbers = true
tab_width = 4
language = "auto"   # "auto" detects from LANG/LC_ALL; or set "zh-CN", "en-US", "ru-RU", …

[ai]
api_base_url = "https://api.openai.com/v1"
api_key = ""        # or set HI_API_KEY environment variable
model = "gpt-4o"
yolo_mode = false   # skip confirmation for AI execution plans

[theme]
colorscheme = "default"
editor_theme = "base16-ocean.dark"   # syntect theme for the editor text area
chat_theme   = "dark"                # Markdown theme for the AI Chat panel
```

## Internationalization (i18n)

`hi` ships with built-in **zh-CN** and **en-US** locales. The active language is auto-detected from your `LANG` / `LC_ALL` environment variable, or you can pin it in `~/.hirc`:

```toml
[general]
language = "zh-CN"   # force Simplified Chinese
```

Community translations live in `~/.config/hi/locales/`. Drop a `ru-RU.toml` (or any BCP-47 tag) there and set `language = "ru-RU"` — untranslated keys fall back to en-US automatically. See [`locales/CONTRIBUTING.md`](../locales/CONTRIBUTING.md) for the translation guide.

## Built-in Themes

Switch themes live with `:theme` (opens an interactive picker with real-time preview). Your choice is persisted to `~/.hirc` and survives restarts.

Available themes: `base16-ocean.dark`, `Solarized (dark)`, `base16-eighties.dark`, `dracula`, `tokyo-night`, `monokai-pro`, `github-dark`, `one-dark-pro`, `electric`, `synthwave`, and more.

## Quick Reference

| Key / Command | Action |
|---|---|
| `Ctrl+P` | Fuzzy file picker |
| `Ctrl+F` | Global grep panel |
| `Ctrl+\` | Toggle file tree |
| `Ctrl+G` | Toggle AI Chat panel |
| `Ctrl+L` | LeetCode mode (requires `--features leetcode`) |
| `?` | AI prompt (Normal mode) |
| `q{a-z}` / `q` | Start / stop macro recording |
| `@{a-z}` | Play back macro |
| `/` | In-file search |
| `n` / `N` | Next / previous search match |
| `:grep <pat>` | Global search (literal) |
| `:grep /<regex>/` | Global search (regex) |
| `:theme` | Interactive theme picker |
| `:preview` | Markdown preview in browser |
| `:w` / `:q` / `:wq` | Save / quit / save and quit |
| `u` / `Ctrl+R` | Undo / redo |
| `.` | Repeat last change |
