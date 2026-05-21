# Comparison with Other Terminal Tools

## vs. Terminal Editors

| Feature | hi | Vim/Neovim | Helix | micro | nano |
|---|---|---|---|---|---|
| True-color syntax highlighting | ✅ Built-in (Tree-sitter, incremental) | ✅ (requires config) | ✅ Tree-sitter | ✅ Limited | ❌ Basic |
| AI integration | ✅ Native (`?` key) | ⚠️ Plugin (Copilot.vim) | ❌ None | ❌ None | ❌ None |
| Fuzzy file picker | ✅ `Ctrl+P` built-in | ⚠️ Plugin (fzf.vim) | ✅ Built-in | ❌ None | ❌ None |
| Global grep | ✅ `Ctrl+F` / `:grep` | ⚠️ Plugin (fzf / telescope) | ✅ Built-in | ❌ None | ❌ None |
| Macro recording | ✅ `q{reg}` / `@{reg}` | ✅ Full | ❌ None | ❌ None | ❌ None |
| Markdown preview | ✅ `:preview` (browser) | ⚠️ Plugin | ❌ None | ❌ None | ❌ None |
| LeetCode integration | ✅ Built-in (`Ctrl+L`) | ⚠️ Plugin (leetcode.vim) | ❌ None | ❌ None | ❌ None |
| Theme live-switching | ✅ `:theme` with real-time preview | ⚠️ `:colorscheme` (no preview) | ✅ `:theme` | ⚠️ Config file | ❌ N/A |
| Learning curve | Low (hint bar + AI) | Very high | Medium | Low | Very low |
| Startup time | ~5ms | ~50ms (Neovim + plugins) | ~10ms | ~10ms | ~5ms |
| Language | Rust | C / Lua | Rust | Go | C |
| Config format | TOML (`~/.hirc`) | Vimscript / Lua | TOML | JSON | nanorc |

## vs. Markdown Renderers

| Feature | hi `:preview` | glow | mdcat | grip | Marked (VS Code) |
|---|---|---|---|---|---|
| Rendering target | Browser (full HTML/CSS) | Terminal (ANSI) | Terminal (ANSI) | Browser (GitHub API) | VS Code panel |
| Visual fidelity | ★★★★★ Full CSS styling | ★★★ Limited by terminal | ★★★ Limited by terminal | ★★★★★ GitHub-identical | ★★★★★ Full CSS |
| Tables | ✅ Proper HTML tables | ✅ Box-drawing | ✅ Box-drawing | ✅ GitHub-rendered | ✅ HTML tables |
| Code blocks | ✅ Monospace, styled | ✅ Colored background | ✅ Syntax highlighted | ✅ GitHub highlighting | ✅ Syntax highlighted |
| Images | ✅ Full rendering | ❌ Not displayed | ⚠️ iTerm2/Kitty only | ✅ Full rendering | ✅ Full rendering |
| Requires network | ❌ Fully offline | ❌ Offline | ❌ Offline | ✅ GitHub API | ❌ Offline |
| Integrated with editor | ✅ One keystroke | ❌ Separate tool | ❌ Separate tool | ❌ Separate tool | ✅ VS Code only |

## Key Design Decisions

| Decision | Choice | Why |
|---|---|---|
| Language | Rust | Memory safety, zero-cost abstractions, modern tooling, proven by Helix/Zed |
| Text storage | Rope (ropey) | O(log n) edits on large files; undo history stored as text patches |
| Terminal | crossterm | Cross-platform, no ncurses dependency |
| Editor syntax highlighting | Tree-sitter | Incremental CST, viewport-only query, zero frame drops |
| Chat syntax highlighting | syntect | Sublime Text grammars, 200+ languages |
| Markdown rendering | pulldown-cmark + syntect | CommonMark + GFM, code blocks share the syntect engine |
| Config | `~/.hirc` (TOML) | Simple path, readable format |
| AI trigger | `?` | Semantic fit (question mark = ask), symmetric with `/` (search) |
| LLM backend | Configurable | Any OpenAI-compatible API: OpenAI, Claude, Ollama, others |
