# Feature Overview

`hi` is organized around four capability pillars: **Navigation**, **Editing**, **Search**, and **AI**. Each pillar is designed to be immediately usable without memorizing a manual.

---

## Pillar 1 — Navigation

Getting to the right file and the right line should be instant. `hi` provides three complementary navigation tools that cover every scale of movement.

### File Tree

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

### Fuzzy File Picker — `Ctrl+P`

Press `Ctrl+P` to open the fuzzy file picker overlay. Type any subsequence of the filename — characters don't need to be adjacent. The picker scores matches by consecutive runs and highlights matched characters in the result list.

### Jump List

`hi` maintains a jump list across file positions. Use `Ctrl+O` to jump back and `Ctrl+I` to jump forward — the same muscle memory as Vim. Marks (`m{a-z}`, `` `{a-z} ``) let you pin specific positions for instant return.

---

## Pillar 2 — Editing

`hi` is a modal editor. Normal mode is for navigation and commands; Insert mode is for typing. The hint bar at the bottom of the screen always shows what keys are available in the current mode — you never need to remember.

### Modal Editing (Vim-compatible)

All standard Vim motions and operators work as expected: `w`/`b`/`e` for word movement, `f`/`t`/`;`/`,` for character search, `d`/`y`/`c`/`p` for delete/yank/change/put, `gg`/`G` for file navigation, `%` for bracket matching, and so on. Text objects (`iw`, `aw`, `i"`, `a(`, etc.) are fully supported.

### Undo / Redo

`u` undoes, `Ctrl+R` redoes. Undo history is stored as a grouped transaction tree — each insert session, substitution, or AI edit is a single undoable unit.

### Dot Repeat

`.` repeats the last change at the current cursor position. Works for insertions, deletions, substitutions, and character replacements.

### Named Registers

`"{a-z}y` yanks into a named register; `"{a-z}p` pastes from it. The `+` register maps to the system clipboard. Use `"` in Normal mode to set the active register before any yank or delete.

### Macro Recording and Playback

Record a sequence of keystrokes into a named register and replay it any number of times.

```
q{a-z}        start recording into register {a-z}
              (status bar shows  ● REC [a]  while recording)
q             stop recording
@{a-z}        play back the macro in register {a-z}
@@            replay the last-used macro
{n}@{a-z}     play back n times
```

### Visual Modes

`v` enters character-wise Visual, `V` enters line-wise Visual, `Ctrl+V` enters Visual Block. In Visual Block, `I` inserts text at the start of every selected line simultaneously.

### Command Line

`:` opens the command line. Supported commands include `:w`, `:q`, `:wq`, `:e {file}`, `:{n}` (go to line), `:%s/pat/rep/flags` (substitution), `:set nu`/`:set nonu`, `:!{cmd}` (shell command), `:theme`, `:grep`, and `:preview`. Command history is navigable with `↑`/`↓`, and Tab-completion is available for command names.

---

## Pillar 3 — Search

### In-file Search — `/`

Press `/` to enter search mode. Type a pattern (literal or regex), press `Enter` to confirm. `n`/`N` jump to the next/previous match. All matches are highlighted in the buffer. `:noh` clears the highlight.

### Global Grep — `Ctrl+F` or `:grep`

Press `Ctrl+F` (or type `:grep <pattern>`) to search across every file in the project. Results appear in a scrollable overlay showing the filename, line number, and the matching line with the match highlighted.

Regex mode is available with `:grep /pattern/` (slash-delimited). Pressing Enter on a result opens the file and jumps the cursor to the exact match position, centred in the viewport.

---

## Pillar 4 — AI

`?` is the AI key. Press it from Normal mode to describe your intent in plain language. `hi` reads the complexity of your request and responds accordingly.

### Advisor Mode — questions and explanations

When you ask a question, `hi` answers in the Chat panel without touching your file.

### Plan Mode — multi-step edits

When you ask for a complex transformation, `hi` shows you a plan and waits for your confirmation before making any changes.

### Ghost Text — command completion

For simple requests, `hi` fills in the command as ghost text. Press Tab to confirm.

### Chat Panel — `Ctrl+G`

`Ctrl+G` opens the persistent Chat panel on the right side of the screen. All AI responses accumulate here. You can scroll through the history, ask follow-up questions, and reference previous answers while editing.

**`hi` never acts without being asked.** `?` is the only trigger. No interruptions, no unsolicited suggestions.

---

## Syntax Highlighting — Two Engines, One Renderer

`hi` uses two purpose-built highlighting engines, each optimal for its role, feeding into a single unified renderer.

**Editor buffer → Tree-sitter** — an incremental, error-tolerant parser that builds a concrete syntax tree of your file. On every keystroke only the dirty subtree is re-parsed, so highlighting stays instant even on large files.

**AI Chat panel → syntect** — the same library that powers Sublime Text's highlighting, driven by `.tmLanguage` grammars. Ideal for rendering isolated code blocks inside Markdown responses.

**What this gives you:**

- Incremental parsing — Tree-sitter re-parses only the changed region on every keystroke.
- Viewport-only highlighting — rendering cost is O(tokens on screen) regardless of file size.
- 12 languages built-in for the editor: Rust, Python, Java, Go, JSON, YAML, TOML, Bash, HTML, JavaScript, TypeScript, Markdown.
- 200+ languages in Chat — any language Sublime Text supports is highlighted correctly in AI responses.
- Theme unification — one `[theme]` section in `~/.hirc` controls both the editor palette and the Chat panel theme.

---

## Markdown Preview — `:preview`

`hi` includes a built-in Markdown preview command that renders your `.md` file as a beautifully styled HTML page and opens it in your default browser.

---

## LeetCode Mode — `Ctrl+L`

`hi` includes a built-in LeetCode practice environment with a retro phosphor-green terminal aesthetic. Press `Ctrl+L` from the editor to enter LeetCode mode.

### Features

- Problem browser — browse, filter by difficulty (1/2/3), and search problems.
- Full Vim editing — the code editor uses the same Vim mode system as the main editor.
- Tree-sitter syntax highlighting — code is highlighted using the same incremental engine.
- Multi-language support — press `Ctrl+L` in the coding view to switch languages.
- Auto-save & restore — your solution is auto-saved every 5 seconds.
- Run & Submit — press `Ctrl+R` to run against test cases, `Ctrl+S` to submit.
- AI chat panel — toggle with `Ctrl+A` for AI-assisted problem solving.

### Setup

LeetCode mode requires authentication. On first launch, you'll be prompted to paste your `LEETCODE_SESSION` and `csrftoken` cookies from your browser. These are stored locally in `~/.hi/leetcode/`.

To build `hi` with LeetCode support:

```bash
cargo install --path . --features leetcode
```
