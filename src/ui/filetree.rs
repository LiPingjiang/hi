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
        };
        ft.refresh();
        ft
    }


    pub fn refresh(&mut self) {
        self.nodes.clear();
        self.build_nodes(&self.root.clone(), 0);
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
        if self.cursor + 1 < self.nodes.len() {
            self.cursor += 1;
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 { self.cursor -= 1; }
    }

    /// Enter / expand; returns Some(path) if a file was selected.
    pub fn enter(&mut self) -> Option<PathBuf> {
        let Some(node) = self.nodes.get(self.cursor) else { return None };
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
        let node_depth = self.nodes.get(self.cursor).map(|n| n.depth).unwrap_or(0);
        // If current node is an expanded dir, collapse it
        if let Some(node) = self.nodes.get(self.cursor) {
            if node.is_dir && node.expanded {
                let path = node.path.clone();
                self.expanded_paths.remove(&path);
                self.refresh();
                return;
            }
        }
        if node_depth == 0 { return; }
        // Otherwise walk back to find parent dir and collapse it
        let mut i = self.cursor;
        loop {
            if i == 0 { break; }
            i -= 1;
            if let Some(n) = self.nodes.get(i) {
                if n.is_dir && n.depth < node_depth {
                    let path = n.path.clone();
                    self.expanded_paths.remove(&path);
                    self.cursor = i;
                    self.refresh();
                    break;
                }
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
        self.nodes.get(self.cursor).map(|n| n.path.as_path())
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.refresh();
    }
}
