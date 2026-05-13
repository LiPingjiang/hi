//! Command-line completion engine for `:` command mode.
//!
//! Provides a static registry of all known commands with descriptions,
//! a prefix-matching engine, and state management for the completion popup.

/// A single command entry in the registry.
#[derive(Debug, Clone)]
pub struct CmdEntry {
    /// The command trigger text (what the user types after `:`).
    pub trigger: &'static str,
    /// Short description shown in the completion popup.
    pub desc: &'static str,
    /// If true, the command expects an argument (space appended on accept).
    pub has_arg: bool,
}

/// Static registry of all known commands.
/// Ordered by rough frequency-of-use so the default list feels natural.
const CMD_REGISTRY: &[CmdEntry] = &[
    CmdEntry { trigger: "w",           desc: "保存文件",                     has_arg: false },
    CmdEntry { trigger: "q",           desc: "退出",                         has_arg: false },
    CmdEntry { trigger: "q!",          desc: "强制退出（不保存）",           has_arg: false },
    CmdEntry { trigger: "wq",          desc: "保存并退出",                   has_arg: false },
    CmdEntry { trigger: "x",           desc: "保存并退出",                   has_arg: false },
    CmdEntry { trigger: "e",           desc: "打开文件",                     has_arg: true  },
    CmdEntry { trigger: "e!",          desc: "重新加载当前文件",             has_arg: false },
    CmdEntry { trigger: "w ",          desc: "另存为…",                      has_arg: true  },
    CmdEntry { trigger: "set nu",      desc: "显示行号",                     has_arg: false },
    CmdEntry { trigger: "set nonu",    desc: "隐藏行号",                     has_arg: false },
    CmdEntry { trigger: "set tabstop=",desc: "设置 Tab 宽度",               has_arg: true  },
    CmdEntry { trigger: "noh",         desc: "清除搜索高亮",                 has_arg: false },
    CmdEntry { trigger: "theme",       desc: "打开主题选择器",               has_arg: false },
    CmdEntry { trigger: "theme ",      desc: "切换主题（输入名称）",         has_arg: true  },
    CmdEntry { trigger: "u",           desc: "撤销",                         has_arg: false },
    CmdEntry { trigger: "d",           desc: "删除当前行",                   has_arg: false },
    CmdEntry { trigger: "s/",          desc: "替换（当前行）s/pat/rep/",     has_arg: true  },
    CmdEntry { trigger: "%s/",         desc: "全文替换 %s/pat/rep/g",        has_arg: true  },
    CmdEntry { trigger: "!",           desc: "执行 Shell 命令",              has_arg: true  },
    CmdEntry { trigger: "preview",     desc: "浏览器预览 Markdown",           has_arg: false },
];

/// A matched completion candidate (trigger + description + match score).
#[derive(Debug, Clone)]
pub struct Candidate {
    pub trigger: &'static str,
    pub desc: &'static str,
    pub has_arg: bool,
    /// Lower is better. 0 = exact prefix match.
    pub score: u32,
}

/// Completion popup state, held by App.
pub struct CmdCompletionState {
    /// Current list of matching candidates.
    pub items: Vec<Candidate>,
    /// Index of the selected (highlighted) candidate, if any.
    pub selected: Option<usize>,
}

impl CmdCompletionState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected: None,
        }
    }

    /// Recompute candidates based on the current command input.
    /// Returns true if there are any candidates to show.
    pub fn update(&mut self, input: &str) -> bool {
        self.items = match_commands(input);
        // Reset selection when the list changes
        self.selected = if self.items.is_empty() { None } else { Some(0) };
        !self.items.is_empty()
    }

    /// Move selection down (wraps around).
    pub fn select_next(&mut self) {
        if self.items.is_empty() { return; }
        self.selected = Some(match self.selected {
            Some(i) => (i + 1) % self.items.len(),
            None => 0,
        });
    }

    /// Move selection up (wraps around).
    pub fn select_prev(&mut self) {
        if self.items.is_empty() { return; }
        self.selected = Some(match self.selected {
            Some(0) | None => self.items.len().saturating_sub(1),
            Some(i) => i - 1,
        });
    }

    /// Accept the currently selected candidate.
    /// Returns the text to replace the command input with.
    pub fn accept(&self) -> Option<String> {
        let idx = self.selected?;
        let c = self.items.get(idx)?;
        if c.has_arg {
            // Append space for commands that take arguments
            // (unless trigger already ends with space or special char)
            let t = c.trigger.to_string();
            Some(t)
        } else {
            Some(c.trigger.to_string())
        }
    }

    /// Whether the popup should be visible.
    pub fn visible(&self) -> bool {
        !self.items.is_empty()
    }
}

/// Match commands against the current input prefix.
/// Returns candidates sorted by relevance (exact prefix first, then partial).
fn match_commands(input: &str) -> Vec<Candidate> {
    if input.is_empty() {
        // Show all commands when input is empty (just pressed `:`)
        return CMD_REGISTRY
            .iter()
            .map(|e| Candidate {
                trigger: e.trigger,
                desc: e.desc,
                has_arg: e.has_arg,
                score: 100,
            })
            .collect();
    }

    let input_lower = input.to_lowercase();
    let mut candidates: Vec<Candidate> = Vec::new();

    for entry in CMD_REGISTRY {
        let trigger_lower = entry.trigger.to_lowercase();

        if trigger_lower.starts_with(&input_lower) {
            // Input is a prefix of the trigger — strong match
            let score = if trigger_lower == input_lower { 0 } else { 1 };
            candidates.push(Candidate {
                trigger: entry.trigger,
                desc: entry.desc,
                has_arg: entry.has_arg,
                score,
            });
        } else if input_lower.starts_with(&trigger_lower) {
            // Trigger is a prefix of input — user has typed past this command
            // Still show it but with lower priority (helps with subcommands)
            candidates.push(Candidate {
                trigger: entry.trigger,
                desc: entry.desc,
                has_arg: entry.has_arg,
                score: 50,
            });
        }
    }

    // Sort: exact match first, then prefix matches, then partial
    candidates.sort_by_key(|c| (c.score, c.trigger.len()));

    // Deduplicate: if we have an exact match, don't show it again
    // Limit to reasonable number
    candidates.truncate(10);
    candidates
}
