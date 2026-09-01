# Route 维护文档

> **One Markdown, give it to your AI, and start using Route.**
> *From Route to Routine.*
> Version 0.5

---

## 目录

1. [项目概述](#1-项目概述)
2. [架构分层 (MECE)](#2-架构分层-mece)
3. [Crate 详解](#3-crate-详解)
4. [五接口功能矩阵](#4-五接口功能矩阵)
5. [PPAM 集成](#5-ppam-集成)
6. [Root SOP](#6-root-sop)
7. [记忆与搜索复合系统](#7-记忆与搜索复合系统)
8. [开发工作流](#8-开发工作流)
9. [构建与发布](#9-构建与发布)

---

## 1. 项目概述

Route 是一个轻量级版本管理系统，专为 Vibe Coding 场景设计。核心理念：

- **零门槛分发**：单个 `ROUTE.md` 文件交给 AI，AI 自动下载并操作
- **五接口覆盖**：CLI / MCP / HTTP / TUI / PyO3，所有功能对等
- **Root SOP**：AI 编码前必须检查项目结构、记忆和因果链
- **记忆 + 搜索复合体**：ContextPipeline 提供上下文，Engine 超限降级模糊匹配
- **版本保护**：Guard 机制防止 AI 误删 `.route/` 和 `.route-basic/` 数据

### 技术栈

- **语言**：Rust (所有核心 crate)，TypeScript (packages/cli, packages/core)，Python (packages/route-py)
- **数据库**：SQLite (via rusqlite)
- **序列化**：serde + serde_json
- **CLI 框架**：clap
- **HTTP 服务**：axum + tokio + tower-http
- **MCP 协议**：自定义 JSON-RPC 实现
- **TUI**：rustyline REPL
- **Python 绑定**：PyO3 + maturin
- **备份传输**：WebDAV / S3 / SSH
- **搜索引警**：FuzzyMatcher (Levenshtein) + BM25 + HybridVectorizer

---

## 2. 架构分层 (MECE)

Route 的 crates 按四层互斥分类，每层内部高内聚、层间低耦合：

```
┌─────────────────────────────────────────────────────────────────────┐
│                     Core Engine (原始能力)                           │
│  route-core      route-basic                                        │
│  原始能力层：路径、哈希、Guard、Schema、SQLite 迁移                   │
│  仓库引擎：图式提交模型、快照管理、分支、标签、冲突                    │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    Intelligence (智能层)                              │
│  route-engine     route-memory                                       │
│  搜索：模糊匹配、关键词索引、Token 预算估算、向量化器                   │
│  记忆：分层记忆、因果链、对话跟踪、上下文管道、项目结构                  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    Interfaces (用户接口层)                            │
│  route-cli   route-mcp   route-tui   route-http   route-pyo3        │
│  终端命令    AI 协议      交互式终端  REST API      Python 原生       │
│  所有接口功能对等，底层复用 route-basic + route-memory                │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    Services (服务层)                                  │
│  route-sync    route-plugins    route-stats                         │
│  备份同步      插件系统          统计报表                              │
│  (WebDAV/S3/SSH) (Webhook/Logger) (提交/快照统计)                    │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. Crate 详解

### 3.1 Core Engine

| Crate | 职责 | 关键文件 | 说明 |
|-------|------|---------|------|
| **route-core** | 原始能力层 | `guard.rs`, `hash.rs`, `paths.rs`, `schema.rs`, `storage.rs` | 文件路径标准化、xxh3/SHA-256 哈希、Guard 保护机制、SQLite schema 迁移、存储读写 |
| **route-basic** | 仓库引擎 | `repository.rs`, `models.rs`, `index.rs`, `ai_conflict.rs`, `export/` | 图式提交模型 (Edge-on-Node)、快照管理、分支/标签、CommitOptions、DiffSummary、Conflict 裁决、Index 构建、导出 (JSON/Markdown) |

### 3.2 Intelligence

| Crate | 职责 | 关键文件 | 说明 |
|-------|------|---------|------|
| **route-engine** | 搜索引警 | `fuzzy.rs`, `index.rs`, `token.rs` | FuzzyMatcher (Levenshtein)、KeywordIndex (倒排索引)、TokenBudget (预算估算/裁剪)、HybridVectorizer (Fast/ML/Auto) |
| **route-memory** | 记忆系统 | `memory.rs`, `causal.rs`, `conversation.rs`, `context.rs`, `structure.rs` | MemoryStore (Tiered: Core/Hot/Cold/Archived)、CausalChain (因果链)、ConversationStore (对话跟踪+快照回滚)、ContextPipeline (上下文组装)、ProjectStructure (文件树扫描) |

### 3.3 Interfaces

| Crate | 职责 | 关键文件 | 说明 |
|-------|------|---------|------|
| **route-cli** | CLI 二进制 | `main.rs`, `commands.rs`, `git_commands.rs`, `sync_commands.rs`, `plugin_commands.rs` | 所有 CLI 子命令，Git 风格参数，MCP 配置输出，Permission 管理 |
| **route-mcp** | MCP 服务器 | `main.rs` | JSON-RPC 工具处理器：tracking/*、extensions/*、project_context、ai_chat、conversation_* |
| **route-tui** | TUI REPL | `main.rs`, `repl.rs`, `commands.rs`, `ai.rs`, `git.rs`, `fmt.rs`, `banner.rs` | 交互式终端，AI 对话命令，Git 操作，彩色格式化输出 |
| **route-http** | HTTP REST API | `main.rs`, `lib.rs`, `handlers/*` | 80+ 端点，axum 框架，CORS，统一错误格式，所有操作覆盖 |
| **route-pyo3** | Python 绑定 | `lib.rs` | 28 个导出函数，仓库操作/分支/标签/对话跟踪，json_to_py 转换器 |

### 3.4 Services

| Crate | 职责 | 关键文件 | 说明 |
|-------|------|---------|------|
| **route-sync** | 备份同步 | `engine.rs`, `config.rs`, `modes.rs`, `scheduler.rs`, `transports/*` | Mirror/Backup/Archive 三种模式，WebDAV/S3/SSH 传输，定时调度，文件过滤器 |
| **route-plugins** | 插件系统 | `bus.rs`, `events.rs`, `plugins.rs`, `config.rs`, `context.rs` | EventBus 事件总线，Webhook/Logger 内置插件，插件生命周期管理 |
| **route-stats** | 统计报表 | `collector.rs`, `models.rs`, `render.rs` | 提交统计、快照统计、时间序列、渲染输出 |

### 3.5 Packages

| Package | 职责 | 说明 |
|---------|------|------|
| **packages/cli** | TypeScript CLI 封装 | 用 Node.js 包装 Route 命令行 |
| **packages/core** | TypeScript 核心库 | Route 核心数据结构的 TypeScript 实现 |
| **packages/route-py** | Python 包 | `maturin build` 构建，`pip install route-vc` 安装 |

### 3.6 存储与开发历史治理

Route 的开发历史语义由内容身份以及 snapshot / manifest / commit
关系决定，独立于文件在磁盘上的物理表示。改变压缩方式不得改变对象哈希、
引用关系或读取语义，也不得借存储优化之名重写历史。

- `Wiki/` 是可增删、可热插拔的外部参考资料，不进入 Basic snapshot。
- `Release/`、`target/`、`target-*` 以及其他构建输出不是开发历史，扫描时排除。
- NTFS 透明压缩属于宿主环境的物理空间优化；Route 仍按解压后的逻辑字节读取和
  计算内容身份，不把压缩属性写入 manifest。
- Packed Object Storage 仅是未来候选物理后端，当前**未实现**。任何未来方案都必须
  保持 SHA/内容身份和 manifest/commit 引用不变，支持 loose/packed 迁移期互操作、
  独立索引、原子建包、崩溃安全迁移、删除 loose object 前验证、可逆回退，并在晋级前
  提供基准证据。

---

## 4. 五接口功能矩阵

所有接口功能对等，当前状态：

### 版本跟踪 (18 项)

| 功能 | CLI | MCP | HTTP | TUI | PyO3 |
|------|:---:|:---:|:----:|:---:|:----:|
| init | ✅ | ✅ | ✅ | ✅ | ✅ |
| status | ✅ | ✅ | ✅ | ✅ | ✅ |
| commit | ✅ | ✅ | ✅ | ✅ | ✅ |
| log | ✅ | ✅ | ✅ | ✅ | ✅ |
| rollback | ✅ | ✅ | ✅ | ✅ | ✅ |
| changes | ✅ | ✅ | ✅ | ✅ | ✅ |
| diff | ✅ | ✅ | ✅ | ✅ | ✅ |
| undo | ✅ | ✅ | ✅ | ✅ | ✅ |
| redo | ✅ | ✅ | ✅ | ✅ | ✅ |
| history | ✅ | ✅ | ✅ | ✅ | ✅ |
| checkpoint | ✅ | ✅ | ✅ | ✅ | ✅ |
| stats | ✅ | ✅ | ✅ | ✅ | ✅ |
| backup | ✅ | ✅ | ✅ | ✅ | ✅ |
| annotate | ✅ | ✅ | ✅ | ✅ | ✅ |
| annotations | ✅ | ✅ | ✅ | ✅ | ✅ |
| export | ✅ | ✅ | ✅ | ✅ | ❌ |
| branch | ✅ | ✅ | ✅ | ✅ | ✅ |
| tag | ✅ | ✅ | ✅ | ✅ | ✅ |

### 对话跟踪 (7 项)

| 功能 | CLI | MCP | HTTP | TUI | PyO3 |
|------|:---:|:---:|:----:|:---:|:----:|
| new | ✅ | ✅ | ✅ | ✅ | ✅ |
| list | ✅ | ✅ | ✅ | ✅ | ✅ |
| show | ✅ | ✅ | ✅ | ✅ | ✅ |
| record | ✅ | ✅ | ✅ | ✅ | ✅ |
| rollback | ✅ | ✅ | ✅ | ✅ | ✅ |
| archive | ✅ | ✅ | ✅ | ✅ | ✅ |
| delete | ✅ | ✅ | ✅ | ✅ | ✅ |

### 高级功能 (8 项)

| 功能 | CLI | MCP | HTTP | TUI | PyO3 |
|------|:---:|:---:|:----:|:---:|:----:|
| tracking | ✅ | ✅ | ✅ | ✅ | ❌ |
| extensions | ✅ | ✅ | ✅ | ❌ | ❌ |
| ai_chat | ✅ | ✅ | ✅ | ✅ | ❌ |
| permission | ✅ | ✅ | ✅ | ❌ | ❌ |
| sync | ✅ | ✅ | ✅ | ❌ | ❌ |
| project_context | ✅ | ✅ | ✅ | ✅ | ❌ |
| git | ✅ | ✅ | ✅ | ✅ | ❌ |
| plugins | ✅ | ❌ | ❌ | ❌ | ❌ |

---

## 5. PPAM 集成

### 架构

```
Route Repository (远程: longqiyua/route)          PPAM Repository (远程: longqiyua/ppam)
┌──────────────────────────────────┐             ┌──────────────────────────────┐
│ .ppam-link (指针文件)             │────指针──→  │ PPAM 可插拔文档增强组件       │
│ crates/route-ppam/ (被 git 忽略)  │             │ (独立项目，独立仓库)          │
│ ppam/ (被 git 忽略, 本地安装)      │             │                              │
└──────────────────────────────────┘             └──────────────────────────────┘
         │
         ▼
本地开发环境
┌──────────────────────────────────────────────┐
│ crates/route-ppam/ (本地桥接层)               │
│ route_ppam::PpamBridge                       │
│  ├─ project_context()                        │
│  ├─ track_document_version()                 │
│  ├─ search_memory()                          │
│  ├─ record_memory()                          │
│  ├─ document_history()                       │
│  ├─ rollback_document()                      │
│  ├─ remote_url()    ← 读取 .ppam-link 远程   │
│  ├─ ppam_path()     ← 读取 .ppam-link 本地   │
│  └─ ppam_read_file()← 读取 ppam/ 下文件       │
└──────────────────────────────────────────────┘
```

### 隔离策略

PPAM 的文件（`1_Configuration/`, `2_Request/`, `3_Prompt/`, `4_Expand/`, `.trae/` 等）**全部安装在 `ppam/` 子目录下**，不与 Route 项目根目录的文件混在一起。这保证了：

- Route 的根目录保持干净（只有 `Cargo.toml`, `.ppam-link`, `MAINTENANCE.md` 等 Route 自身文件）
- PPAM 的 naming/trim 规则不会误应用到 Route 的根目录
- 两者互不干扰，各自独立升级

### 关键文件

| 文件 | 位置 | 仓库 | 说明 |
|------|------|------|------|
| `.ppam-link` | Route 仓库根目录 | ✅ 跟踪在 git | 指针文件，指向远程 PPAM 仓库 + 本地安装路径 |
| `crates/route-ppam/` | Route 仓库内 | ❌ git 忽略 | 本地桥接层，远程 PPAM 仓库的本地副本 |
| `ppam/` | Route 仓库根目录 | ❌ git 忽略 | PPAM 安装目录（克隆远程仓库到此处） |
| `PpamBridge::remote_url()` | `route-ppam/src/lib.rs` | 读取 `.ppam-link` | 获取远程 PPAM 仓库地址 |
| `PpamBridge::ppam_path()` | `route-ppam/src/lib.rs` | 读取 `.ppam-link` | 获取本地 PPAM 安装路径（默认 `ppam/`） |

### 安装方式

```bash
# 首次安装：克隆 PPAM 到 ppam/ 子目录
git clone https://github.com/longqiyua/ppam.git ppam/

# 更新 PPAM
cd ppam && git pull && cd ..
```

### 维护说明

- **本地**：`crates/route-ppam/` 是 Route 仓库的一部分（本地开发用），但**不推送到远程 Route 仓库**
- **远程**：PPAM 有自己独立的远程仓库（`longqiyua/ppam`），`ppam/` 目录克隆自该远程
- **指针**：`.ppam-link` 是 Route 仓库中唯一关于 PPAM 的跟踪文件，其他 AI/用户通过它找到 PPAM
- **分离**：修改 Route 仓库时不需要修改 PPAM 代码，反之亦然
- **隔离**：PPAM 文件全部在 `ppam/` 下，不污染 Route 根目录

---

## 6. Root SOP

Root SOP 是 Route 最核心的操作规程，AI 必须严格遵守：

```
┌─────────────────────────────────────────────────────────────┐
│              BEFORE any code change                          │
├─────────────────────────────────────────────────────────────┤
│  1. Read project structure (file tree)                      │
│  2. Read project memory (architecture, decisions)           │
│  3. Read causal chain (recent operations)                   │
│  4. Consult references (conventions, docs)                  │
│  5. THEN write code via route commit                        │
│                                                             │
│              AFTER any code change                           │
├─────────────────────────────────────────────────────────────┤
│  1. Update project memory (new decisions)                   │
│  2. Update causal chain (what changed and why)              │
│  3. Update references (if conventions changed)              │
└─────────────────────────────────────────────────────────────┘
```

### Guard 保护机制

- **保护目录**: `.route/`, `.route-basic/`
- **保护文件**: `.route-guard` (锚点文件)
- **版本控制**: `RouteGuard` 记录 `version: "0.5.0"`，`check_version()` 验证版本一致性
- **权限等级**: `PermissionLevel::High` / `Normal`，CLI 子命令 `route permission status/set`

### 对话跟踪与快照回滚

- **存储**: `.route/conversations.json` (单一 JSON 文件，热插拔)
- **结构**: Session → Message (两级)，Message.snapshot_id 关联项目快照
- **回滚**: 截断消息列表 + 恢复关联快照 + 生成系统消息
- **ID 生成**: `{prefix}-{timestamp}-{counter}` (高并发安全)

---

## 7. 记忆与搜索复合系统

```
MemoryStore (分层记忆)
├── Core (不可变：架构决策、项目事实)
├── Hot (近期活跃：当前变更、活动决策)
├── Cold (历史数据：已完成任务)
└── Archived (按需加载)
     │
     ▼
ContextPipeline (上下文组装)
├── 读取记忆条目
├── 读取项目结构 (StructureSnapshot)
├── 读取因果链 (CausalChain)
└── 读取对话历史 (最近消息)
     │
     ▼
TokenBudget (Token 预算估算)
├── 未超限 → 直接返回完整上下文
└── 超限 → 调起 Engine 降级
     │
     ▼
route-engine (降级搜索)
├── FuzzyMatcher (Levenshtein 模糊匹配)
├── KeywordIndex (倒排索引)
├── BM25 (语义检索)
├── HybridVectorizer (Fast/ML/Auto)
└── TokenBudget (智能裁剪)
```

### 搜索方法链

| 优先级 | 方法 | 来源 | 说明 |
|--------|------|------|------|
| 1 | `MemoryStore::search(query)` | route-memory | 前缀匹配 → 模糊回退 |
| 2 | `MemoryStore::search_by_prefix(prefix)` | route-memory | 精确前缀匹配 |
| 3 | `MemoryStore::search_by_fuzzy(query, threshold)` | route-memory → route-engine | Levenshtein 模糊匹配 |
| 4 | `FuzzyMatcher::fuzzy_search(query, candidates)` | route-engine | 批量候选模糊匹配 |

---

## 8. 开发工作流

### 8.1 本地开发

Route workspace 位于 `tool/route/`。以下命令均在该目录执行：

```bash
# 完整构建
cd tool/route
cargo build --release

# 构建特定 crate
cargo build -p route-cli

# 运行测试（全部）
cargo test --workspace

# 运行特定测试
cargo test -p route-memory -p route-core

# 运行 Python 绑定测试
maturin develop -m tool/route/packages/route-py/pyproject.toml
python -c "import route; print(route.status())"
```

### 8.2 代码规范

- **提交消息格式**: `type: description` (feat/fix/refactor/docs/test)
- **分支策略**: main (稳定) / feature-* (开发) / fix-* (修复)
- **测试要求**: 核心逻辑必须附带单元测试，API 变化需更新对应接口测试
- **错误处理**: 使用 `anyhow::Result`，关键错误必须记录日志

### 8.3 新增功能流程

1. 在 `route-basic` 或 `route-core` 添加核心逻辑
2. 同步更新 `route-memory`（如果涉及记忆/对话）
3. 通过 CLI 暴露（`route-cli/src/commands.rs`）
4. 同步更新 MCP（`route-mcp/src/main.rs`）
5. 同步更新 HTTP（`route-http/src/handlers/`）
6. 同步更新 TUI（`route-tui/src/commands.rs`）
7. 同步更新 PyO3（`route-pyo3/src/lib.rs`）
8. 更新 ROUTE.md 和 MAINTENANCE.md

---

## 9. 构建与发布

### 9.1 构建产物

| 产物 | 路径 | 说明 |
|------|------|------|
| `route` CLI 二进制 | `target/release/route.exe` | 主命令行工具 |
| `route-mcp` 服务器 | `target/release/route-mcp.exe` | MCP 协议服务器 |
| `route-tui` REPL | `target/release/route-tui.exe` | 交互式终端 |
| `route-http` 服务器 | `target/release/route-http.exe` | HTTP REST API |
| `route_pyo3` DLL | `target/release/route.dll` | Python 原生模块 |
| `route-vc` wheel | `target/wheels/route_vc-*.whl` | PyPI 安装包 |

### 9.2 发布顺序

**crates.io**（按依赖顺序）：
```
route-core → route-plugins → route-basic → route-engine → route-memory
→ route-sync → route-stats → route-cli → route-mcp → route-tui
→ route-http → route-pyo3
```

**PyPI**：
```bash
maturin build --release -m tool/route/packages/route-py/pyproject.toml
maturin upload target/wheels/route_vc-*.whl
```

**GitHub Release**：
- 标题：`V---`
- 描述：`One Markdown, give it to your AI, and start using Route.`
- 附带：`ROUTE.md` 和所有构建产物

### 9.3 版本号规则

| 位置 | 版本 | 说明 |
|------|------|------|
| `Cargo.toml` workspace | `0.4.0-beta` | 开发版本 |
| `ROUTE.md` 标题 | `V---` | 发布版本（不递增） |
| `route-ppam/Cargo.toml` | `0.5.0` | 独立版本 |
| `.route-guard` | `0.5.0` | Guard 版本 |
| `tool/route/packages/route-py/pyproject.toml` | `0.4.0b0` | Python 包版本 |

---

## 附录：文件清单

### 根目录

| 文件 | 说明 | 跟踪 |
|------|------|------|
| `ROUTE.md` | AI 分发入口文件 | ✅ |
| `README.md` | GitHub 仓库主页 | ✅ |
| `MAINTENANCE.md` | 本文件，维护文档 | ✅ |
| `LICENSE` | AGPL-3.0 许可证 | ✅ |
| `Cargo.toml` | Workspace 配置 | ✅ |
| `.gitignore` | Git 忽略规则 | ✅ |
| `.ppam-link` | PPAM 远程指针 | ✅ |
| `explain.md` | Feature Toggle 说明 | ✅ |

### 关键目录

| 目录 | 说明 | 跟踪 |
|------|------|------|
| `crates/` | Rust crates (13 个) | ✅ |
| `crates/route-ppam/` | PPAM 桥接层 (本地) | ❌ |
| `packages/` | TypeScript + Python 包 | ✅ |
| `archive/` | 归档的 GUI 代码 | ✅ |
| `document/` | 架构文档 | ✅ |
| `target/` | 构建产物 | ❌ |
| `.venv/` | Python 虚拟环境 | ❌ |

---

> 最后更新: 2026-08-04
> 维护者: Route Contributors
> 协议: AGPL-3.0
