//! Status bar rendering: info line + context-aware hint line.
use crate::editor::Editor;
use crate::mode::Mode;
use crate::syntax::highlight::FileType;

impl Editor {
    pub fn hint_line(&self) -> String {
        // File tree has focus — override all other hints
        if self.filetree_visible && self.filetree_focus {
            return "[j/k]上下移动  [l/Enter]打开/展开  [h]折叠  [g/G]顶/底  [H]显隐文件  [Ctrl+w/Esc]返回编辑".to_string();
        }
        match &self.mode {
            Mode::Visual { .. } =>
                "[y]复制  [d]删除  [c]替换  [>]缩进  [<]反缩进  [?]AI操作选区  [Esc]退出".to_string(),
            Mode::Insert =>
                "正在输入...  [Esc]返回Normal  [Ctrl+w]删词  [Ctrl+u]删至行首".to_string(),
            Mode::Command(_) =>
                ":w保存  :q退出  :wq保存退出  :%s/查找/替换/g  [Esc]取消".to_string(),
            Mode::Search(_) =>
                "输入搜索词，Enter确认  n/N跳转  [Esc]取消".to_string(),
            Mode::Ai(_) =>
                "描述你的意图，按Enter发送  [Tab]确认建议  [Esc]取消  示例：把所有ERROR替换为WARN".to_string(),
            Mode::Normal => {
                // Macro recording takes highest priority
                if let Some(reg) = self.macro_recording {
                    return format!("● 录制宏 @{}  [q]停止录制  操作将被记录", reg);
                }
                // Register prefix waiting
                if self.active_register.is_some() || self.pending_key == Some('"') {
                    return "[a-z]选择寄存器  \"ayy→复制到a  \"ap→从a粘贴  \"+y→系统剪贴板".to_string();
                }
                // Search highlight active
                if self.search_highlight {
                    return "搜索高亮中  [n]下一个  [N]上一个  [/]新搜索  [:noh]清除高亮".to_string();
                }

                let line = self.buffer.line_str(self.cursor_line);
                let col = self.cursor_col;
                let chars: Vec<char> = line.chars().collect();
                let trimmed = line.trim();

                // Empty line
                if trimmed.is_empty() {
                    return "[i]在此插入  [o]下方新行  [O]上方新行  [dd]删除空行  [?]AI".to_string();
                }

                // Comment line (// # -- /* )
                if trimmed.starts_with("//") || trimmed.starts_with('#')
                    || trimmed.starts_with("--") || trimmed.starts_with("/*")
                {
                    return "[gcc]切换注释  [yy]复制注释  [dd]删除注释  [A]行尾追加  [?]AI".to_string();
                }

                // Cursor on '<' (XML/HTML tag)
                if col < chars.len() && chars[col] == '<' {
                    return "[cit]修改tag内容  [dit]删除tag内容  [vat]选中含tag  [ci\"]修改属性值  [?]AI".to_string();
                }

                let word = word_at(&chars, col);

                // Cursor on URL or file path
                if word.starts_with("http://") || word.starts_with("https://")
                    || word.starts_with('/') || (word.contains('/') && !word.is_empty())
                {
                    return "[gf]打开文件  [yiw]复制路径  [ciw]替换路径  [?]AI".to_string();
                }

                // Cursor on a number
                if !word.is_empty() && word.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') {
                    return "[Ctrl+a]数字+1  [Ctrl+x]数字-1  [ciw]修改数字  [yiw]复制数字  [?]AI".to_string();
                }

                // Cursor inside a string (on or after a quote char)
                let on_quote = col < chars.len() && (chars[col] == '"' || chars[col] == '\'' || chars[col] == '`');
                let in_string = !on_quote && col > 0 && is_inside_quotes(&chars, col);
                if on_quote || in_string {
                    return "[ci\"]修改字符串内容  [di\"]删除内容  [yi\"]复制内容  [va\"]选中含引号  [?]AI".to_string();
                }

                // Cursor on a word (identifier / keyword)
                if !word.is_empty() && word.chars().next().map(|c| c.is_alphabetic() || c == '_').unwrap_or(false) {
                    return "[ciw]修改单词  [diw]删除单词  [yiw]复制单词  [*]搜索此词  [?]AI".to_string();
                }

                // Default Normal
                "[i]插入  [v]选择  [dd]删行  [yy]复制行  [p]粘贴  [.]重复  [u]撤销  [?]AI  [Ctrl+l]对话面板".to_string()
            }
        }
    }

    pub fn info_line(&self, filetype: FileType) -> String {
        let mode = self.mode.name();
        let filename = self.buffer.display_name();
        let modified = if self.buffer.modified { " [+]" } else { "" };
        let line = self.cursor_line + 1;
        let col  = self.cursor_col + 1;
        let ft   = filetype.name();
        format!("{:<8} {}{:<30}  {:>5}:{:<5} {}", mode, filename, modified, line, col, ft)
    }
}

fn word_at(chars: &[char], col: usize) -> String {
    if col >= chars.len() { return String::new(); }
    let is_word = |c: char| c.is_alphanumeric() || "/_.-:".contains(c);
    let mut s = col;
    let mut e = col;
    while s > 0 && is_word(chars[s-1]) { s -= 1; }
    while e < chars.len() && is_word(chars[e]) { e += 1; }
    chars[s..e].iter().collect()
}

/// Returns true if `col` is inside a matched pair of " ' or ` on the same line.
fn is_inside_quotes(chars: &[char], col: usize) -> bool {
    for &q in &['"', '\'', '`'] {
        let mut inside = false;
        for (i, &c) in chars.iter().enumerate() {
            if c == q {
                inside = !inside;
            }
            if i == col {
                return inside;
            }
        }
    }
    false
}
