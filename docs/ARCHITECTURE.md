# hi — 架构设计文档

> 状态：草稿 v0.1
> 最后更新：2026-05-09

---

## 一、为什么不用 C

hi 的前身研究对象是 Vim。Vim 有 55 万行 C 代码，在深入审计后，发现以下核心问题：

- **304 个全局变量**：整个编辑器是一个巨型有状态单例，模块间直接读写全局状态，没有封装边界，无法做单元测试
- **双脚本引擎并存**：VimL 旧引擎（~39,600 行）和 Vim9 新引擎（~34,100 行）完全并存，因历史兼容无法删除，维护成本极高
- **废弃平台代码**：Amiga、VMS、QNX 等已死亡平台的代码仍在维护，散落数百个 `#ifdef` 条件编译块
- **过时集成**：cscope（被 LSP 取代）、NetBeans 协议（Sun 已不存在）、OLE/COM（1990 年代技术）等共计约 11,500 行死代码
- **渲染层直接写终端**：没有 UI 抽象层，无法支持现代 GUI 后端

Vim 的性能瓶颈从来不是 CPU，而是 I/O 和终端刷新（受限于 60Hz 屏幕带宽）。用 C 维护 55 万行充满 `#ifdef` 的代码，工程效率的损失远大于语言切换带来的性能损失。

---

## 二、技术选型：Rust

### 为什么选 Rust

| 关注点 | Rust 的答案 |
|---|---|
| 内存安全 | 编译期所有权检查，消除 use-after-free / buffer overflow（Vim 历史上有多个此类 CVE） |
| 性能 | 零成本抽象，热路径（按键→处理→重绘）性能与 C 等价 |
| 类型系统 | 可自然表达 Buffer/Window/TabPage 的所有权与生命周期关系 |
| 可测试性 | 无全局状态，模块间通过接口通信，单元测试友好 |
| 生态 | `crossterm`（跨平台终端）、`tokio`（异步运行时）、`serde`（配置序列化）均已成熟 |
| 构建系统 | Cargo，相比 Vim 的 autoconf/Makefile 现代化程度提升巨大 |
| 先例 | Helix（modal editor）、Zed、Lapce 均选择 Rust，可行性已验证 |

### 参考项目

- **Helix**：Rust 实现的 modal editor，证明了完整编辑器实现的可行性，但无 AI 能力
- **Neovim**：保持 C 核心，但引入 UI Protocol（msgpack RPC）抽象渲染层，这个思路值得借鉴
- **Zed**：Rust + 自研 GPU UI 框架，定位 GUI，不适合 terminal 场景

---

## 三、整体分层架构

```
┌─────────────────────────────────────────────────────────────────┐
│                        UI 层（Terminal）                          │
│                                                                 │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │  TUI Renderer（基于 crossterm）                          │   │
│   │  - 编辑区渲染    - 状态栏渲染    - 文件树渲染            │   │
│   │  - AI 规划面板   - 幽灵文字覆盖层                        │   │
│   └────────────────────────┬────────────────────────────────┘   │
└───────────────────────────┬┼────────────────────────────────────┘
                            ││  UI Events（按键输入 / resize）
                            ││  Render Commands（差量重绘指令）
┌───────────────────────────▼▼────────────────────────────────────┐
│                      核心编辑器引擎（Rust）                        │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │ Buffer        │  │ Window       │  │ Command Parser        │  │
│  │ Manager       │  │ Layout       │  │ (Ex commands)         │  │
│  │               │  │              │  │                       │  │
│  │ - 文本存储    │  │ - 分割布局   │  │ - :w :q :s 等         │  │
│  │ - undo/redo   │  │ - 文件树面板 │  │ - 命令历史            │  │
│  │ - 语法高亮   │  │ - 焦点管理   │  │                       │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │ Motion        │  │ Keymap       │  │ Context Engine        │  │
│  │ Engine        │  │ & Mode FSM   │  │                       │  │
│  │               │  │              │  │ - 光标位置            │  │
│  │ - w/b/e/gg/G  │  │ - Normal     │  │ - 文件类型            │  │
│  │ - f/t/;/,     │  │ - Insert     │  │ - 选区信息            │  │
│  │ - { } 段落    │  │ - Visual     │  │ - 周围文本            │  │
│  └──────────────┘  │ - Command    │  └──────────┬───────────┘  │
│                    │ - AI (?)     │             │               │
│                    └──────────────┘             │               │
│                                                 │               │
│  ┌──────────────────────────────────────────────▼─────────────┐ │
│  │                    AI Engine                                │ │
│  │                                                             │ │
│  │  ┌─────────────────┐        ┌────────────────────────────┐ │ │
│  │  │ Intent Analyzer  │        │ Statusbar Hint Engine       │ │ │
│  │  │                 │        │                            │ │ │
│  │  │ 判断意图复杂度:  │        │ 根据上下文预测             │ │ │
│  │  │ - 简单→幽灵文字 │        │ 最可能用到的按键           │ │ │
│  │  │ - 复杂→执行规划 │        │ 动态生成状态栏提示         │ │ │
│  │  │ - 提问→顾问模式 │        └────────────────────────────┘ │ │
│  │  └────────┬────────┘                                        │ │
│  │           │                                                 │ │
│  │  ┌────────▼────────┐        ┌────────────────────────────┐ │ │
│  │  │ ReAct Orchestr. │        │ Ghost Text Renderer         │ │ │
│  │  │ （Phase 2）      │        │ 将 AI 建议注入命令行覆盖层  │ │ │
│  │  └────────┬────────┘        └────────────────────────────┘ │ │
│  └───────────┼─────────────────────────────────────────────────┘ │
└──────────────┼──────────────────────────────────────────────────┘
               │  async HTTP（tokio）
┌──────────────▼──────────────────────────────────────────────────┐
│                     LLM Backend（可配置）                          │
│                                                                 │
│   OpenAI API  │  Anthropic Claude  │  Ollama（本地）  │  其他     │
│                                                                 │
│   统一通过 ~/.hirc 配置 api_base_url + model + api_key            │
└─────────────────────────────────────────────────────────────────┘
```

---

## 四、核心模块设计

### 4.1 Buffer Manager

文本存储的核心数据结构选用 **Rope**（绳索树）：

- Rope 是专为文本编辑设计的树形数据结构，相比简单数组，在大文件的插入/删除操作上时间复杂度为 O(log n)
- Rust 生态有成熟的 `ropey` crate 可直接使用
- 每次编辑操作生成一个新的 Rope 节点，天然支持高效的 undo/redo（保留历史版本而非记录操作序列）

```
Buffer {
    rope: Rope,              // 文本内容
    history: Vec<Rope>,      // undo 历史（保留快照）
    history_cursor: usize,   // 当前历史位置
    path: Option<PathBuf>,   // 文件路径（None 表示新建未保存）
    modified: bool,          // 是否有未保存修改
    filetype: FileType,      // 文件类型（影响语法高亮和 AI 上下文）
}
```

### 4.2 Mode State Machine（模式状态机）

模式切换是编辑器的核心状态流转，用有限状态机（FSM）精确描述：

```
                    ┌─────────────────────────────────────┐
                    │            Normal Mode               │◀─── 启动
                    └──┬──────┬──────┬──────┬─────────────┘
                       │ i    │ v/V  │ :    │ ?
                       ▼      ▼      ▼      ▼
                   Insert  Visual Command  AI
                   Mode    Mode    Mode    Mode
                       │      │      │      │
                       └──────┴──────┴──────┘
                                  │ Esc
                                  ▼
                            Normal Mode
```

每个模式对应不同的按键处理逻辑和状态栏提示内容，模式切换由 FSM 统一管理，杜绝状态混乱。

### 4.3 AI Engine

AI Engine 是 hi 区别于所有现有编辑器的核心差异。分两个子系统：

#### Statusbar Hint Engine（常驻提示，无需 LLM）

状态栏的智能提示**不依赖 LLM**，是本地计算的结果：

- 维护一个**上下文→提示映射表**，根据当前模式、光标位置、文件类型、选区状态，查找最相关的按键提示
- 这部分完全本地运行，零延迟，零网络依赖
- 随着用户使用积累，可以学习用户的操作习惯，优化提示的优先级排序

#### Intent Processor（意图处理，依赖 LLM）

用户按 `?` 触发后的处理流程：

```
用户输入自然语言
       │
       ▼
 Intent Classifier（本地轻量模型或规则）
 判断意图类型：
   - query（提问）→ 顾问模式
   - simple_edit（简单编辑）→ 幽灵文字模式
   - complex_edit（复杂编辑）→ 执行规划模式
       │
       ▼
 Context Collector
 收集：文件类型、光标行列、选区范围、周围文本（±10行）、文件名
       │
       ▼
 Prompt Builder
 构建发往 LLM 的请求（见 Prompt 设计章节）
       │
       ▼
 LLM API Call（async，tokio）
       │
       ▼
 Response Parser
 提取命令序列 / 解释文字 / 置信度
       │
       ▼
 根据意图类型路由到对应 UI 渲染器
```

### 4.4 ReAct Orchestrator（第二阶段）

复杂任务的多步执行引擎，基于 ReAct（Reasoning + Acting）模式：

- **Thought**：LLM 推理当前状态，决定下一步动作
- **Action**：调用预定义工具集（read_buffer / search / execute_cmd / ask_user）
- **Observation**：收到工具执行结果，继续下一轮
- **Done**：任务完成，输出总结

工具集（Actions）：

```
read_buffer(start, end)     → 读取指定行范围的文本
search(pattern, flags)      → 在 buffer 中搜索，返回匹配位置列表
execute_cmd(cmd)            → 执行单条 Ex 命令（经安全过滤）
select_range(start, end)    → 设置 Visual 选区
ask_user(question)          → 暂停，在状态栏向用户请求补充信息
undo()                      → 回撤上一步操作
```

安全约束：`execute_cmd` 黑名单过滤 `:!`（shell 调用）、`:w!`（强制写入）等危险命令。

### 4.5 TUI Renderer

基于 `crossterm` 构建，渲染分区：

```
┌──────────────────────────────────────────────────┐
│  文件树面板（可选）  │  编辑区（主区域）            │
│  （左侧，可折叠）    │                             │
│                    │  行号  文本内容               │
│                    │                             │
│                    │                             │
├────────────────────┴─────────────────────────────┤
│  AI 规划面板（按需弹出，覆盖在编辑区上方）           │
├──────────────────────────────────────────────────┤
│  状态栏（常驻底部一行）：模式 | 文件名 | 行列 | 提示 │
└──────────────────────────────────────────────────┘
```

差量重绘：只重绘发生变化的区域，避免全屏刷新造成闪烁。

### 4.6 统一语法高亮架构（Unified Syntax Highlighting）

这是 hi 的核心特色之一。编辑器文本区域和 AI Chat 面板的代码块共享同一套 syntect 引擎，实现了颜色一致性、语言覆盖一致性和主题一致性。

#### 设计动机

传统终端编辑器的语法高亮通常是手写正则规则，每种语言一套函数，维护成本高、覆盖面窄、颜色粗糙（只能用 16 色 ANSI 枚举）。当编辑器增加 Chat 面板等辅助视图时，又需要另一套独立的高亮系统，导致同一段代码在不同面板中颜色不一致。

hi 的解决方案是引入 [syntect](https://github.com/trishume/syntect)（Sublime Text 的语法高亮引擎的 Rust 移植），作为唯一的语法分析后端，同时服务编辑器和 Chat 面板。

#### 整体架构

```
                          ~/.hirc [theme]
                               │
                    ┌──────────┴──────────┐
                    │                     │
              editor_theme           chat_theme
           "base16-ocean.dark"       "dracula"
                    │                     │
                    ▼                     ▼
┌─────────────────────────────────────────────────────────────┐
│                     syntect engine                           │
│                                                             │
│   SyntaxSet::load_defaults_newlines()  ← 200+ 语言语法定义  │
│   ThemeSet::load_defaults()            ← Sublime Text 主题  │
│                                                             │
│   ┌─────────────────────┐    ┌────────────────────────────┐ │
│   │  SyntectHighlighter  │    │       MdRenderer            │ │
│   │  (编辑器文本区域)     │    │  (Chat 面板 Markdown 渲染)  │ │
│   │                     │    │                            │ │
│   │  HighlightLines     │    │  pulldown-cmark 解析器      │ │
│   │  (有状态逐行解析)    │    │  + HighlightLines 代码块    │ │
│   │                     │    │                            │ │
│   │  输出: SyntectSpan[]│    │  输出: MdLine<StyledSpan>  │ │
│   └──────────┬──────────┘    └─────────────┬──────────────┘ │
│              │                             │                │
└──────────────┼─────────────────────────────┼────────────────┘
               │                             │
               ▼                             ▼
┌─────────────────────────────────────────────────────────────┐
│                  Renderer (crossterm)                        │
│                                                             │
│   render_line_with_spans(&[SyntectSpan])  ← 编辑器行渲染    │
│   render_chat_panel(MdLine[])             ← Chat 面板渲染   │
│                                                             │
│   统一 ANSI 属性绘制：                                       │
│   SetForegroundColor(RGB) + SetBackgroundColor + Bold/Italic │
└─────────────────────────────────────────────────────────────┘
```

#### 双层类型设计

编辑器和 Chat 面板的输出类型不同，但底层颜色来源相同：

**编辑器侧 — `SyntectSpan`**

```rust
pub struct SyntectSpan {
    pub start: usize,           // 行内字节偏移（起始）
    pub end: usize,             // 行内字节偏移（结束，不含）
    pub fg: Color,              // RGB 前景色，直接来自 syntect 主题
    pub bold: bool,
    pub italic: bool,
    pub overlay: Option<OverlayKind>,  // 搜索高亮 / Visual Block 叠加
}
```

`SyntectSpan` 使用字节范围索引，与 Rust 的 `&str` 切片语义一致。`overlay` 字段实现了叠加合成：搜索高亮和 Visual Block 选区的背景色覆盖在语法颜色之上，而不是替换它们。

**Chat 侧 — `StyledSpan` / `MdLine`**

```rust
pub struct StyledSpan {
    pub text: String,           // 文本内容（已解析，非原始 Markdown）
    pub fg: Option<Color>,      // RGB 前景色
    pub bg: Option<Color>,      // RGB 背景色
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub dim: bool,
}

pub struct MdLine {
    pub spans: Vec<StyledSpan>,
    pub indent: usize,
    pub border: Option<(String, Color)>,  // 块引用边框
}
```

`StyledSpan` 携带完整的 ANSI 属性集（包括下划线、删除线、暗淡），因为 Markdown 渲染需要比纯语法高亮更丰富的样式。`MdLine` 额外携带缩进和边框信息，用于块引用、列表等结构化元素。

#### 有状态解析与 Overlay 合成

`SyntectHighlighter` 内部持有一个 `HighlightLines<'static>` 状态机。每次调用 `highlight_line()` 时，状态机会推进，使得跨行的块注释、heredoc、多行字符串等结构能被正确识别。当文件类型切换或文件重新加载时，调用 `reset_state()` 重置状态机。

Overlay 合成的工作方式：

```
正常渲染:  syntect 输出 → SyntectSpan { fg: RGB, overlay: None }
搜索高亮:  额外推入 → SyntectSpan { fg: White, overlay: Some(SearchMatch) }
Visual Block: 额外推入 → SyntectSpan { fg: White, overlay: Some(VisualBlock) }

Renderer 绘制时:
  if overlay.is_some() → 使用 overlay.bg_color() 作为背景
  else                 → 使用 span.fg 作为前景，无特殊背景
```

这种设计让搜索高亮和语法高亮完全解耦，互不干扰。

#### 主题系统

主题配置通过 `~/.hirc` 的 `[theme]` 段驱动：

```toml
[theme]
editor_theme = "base16-ocean.dark"   # syntect 主题名
chat_theme   = "dracula"             # MdTheme 名称
```

配置流转路径：

```
~/.hirc → Config::theme → Renderer::new(&config)
                              │
                    ┌─────────┴─────────┐
                    ▼                   ▼
         SyntectHighlighter::new   MdRenderer::new
         (editor_theme)            (MdTheme::by_name(chat_theme))
```

编辑器侧的 `SyntectHighlighter` 支持运行时切换主题（`set_theme()`），为未来的 `:colorscheme` 命令预留了接口。Chat 侧的 `MdTheme` 提供三套内置主题（dark / dracula / tokyo-night），每套包含 30+ 个精调的 RGB 颜色值，覆盖 H1-H6 标题、行内代码、代码块、块引用、表格、链接等所有 Markdown 元素。

#### 与旧系统的关系

旧的手写 `Highlighter`（12 种语言的正则规则）仍然保留在代码中，但不再用于编辑器文本渲染。它的 `TokenKind` 枚举和 `Span` 类型仅在以下场景使用：

- 作为 `FileType` 检测的载体（`from_ext` / `from_path` / `from_content`）
- 集成测试中验证旧高亮规则的正确性

所有面向用户的渲染路径已切换到 syntect。

---

## 五、配置系统

配置文件路径：`~/.hirc`，TOML 格式。

```toml
[general]
line_numbers = true
tab_width = 4
auto_indent = true

[ai]
api_base_url = "https://api.openai.com/v1"
api_key = ""          # 优先读环境变量 HI_API_KEY
model = "gpt-4o"
timeout_secs = 30
yolo_mode = false     # true 则跳过执行计划确认步骤

[theme]
colorscheme = "default"                  # 旧字段，保持兼容
editor_theme = "base16-ocean.dark"       # syntect 主题（编辑器文本区域）
chat_theme   = "dark"                    # MdTheme 名称（Chat 面板 Markdown）

[keymaps]
# 未来支持自定义按键绑定
```

配置读取优先级：**环境变量 > ~/.hirc > 内置默认值**

---

## 六、LLM 通信设计

统一使用 OpenAI 兼容的 Chat Completion API 格式，通过配置 `api_base_url` 适配不同后端：

```
OpenAI          → api_base_url = "https://api.openai.com/v1"
Ollama（本地）   → api_base_url = "http://localhost:11434/v1"
其他兼容服务     → 修改 api_base_url 即可
```

请求结构（简化）：

```json
{
  "model": "gpt-4o",
  "messages": [
    { "role": "system", "content": "<系统 Prompt，含编辑器能力说明和安全约束>" },
    { "role": "user",   "content": "<用户意图 + 当前上下文 JSON>" }
  ],
  "response_format": { "type": "json_object" }
}
```

响应格式（简单意图）：

```json
{
  "mode": "ghost",
  "command": ":%s/中国/法国/g",
  "explanation": "全局替换"
}
```

响应格式（复杂意图）：

```json
{
  "mode": "plan",
  "steps": [
    { "description": "匹配所有数字", "command": "..." },
    { "description": "按映射表替换", "command": "..." }
  ],
  "explanation": "将阿拉伯数字替换为中文"
}
```

---

## 七、开发分期

### Phase 0 — 文档与设计（当前）
- [x] 产品理念文档（PRODUCT.md）
- [x] 架构设计文档（ARCHITECTURE.md）
- [ ] 技术规格文档（SPEC.md）
- [ ] Prompt 设计文档

### Phase 1 — Rust 核心骨架
- [ ] 项目初始化（Cargo workspace）
- [ ] Buffer Manager（基于 ropey）
- [ ] 模式状态机（Normal / Insert / Visual / Command）
- [ ] 核心按键绑定（hjkl / w/b/e / dd/yy/p / i/a/o / v/V）
- [ ] TUI 渲染（基于 crossterm，含状态栏）
- [ ] `:w` `:q` `:wq` 基础 Ex 命令
- [ ] 文件树（基础版）
- [ ] 语法高亮（XML / YAML / JSON 优先）

### Phase 2 — AI 建议模式
- [ ] `?` 键触发 AI 模式
- [ ] 上下文收集模块
- [ ] LLM API 客户端（tokio async）
- [ ] Intent Classifier
- [ ] 幽灵文字渲染
- [ ] 执行规划面板 UI
- [ ] 顾问模式文字显示
- [ ] 命令安全过滤
- [ ] Undo 保护（AI 操作前自动打断点）
- [ ] `~/.hirc` 配置读取

### Phase 3 — AI 编排模式（ReAct）
- [ ] 工具调用框架（Actions API）
- [ ] ReAct 循环驱动器
- [ ] 推理过程可视化（侧边展示 Thought/Action/Observation）
- [ ] 多步回滚机制
- [ ] yolo 模式

### Phase 4 — 完善与生态
- [ ] 更多语法高亮（Java / Python / Rust / Go）
- [ ] 状态栏提示智能化（基于使用习惯学习）
- [ ] 配置热重载
- [ ] 主题系统
