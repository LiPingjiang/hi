pub mod normal;
pub mod insert;
pub mod visual;
pub mod command;
pub mod ai;

/// Visual selection variant.
#[derive(Debug, Clone, PartialEq)]
pub enum VisualKind {
    Char,
    Line,
    Block,
}

/// The editor mode state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Visual {
        kind: VisualKind,
        anchor: usize, // char index where selection started
    },
    Command(String),  // accumulated command-line text
    Search(String),   // accumulated search pattern
    Ai(String),       // accumulated AI query text
}

impl Mode {
    pub fn name(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Visual { kind: VisualKind::Block, .. } => "V-BLOCK",
            Mode::Visual { kind: VisualKind::Line, .. }  => "V-LINE",
            Mode::Visual { .. } => "VISUAL",
            Mode::Command(_) => "COMMAND",
            Mode::Search(_) => "SEARCH",
            Mode::Ai(_) => "AI",
        }
    }

    pub fn is_normal(&self) -> bool { matches!(self, Mode::Normal) }
    pub fn is_insert(&self) -> bool { matches!(self, Mode::Insert) }
    pub fn is_visual(&self) -> bool { matches!(self, Mode::Visual { .. }) }
    pub fn is_command(&self) -> bool { matches!(self, Mode::Command(_)) }
    pub fn is_search(&self) -> bool { matches!(self, Mode::Search(_)) }
    pub fn is_ai(&self) -> bool { matches!(self, Mode::Ai(_)) }
}
