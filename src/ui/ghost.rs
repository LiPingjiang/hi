//! Ghost text overlay: shows a pending AI command suggestion in the command bar.

#[derive(Debug, Clone, Default)]
pub struct GhostText {
    /// Full text of the inline completion (may be multi-line).
    pub text: String,
    /// Single-line command summary shown in the command prompt area.
    pub command: String,
    pub explanation: String,
    pub visible: bool,
}

impl GhostText {
    pub fn show(&mut self, command: String, explanation: String) {
        self.command = command;
        self.explanation = explanation;
        self.visible = true;
    }

    pub fn clear(&mut self) {
        self.command.clear();
        self.explanation.clear();
        self.visible = false;
    }
}
