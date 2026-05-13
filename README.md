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

## The Problem with Vim

Vim is powerful. But its power is invisible until you've memorized hundreds of key combinations. Most people give up not because Vim can't do something — it almost always can — but because they didn't know the right keys to press.

**hi solves this with two ideas:**

**1. A persistent hint bar at the bottom of the screen.**
Always on. Always predicting what you're likely to want next based on your cursor position, file type, and current selection. Like IDE autocomplete, but for editor operations. You see the key, you press it, you move on.

**2. Natural language via the `?` key.**
Press `?`, describe what you want in plain language, and hi figures out the rest.

---

## How AI Works in hi

`?` is the AI key. Press it from Normal mode to describe your intent.

**hi reads the complexity of your request and responds accordingly:**

For simple requests, hi fills in the command for you as ghost text. Press Tab to confirm:

```
? replace all "China" with "France"

→ :%s/China/France/g█        ← ghost text, Tab to confirm
```

For complex requests that need multiple steps, hi shows you a plan and waits for your confirmation:

```
? convert all numbers to Chinese characters

╭─ AI Execution Plan ────────────────────────────────╮
│ Step 1  Match all numbers with \d+                 │
│ Step 2  Replace using mapping (1→一, 2→二, ...)    │
│ Step 3  Handle multi-digit (11→十一, 100→一百)     │
╰────────────────────────────────────────────────────╯
[y] confirm    [n] cancel    [e] edit plan
```

When you're just asking how to do something:

```
? how do I select the current paragraph

[AI] In Normal mode, press vip
     v  → enter Visual mode
     ip → select inner paragraph
```

**hi never acts without being asked.** `?` is the only trigger. No interruptions, no unsolicited suggestions.

---

## Syntax Highlighting — One Engine, Everywhere

Most terminal editors ship two completely separate highlighting systems: one for the editing buffer, another (if any) for auxiliary panels like chat or preview. Colors don't match, language coverage diverges, and themes can't be shared.

**hi takes a different approach.** A single [syntect](https://github.com/trishume/syntect) engine — the same library that powers Sublime Text's highlighting — drives both the editor text area and the AI Chat panel's Markdown code blocks. Open a `.rs` file and ask the AI a question that includes a Rust snippet: the colors are identical, because they come from the same Sublime Text `.tmLanguage` grammar and the same theme.

```
┌─────────────────────────────────────────────────────────────────────┐
│                        syntect engine                               │
│                  SyntaxSet (200+ languages)                         │
│                  ThemeSet  (Sublime Text themes)                    │
│                                                                     │
│         ┌──────────────────┐       ┌──────────────────────┐        │
│         │ SyntectHighlighter│       │     MdRenderer        │        │
│         │ (editor buffer)  │       │ (Chat panel Markdown) │        │
│         │                  │       │                       │        │
│         │ stateful per-line│       │ pulldown-cmark parser │        │
│         │ HighlightLines   │       │ + syntect code blocks │        │
│         └────────┬─────────┘       └───────────┬──────────┘        │
│                  │                              │                   │
│           SyntectSpan[]                   StyledSpan[]              │
│          (byte range + RGB)            (text + RGB + attrs)         │
│                  │                              │                   │
│                  └──────────┬───────────────────┘                   │
│                             ▼                                       │
│                     Renderer (crossterm)                            │
│                     unified ANSI painting                           │
└─────────────────────────────────────────────────────────────────────┘
```

**What this gives you:**

- **200+ languages** highlighted out of the box — Rust, Python, Go, Java, TypeScript, C/C++, SQL, YAML, TOML, Markdown, and everything else Sublime Text supports.
- **Pixel-perfect color consistency** between the file you're editing and the code the AI shows you.
- **Stateful multi-line parsing** — block comments, heredocs, and multi-line strings are tracked correctly across lines via `HighlightLines` state machine.
- **Overlay compositing** — search highlights and Visual Block selections are painted on top of syntax colors without destroying them.
- **Theme unification** — one `[theme]` section in `~/.hirc` controls both the editor and the Chat panel. Switch once, everything follows.

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

**Supported Markdown features:**

- Headings (h1–h6) with bottom borders
- Fenced code blocks with monospace font
- Tables with alternating row colors
- Blockquotes with colored left border
- Task lists with checkboxes
- Footnotes
- Strikethrough text
- Images, links, horizontal rules
- Nested lists (ordered and unordered)

---

## Comparison with Other Terminal Tools

### vs. Terminal Editors

| Feature | hi | Vim/Neovim | Helix | micro | nano |
|---|---|---|---|---|---|
| True-color syntax highlighting | ✅ Built-in (syntect, 200+ langs) | ✅ (requires config) | ✅ Tree-sitter | ✅ Limited | ❌ Basic |
| AI integration | ✅ Native (`?` key) | ⚠️ Plugin (Copilot.vim) | ❌ None | ❌ None | ❌ None |
| Markdown preview | ✅ `:preview` (browser) | ⚠️ Plugin (markdown-preview.nvim) | ❌ None | ❌ None | ❌ None |
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

**Why `:preview` over glow/mdcat?**

Terminal-based Markdown renderers like [glow](https://github.com/charmbracelet/glow) and [mdcat](https://github.com/swsnr/mdcat) are excellent for quick reads, but they're fundamentally limited by what a terminal can display. You can't render proportional fonts, complex table layouts, or inline images reliably in a character grid. `hi`'s `:preview` takes a different approach: it generates a complete HTML document with professional CSS and opens it in a real browser, giving you pixel-perfect rendering with zero compromise. And since it's integrated into the editor, it's just one command away — no context switching, no piping, no separate tool to install.

**Why `:preview` over grip?**

[grip](https://github.com/joeyespo/grip) renders Markdown through GitHub's API, which means it requires network access and is rate-limited. `hi`'s preview is fully offline, instant, and doesn't send your content anywhere.

---

## Configuration

```toml
# ~/.hirc

[general]
line_numbers = true
tab_width = 4

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

---

## Key Design Decisions

| Decision | Choice | Why |
|---|---|---|
| Language | Rust | Memory safety, zero-cost abstractions, modern tooling, proven by Helix/Zed |
| Text storage | Rope (ropey) | O(log n) edits on large files, efficient undo/redo snapshots |
| Terminal | crossterm | Cross-platform, no ncurses dependency |
| Syntax highlighting | syntect | Sublime Text grammars, 200+ languages, unified across editor + Chat |
| Markdown rendering | pulldown-cmark + syntect | CommonMark + GFM, code blocks share the same highlight engine |
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
