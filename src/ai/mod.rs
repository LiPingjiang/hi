pub mod context;
pub mod client;
pub mod log;
pub mod prompt;
pub mod parser;
pub mod hint;
pub mod tools;
pub mod edit_session;

pub use context::AiContext;
pub use client::AiClient;
pub use hint::HintKind;
pub use tools::{AiTool, ToolResult, EditDiff, DiffHunk, HunkKind, OutlineItem};
pub use edit_session::{AiEditSession, EditPhase, EditSource, AiEditStepResult};
