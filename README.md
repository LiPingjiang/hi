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

## Key Design Decisions

| Decision | Choice | Why |
|---|---|---|
| Language | Rust | Memory safety, zero-cost abstractions, modern tooling, proven by Helix/Zed |
| Text storage | Rope (ropey) | O(log n) edits on large files, efficient undo/redo snapshots |
| Terminal | crossterm | Cross-platform, no ncurses dependency |
| Config | `~/.hirc` (TOML) | Simple path, readable format |
| AI trigger | `?` | Semantic fit (question mark = ask), symmetric with `/` (search) |
| LLM backend | Configurable | Any OpenAI-compatible API: OpenAI, Claude, Ollama, others |

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
```

---

## Status

🚧 **Pre-alpha. Docs and architecture in progress.**

See [`docs/PRODUCT.md`](docs/PRODUCT.md) for the full product vision.
See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the technical design.

---

## Philosophy

Vim taught us that modal editing is the right model for keyboard-driven text manipulation. We keep that. Everything else is rebuilt.

The goal is not to be a better Vim. The goal is to be the editor you reach for when you open a terminal and need to get something done — without stopping to remember which key does what.
