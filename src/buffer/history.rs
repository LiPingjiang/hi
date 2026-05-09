/// One atomic change: before/after snapshots + cursor positions.
/// Using full snapshots keeps the implementation simple and correct;
/// for very large files this can be optimised to delta patches later.
#[derive(Clone)]
pub struct ChangeSet {
    pub before: String,
    pub after: String,
    pub cursor_before: usize,
    pub cursor_after: usize,
}

pub struct History {
    undo_stack: Vec<ChangeSet>,
    redo_stack: Vec<ChangeSet>,
    /// When true, the next push merges into the top change group
    /// (used to batch AI-generated multi-step operations).
    group_next: bool,
}

impl History {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            group_next: false,
        }
    }

    pub fn push(&mut self, cs: ChangeSet) {
        // Any new change clears the redo stack.
        self.redo_stack.clear();
        if self.group_next {
            // Merge: extend the top entry's "after" so the whole group
            // undoes in one step.
            if let Some(top) = self.undo_stack.last_mut() {
                top.after = cs.after;
                top.cursor_after = cs.cursor_after;
                self.group_next = false;
                return;
            }
        }
        self.undo_stack.push(cs);
    }

    /// Call before an AI-triggered sequence so all steps undo together.
    pub fn breakpoint(&mut self) {
        self.group_next = false; // next push starts a fresh group normally
    }

    /// Begin grouping: subsequent pushes merge into the current top.
    pub fn begin_group(&mut self) {
        self.group_next = true;
    }

    pub fn undo(&mut self) -> Option<ChangeSet> {
        let cs = self.undo_stack.pop()?;
        self.redo_stack.push(cs.clone());
        Some(cs)
    }

    pub fn redo(&mut self) -> Option<ChangeSet> {
        let cs = self.redo_stack.pop()?;
        self.undo_stack.push(cs.clone());
        Some(cs)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}
