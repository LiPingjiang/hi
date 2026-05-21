# Changelog

## [0.1.7] - 2026-05-19

### Added
- **LeetCode Mode** (`Ctrl+L`) — built-in LeetCode practice environment with retro phosphor-green terminal aesthetic
  - Problem browser with difficulty filtering (Easy/Medium/Hard), search, and pagination
  - Full Vim mode system in the code editor: Normal, Insert, Visual, Command, and Search modes — identical UX to the main editor
  - **Tree-sitter syntax highlighting** for LeetCode code: Rust, Python, Java, Go, JavaScript, TypeScript, Bash
  - Multi-language support with `Ctrl+L` language switcher
  - Auto-save every 5 seconds with full state restore (code, cursor position, language selection)
  - Run (`Ctrl+R` / `:run`) and Submit (`Ctrl+S` / `:submit`) with result display
  - Problem description panel (toggle `Ctrl+D`) and AI chat panel (toggle `Ctrl+A`)
  - Session-based authentication with cookie persistence
  - LeetCode CN and Global site support
- **Blinking cursor** in LeetCode code editor (Insert mode: blinking bar, Normal mode: steady block)
- **Code persistence** — solutions are saved to `~/.hi/leetcode/` and restored on reopen

### Fixed
- Program exit (panic) when pressing Enter on a LeetCode problem with non-UTF-8 content
- Esc key in LeetCode editor now switches Vim modes instead of exiting to problem list

## [0.1.2] - 2025-05-12

### Added
- **Markdown rendering engine** (`src/ui/mdrender.rs`) — surpasses glow's visual quality
  - Full pulldown-cmark parser: headings, paragraphs, lists, blockquotes, tables, code blocks, inline elements
  - **syntect-powered code block syntax highlighting** with 200+ language support (Rust, Python, Go, Java, JS, C/C++, SQL, etc.)
  - H1 with background color + bold underline (━), H2 with thin underline (─), H3-H6 with distinct colors
  - Inline code with background highlight, **bold**, *italic*, ~~strikethrough~~, [links](url) with underline
  - Blockquotes with colored │ border decoration, nested blockquote support
  - Tables with Unicode box-drawing borders (┌┬┐├┼┤└┴┘), bold headers
  - Code blocks with rounded borders (╭╮╰╯), language label, full-width background
  - Task list markers (☑/☐), horizontal rules, image placeholders
  - Word-wrapping for styled spans respecting CJK double-width characters
- **3 built-in themes**: dark (default), dracula, tokyo-night — all with carefully tuned RGB colors
- 13 new unit tests for the Markdown renderer
- **Unified syntax highlighting architecture** (Phase 6) — editor text area and Chat panel code blocks now share the same syntect engine
  - `SyntectHighlighter`: stateful per-file highlighter with `HighlightLines` state machine for correct multi-line parsing (block comments, heredocs, multi-line strings)
  - `SyntectSpan`: new rendering primitive with direct RGB colors from Sublime Text themes, replacing the legacy `TokenKind` enum
  - `OverlayKind`: compositing system for search highlights and Visual Block selections painted on top of syntax colors
  - `FileType::syntect_token()`: maps 12+ file types to syntect syntax lookup tokens
  - Pixel-perfect color consistency between the file being edited and AI code snippets
- **Theme system with config support** (Phase 7) — `~/.hirc` `[theme]` section now drives both editor and Chat panel themes
  - `editor_theme`: syntect theme name for the editor text area (e.g. `"base16-ocean.dark"`, `"Solarized (dark)"`)
  - `chat_theme`: MdTheme name for the Chat panel Markdown renderer (e.g. `"dark"`, `"dracula"`, `"tokyo-night"`)
  - `SyntectHighlighter::set_theme()`: runtime theme switching support (foundation for future `:colorscheme` command)

### Changed
- Chat panel now renders AI responses with full Markdown formatting instead of plain text
- `Renderer` struct now holds `MdRenderer` and `SyntectHighlighter` instances
- `Renderer::new()` now accepts `&Config` to initialize themes from user configuration
- `Renderer::render_line_with_spans()` rewritten to accept `&[SyntectSpan]` with overlay compositing
- `ChatPanel::render_lines_styled()` replaces plain `render_lines()` for the rendering pipeline
- Editor text rendering switched from hand-written regex rules to syntect (200+ languages)
- `ThemeConfig` expanded with `editor_theme` and `chat_theme` fields

### Dependencies
- Added `pulldown-cmark` 0.12 (Markdown parser, CommonMark + GFM extensions)
- Added `syntect` 5 (syntax highlighting, 200+ languages, Sublime Text themes)

## [0.1.1] - 2025-05-11

### Added
- Right-side AI Chat panel with editable input line and cursor positioning
- Mouse event handling: scroll follows mouse position, click sets focus+cursor, drag enters Visual mode
- Markdown syntax highlighting (headings, code blocks, bold, italic, links, blockquotes, list markers, horizontal rules)
- Rust syntax highlighting (keywords, comments, strings, numbers, macros, lifetimes, attributes, types)
- `FocusZone` system: Editor / FileTree / Chat with Tab/Shift+Tab cycling
- `layout_regions()` and `zone_at()` helpers for screen coordinate → zone mapping

### Changed
- Chat panel no longer requires pressing `i` to start typing — input mode activates automatically when focused
- `ToggleChatPanel` (Ctrl+l) now auto-focuses Chat on open
- Enter submits message and stays in input mode for follow-up conversation
- Esc in Chat returns focus to Editor directly
- Chat history scrolling available via Ctrl+p/n (single line) and PageUp/PageDown (page)
- Ctrl+d clears chat history (replaces uppercase D in old browse mode)

### Fixed
- Unused variable warnings in `layout_regions()` destructuring
- Unnecessary parentheses in `highlight_markdown`

## [0.1.0] - Initial Release

### Added
- Modal editing (Normal / Insert / Visual / Command modes)
- File tree sidebar with toggle
- Syntax highlighting for 10+ languages
- Search and replace
- Macro recording and playback
- Configurable via `~/.config/hi/config.toml`
