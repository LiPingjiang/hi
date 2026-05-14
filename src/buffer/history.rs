/// One atomic change stored as a **patch** (char-index range + replaced text).
///
/// Instead of snapshotting the full buffer before and after every edit
/// (which costs two O(n) `to_string()` calls per keystroke), we record only
/// the minimal information needed to invert the operation:
///
/// ```text
///   undo: rope.remove(start..start+new_len); rope.insert(start, &old_text)
///   redo: rope.remove(start..start+old_len); rope.insert(start, &new_text)
/// ```
///
/// For a single-character insert `old_text` is empty and `new_text` is one
/// char; for a delete it is the reverse.  This keeps memory and CPU cost
/// proportional to the *edit size*, not the *file size*.
#[derive(Clone)]
pub struct ChangeSet {
    /// Char index where the edit starts.
    pub start: usize,
    /// Text that was at `start..start+old_text.chars().count()` before the edit.
    pub old_text: String,
    /// Text that replaced it (i.e. what is now at `start..start+new_text.chars().count()`).
    pub new_text: String,
    /// Cursor position before the edit (for undo).
    pub cursor_before: usize,
    /// Cursor position after the edit (for redo).
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
            // Merge: extend the top entry's new_text so the whole group
            // undoes in one step.  This is a best-effort merge for
            // sequential appends; complex overlapping edits start fresh.
            if let Some(top) = self.undo_stack.last_mut() {
                // Only merge if the new edit starts right where the previous one ended.
                let top_new_end = top.start + top.new_text.chars().count();
                if cs.start == top_new_end {
                    top.new_text.push_str(&cs.new_text);
                    top.cursor_after = cs.cursor_after;
                    self.group_next = false;
                    return;
                }
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
