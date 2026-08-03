# Route — 版本管理 Agent 架构 V1.0

## 核心定位

Route 是一个**版本管理 Agent**，以 **Root Engine → Root Memory → Root Base** 三层架构驱动，
为 vibecoding 场景提供专业级版本管理、代码理解、记忆追溯和 AI 漂移控制。

```
用户 Vibecoding AI (Cursor/Windsurf/Codex/Claude Code)
    │
    ├── MCP 调用 ──────────────────────────────────┐
    │                                               ▼
    │                                      ┌─────────────────┐
    │                                      │   Root Base     │
    └──→ CLI 调用 ────────────────────────→│  (顶层编排器)     │
    │                                      └────────┬────────┘
    │                                               │
    ▼                                               ▼
┌──────────────────────────────────────────────────────────────────────┐
│                        Root Engine (驱动层)                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐│
│  │ Code     │  │ GraphRAG │  │ Adaptive │  │ 索引管理器            ││
│  │ Symbol   │  │ +AST+LSP │  │ Debounce │  │ (热索引/冷索引)       ││
│  └──────────┘  └──────────┘  └──────────┘  └──────────────────────┘│
│  ┌──────────────────────────────────────────────────────────────────┐│
│  │ Code Graph 三机制                                                ││
│  │  • Vector 向量检索 (相似度匹配)                                  ││
│  │  • Graph 图索引 (总索引 - 依赖关系/调用链)                       ││
│  │  • Keyword 关键词索引 (关键索引 - 精确匹配)                     ││
│  └──────────────────────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌──────────────────────────────────────────────────────────────────────┐
│                        Root Memory (存储层)                          │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────────┐│
│  │ 项目记忆      │  │ 因果链        │  │ 热/冷索引管理              ││
│  │ (元信息+结构) │  │ (副作用追踪)  │  │ (内存水位控制)             ││
│  └──────────────┘  └──────────────┘  └────────────────────────────┘│
│  输出: JSON (结构化) + MD/Mermaid (人类可读)                         │
└──────────────────────────────────────────────────────────────────────┘
```

## 三层架构

### 1. Root Base（顶层编排器）

Root Base 是系统的**唯一入口**，负责：
- 初始化 Root Engine 和 Root Memory
- 提供统一 API（CLI / MCP / GUI）
- 管理生命周期（启动/优雅关闭/重置）
- 服务注册与发现（支持后续扩展）

```rust
pub struct RootBase {
    pub engine: RootEngine,   // 驱动层
    pub memory: RootMemory,   // 存储层
    pub config: BaseConfig,    // 全局配置
    pub registry: ServiceRegistry, // 服务注册表（可扩展）
}
```

**扩展规划**：Root Base 目前只包含 Root Memory，后续可扩展：
- `RootBase.storage` — 持久化存储层
- `RootBase.sync` — 同步引擎
- `RootBase.plugin` — 插件系统
- `RootBase.network` — 网络层

### 2. Root Engine（驱动层）

Root Engine 是系统的**计算核心**，包含：

#### 2.1 Code Symbol（Tree-sitter 集成）
- 解析代码为符号（函数、变量、类、接口）
- 支持多语言（Rust、TypeScript、Python、Go、Java 等）
- 输出结构化符号表

#### 2.2 Code Graph（MECE 模块化）
- 按函数而非文件进行 MECE 法则模块化区分
- 构建代码依赖关系图（调用图、继承图、引用图）
- 支持 LSP 协议补充语义信息

#### 2.3 GraphRAG
- 全面采用 GraphRAG 架构
- 生成依赖关系图
- 基于 AST 的语义索引
- LSP 语义补全

#### 2.4 三机制检索

| 机制 | 类型 | 用途 | 算法 |
|------|------|------|------|
| Vector | 向量检索 | 相似匹配 | Cosine + HNSW |
| Graph | 图索引 | 总索引 - 依赖/调用链 | PageRank + BFS |
| Keyword | 关键词索引 | 关键索引 - 精确/模糊匹配 | Trie + BM25 + Jaccard |

#### 2.5 自适应防抖
- 移植自 GOTO Engine 的 `adaptive_refresh.rs`
- 跟踪用户输入速度（EMA 平滑）
- 动态计算防抖/节流时间
- 搜索编排（防抖等待用户停顿 + 节流保证最低刷新间隔）

### 3. Root Memory（存储层）

Root Memory 是系统的**持久化大脑**，负责：

#### 3.1 项目记忆
- 元信息：项目名称、用途、约束、语言、框架
- 结构图：Mermaid 格式的项目模块结构
- 关键模块：函数的入口点、核心模块、数据流

#### 3.2 因果链
- 每次变更记录原因、效果、副作用
- 可追溯的完整因果链条
- 分支决策记录

#### 3.3 热索引/冷索引分离

```
┌─────────────────────────────────────────────┐
│ 内存 (RAM)                                   │
│  ┌─────────────────────────────────────────┐│
│  │ 热索引 (Hot Index)                      ││
│  │  • 当前活跃项目的完整索引                ││
│  │  • 最近修改的文件索引                    ││
│  │  • 搜索缓存 (LRU)                       ││
│  └─────────────────────────────────────────┘│
├─────────────────────────────────────────────┤
│ 磁盘 (Disk)                                  │
│  ┌─────────────────────────────────────────┐│
│  │ 冷索引 (Cold Index)                      ││
│  │  • 历史项目的完整索引                    ││
│  │  • 久未访问的文件索引                    ││
│  │  • 序列化到 JSON/MD                     ││
│  └─────────────────────────────────────────┘│
└─────────────────────────────────────────────┘
```

**内存水位控制**：
- 热索引上限：`max_hot_mb` (默认 256MB)
- 当热索引超过阈值时，自动将最久未访问的 20% 降温至冷索引
- 冷索引按需加载，加载后缓存 5 分钟

#### 3.4 输出格式
- **JSON**: 结构化数据，供 AI 和程序读取
- **MD**: Mermaid 图表，供人类读取

## 自反性（Self-Referential）

Route 软件本身使用自己的架构进行代码管理：

```
Route 的源代码
    │
    ├── Root Engine 解析 Route 自身代码
    │   ├── Tree-sitter 解析 Rust 符号
    │   ├── Code Graph 生成 Route 模块依赖图
    │   └── GraphRAG 索引 Route 自身代码
    │
    ├── Root Memory 记忆 Route 自身项目
    │   ├── project.json (Route 的项目元信息)
    │   ├── structure.mermaid (Route 的模块结构图)
    │   └── chain.jsonl (Route 的变更因果链)
    │
    └── Root Base 用自身架构管理自身版本
        └── route 命令管理自己的仓库
```

## 模块分层

```
route/
│
├── route-base/          ← Root Base（顶层编排器）
│   ├── lib.rs           ← RootBase 主结构体
│   └── registry.rs      ← 服务注册表
│
├── route-engine/        ← Root Engine（驱动层）
│   ├── symbol.rs        ← Tree-sitter 代码符号解析
│   ├── code_graph.rs    ← 代码图谱 + MECE 模块化
│   ├── graphrag.rs      ← GraphRAG + AST + LSP
│   ├── vector.rs        ← Vector 向量检索
│   ├── graph_idx.rs     ← Graph 图索引
│   ├── keyword.rs       ← Keyword 关键词索引
│   ├── adaptive.rs      ← 自适应防抖（移植自 GOTO Engine）
│   ├── indexer.rs       ← 热/冷索引管理器
│   ├── trie.rs          ← Trie 前缀树
│   ├── fuzzy.rs         ← 模糊匹配
│   ├── bm25.rs          ← BM25 语义检索
│   └── pipeline.rs      ← 搜索管线
│
├── route-memory/        ← Root Memory（存储层）
│   ├── project.rs       ← 项目元信息
│   ├── structure.rs     ← 项目结构（Mermaid）
│   ├── chain.rs         ← 因果链
│   ├── hot_cold.rs      ← 热/冷索引分离
│   └── output.rs        ← JSON + MD 输出
│
├── route-cli/           ← CLI 主入口（面向专业开发者）
├── route-mcp/           ← MCP 服务入口（面向 AI）
├── route-tauri/         ← GUI（降级为辅助，最低优先级）
├── route-vm/            ← 版本管理 Agent
├── route-vibe/          ← Vibecoding 集成层
├── route-skill/         ← 技能系统
├── route-test/          ← 内置 Benchmark
└── route-plugins/       ← 预设插件
```

## 数据流

```
用户输入 (自然语言 / 代码操作)
    │
    ▼
Root Base (入口)
    │
    ├── Adaptive Debounce (防抖/节流)
    │
    ▼
Root Engine (驱动)
    │
    ├── Keyword 检索 (精确匹配) → 快速命中 → 返回
    ├── Graph 检索 (依赖关系) → 上下文扩展
    └── Vector 检索 (相似匹配) → 语义扩展
    │
    ▼
Code Graph (MECE 模块化)
    │
    ├── Tree-sitter 解析符号
    ├── 构建函数级依赖图
    └── GraphRAG 增强检索
    │
    ▼
Root Memory (存储)
    │
    ├── 热索引 (内存) → 即时响应
    └── 冷索引 (磁盘) → 按需加载
    │
    ▼
Root VM (版本管理 Agent)
    │
    ├── plan → act → observe 循环
    ├── 因果控制 (副作用检测)
    └── git 操作 (提交/分支/合并/回滚)
    │
    ▼
输出 (JSON / MD / Mermaid)
```

## CLI 集成 (Root Base)

`route base` 子命令提供 Root Base 的完整 CLI 集成：

| 命令 | 功能 |
|------|------|
| `route base status` | 显示 Root Base 状态（内存模式、因果控制、索引统计） |
| `route base init` | 初始化项目记忆和引擎索引 |
| `route base search <query>` | 三机制代码搜索（Vector + Graph + Keyword） |
| `route base memory` | 显示内存统计和项目结构 |
| `route base causal` | 显示因果链记录 |

### 自反性管理

`route base self-manage` 子命令实现 Route 管理自身代码：

| 命令 | 功能 |
|------|------|
| `route base self-manage index` | 用 Root Engine 索引 Route 自身源码 |
| `route base self-manage structure` | 生成 Route 自身模块结构的 Mermaid 图 |
| `route base self-manage record` | 记录 Route 开发过程中的因果链 |
| `route base self-manage git-log` | 展示 Route 自身 git 历史与因果链 |
| `route base self-manage introspect` | 全量自省：索引代码 + 平衡索引 + 记录因果 |

实现方式：`route-base` feature gate 控制，开启后自动注册 `Base` 子命令。

## MCP 集成 (Root Base)

`route-mcp` 服务器新增三个 Root Base 工具，供 AI 客户端调用：

| 工具 | 描述 | 参数 |
|------|------|------|
| `route_base_status` | 获取 Root Base 状态 | 无参数 |
| `route_base_search` | 三机制代码搜索 | `query` (必填), `top_k` (可选) |
| `route_base_memory` | 内存统计与项目结构 | 无参数 |

AI 使用示例：
1. 调用 `route_base_status` 了解项目记忆状态
2. 调用 `route_base_search` 搜索特定代码
3. 调用 `route_base_memory` 获取项目结构和因果链

## 自反性实践 (Self-Referential in Practice)

Route 的自我管理遵循以下流程：

```
[开发者] → route base self-manage introspect
    │
    ├── [1/3] Root Engine 扫描 crates/ 目录
    │   ├── 递归遍历所有 .rs 文件
    │   ├── 跳过 target/.git/node_modules/.route/gen/dist
    │   └── 每行非空非注释代码 → add_line() 索引
    │
    ├── [2/3] HotColdIndex.auto_balance()
    │   ├── 热索引保持在 max_hot_mb 水位以下
    │   └── 自动转移冷数据到磁盘
    │
    └── [3/3] record_causal_link()
        ├── action: "introspect"
        ├── reason: "Self-introspection: full scan of Route codebase"
        └── effect: "Indexed N files, M symbols"
```

这种自反性保证：
- **稳定性**：Route 的每个版本都使用自身架构管理
- **兼容性**：架构变更时，Route 自身代码立即验证可行性
- **可追溯**：每次开发操作都有因果链记录

## 工程标准

1. **模块化设计**：每个 crate 职责单一，通过 trait 隔离
2. **可测试性**：每个模块有单元测试，集成测试覆盖完整流程
3. **内存安全**：Rust 所有权系统 + 热/冷索引水位控制
4. **兼容性**：CLI + MCP 双入口，GUI 可选
5. **自反性**：软件用自身架构管理自身代码

## 版本号

V1.0 — 架构重构版本 (Root Engine + Root Memory + Root Base)