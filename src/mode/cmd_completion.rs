//! Command-line completion engine for `:` command mode.
//!
//! Provides a static registry of all known commands with descriptions,
//! a prefix-matching engine, and state management for the completion popup.

use crate::locale::Locale;

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
/// Descriptions here are en-US fallbacks; at runtime they are replaced by
/// locale strings via `CmdCompletionState::update_with_locale`.
const CMD_REGISTRY: &[CmdEntry] = &[
    CmdEntry { trigger: "w",            desc: "Save file",                          has_arg: false },
    CmdEntry { trigger: "q",            desc: "Quit",                               has_arg: false },
    CmdEntry { trigger: "q!",           desc: "Force quit (discard changes)",       has_arg: false },
    CmdEntry { trigger: "wq",           desc: "Save and quit",                      has_arg: false },
    CmdEntry { trigger: "x",            desc: "Save and quit",                      has_arg: false },
    CmdEntry { trigger: "e",            desc: "Open file",                          has_arg: true  },
    CmdEntry { trigger: "e!",           desc: "Reload current file",                has_arg: false },
    CmdEntry { trigger: "w ",           desc: "Save as\u{2026}",                    has_arg: true  },
    CmdEntry { trigger: "set nu",       desc: "Show line numbers",                  has_arg: false },
    CmdEntry { trigger: "set nonu",     desc: "Hide line numbers",                  has_arg: false },
    CmdEntry { trigger: "set tabstop=", desc: "Set tab width",                      has_arg: true  },
    CmdEntry { trigger: "noh",          desc: "Clear search highlight",             has_arg: false },
    CmdEntry { trigger: "theme",        desc: "Open theme picker",                  has_arg: false },
    CmdEntry { trigger: "theme ",       desc: "Switch theme by name",               has_arg: true  },
    CmdEntry { trigger: "u",            desc: "Undo",                               has_arg: false },
    CmdEntry { trigger: "d",            desc: "Delete current line",                has_arg: false },
    CmdEntry { trigger: "s/",           desc: "Substitute (current line) s/pat/rep/", has_arg: true },
    CmdEntry { trigger: "%s/",          desc: "Substitute all  %s/pat/rep/g",       has_arg: true  },
    CmdEntry { trigger: "!",            desc: "Run shell command",                  has_arg: true  },
    CmdEntry { trigger: "preview",      desc: "Preview Markdown in browser",        has_arg: false },
];

/// Returns the locale-translated description for a command trigger.
fn localized_desc<'a>(trigger: &str, locale: &'a Locale) -> &'a str {
    let c = &locale.commands;
    match trigger {
        "w"            => &c.cmd_w,
        "q"            => &c.cmd_q,
        "q!"           => &c.cmd_q_force,
        "wq"           => &c.cmd_wq,
        "x"            => &c.cmd_x,
        "e"            => &c.cmd_e,
        "e!"           => &c.cmd_e_reload,
        "w "           => &c.cmd_w_saveas,
        "set nu"       => &c.cmd_set_nu,
        "set nonu"     => &c.cmd_set_nonu,
        "set tabstop=" => &c.cmd_set_tabstop,
        "noh"          => &c.cmd_noh,
        "theme"        => &c.cmd_theme,
        "theme "       => &c.cmd_theme_name,
        "u"            => &c.cmd_u,
        "d"            => &c.cmd_d,
        "s/"           => &c.cmd_s,
        "%s/"          => &c.cmd_percent_s,
        "!"            => &c.cmd_shell,
        "preview"      => &c.cmd_preview,
        _              => "",
    }
}

/// A matched completion candidate (trigger + description + match score).
#[derive(Debug, Clone)]
pub struct Candidate {
    pub trigger: &'static str,
    pub desc: String,
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
    pub fn update(&mut self, input: &str, locale: &Locale) -> bool {
        self.items = match_commands(input, locale);
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
        Some(c.trigger.to_string())
    }

    /// Whether the popup should be visible.
    pub fn visible(&self) -> bool {
        !self.items.is_empty()
    }
}

/// Match commands against the current input prefix.
/// Returns candidates sorted by relevance (exact prefix first, then partial).
fn match_commands(input: &str, locale: &Locale) -> Vec<Candidate> {
    if input.is_empty() {
        // Show all commands when input is empty (just pressed `:`)
        return CMD_REGISTRY
            .iter()
            .map(|e| Candidate {
                trigger: e.trigger,
                desc: localized_desc(e.trigger, locale).to_string(),
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
                desc: localized_desc(entry.trigger, locale).to_string(),
                has_arg: entry.has_arg,
                score,
            });
        } else if input_lower.starts_with(&trigger_lower) {
            // Trigger is a prefix of input — user has typed past this command
            candidates.push(Candidate {
                trigger: entry.trigger,
                desc: localized_desc(entry.trigger, locale).to_string(),
                has_arg: entry.has_arg,
                score: 50,
            });
        }
    }

    // Sort: exact match first, then prefix matches, then partial
    candidates.sort_by_key(|c| (c.score, c.trigger.len()));
    candidates.truncate(10);
    candidates
}
