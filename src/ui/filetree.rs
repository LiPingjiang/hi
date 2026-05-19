//! File tree panel state and navigation.
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::fs;

#[derive(Debug, Clone)]
pub struct FileTreeNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
}

pub struct FileTree {
    pub root: PathBuf,
    pub nodes: Vec<FileTreeNode>,
    pub cursor: usize,
    pub show_hidden: bool,
    /// Paths that are currently expanded (persists across refresh)
    expanded_paths: HashSet<PathBuf>,
    /// Active search filter (None = show all, Some("") = search mode but no filter yet)
    pub filter: Option<String>,
    /// Indices into `nodes` that match the current filter
    pub visible_indices: Vec<usize>,
}

impl FileTree {
    pub fn new(root: impl Into<PathBuf>, show_hidden: bool) -> std::io::Result<Self> {
        let root = root.into();
        if !root.exists() { return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "dir not found")); }
        let mut ft = Self {
            root,
            nodes: vec![],
            cursor: 0,
            show_hidden,
            expanded_paths: HashSet::new(),
            filter: None,
            visible_indices: vec![],
        };
        ft.refresh();
        Ok(ft)
    }

    pub fn _new_unchecked(root: PathBuf, show_hidden: bool) -> Self {
        let mut ft = Self {
            root: root.clone(),
            nodes: vec![],
            cursor: 0,
            show_hidden,
            expanded_paths: HashSet::new(),
            filter: None,
            visible_indices: vec![],
        };
        ft.refresh();
        ft
    }


    pub fn refresh(&mut self) {
        self.nodes.clear();
        self.build_nodes(&self.root.clone(), 0);
        self.rebuild_visible();
    }

    /// Rebuild the visible_indices based on current filter.
    pub fn rebuild_visible(&mut self) {
        match &self.filter {
            None => {
                // No filter — all nodes visible
                self.visible_indices = (0..self.nodes.len()).collect();
            }
            Some(query) if query.is_empty() => {
                // Search mode active but empty query — show all
                self.visible_indices = (0..self.nodes.len()).collect();
            }
            Some(query) => {
                let q_lower = query.to_lowercase();
                // Find all nodes whose name matches the query
                let mut matched: HashSet<usize> = HashSet::new();
                for (i, node) in self.nodes.iter().enumerate() {
                    if node.name.to_lowercase().contains(&q_lower) {
                        matched.insert(i);
                        // Also include all ancestor directories to preserve tree structure
                        self.add_ancestors(i, &mut matched);
                    }
                }
                self.visible_indices = (0..self.nodes.len())
                    .filter(|i| matched.contains(i))
                    .collect();
            }
        }
        // Clamp cursor to visible range
        if !self.visible_indices.is_empty() {
            if self.cursor >= self.visible_indices.len() {
                self.cursor = self.visible_indices.len() - 1;
            }
        } else {
            self.cursor = 0;
        }
    }

    /// Walk backwards to find ancestor directories of node at `idx`.
    fn add_ancestors(&self, idx: usize, set: &mut HashSet<usize>) {
        let target_depth = self.nodes[idx].depth;
        if target_depth == 0 { return; }
        let mut looking_for_depth = target_depth - 1;
        let mut i = idx;
        while i > 0 && looking_for_depth < target_depth {
            i -= 1;
            let node = &self.nodes[i];
            if node.is_dir && node.depth == looking_for_depth {
                set.insert(i);
                if looking_for_depth == 0 { break; }
                looking_for_depth -= 1;
            }
        }
    }

    /// Start search mode.
    pub fn start_search(&mut self) {
        self.filter = Some(String::new());
        self.rebuild_visible();
    }

    /// Update the search query (called on each keystroke).
    pub fn update_filter(&mut self, query: &str) {
        self.filter = Some(query.to_string());
        self.rebuild_visible();
    }

    /// End search mode, keeping cursor at current position.
    pub fn end_search(&mut self) {
        // Translate visible cursor back to real node index
        let real_idx = self.visible_indices.get(self.cursor).copied().unwrap_or(0);
        self.filter = None;
        self.rebuild_visible();
        // Find the position of real_idx in the full visible list
        self.cursor = self.visible_indices.iter().position(|&i| i == real_idx).unwrap_or(0);
    }

    /// Cancel search mode, restore original view.
    pub fn cancel_search(&mut self) {
        self.filter = None;
        self.rebuild_visible();
    }

    /// Get the real node index for the current visible cursor position.
    pub fn real_cursor_idx(&self) -> Option<usize> {
        self.visible_indices.get(self.cursor).copied()
    }

    fn build_nodes(&mut self, dir: &Path, depth: usize) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        let mut dirs: Vec<PathBuf> = vec![];
        let mut files: Vec<PathBuf> = vec![];

        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("").to_string();
            if !self.show_hidden && name.starts_with('.') { continue; }
            if path.is_dir() { dirs.push(path); } else { files.push(path); }
        }
        dirs.sort();
        files.sort();

        for path in dirs.iter().chain(files.iter()) {
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?").to_string();
            let is_dir = path.is_dir();
            // Look up expanded state from the persistent set, not from nodes
            // (nodes is already cleared at this point)
            let expanded = is_dir && self.expanded_paths.contains(path);
            self.nodes.push(FileTreeNode {
                path: path.clone(),
                name,
                is_dir,
                depth,
                expanded,
            });
            if is_dir && expanded {
                self.build_nodes(path, depth + 1);
            }
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.visible_indices.len() {
            self.cursor += 1;
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 { self.cursor -= 1; }
    }

    /// Enter / expand; returns Some(path) if a file was selected.
    pub fn enter(&mut self) -> Option<PathBuf> {
        let real_idx = self.real_cursor_idx()?;
        let node = self.nodes.get(real_idx)?;
        if node.is_dir {
            let path = node.path.clone();
            // Toggle the persistent expanded set first, then rebuild
            if self.expanded_paths.contains(&path) {
                self.expanded_paths.remove(&path);
            } else {
                self.expanded_paths.insert(path);
            }
            self.refresh();
            None
        } else {
            Some(node.path.clone())
        }
    }

    /// Collapse current dir / go to parent.
    pub fn collapse_or_parent(&mut self) {
        let real_idx = match self.real_cursor_idx() {
            Some(i) => i,
            None => return,
        };
        let node_depth = self.nodes[real_idx].depth;
        // If current node is an expanded dir, collapse it
        if self.nodes[real_idx].is_dir && self.nodes[real_idx].expanded {
            let path = self.nodes[real_idx].path.clone();
            self.expanded_paths.remove(&path);
            self.refresh();
            return;
        }
        if node_depth == 0 { return; }
        // Walk back in the full nodes list to find parent dir
        let mut i = real_idx;
        loop {
            if i == 0 { break; }
            i -= 1;
            if self.nodes[i].is_dir && self.nodes[i].depth < node_depth {
                let path = self.nodes[i].path.clone();
                self.expanded_paths.remove(&path);
                self.refresh();
                // Try to place cursor at the parent in visible list
                if let Some(pos) = self.visible_indices.iter().position(|&vi| vi == i) {
                    self.cursor = pos;
                }
                break;
            }
        }
    }

    /// Returns lines to render, one per node.
    pub fn render_lines(&self) -> Vec<String> {
        self.nodes.iter().map(|n| {
            let indent = "  ".repeat(n.depth);
            let prefix = if n.is_dir {
                if n.expanded { "▾ " } else { "▸ " }
            } else {
                // Use different icons by extension
                let icon = match n.path.extension().and_then(|e| e.to_str()) {
                    Some("rs")                          => "🦀 ",
                    Some("toml") | Some("yaml") | Some("yml") | Some("json") => "⚙ ",
                    Some("md") | Some("txt")            => "📄 ",
                    Some("sh") | Some("bash") | Some("zsh") => "⚡ ",
                    Some("py")                          => "🐍 ",
                    Some("js") | Some("ts") | Some("jsx") | Some("tsx") => "🌐 ",
                    Some("go")                          => "🐹 ",
                    Some("lock")                        => "🔒 ",
                    _                                   => "  ",
                };
                icon
            };
            format!("{}{}{}", indent, prefix, n.name)
        }).collect()
    }

    pub fn selected_path(&self) -> Option<&Path> {
        let real_idx = self.real_cursor_idx()?;
        self.nodes.get(real_idx).map(|n| n.path.as_path())
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.refresh();
    }
}
