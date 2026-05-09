# hi — 技术规格文档 (SPEC)

> 状态：草稿 v0.1
> 最后更新：2026-05-09
> 本文档是编码实现的直接依据，所有行为以此为准。

---

## 一、按键绑定规格

### 1.1 模式总览

```
启动 → Normal Mode
Normal → Insert    : i / I / a / A / o / O
Normal → Visual    : v（字符选择）/ V（行选择）
Normal → Command   : :
Normal → AI        : ?
Normal → Search    : /（向前搜索）
Any    → Normal    : Esc
```

### 1.2 Normal 模式按键全集

#### 光标移动

| 按键 | 行为 |
|---|---|
| `h` | 左移一字符 |
| `l` | 右移一字符 |
| `j` | 下移一行 |
| `k` | 上移一行 |
| `w` | 跳到下一个单词开头 |
| `b` | 跳到上一个单词开头 |
| `e` | 跳到当前/下一个单词结尾 |
| `W` / `B` / `E` | 同上，以空白符分隔的 WORD 为单位 |
| `0` | 跳到行首（第一列） |
| `^` | 跳到行首第一个非空字符 |
| `$` | 跳到行尾 |
| `gg` | 跳到文件第一行 |
| `G` | 跳到文件最后一行 |
| `{数字}G` | 跳到指定行号，如 `10G` 跳到第 10 行 |
| `{` | 跳到上一个空行（段落开头） |
| `}` | 跳到下一个空行（段落结尾） |
| `f{char}` | 跳到当前行下一个 `{char}` 字符处 |
| `F{char}` | 跳到当前行上一个 `{char}` 字符处 |
| `t{char}` | 跳到当前行下一个 `{char}` 字符的前一位 |
| `T{char}` | 跳到当前行上一个 `{char}` 字符的后一位 |
| `;` | 重复上一次 f/F/t/T |
| `,` | 反向重复上一次 f/F/t/T |
| `%` | 跳到匹配的括号 `()`、`[]`、`{}` |
| `Ctrl+d` | 向下滚动半屏 |
| `Ctrl+u` | 向上滚动半屏 |
| `Ctrl+f` | 向下滚动整屏 |
| `Ctrl+b` | 向上滚动整屏 |
| `zz` | 将当前行滚动到屏幕中央 |
| `zt` | 将当前行滚动到屏幕顶部 |
| `zb` | 将当前行滚动到屏幕底部 |

#### 编辑操作

| 按键 | 行为 |
|---|---|
| `x` | 删除光标处字符 |
| `X` | 删除光标前字符 |
| `dd` | 删除当前行（存入剪贴板） |
| `D` | 删除从光标到行尾 |
| `yy` | 复制当前行 |
| `Y` | 同 `yy` |
| `p` | 在光标后粘贴 |
| `P` | 在光标前粘贴 |
| `u` | 撤销（undo） |
| `Ctrl+r` | 重做（redo） |
| `r{char}` | 替换光标处字符为 `{char}` |
| `~` | 切换光标处字符大小写 |
| `>>` | 当前行向右缩进一级 |
| `<<` | 当前行向左缩进一级 |
| `J` | 合并当前行与下一行 |
| `.` | 重复上一次修改操作 |

#### 进入 Insert 模式的方式

| 按键 | 进入位置 |
|---|---|
| `i` | 光标前 |
| `I` | 行首第一个非空字符前 |
| `a` | 光标后 |
| `A` | 行尾 |
| `o` | 在当前行下方新建一行 |
| `O` | 在当前行上方新建一行 |
| `s` | 删除光标处字符并进入 Insert |
| `S` | 删除当前行内容并进入 Insert（同 `cc`） |
| `C` | 删除从光标到行尾并进入 Insert |

#### 文本对象操作（Operator + Text Object）

Operator：`d`（删除）、`y`（复制）、`c`（修改）、`>`（缩进）、`<`（反缩进）

Text Object：

| 组合 | 范围 |
|---|---|
| `iw` / `aw` | inner word / a word（含周围空格） |
| `iW` / `aW` | inner WORD / a WORD |
| `is` / `as` | inner sentence / a sentence |
| `ip` / `ap` | inner paragraph / a paragraph |
| `i"` / `a"` | 双引号内 / 含双引号 |
| `i'` / `a'` | 单引号内 / 含单引号 |
| `i(` / `a(` | 圆括号内 / 含圆括号（同 `ib`/`ab`） |
| `i[` / `a[` | 方括号内 / 含方括号 |
| `i{` / `a{` | 花括号内 / 含花括号（同 `iB`/`aB`） |
| `it` / `at` | XML/HTML tag 内 / 含 tag |

示例：`diw` 删除当前单词，`ci"` 修改双引号内内容，`vip` 选中当前段落。

#### 数字前缀

所有移动和操作命令支持数字前缀，表示重复次数：

- `3j` → 向下移动 3 行
- `5dd` → 删除 5 行
- `2w` → 跳过 2 个单词

#### 窗口 / 文件树操作

| 按键 | 行为 |
|---|---|
| `Ctrl+t` | 切换文件树面板（打开/关闭） |
| `Ctrl+w` | 切换焦点（文件树 ↔ 编辑区） |

### 1.3 Insert 模式按键

| 按键 | 行为 |
|---|---|
| 普通字符 | 插入字符 |
| `Backspace` | 删除前一字符 |
| `Delete` | 删除后一字符 |
| `Enter` | 换行（自动缩进） |
| `Tab` | 插入缩进（按配置 tab_width） |
| `Ctrl+w` | 删除前一个单词 |
| `Ctrl+u` | 删除从光标到行首 |
| `Esc` | 返回 Normal 模式，光标左移一位 |

### 1.4 Visual 模式按键

进入后光标所在位置为选区起点，移动光标扩展选区（支持所有 Normal 移动键）。

| 按键 | 行为 |
|---|---|
| `h/j/k/l` 等 | 扩展/缩小选区 |
| `y` | 复制选区 |
| `d` | 删除选区 |
| `c` | 删除选区并进入 Insert 模式 |
| `>` | 选区右缩进 |
| `<` | 选区左缩进 |
| `~` | 切换选区大小写 |
| `?` | 对选区触发 AI（上下文包含选区内容） |
| `Esc` | 退出 Visual 模式 |

### 1.5 Command 模式（`:` 触发）

#### 文件操作

| 命令 | 行为 |
|---|---|
| `:w` | 保存当前文件 |
| `:w {file}` | 另存为 `{file}` |
| `:q` | 退出（有未保存修改时报错） |
| `:q!` | 强制退出，放弃修改 |
| `:wq` / `:x` | 保存并退出 |
| `:e {file}` | 打开文件 |
| `:e!` | 重新从磁盘加载当前文件（放弃修改） |

#### 编辑操作

| 命令 | 行为 |
|---|---|
| `:s/{pat}/{rep}/` | 替换当前行第一个匹配 |
| `:s/{pat}/{rep}/g` | 替换当前行所有匹配 |
| `:%s/{pat}/{rep}/g` | 替换全文所有匹配 |
| `:%s/{pat}/{rep}/gc` | 替换全文，每处需确认 |
| `:{range}s/...` | 在指定行范围内替换，如 `:1,10s/...` |
| `:d` | 删除当前行 |
| `:{range}d` | 删除指定行范围 |
| `:u` | 撤销（同 `u`） |

#### 导航

| 命令 | 行为 |
|---|---|
| `:{n}` | 跳到第 n 行 |
| `:/{pattern}` | 搜索（同 `/`） |

#### 配置

| 命令 | 行为 |
|---|---|
| `:set number` / `:set nu` | 显示行号 |
| `:set nonumber` / `:set nonu` | 隐藏行号 |
| `:set tabstop={n}` | 设置 tab 宽度 |

### 1.6 Search 模式（`/` 触发）

| 操作 | 行为 |
|---|---|
| 输入 pattern + `Enter` | 跳到第一个匹配，高亮所有匹配 |
| `n` | 跳到下一个匹配 |
| `N` | 跳到上一个匹配 |
| `Esc` | 退出搜索，清除高亮 |

支持正则表达式语法（Rust regex crate 标准）。

### 1.7 AI 模式（`?` 触发）

| 操作 | 行为 |
|---|---|
| `?` | 打开 AI 输入栏（底部命令行区域） |
| 输入自然语言 + `Enter` | 发送给 AI 处理 |
| `Esc` | 取消，返回 Normal 模式 |
| `Tab`（幽灵文字显示时） | 确认并执行 AI 建议的命令 |
| `y`（规划模式显示时） | 确认执行多步计划 |
| `n`（规划模式显示时） | 取消执行计划 |
| `e`（规划模式显示时） | 进入编辑模式修改计划内容 |

---

## 二、状态栏规格

### 2.1 布局

状态栏固定在屏幕最底部，占 **2 行**：

```
第 N-1 行（提示行）：上下文感知的按键提示，动态内容
第 N   行（信息行）：模式指示 | 文件名 | 修改状态 | 行:列 | 文件类型
```

示例：
```
[i]插入  [v]选择  [dd]删行  [yy]复制  [p]粘贴  [u]撤销  [?]AI  [/]搜索
NORMAL  config.yaml [+]                                    42:8  YAML
```

### 2.2 信息行固定内容

| 区域 | 内容 | 说明 |
|---|---|---|
| 左 | 模式名称 | `NORMAL` / `INSERT` / `VISUAL` / `COMMAND` / `AI` |
| 中左 | 文件名 | 相对路径，未保存新文件显示 `[No Name]` |
| 中 | 修改标记 | 有未保存修改时显示 `[+]` |
| 右 | 行:列 | 当前光标位置，如 `42:8` |
| 最右 | 文件类型 | `YAML` / `JSON` / `XML` / `JAVA` 等 |

### 2.3 提示行内容规则

提示行内容根据以下上下文动态生成，优先级从高到低：

**规则 1：Visual 模式已选中内容**
```
[y]复制  [d]删除  [c]替换  [>]缩进  [<]反缩进  [?]AI操作选区  [Esc]退出
```

**规则 2：光标在 URL 或文件路径上（Normal 模式）**
```
[gf]打开文件  [yiw]复制路径  [ciw]替换  [?]AI  
```

**规则 3：光标在数字上（Normal 模式）**
```
[Ctrl+a]加1  [Ctrl+x]减1  [ciw]替换数字  [?]AI  [yiw]复制
```

**规则 4：光标在 XML/HTML tag 上**
```
[cit]修改tag内容  [dit]删除tag内容  [vat]选中含tag  [?]AI
```

**规则 5：Normal 模式默认**
```
[i]插入  [v]选择  [dd]删行  [yy]复制行  [p]粘贴  [u]撤销  [?]AI  [gg/G]首/尾
```

**规则 6：Insert 模式**
```
正在输入...  [Esc]返回Normal  [Ctrl+w]删词  [Ctrl+u]删至行首
```

**规则 7：Command 模式**
```
输入命令  :w保存  :q退出  :wq保存退出  :%s/查找/替换/g  [Esc]取消
```

**规则 8：AI 模式输入中**
```
描述你的意图，按Enter发送  [Tab]确认建议  [Esc]取消  示例：把所有ERROR替换为WARN
```

---

## 三、文件树规格

### 3.1 触发与布局

- `hi .` 启动时，左侧展示文件树，宽度默认 30 字符，可调整
- `hi {file}` 启动时，不显示文件树，`Ctrl+t` 可随时切换
- 文件树与编辑区之间有 `│` 分隔线

```
┌─────────────────┬─────────────────────────────────────────┐
│ . (当前目录)    │                                         │
│ ├── config/     │  编辑区内容                             │
│ │   ├── app.yml │                                         │
│ │   └── db.toml │                                         │
│ ├── logs/       │                                         │
│ │   └── app.log │                                         │
│ └── README.md   │                                         │
├─────────────────┴─────────────────────────────────────────┤
│ [Enter]打开  [a]新建  [d]删除  [r]重命名  [Ctrl+w]切换焦点 │
│ NORMAL  .                                          TREE   │
└───────────────────────────────────────────────────────────┘
```

### 3.2 文件树按键

| 按键 | 行为 |
|---|---|
| `j` / `↓` | 移到下一项 |
| `k` / `↑` | 移到上一项 |
| `Enter` / `l` | 打开文件（进入编辑区）/ 展开目录 |
| `h` | 折叠当前目录 / 跳到父目录 |
| `a` | 新建文件（提示输入文件名） |
| `A` | 新建目录 |
| `d` | 删除文件/目录（需二次确认） |
| `r` | 重命名（底部提示输入新名） |
| `y` | 复制文件路径到剪贴板 |
| `R` | 刷新文件树 |
| `Ctrl+w` | 切换焦点到编辑区 |
| `Ctrl+t` | 关闭文件树 |
| `Esc` | 切换焦点到编辑区（同 `Ctrl+w`） |

### 3.3 文件树显示规则

- 目录排在文件前面
- 隐藏文件（`.`开头）默认不显示，`zh` 切换显示/隐藏
- 目录展开/折叠状态在同次会话内保持
- 当前打开的文件在文件树中高亮标记

---

## 四、语法高亮规格

### 4.1 文件类型识别顺序

1. 文件扩展名（优先）
2. 文件名（如 `Makefile`、`Dockerfile`）
3. Shebang 首行（如 `#!/bin/bash`）
4. 内容特征检测（如 `<?xml` 开头识别为 XML）

### 4.2 支持的文件类型（Phase 1 优先实现）

| 文件类型 | 扩展名 | 高亮重点 |
|---|---|---|
| XML | `.xml`, `.pom`, `.xsd`, `.wsdl` | tag、属性名、属性值、注释、CDATA |
| HTML | `.html`, `.htm` | tag、属性、内嵌 JS/CSS |
| YAML | `.yml`, `.yaml` | key、value、注释、锚点/引用 |
| JSON | `.json` | key、string、number、boolean、null |
| TOML | `.toml` | section header、key、value、注释 |
| Properties | `.properties`, `.env` | key、value、注释 |
| Shell | `.sh`, `.bash`, `.zsh` | 关键字、变量、字符串、注释 |
| Log | `.log` | ERROR（红）、WARN（黄）、INFO（绿）、DEBUG（灰）、时间戳、堆栈 |
| Java | `.java` | 关键字、类型、字符串、注释、注解 |
| Python | `.py` | 关键字、字符串、装饰器、注释、f-string |

### 4.3 高亮颜色规范（默认主题）

| Token 类型 | 颜色 |
|---|---|
| 关键字 | 蓝色 / Bold |
| 字符串 | 绿色 |
| 数字 | 青色 |
| 注释 | 灰色 / Italic |
| 错误/ERROR | 红色 |
| 警告/WARN | 黄色 |
| 成功/INFO | 绿色 |
| 类型/Class | 青色 / Bold |
| 属性名 | 紫色 |
| 当前搜索匹配 | 黄色背景 |
| 其他搜索匹配 | 灰色背景 |

---

## 五、AI 交互规格

### 5.1 意图分类规则

AI Engine 收到用户输入后，首先判断意图类型：

| 意图类型 | 判断信号 | 响应形式 |
|---|---|---|
| `query`（询问型） | 含"怎么"、"如何"、"什么是"、"how"、"what"等疑问词 | 顾问模式：文字说明 |
| `simple_edit`（简单编辑） | 单一操作，可映射到一条 Ex 命令 | 幽灵文字模式 |
| `complex_edit`（复杂编辑） | 需要多步、含条件逻辑、含映射转换 | 执行规划模式 |

判断由 LLM 自身完成，通过 System Prompt 约束其返回固定的 `mode` 字段。

### 5.2 LLM 请求格式

```json
{
  "model": "{config.ai.model}",
  "messages": [
    {
      "role": "system",
      "content": "<见 5.3 System Prompt>"
    },
    {
      "role": "user",
      "content": {
        "instruction": "用户的自然语言输入",
        "context": {
          "filetype": "yaml",
          "filename": "config.yml",
          "total_lines": 120,
          "cursor_line": 34,
          "cursor_col": 8,
          "visual_selection": null,
          "current_line": "  timeout: 30",
          "surrounding_lines": {
            "before": ["  host: localhost", "  port: 3306"],
            "after": ["  retries: 3", "  pool_size: 10"]
          }
        }
      }
    }
  ],
  "response_format": { "type": "json_object" }
}
```

### 5.3 System Prompt

```
你是 hi 编辑器的 AI 助手，专门将用户的自然语言编辑意图转换为精确的编辑器操作。

【输出格式】严格输出 JSON，三种形式之一：

1. 顾问模式（用户在询问如何操作）：
{
  "mode": "query",
  "explanation": "用中文解释如何操作，包含具体按键"
}

2. 幽灵文字模式（简单编辑，单条命令可完成）：
{
  "mode": "ghost",
  "command": "完整的 Ex 命令，如 :%s/foo/bar/g",
  "explanation": "一句话说明这条命令做什么"
}

3. 执行规划模式（复杂编辑，需多步完成）：
{
  "mode": "plan",
  "steps": [
    { "description": "步骤说明", "command": "对应的 Ex 命令或操作" }
  ],
  "explanation": "整体说明"
}

【判断标准】
- 用户在询问（含疑问词）→ mode: query
- 操作可用一条 :%s 或单个 Ex 命令完成 → mode: ghost
- 需要多步、含逻辑判断、含数据转换 → mode: plan

【安全约束】绝对禁止在 command 字段中包含：
- :! 开头的 shell 调用
- :w! 强制写入
- 任何对编辑器外部系统的操作
违反约束时，返回 mode: query，explanation 说明无法执行的原因。

【质量要求】
- 命令必须是合法的 Vim-compatible Ex 命令
- 正则表达式使用 Vim 正则语法
- 优先使用简洁的命令，避免过度复杂
```

### 5.4 响应处理流程

```
收到 LLM 响应
       │
       ├── mode: query  → 在状态栏提示行显示 explanation，不执行任何操作
       │
       ├── mode: ghost  → 将 command 以幽灵文字显示在命令行
       │                   Tab 确认 → 执行命令，打 undo 断点
       │                   继续输入 → 幽灵文字消失
       │
       └── mode: plan   → 打开规划面板，显示 steps 列表
                           y 确认 → 依次执行每个 step 的 command，全程打 undo 断点
                           n 取消 → 关闭面板
                           e 编辑 → 进入规划编辑模式（可修改步骤）
```

### 5.5 Undo 保护机制

所有 AI 触发的操作，执行前自动插入 undo 断点（等效 Vim 的 `undojoin` 反操作）。确保用户按 `u` 可以一次性撤销整个 AI 操作序列，而不是逐条撤销。

### 5.6 错误处理

| 错误类型 | 处理方式 |
|---|---|
| 网络超时（默认 30s） | 状态栏显示"AI 请求超时，请重试" |
| API Key 未配置 | 状态栏显示"请在 ~/.hirc 中配置 ai.api_key" |
| API 返回错误 | 状态栏显示错误信息 |
| JSON 解析失败 | 降级为顾问模式，显示原始响应文字 |
| 命令安全过滤拦截 | 状态栏显示"该操作被安全策略阻止" |

---

## 六、配置文件规格

路径：`~/.hirc`，格式：TOML

### 完整配置项

```toml
# ─────────────────────────────────────────────
# hi 编辑器配置文件 (~/.hirc)
# ─────────────────────────────────────────────

[general]
# 显示行号
line_numbers = true
# Tab 宽度（空格数）
tab_width = 4
# 是否将 Tab 展开为空格
expand_tab = true
# 自动缩进
auto_indent = true
# 搜索时忽略大小写
ignore_case = true
# 有大写字母时区分大小写（需配合 ignore_case = true）
smart_case = true
# 滚动保留行数（光标距屏幕边缘保留的行数）
scroll_off = 5

[ai]
# LLM 服务地址，兼容所有 OpenAI API 格式的服务
api_base_url = "https://api.openai.com/v1"
# API Key（也可通过环境变量 HI_API_KEY 设置，环境变量优先）
api_key = ""
# 使用的模型名称
model = "gpt-4o"
# 请求超时秒数
timeout_secs = 30
# yolo 模式：跳过执行规划的用户确认步骤
yolo_mode = false
# 发送给 AI 的上下文行数（光标前后各取多少行）
context_lines = 10

[theme]
# 内置主题：default / dark / light / solarized
colorscheme = "default"

[filetree]
# 文件树默认宽度（字符数）
width = 30
# 是否显示隐藏文件（.开头）
show_hidden = false
```

### 环境变量

| 变量名 | 对应配置项 | 说明 |
|---|---|---|
| `HI_API_KEY` | `ai.api_key` | API Key，优先级高于配置文件 |
| `HI_MODEL` | `ai.model` | 覆盖配置文件中的模型 |
| `HI_CONFIG` | — | 指定配置文件路径，默认 `~/.hirc` |

### 6.3 缺省行为

`~/.hirc` 不存在时，所有配置项使用上表中的默认值，正常启动，不报错。

---

## 七、Rust 项目结构与依赖

### 7.1 目录结构

```
hi/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── docs/
│   ├── PRODUCT.md
│   ├── ARCHITECTURE.md
│   └── SPEC.md
└── src/
    ├── main.rs          # 入口：解析 CLI 参数，启动 App
    ├── app.rs           # App 主循环：事件分发、模式切换
    ├── buffer/
    │   ├── mod.rs       # Buffer 结构体定义
    │   ├── rope.rs      # 基于 ropey 的文本存储
    │   └── history.rs   # Undo/Redo 历史管理（ChangeSets）
    ├── editor/
    │   ├── mod.rs       # Editor 状态（当前 buffer、窗口布局等）
    │   └── motion.rs    # 光标移动逻辑
    ├── command/
    │   ├── mod.rs       # Ex 命令解析
    │   └── executor.rs  # 命令执行器
    ├── mode/
    │   ├── mod.rs       # 模式枚举定义
    │   ├── normal.rs    # Normal 模式按键处理
    │   ├── insert.rs    # Insert 模式按键处理
    │   ├── visual.rs    # Visual 模式按键处理
    │   ├── command.rs   # Command 模式（:）处理
    │   └── ai.rs        # AI 模式（?）处理
    ├── ui/
    │   ├── mod.rs       # UI 顶层协调
    │   ├── renderer.rs  # crossterm 渲染主逻辑
    │   ├── statusbar.rs # 状态栏 + 提示渲染
    │   ├── filetree.rs  # 文件树面板
    │   └── ghost.rs     # 幽灵文字覆盖层
    ├── ai/
    │   ├── mod.rs       # AI Engine 入口
    │   ├── context.rs   # 上下文收集（光标周围行、选区等）
    │   ├── prompt.rs    # Prompt 构建
    │   ├── client.rs    # LLM HTTP 客户端（tokio async）
    │   ├── parser.rs    # 响应 JSON 解析
    │   ├── hint.rs      # 状态栏提示引擎（本地，无需 LLM）
    │   └── react.rs     # ReAct 编排循环（Phase 3 预留）
    ├── syntax/
    │   ├── mod.rs       # 语法高亮调度
    │   └── highlight.rs # 高亮规则应用
    └── config/
        ├── mod.rs       # 配置结构体（serde Deserialize）
        └── loader.rs    # ~/.hirc 读取与默认值填充
```

### 7.2 Cargo.toml

```toml
[package]
name = "hi"
version = "0.1.0"
edition = "2021"
description = "A modal text editor with native AI assistance"
license = "MIT"

[[bin]]
name = "hi"
path = "src/main.rs"

[dependencies]
crossterm = "0.28"                               # TUI 渲染：跨平台终端控制
ropey = "1.6"                                   # 高效文本存储（Rope 数据结构）
toml = "0.8"                                    # TOML 配置文件解析
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }  # 异步运行时
reqwest = { version = "0.12", features = ["json"] }  # HTTP 客户端
regex = "1"                                     # 正则表达式
clap = { version = "4", features = ["derive"] }  # CLI 参数解析
anyhow = "1"                                    # 错误处理

[profile.release]
opt-level = 3
lto = true
strip = true
```

---

## 八、Phase 1 交付验收标准

以下所有条目通过后，Phase 1 视为完成：

**基础编辑**
- [ ] 能打开、编辑、保存任意文本文件
- [ ] Normal / Insert / Visual / Command 四种模式可正确切换
- [ ] `h j k l w b e 0 ^ $ gg G {n}G` 光标移动正确
- [ ] `dd yy p P u Ctrl+r x r ~` 等编辑操作正确
- [ ] 数字前缀（如 `3j`、`5dd`）生效

**操作符 + 文本对象**
- [ ] `diw` `ciw` `yip` `ci"` `di(` 等组合正确

**搜索与替换**
- [ ] `/pattern` 搜索高亮正常，`n N` 跳转正确
- [ ] `:%s/pat/rep/g` 全局替换正确
- [ ] `:noh` 清除高亮
- [ ] `*` `#` 快速搜索当前词

**状态栏**
- [ ] 信息行始终显示模式名称、文件名、行列号、文件类型
- [ ] 提示行在各模式下显示对应提示
- [ ] 文件有修改时显示 `[+]` 标记

**AI 集成**
- [ ] `?` 触发 AI 输入框，`Esc` 可取消
- [ ] ghost 模式：幽灵文字显示，Tab 确认执行
- [ ] query 模式：提示行显示 AI 说明，3 秒后恢复默认
- [ ] plan 模式：规划面板显示，y/n 确认/取消
- [ ] AI 操作可用单次 `u` 整体撤销
- [ ] 无网络/API Key 未配置时优雅降级，不崩溃

**文件树**
- [ ] `hi .` 启动显示文件树
- [ ] `j k Enter h` 导航正常，展开/折叠目录
- [ ] `Ctrl+t` 切换显示/隐藏
- [ ] `Ctrl+w` 在文件树和编辑区之间切换焦点

**语法高亮**
- [ ] YAML / JSON / TOML / Properties 文件高亮正确
- [ ] `.log` 文件中 ERROR 红色、WARN 黄色、INFO 绿色

**配置**
- [ ] `~/.hirc` 不存在时使用默认配置，正常启动
- [ ] `~/.hirc` 存在时正确读取各配置项
- [ ] 环境变量 `HI_API_KEY` 优先级高于配置文件
