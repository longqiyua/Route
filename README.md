# Route

> **From Route to Routine.**
>
> **One Markdown, give it to your AI, and start using Route.**
>
> **一个 Markdown，交给你的 AI，然后开始使用 Route。**

---

## 简介

Route 是一个面向 AI 时代的**版本管理 + 代码智能引擎**。它的核心理念是**零门槛分发**——你只需要把 [`ROUTE.md`](ROUTE.md) 交给任意 AI，AI 就能自动接管项目的版本管理、代码理解和自动化工作流。

不同于传统的 DevOps 工具（需要安装、配置、学习），Route 的设计从第一天起就围绕一个目标：**让 AI 成为用户的操作入口**。用户不需要学习 CLI 参数、不需要理解 Git 原理、不需要配置 CI/CD——只需要把需求告诉 AI，AI 通过 Route 的 MCP 协议和 CLI 接口完成所有操作。

Route 本身是一个 Rust 工作区，包含 15+ 个独立 crate，覆盖搜索引擎、向量索引、记忆系统、RAG 管线、备份同步、MCP 服务器、TUI 终端、Python 绑定等能力。所有核心功能通过 `route` CLI 和 `route-mcp` MCP 服务器暴露，GUI 桌面应用作为可选特性开关默认隐藏。

---

## 设计理念

### 1. AI First，Human Optional

传统工具把人作为主要操作者，AI 只是辅助。Route 反过来：**AI 是主要操作者，人类只需要表达需求**。所有功能（CLI、MCP、HTTP、PyO3）都面向 AI 调用设计，MCP 是 AI 与 Route 交互的主通道。

### 2. 零门槛分发

> **One Markdown, give it to your AI, and start using Route.**

整个 Route 的分发单位就是一个 [`ROUTE.md`](ROUTE.md) 文件。用户不需要安装任何东西，只需要把这个文件交给 AI。AI 会自动：
1. 下载仓库
2. 构建项目
3. 配置 MCP 服务器
4. 初始化用户项目
5. 开始管理

### 3. 规则匹配优先，ML 增强兜底

在模糊匹配和向量化中，Route 始终坚持**规则匹配优先**的策略：
- **Fast 模式**：基于词频统计的向量化，64 维，O(n) 复杂度，无状态，极速
- **ML 模式**：基于 n-gram 特征哈希的轻量嵌入，128 维，增量学习，无需外部模型
- **Auto 模式**：根据查询长度自动切换，短查询走规则，长查询走 ML

### 4. 功能隐藏，按需启用

GUI 桌面应用默认不构建、不启用。通过 `route base gui enable/disable` 控制。详见 [`explain.md`](explain.md)。

### 5. 工程级稳定

- 358+ 个测试，0 失败
- 单一 crate 可独立移除（route-base 仅依赖 route-engine + route-memory）
- 统一错误处理、全链路日志、因果控制

---

## 功能理念

### 核心功能

| 功能 | 说明 |
|------|------|
| **语义模糊匹配** | 双模式动态切换（精确模式 / 语义模式 / 混合模式），识别函数实现逻辑而非函数名 |
| **六步 RAG 循环** | ①写代码 → ②AI 联想 → ③模块化重构 → ④RAG 重排 → ⑤RAG 向量化 → ⑥回到① |
| **混合向量化器** | 规则匹配（Fast）+ ML 嵌入（HashedNGramEmbedding），自动切换 |
| **项目记忆** | 结构化记忆条目、因果链追踪、热/冷分层存储 |
| **代码图** | 函数调用图、跨文件依赖分析、GraphRAG 图检索 |
| **自适应防抖** | 基于 EMA 打字速度和错误率的动态搜索延迟优化 |
| **备份同步** | 支持 WebDAV / S3 / SSH 三种传输，Mirror / Backup / Archive 三种模式 |
| **技能系统** | 通过 `.route/skills/` 定义 AI 技能，通过 `.route/references/` 注入参考资料 |

### 接口

| 接口 | 说明 | AI 优先 |
|------|------|---------|
| **CLI** | `route` 命令行，所有核心功能 | ✅ |
| **MCP** | Model Context Protocol，AI 工具调用 | ✅ |
| **TUI** | 终端用户界面，支持 AI 对话 | ✅ |
| **PyO3** | Python 原生模块 | ✅ |
| **HTTP** | REST API（开发中） | ✅ |
| **GUI** | Tauri 桌面应用（特性开关，默认隐藏） | ❌ |

---

## 快速开始

### 给 AI 用

把 [`ROUTE.md`](ROUTE.md) 文件交给你的 AI。AI 会按照文件中的 6 步指令自动完成所有操作。

### 自己用

```bash
# 克隆仓库
git clone https://github.com/longqiyua/route.git
cd route

# 构建
cargo build --release

# 初始化项目
cd /path/to/your/project
route init

# 使用
route base status
route base search "a + b"
route ai chat "帮我分析当前项目的架构"
```

---

## 架构

```
Root Base (orchestrator)
├── Root Engine (driver layer)
│   ├── Trie Index           — exact & prefix match
│   ├── Fuzzy Match          — Levenshtein-based fuzzy search
│   ├── BM25 Index           — semantic text retrieval
│   ├── Vector Index         — hybrid vectorizer (Fast rule + ML embed)
│   ├── Code Graph           — function call graph & dependency analysis
│   ├── GraphRAG             — graph-based retrieval augmented generation
│   ├── Semantic Index       — dual-mode (exact/semantic) fuzzy matching
│   ├── Keyword Index        — keyword-based search
│   ├── Hot/Cold Index       — adaptive memory management
│   └── Search Pipeline      — multi-stage search orchestrator
├── Root Memory (storage layer)
│   ├── Project Memory       — structured memory entries
│   ├── Causal Chain         — action → effect tracking
│   ├── Tiered Memory        — hot/cold/archived memory tiers
│   └── Mermaid Output       — visual structure & chain diagrams
└── Interfaces
    ├── CLI                  — route command
    ├── MCP                  — model context protocol server
    ├── HTTP                 — REST API (in development)
    └── PyO3                 — Python native module
```

---

## 仓库结构

```
route/
├── ROUTE.md              ← AI 分发文件（给 AI 的 6 步指令）
├── README.md             ← 本文件
├── explain.md            ← 特性开关 & 扩展接口说明
├── LICENSE               ← AGPL-3.0
├── crates/
│   ├── route-base/       ← 顶层编排器
│   ├── route-engine/     ← 搜索引擎、模糊匹配、向量、代码图、RAG
│   ├── route-memory/     ← 项目记忆、因果链、热/冷索引
│   ├── route-cli/        ← CLI 二进制
│   ├── route-mcp/        ← MCP 服务器二进制
│   ├── route-tui/        ← 终端 UI 二进制
│   ├── route-pyo3/       ← Python 绑定 (PyO3)
│   ├── route-sync/       ← 备份同步 (WebDAV / S3 / SSH)
│   ├── route-skill/      ← 技能 & 参考系统、RAG 引擎
│   ├── route-vibe/       ← Vibe session & model proxy
│   ├── route-vm/         ← VM agent、因果控制、Git 操作
│   ├── route-plugins/    ← 插件系统 (webhook, logger)
│   ├── route-stats/      ← 统计收集器
│   └── route-tauri/      ← 桌面应用 (特性开关，默认隐藏)
└── packages/
    └── desktop/           ← GUI 前端 (特性开关，默认隐藏)
```

---

## 特性开关

GUI 桌面应用默认隐藏。通过以下命令控制：

```bash
route base gui enable     # 启用 GUI
route base gui disable    # 禁用 GUI
route base gui status     # 查看 GUI 状态
```

详见 [`explain.md`](explain.md)。

---

## 许可证

AGPL-3.0 — 详见 [LICENSE](LICENSE)。