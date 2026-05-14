# hi

> A terminal text editor for the AI era.

---

## What is hi

`hi` is what you type to say hello to a file.

```bash
hi config.yaml
hi /etc/nginx/nginx.conf
hi .
```

The name carries everything this editor believes in:

- **`hi` ≈ `vi`** — vi defined modal editing for Unix. Vim was "Vi IMproved". `hi` is the next evolution, built for the AI era from the ground up.
- **`hi` ≈ `ai`** — AI is not a plugin here. It is the core of how you interact with text.
- **`hi` = hello** — You greet a file, it greets you back. Friendly, immediate, intelligent.

Two letters. Fast to type. Easy to remember. That matters when you're in a terminal.

---

## Terminal Compatibility

> **Important:** `hi` requires a modern terminal emulator with true-color (24-bit) support.

The built-in macOS Terminal.app only supports 256 colors and lacks the color depth needed for `hi`'s syntax highlighting engine. You will see washed-out, incorrect colors or missing highlights if you run `hi` in Terminal.app.

**Recommended terminals:**

| Terminal | Platform | True-color | Notes |
|---|---|---|---|
| **iTerm2** | macOS | ✅ | Recommended for macOS users. Full 24-bit color, ligatures, GPU rendering. |
| **Kitty** | macOS / Linux | ✅ | GPU-accelerated, excellent performance. |
| **Alacritty** | macOS / Linux / Windows | ✅ | Minimal, fast, Rust-based. |
| **WezTerm** | macOS / Linux / Windows | ✅ | Feature-rich, Lua-configurable. |
| **Windows Terminal** | Windows | ✅ | Default choice on Windows 11. |
| **Ghostty** | macOS / Linux | ✅ | New, fast, native platform integration. |
| macOS Terminal.app | macOS | ❌ | **Not supported.** 256-color only, no true-color. |

To verify your terminal supports true-color, run:

```bash
printf "\x1b[38;2;255;100;0mTRUE COLOR\x1b[0m\n"
```

If you see "TRUE COLOR" in orange, you're good to go.

---

## Install

### One-line installer (Linux & macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/LiPingjiang/hi/main/install.sh | sh
```

Detects your OS and architecture, downloads the matching pre-built binary from
GitHub Releases, verifies the SHA256 checksum, and installs to `/usr/local/bin`
(or `~/.local/bin` if you don't have write access).

**Options:**

```bash
# Install a specific version
HI_VERSION=v0.1.2 curl -fsSL .../install.sh | sh

# Install to a custom directory
HI_INSTALL=~/.bin curl -fsSL .../install.sh | sh
```

### Homebrew (macOS)

```bash
brew tap LiPingjiang/tap
brew install hi
```

### cargo install (requires Rust toolchain)

```bash
cargo install hi
```

### Download manually

Pre-built binaries for every release are available on the
[Releases page](https://github.com/LiPingjiang/hi/releases):

| Platform | Archive |
|---|---|
| macOS Apple Silicon | `hi-<version>-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `hi-<version>-x86_64-apple-darwin.tar.gz` |
| Linux x86\_64 (static) | `hi-<version>-x86_64-linux-musl.tar.gz` |
| Linux x86\_64 (glibc) | `hi-<version>-x86_64-linux-gnu.tar.gz` |
| Linux ARM64 | `hi-<version>-aarch64-linux-gnu.tar.gz` |

Each archive includes a `.sha256` checksum file.

---

## Feature Overview

`hi` is organized around four capability pillars: **Navigation**, **Editing**, **Search**, and **AI**. Each pillar is designed to be immediately usable without memorizing a manual.

---

### Pillar 1 — Navigation

Getting to the right file and the right line should be instant. `hi` provides three complementary navigation tools that cover every scale of movement.

#### File Tree

Press `Ctrl+\` to toggle the file tree sidebar. Navigate with `j`/`k`, expand/collapse directories with `Enter` or `Space`, and open files with `Enter`. The tree respects `.gitignore` and supports creating, renaming, and deleting files directly from the sidebar.

```
Ctrl+\        toggle file tree
j / k         move up / down
Enter         open file or expand directory
n             new file in current directory
N             new directory
r             rename
d             delete (with confirmation)
```

#### Fuzzy File Picker — `Ctrl+P`

Press `Ctrl+P` to open the fuzzy file picker overlay. Type any subsequence of the filename — characters don't need to be adjacent. The picker scores matches by consecutive runs and highlights matched characters in the result list.

```
╭──────────────────────────────────────────────────────╮
│ 🔍 app                                               │
├──────────────────────────────────────────────────────┤
│ ▶  src/app.rs                                        │
│    src/ui/chatpanel.rs                               │
│    src/mode/command.rs                               │
│    ...                                               │
╰ ↑↓ navigate  Enter open  Esc cancel  (312 files) ────╯
```

Matched characters are highlighted in peach. The picker searches the entire project tree, skipping `target/`, `node_modules/`, and other noise directories.

#### Jump List

`hi` maintains a jump list across file positions. Use `Ctrl+O` to jump back and `Ctrl+I` to jump forward — the same muscle memory as Vim. Marks (`m{a-z}`, `` `{a-z} ``) let you pin specific positions for instant return.

---

### Pillar 2 — Editing

`hi` is a modal editor. Normal mode is for navigation and commands; Insert mode is for typing. The hint bar at the bottom of the screen always shows what keys are available in the current mode — you never need to remember.

#### Modal Editing (Vim-compatible)

All standard Vim motions and operators work as expected: `w`/`b`/`e` for word movement, `f`/`t`/`;`/`,` for character search, `d`/`y`/`c`/`p` for delete/yank/change/put, `gg`/`G` for file navigation, `%` for bracket matching, and so on. Text objects (`iw`, `aw`, `i"`, `a(`, etc.) are fully supported.

#### Undo / Redo

`u` undoes, `Ctrl+R` redoes. Undo history is stored as a grouped transaction tree — each insert session, substitution, or AI edit is a single undoable unit.

#### Dot Repeat

`.` repeats the last change at the current cursor position. Works for insertions, deletions, substitutions, and character replacements.

#### Named Registers

`"{a-z}y` yanks into a named register; `"{a-z}p` pastes from it. The `+` register maps to the system clipboard. Use `"` in Normal mode to set the active register before any yank or delete.

#### Macro Recording and Playback

Record a sequence of keystrokes into a named register and replay it any number of times.

```
q{a-z}        start recording into register {a-z}
              (status bar shows  ● REC [a]  while recording)
q             stop recording
@{a-z}        play back the macro in register {a-z}
@@            replay the last-used macro
{n}@{a-z}     play back n times
```

Macros capture both Normal-mode and Insert-mode keystrokes, so a macro that enters insert, types text, and returns to Normal mode replays the full sequence faithfully.

#### Visual Modes

`v` enters character-wise Visual, `V` enters line-wise Visual, `Ctrl+V` enters Visual Block. In Visual Block, `I` inserts text at the start of every selected line simultaneously.

#### Command Line

`:` opens the command line. Supported commands include `:w`, `:q`, `:wq`, `:e {file}`, `:{n}` (go to line), `:%s/pat/rep/flags` (substitution), `:set nu`/`:set nonu`, `:!{cmd}` (shell command), `:theme`, `:grep`, and `:preview`. Command history is navigable with `↑`/`↓`, and Tab-completion is available for command names.

---

### Pillar 3 — Search

#### In-file Search — `/`

Press `/` to enter search mode. Type a pattern (literal or regex), press `Enter` to confirm. `n`/`N` jump to the next/previous match. All matches are highlighted in the buffer. `:noh` clears the highlight.

#### Global Grep — `Ctrl+F` or `:grep`

Press `Ctrl+F` (or type `:grep <pattern>`) to search across every file in the project. Results appear in a scrollable overlay showing the filename, line number, and the matching line with the match highlighted.

```
╭──────────────── 🔎 Grep ─────────────────────────────╮
│ / render_file_picker                                  │
├───────────────────────────────────────────────────────┤
│ ▶  renderer.rs:1080  │ pub fn render_file_picker(    │
│    app.rs:246        │ self.renderer.render_file_pi…  │
│    app.rs:294        │ if self.file_picker.is_some()  │
│    ...                                                │
├───────────────────────────────────────────────────────┤
│  3/8 matches  ↑↓ navigate  Enter jump  Esc cancel    │
╰───────────────────────────────────────────────────────╯
```

Regex mode is available with `:grep /pattern/` (slash-delimited). Pressing Enter on a result opens the file and jumps the cursor to the exact match position, centred in the viewport.

```
Ctrl+F            open grep panel (empty query)
:grep foo         search for literal "foo" across all files
:grep /foo.bar/   search with regex
↑ / ↓             navigate results
Enter             run search (first press) or jump to match
Esc               close panel
```

The search skips binary files, `target/`, `node_modules/`, and other noise directories. Results are capped at 1,000 matches for responsiveness.

---

### Pillar 4 — AI

`?` is the AI key. Press it from Normal mode to describe your intent in plain language. `hi` reads the complexity of your request and responds accordingly.

#### Advisor Mode — questions and explanations

When you ask a question, `hi` answers in the Chat panel without touching your file.

```
? how do I select the current paragraph

[AI] In Normal mode, press vip
     v  → enter Visual mode
     ip → select inner paragraph
```

#### Plan Mode — multi-step edits

When you ask for a complex transformation, `hi` shows you a plan and waits for your confirmation before making any changes.

```
? convert all numbers to Chinese characters

╭─ AI Execution Plan ────────────────────────────────╮
│ Step 1  Match all numbers with \d+                 │
│ Step 2  Replace using mapping (1→一, 2→二, ...)    │
│ Step 3  Handle multi-digit (11→十一, 100→一百)     │
╰────────────────────────────────────────────────────╯
[y] confirm    [n] cancel
```

#### Ghost Text — command completion

For simple requests, `hi` fills in the command as ghost text. Press Tab to confirm.

```
? replace all "China" with "France"

→ :%s/China/France/g█        ← ghost text, Tab to confirm
```

#### Chat Panel — `Ctrl+G`

`Ctrl+G` opens the persistent Chat panel on the right side of the screen. All AI responses accumulate here. You can scroll through the history, ask follow-up questions, and reference previous answers while editing.

**`hi` never acts without being asked.** `?` is the only trigger. No interruptions, no unsolicited suggestions.

---

## Syntax Highlighting — Two Engines, One Renderer

`hi` uses two purpose-built highlighting engines, each optimal for its role, feeding into a single unified renderer.

**Editor buffer → [Tree-sitter](https://tree-sitter.github.io/tree-sitter/)** — an incremental, error-tolerant parser that builds a concrete syntax tree of your file. On every keystroke only the dirty subtree is re-parsed (O(changed\_bytes × log n)), so highlighting stays instant even on large files. The tree is also used for future structural editing features (select-by-node, smart indent, etc.).

**AI Chat panel → [syntect](https://github.com/trishume/syntect)** — the same library that powers Sublime Text's highlighting, driven by `.tmLanguage` grammars. Ideal for rendering isolated code blocks inside Markdown responses where stateful line-by-line parsing is the right model.

```
┌──────────────────────────────────────────────────────────────────────┐
│  Editor buffer                    AI Chat panel                      │
│                                                                      │
│  ┌─────────────────────┐          ┌──────────────────────────┐       │
│  │   Tree-sitter        │          │   syntect                │       │
│  │   TsHighlighter      │          │   MdRenderer             │       │
│  │                      │          │                          │       │
│  │  incremental parse   │          │  pulldown-cmark parser   │       │
│  │  highlight_viewport()│          │  + syntect code blocks   │       │
│  │  (viewport-only)     │          │  (200+ tmLanguage grammars)│     │
│  └──────────┬───────────┘          └────────────┬─────────────┘       │
│             │                                   │                     │
│      SyntectSpan[]                        StyledSpan[]                │
│     (byte range + RGB)               (text + RGB + attrs)             │
│             │                                   │                     │
│             └──────────────┬────────────────────┘                     │
│                            ▼                                          │
│                    Renderer (crossterm)                               │
│                    unified ANSI painting                              │
└──────────────────────────────────────────────────────────────────────┘
```

**What this gives you:**

- **Incremental parsing** — Tree-sitter re-parses only the changed region on every keystroke. No full-file re-scan, no frame drops on large files.
- **Viewport-only highlighting** — `highlight_viewport()` queries only the visible line range, so rendering cost is O(tokens on screen) regardless of file size or scroll position.
- **12 languages built-in** for the editor: Rust, Python, Java, Go, JSON, YAML, TOML, Bash, HTML, JavaScript, TypeScript, Markdown.
- **200+ languages in Chat** — any language Sublime Text supports is highlighted correctly in AI responses via syntect.
- **Overlay compositing** — search highlights and Visual Block selections are painted on top of syntax colors without destroying them.
- **Theme unification** — one `[theme]` section in `~/.hirc` controls both the editor palette and the Chat panel theme. Switch once, everything follows.

### Built-in Themes

Switch themes live with `:theme` (opens an interactive picker with real-time preview). Your choice is persisted to `~/.hirc` and survives restarts.

```toml
# ~/.hirc
[theme]
editor_theme = "base16-ocean.dark"   # or "Solarized (dark)", "base16-eighties.dark", etc.
chat_theme   = "dracula"             # or "dark", "tokyo-night"
```

The Chat panel's Markdown renderer goes beyond plain syntax highlighting — it renders headings with background colors and Unicode underlines, blockquotes with colored `│` borders, tables with box-drawing characters (`┌┬┐├┼┤└┴┘`), code blocks with rounded borders (`╭╮╰╯`) and language labels, task lists with `☑`/`☐` markers, and more.

---

## Markdown Preview — `:preview`

`hi` includes a built-in Markdown preview command that renders your `.md` file as a beautifully styled HTML page and opens it in your default browser.

```
:preview
```

**How it works:**

1. The current buffer content is parsed with [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) (CommonMark + GFM extensions: tables, footnotes, strikethrough, task lists).
2. A complete HTML document is generated with an embedded GitHub-dark inspired CSS theme — dark background, proper typography, styled code blocks, tables, blockquotes, and more.
3. The HTML is written to a temporary file (`/tmp/hi-preview-*.html`) and opened via the system browser (`open` on macOS, `xdg-open` on Linux).

The preview is read-only and non-blocking — you continue editing in `hi` while the browser tab stays open. Re-running `:preview` overwrites the same temp file, so refreshing the browser tab shows your latest changes.

---

## Quick Reference

| Key / Command | Action |
|---|---|
| `Ctrl+P` | Fuzzy file picker |
| `Ctrl+F` | Global grep panel |
| `Ctrl+\` | Toggle file tree |
| `Ctrl+G` | Toggle AI Chat panel |
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

---

## Comparison with Other Terminal Tools

### vs. Terminal Editors

| Feature | hi | Vim/Neovim | Helix | micro | nano |
|---|---|---|---|---|---|
| True-color syntax highlighting | ✅ Built-in (Tree-sitter, incremental) | ✅ (requires config) | ✅ Tree-sitter | ✅ Limited | ❌ Basic |
| AI integration | ✅ Native (`?` key) | ⚠️ Plugin (Copilot.vim) | ❌ None | ❌ None | ❌ None |
| Fuzzy file picker | ✅ `Ctrl+P` built-in | ⚠️ Plugin (fzf.vim) | ✅ Built-in | ❌ None | ❌ None |
| Global grep | ✅ `Ctrl+F` / `:grep` | ⚠️ Plugin (fzf / telescope) | ✅ Built-in | ❌ None | ❌ None |
| Macro recording | ✅ `q{reg}` / `@{reg}` | ✅ Full | ❌ None | ❌ None | ❌ None |
| Markdown preview | ✅ `:preview` (browser) | ⚠️ Plugin | ❌ None | ❌ None | ❌ None |
| Theme live-switching | ✅ `:theme` with real-time preview | ⚠️ `:colorscheme` (no preview) | ✅ `:theme` | ⚠️ Config file | ❌ N/A |
| Learning curve | Low (hint bar + AI) | Very high | Medium | Low | Very low |
| Startup time | ~5ms | ~50ms (Neovim + plugins) | ~10ms | ~10ms | ~5ms |
| Language | Rust | C / Lua | Rust | Go | C |
| Config format | TOML (`~/.hirc`) | Vimscript / Lua | TOML | JSON | nanorc |

### vs. Markdown Renderers

| Feature | hi `:preview` | glow | mdcat | grip | Marked (VS Code) |
|---|---|---|---|---|---|
| Rendering target | Browser (full HTML/CSS) | Terminal (ANSI) | Terminal (ANSI) | Browser (GitHub API) | VS Code panel |
| Visual fidelity | ★★★★★ Full CSS styling | ★★★ Limited by terminal | ★★★ Limited by terminal | ★★★★★ GitHub-identical | ★★★★★ Full CSS |
| Tables | ✅ Proper HTML tables | ✅ Box-drawing | ✅ Box-drawing | ✅ GitHub-rendered | ✅ HTML tables |
| Code blocks | ✅ Monospace, styled | ✅ Colored background | ✅ Syntax highlighted | ✅ GitHub highlighting | ✅ Syntax highlighted |
| Images | ✅ Full rendering | ❌ Not displayed | ⚠️ iTerm2/Kitty only | ✅ Full rendering | ✅ Full rendering |
| Requires network | ❌ Fully offline | ❌ Offline | ❌ Offline | ✅ GitHub API | ❌ Offline |
| Integrated with editor | ✅ One keystroke | ❌ Separate tool | ❌ Separate tool | ❌ Separate tool | ✅ VS Code only |
| Dark theme | ✅ Built-in (GitHub-dark) | ✅ Auto-detect | ✅ Auto-detect | ⚠️ Depends on GitHub | ✅ Follows VS Code |
| Footnotes | ✅ | ❌ | ❌ | ✅ | ✅ |
| Task lists | ✅ Checkboxes | ✅ | ✅ | ✅ | ✅ |

---

## Configuration

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

### Internationalization (i18n)

`hi` ships with built-in **zh-CN** and **en-US** locales. The active language is auto-detected from your `LANG` / `LC_ALL` environment variable, or you can pin it in `~/.hirc`:

```toml
[general]
language = "zh-CN"   # force Simplified Chinese
```

Community translations live in `~/.config/hi/locales/`. Drop a `ru-RU.toml` (or any BCP-47 tag) there and set `language = "ru-RU"` — untranslated keys fall back to en-US automatically. See [`locales/CONTRIBUTING.md`](locales/CONTRIBUTING.md) for the translation guide.

---

## Key Design Decisions

| Decision | Choice | Why |
|---|---|---|
| Language | Rust | Memory safety, zero-cost abstractions, modern tooling, proven by Helix/Zed |
| Text storage | Rope (ropey) | O(log n) edits on large files; undo history stored as text patches, not full snapshots |
| Terminal | crossterm | Cross-platform, no ncurses dependency |
| Editor syntax highlighting | Tree-sitter | Incremental CST, viewport-only query, zero frame drops on large files |
| Chat syntax highlighting | syntect | Sublime Text grammars, 200+ languages for AI response code blocks |
| Markdown rendering | pulldown-cmark + syntect | CommonMark + GFM, code blocks share the syntect highlight engine |
| Markdown preview | pulldown-cmark → HTML + browser | Full CSS fidelity, no terminal limitations, offline |
| Config | `~/.hirc` (TOML) | Simple path, readable format |
| AI trigger | `?` | Semantic fit (question mark = ask), symmetric with `/` (search) |
| LLM backend | Configurable | Any OpenAI-compatible API: OpenAI, Claude, Ollama, others |

---

## Status

🚧 **Pre-alpha. Docs and architecture in progress.**

See [`docs/PRODUCT.md`](docs/PRODUCT.md) for the full product vision.
See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the technical design.

---

## Philosophy

Vim taught us that modal editing is the right model for keyboard-driven text manipulation. We keep that. Everything else is rebuilt.

The goal is not to be a better Vim. The goal is to be the editor you reach for when you open a terminal and need to get something done — without stopping to remember which key does what.

---

## Contributing

Contributions are welcome. Before submitting a pull request, please read the
[Contributor License Agreement](CLA.md). By opening a PR you agree to its terms.

This allows the project to remain open-source while preserving the flexibility
to pursue commercial opportunities in the future.

---

## License

Copyright 2026 lipingjiang

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the full text.

In short: you can use, modify, and distribute this software freely, including
for commercial purposes, as long as you include the license notice and attribute
the original work. The Apache 2.0 license also includes an explicit patent grant,
protecting both users and contributors.
