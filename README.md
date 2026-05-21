<p align="center">
  <pre align="center">
  ██╗  ██╗ ██╗
  ██║  ██║ ██║
  ███████║ ██║
  ██╔══██║ ██║
  ██║  ██║ ██║
  ╚═╝  ╚═╝ ╚═╝
  </pre>
</p>

<p align="center">
  <strong>A terminal editor for the AI era.</strong><br>
  <sub>Rust-powered · Vim-compatible · AI-native · Zero config</sub>
</p>

<p align="center">
  <a href="https://github.com/LiPingjiang/hi/releases">Releases</a> ·
  <a href="docs/INSTALL.md">Install</a> ·
  <a href="docs/FEATURES.md">Features</a> ·
  <a href="docs/CONFIGURATION.md">Config</a> ·
  <a href="docs/COMPARISON.md">Comparison</a>
</p>

---

## Why hi

`hi` is what you type to say hello to a file. Two letters, fast to type, easy to remember.

```bash
hi main.rs
hi .
```

The name says it all — `hi` ≈ `vi` (modal editing heritage), `hi` ≈ `ai` (AI-native core), `hi` = hello (friendly, immediate).

---

## Highlights

### ⚡ Rust-Powered, Instant Startup

Pure Rust from the ground up. ~5ms cold start — faster than your shell prompt. Rope-based text storage (ropey) handles large files with O(log n) edits. No GC pauses, no runtime overhead.

### 🎨 Tree-sitter Syntax Highlighting

Incremental parsing that re-processes only the changed subtree on each keystroke. Viewport-only rendering means zero frame drops regardless of file size. 12 languages built-in, 200+ in AI chat via syntect.

### 🤖 AI-Native Interaction

Press `?` to talk to AI. No plugins, no config, no leaving the editor.

<img width="1906" alt="AI chat, error detection, and collaborative editing" src="https://github.com/user-attachments/assets/e0b7ee7a-50a8-4d26-a6d4-45fd2044d973" />

AI understands your context — current file, cursor position, selected text — and responds with one of three modes: direct answer (advisor), ghost-text command completion, or a multi-step execution plan with confirmation.

### 💡 Smart Hints & Command Completion

The bottom hint bar always shows available keys for the current mode. You never need to memorize — just look down.

<img width="825" alt="Smart command hints and completion" src="https://github.com/user-attachments/assets/76353934-7794-4024-b7cc-83bdb5b1f565" />

### 📚 Interactive Learning

Built-in tutorial board teaches you as you edit. AI answers questions about the editor itself — ask `? how do I select a paragraph` and get an instant answer.

<img width="1904" alt="Interactive learning and usage" src="https://github.com/user-attachments/assets/698c4039-4f88-481b-84c7-b8d370818fc0" />

### 🏯 LeetCode 古法时代

Built-in LeetCode environment with retro phosphor-green CRT aesthetic. Browse problems, write solutions with full Vim editing, run tests, submit — all without leaving the terminal.

<img width="1908" alt="LeetCode submit" src="https://github.com/user-attachments/assets/178f3519-69ca-4d1e-972a-ac32d1726ee0" />

---

## Quick Start

```bash
# Install (macOS / Linux)
curl -fsSL https://raw.githubusercontent.com/LiPingjiang/hi/main/install.sh | sh

# 国内加速（推荐）
curl -fsSL https://ghproxy.com/https://raw.githubusercontent.com/LiPingjiang/hi/main/install.sh | sh

# Or via Homebrew
brew tap LiPingjiang/tap && brew install hi

# Or via cargo
cargo install hi
```

> ⚠️ Requires a true-color terminal (iTerm2, Kitty, Alacritty, WezTerm, Ghostty). macOS Terminal.app is **not supported**. See [Install Guide](docs/INSTALL.md) for details.

---

## At a Glance

| | |
|---|---|
| `?` | Ask AI anything |
| `Ctrl+P` | Fuzzy file picker |
| `Ctrl+F` | Global grep |
| `Ctrl+G` | AI Chat panel |
| `Ctrl+L` | LeetCode mode |
| `Ctrl+\` | File tree |
| `:theme` | Live theme switcher |
| `:preview` | Markdown → browser |

Full keybinding reference → [docs/CONFIGURATION.md](docs/CONFIGURATION.md)

---

## Documentation

| Doc | Description |
|---|---|
| [Installation](docs/INSTALL.md) | All install methods + terminal requirements |
| [Features](docs/FEATURES.md) | Complete feature walkthrough |
| [Configuration](docs/CONFIGURATION.md) | `~/.hirc` options, themes, i18n, keybindings |
| [Comparison](docs/COMPARISON.md) | vs. Vim/Neovim, Helix, micro, nano |
| [Architecture](docs/ARCHITECTURE.md) | Technical design & internals |
| [Product Vision](docs/PRODUCT.md) | Roadmap & philosophy |

---

## Philosophy

Vim taught us that modal editing is the right model for keyboard-driven text manipulation. We keep that. Everything else is rebuilt for the AI era — the goal is the editor you reach for when you open a terminal and need to get something done, without stopping to remember which key does what.

---

## Contributing

Contributions are welcome. Before submitting a pull request, please read the [Contributor License Agreement](CLA.md). By opening a PR you agree to its terms.

## License

Apache-2.0 — see [LICENSE](LICENSE) for details.
