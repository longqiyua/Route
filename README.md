# Route

**Route** — 面向 Web Coding / Vibe Coding 的轻量版本管理软件，集成智能权责审计与 AI 协同管理。

以「用户与 AI 的一轮对话」为最小版本单元，支持按对话回退、分支管理、混合备份策略，并提供桌面 GUI、CLI 与 MCP 三种接入方式。

> **版本 0.4.0 — BETA**
>
> 当前处于 Beta 阶段：核心功能（文件追踪 / 回退 / 分支 / 桌面 GUI / CLI / MCP / 权责审计 / 智能提交 / 主动跟踪 / LLM 注入 / 权限系统 / 多端对齐）已可用，AI 协同模式仍在打磨。欢迎使用并[反馈问题](https://github.com/longqiyua/route/issues)。

---

## 为什么选择 Route？

Route 不是 Git 的替代品，而是 Git 的**增强层**。Git 解决了"谁在什么时候改了什么东西"，但 Route 进一步回答了 B 端和 AI 协作时代的关键问题：

### Route vs Git

| 维度 | Git | Route |
|------|-----|-------|
| **权责溯源** | 仅记录作者（git config user.name），可被任意伪造 | 设备指纹哈希注入，每次 commit 附带设备级权责凭证，不可伪造 |
| **AI 归因** | 无 AI 参与度标记 | 自动识别 AI 辅助代码，记录 Prompt 哈希，细分为"提交者"与"提示词提供者" |
| **安全级别** | 依赖开发者自觉 | 多层安全：项目盐值、设备指纹、环境一致性校验、设备白名单 |
| **隐私保护** | N/A | 本地 SHA-256 哈希链，绝不上传明文机器码 |
| **管理粒度** | 文件级追踪 | 对话级追踪（AI 一轮对话 = 一个版本单元） |
| **操作门槛** | 需要手动输入命令 | 桌面 GUI 切换模式、自动提交、一键操作 |
| **备份策略** | 依赖远程仓库 | 混合备份：本地增量 + 云端自动同步 + 定时主动跟踪 |

### 用户场景

- **AI 辅助开发团队**：谁写的代码？谁提供的 Prompt？AI 参与了百分之多少？Route 一键给出答案。
- **多设备协作**：有人用非授权设备提交了代码——Route 立刻检测并发出警告。
- **代码审计需求**：合规部门要求追溯每行代码的"物理设备 + 操作者 + AI 参与度"——Route 的责任索引库提供完整审计链。
- **自动备份焦虑**：担心忘记 push 导致代码丢失——Route 的主动跟踪模式定时自动同步云端。

---

## 核心理念

- **一个指令干一个活儿** — 每条指令声明四个点：开始点、结束点、验收点、紧急回退点。
- **追踪而非替换** — Route 不替代 Git，而是在文件系统层面做增量快照，对任何文件类型都有效。
- **AI 优先** — CLI 与 MCP 接口让 AI 代理可以直接驱动版本管理，桌面 GUI 提供可视化操作。
- **权责透明** — 每一行代码都可追溯至"物理设备 + 操作者 + AI 参与度"，满足 B 端合规要求。

---

## 功能

### 双模式系统

Route 提供两种操作模式，可在桌面 GUI 中切换：

| 模式 | 说明 | 适用场景 |
|------|------|----------|
| **标准模式** | Route 原生快照引擎（增量/全量），不依赖 Git | 纯文件追踪、快速原型、非代码项目 |
| **Git 模式** | 驱动本地 Git 二进制，所有操作映射为真实 Git 命令 | 需要完整 Git 工作流、与团队协作 |

**标准模式**覆盖基本操作：init / commit / rollback / branch / log / checkpoint / undo / redo / tag / export / diff / annotate。

**Git 模式**覆盖所有 Git 操作（见下方 CLI 表），包括远程仓库管理。

### 标准模式（默认）

| 功能 | CLI | 桌面 GUI |
|------|-----|----------|
| 选择文件夹并初始化 | `route init` | 文件夹选择器 |
| 自动追踪文件变更 | — | Watch 开关 |
| 提交（标记一次变更） | `route commit` | 提交输入框 |
| 标记点（Checkpoint） | `route checkpoint` | 标记点对话框 |
| 撤销 / 重做 | `route undo` / `route redo` | 历史导航 |
| 回退到快照 | `route rollback` | 时间线 |
| 历史追踪 | `route log` | 时间线页 |
| 创建/删除分支 | `route branch create/delete` | 分支管理 |
| 分支切换 | `route branch switch` | 分支选择 |
| 导出 | `route export` | Markdown 导出 |

### Git 模式（完整操作）

| 操作 | CLI | 桌面 GUI | 权限限制 |
|------|-----|----------|----------|
| 初始化仓库 | `route git init` | ✓ | 无 |
| 暂存/取消暂存 | `route git add` / `reset` | ✓ | 无 |
| 提交 | `route git commit -m "msg"` | ✓ | 无 |
| 分支管理 | `route git branch create/switch/delete` | ✓ | 无 |
| 合并 | `route git merge` | ✓ | 无 |
| 变基 | `route git rebase` | ✓ | 无 |
| 标签 | `route git tag create/list/delete` | ✓ | 无 |
| 暂存 | `route git stash push/pop/list` | ✓ | 无 |
| 日志/图表 | `route git log` / `log --graph` | ✓ | 无 |
| 差异对比 | `route git diff` | ✓ | 无 |
| 撤销提交 | `route git revert` | ✓ | 无 |
| Cherry-pick | `route git cherry-pick` | ✓ | 无 |
| 清理 | `route git clean` | ✓ | 无 |
| 归档 | `route git archive` | ✓ | 无 |
| 配置 | `route git config get/set` | ✓ | 无 |
| 远程管理 | `route git remote add/list/remove` | ✓ | **Normal 模式受限** |
| 拉取/推送 | `route git fetch/pull/push` | ✓ | **Normal 模式受限** |
| 克隆 | `route git clone` | ✓ | **Normal 模式受限** |
| 安全备份 | `route git backup` | ✓ | 无 |

### 权责审计官（Responsibility Auditor）

Route 的核心差异化功能，在每次 commit 操作中强制注入设备指纹与环境元数据，确保代码资产可精准溯源。

| 功能 | 说明 |
|------|------|
| **项目盐值管理** | 每个项目独立盐值，结合设备标识生成唯一设备指纹（SHA-256），防止哈希泄露后反推原始设备码 |
| **Pre-Commit Hook** | 拦截提交请求，自动采集设备指纹，检测 AI 参与度，执行环境一致性校验 |
| **元数据注入** | 每次 commit 自动附加 `[AUDIT: <device_hash>]` 标签和 Git Note，不可伪造 |
| **AI 参与度归因** | 自动识别 AI 辅助代码，记录 Prompt 哈希（`[AI: <prompt_hash>]`），实现二级权责细分 |
| **环境一致性校验** | 检测多设备交叉提交，异常行为触发 [WARNING] 级别警报 |
| **设备白名单** | 仅允许授权设备提交，非白名单设备自动拦截 |
| **责任索引库** | 本地持久化 `{Commit_ID, Device_Hash, AI_Prompt_Hash, Timestamp, Author}` 记录 |
| **资产溯源图谱** | 可视化展示设备与代码的映射关系、AI 参与度比例、风险等级 |

### 智能 Commit 管理（Commit 省略模式）

| 功能 | 说明 |
|------|------|
| **开关控制** | 桌面 GUI 设置页一键启用/禁用 |
| **自动提交** | 文件变更时自动生成规范 Commit 信息（如 `auto: 3 files changed — src/main.rs, …`） |
| **变更追溯** | 所有自动提交携带 `[ROUTE-AUTO]` 标记，独立日志存储在 `.route/auto-commit-log.json` |
| **速率限制** | 可配置每小时最大自动提交数，防止过量 |
| **审计集成** | 自动提交同样触发权责审计流程 |

### 主动跟踪模式（云端自动备份）

| 功能 | 说明 |
|------|------|
| **文件夹跟踪** | 指定本地文件夹与 GitHub 远程仓库绑定 |
| **自动同步** | 定期 fetch + pull（rebase + autostash），保持本地与远程一致 |
| **冲突检测** | 本地有修改且远程有更新时，明确提示冲突，需手动解决 |
| **同步历史** | 记录每次同步的时间、状态、变更数量 |
| **可配置频率** | 自定义同步间隔（默认 10 分钟） |

### AI 协同模式（BETA）

- AI 抢占 / 释放控制权
- AI 操作冲突检测与解决
- AI 索引文件（供 AI 读取项目结构）
- CLI / MCP 接口供外部 AI 代理调用

### 权限系统

Route 的 CLI 和 MCP 接口设有**权限等级**，防止 AI 代理意外执行远程操作：

| 等级 | 说明 | 本地操作 | 远程操作 |
|------|------|----------|----------|
| **Normal**（默认） | 本地 Git 操作全部允许，远程操作（push/pull/fetch/remote/clone）被阻止 | ✓ | ✗ |
| **High** | 所有操作完全放开 | ✓ | ✓ |

- 桌面 GUI **始终以 High 权限运行**（人类直接操作，无需限制）
- 在设置页可切换 CLI/MCP 的权限等级
- 三方 LLM 接入时，建议在 Normal 模式下测试，确认无误后再开启 High 模式

> **注意**：CLI 和 MCP 的权限等级独立控制。桌面 GUI 始终以 High 权限运行。
> 设置页面提供统一的 CLI/MCP 权限开关，也可以通过 `route permission set` 命令单独设置。

### 三方 LLM 接入

Route 提供 LLM 注入系统，将项目上下文自动注入到 LLM 调用中：

1. **预设提示词注入** — 系统级指令，定义 LLM 行为方式（可自定义）
2. **预留注入位** — 用户自定义文本，每次 LLM 调用时自动附加
3. **文件内容注入** — 按 glob 模式匹配文件，将文件内容注入到提示词中

> ⚠️ **Token 消耗警告**：过长预设文件或大量文件注入可能导致显著 token 消耗。Route 会在注入内容超过阈值（默认 50k 字符 ≈ 12.5k tokens）时显示警告。建议将 `max_files` 和 `max_chars` 控制在合理范围。

配置存储在项目目录下的 `.route/llm-injection.json`，可通过桌面 GUI 设置页或 `llm_injection_*` 命令管理。

### 备份

- **本地备份** — 增量镜像到本机另一目录
- **云端备份** — S3 / WebDAV / SSH 服务器 URI
- 默认增量更新，可手动全量快照

### 系统集成

- 开机自启动、静默启动、启动优先级调节
- 深色 / 浅色主题，实时切换
- 中 / 英双语，实时切换

---

## 快速开始

### 桌面应用（推荐）

```bash
# 安装依赖
cd crates/route-tauri/web
npm install

# 开发模式（启动 Vite + Tauri 桌面窗口）
cd ..
cargo tauri dev

# 生产构建
cargo tauri build
```

### CLI

```bash
# 在项目目录初始化
route init

# 记录一次变更
route commit -m "添加 Hero 区域"

# 查看历史
route log

# 回退到某个快照
route rollback abc12345

# 创建分支
route branch create feature-x
```

### 新增 CLI 命令

| 命令 | 说明 |
|------|------|
| `route ai chat <message>` | 发送消息到 AI 提供商（通过环境变量 ROUTE_AI_KEY 配置） |
| `route ai config` | 查看 AI 配置（脱敏显示 API Key） |
| `route project-context` | 显示项目上下文（git 分支、最近提交、skills/references 文件） |
| `route mcp --config` | 显示 MCP 配置片段（JSON 格式） |
| `route permission status` | 查看当前权限级别 |
| `route permission set high\|normal` | 设置权限级别 |

### MCP 集成

Route 提供 MCP 服务器（`route-mcp` crate），任何 MCP 兼容的 AI 客户端均可通过版本管理工具驱动 Route。

详见 [docs/ai-api.md](./docs/ai-api.md)。

### 新增 MCP 工具

| 工具 | 说明 |
|------|------|
| `route_tracking_list` | 列出所有跟踪目标 |
| `route_tracking_add` | 添加跟踪目标（folder + remote + branch + interval） |
| `route_tracking_remove` | 移除跟踪目标 |
| `route_tracking_sync` | 手动同步指定跟踪目标 |
| `route_tracking_history` | 查看同步历史 |
| `route_extension_skills` | 查看项目 skills 文件 |
| `route_extension_references` | 查看项目 references 文件 |
| `route_project_context` | 获取完整项目上下文（结构、分支、提交、引用） |
| `route_ai_chat` | 发送消息到 AI 提供商 |
| `route_permission_status` | 查看当前权限级别 |
| `route_permission_set` | 设置权限级别 |

---

## 项目结构

```
route/
├── crates/
│   ├── route-core/       # 核心引擎：快照、回退、分支、内容寻址存储
│   ├── route-basic/      # 基础仓库实现（BasicRepository）
│   ├── route-sync/       # 同步与备份（local / S3 / WebDAV / SSH）
│   ├── route-stats/      # 统计与时间线聚合
│   ├── route-plugins/    # 插件系统
│   ├── route-cli/        # 命令行工具
│   ├── route-mcp/        # MCP 服务器（Model Context Protocol）
│   └── route-tauri/      # Tauri + React 桌面 GUI
│       ├── src/          # Rust 后端
│       │   ├── audit.rs         # 权责审计官（盐值/设备指纹/Pre-Commit Hook）
│       │   ├── auto_commit.rs   # 智能 Commit 管理
│       │   ├── tracking.rs      # 主动跟踪模式
│       │   ├── permissions.rs   # 权限系统
│       │   ├── llm_integration.rs # LLM 注入系统
│       │   ├── ai_commands.rs   # AI 对话命令
│       │   ├── git_commands.rs  # 完整 Git 操作（25+ 命令）
│       │   ├── sync_commands.rs # 同步管理命令
│       │   ├── plugin_commands.rs # 插件管理命令
│       │   ├── project_context.rs # 项目上下文命令
│       │   ├── extensions.rs    # 扩展系统（skills/references）
│       │   ├── mcp_commands.rs  # MCP 管理命令
│       │   ├── stats_commands.rs # 统计命令
│       │   └── process_commands.rs # 进程管理命令
│       └── web/          # React 前端（TypeScript + Vite）
├── docs/                 # 架构、AI API、适配器文档
├── scripts/              # 构建/发布脚本
├── packages/             # TypeScript SDK 包（core / cli / desktop-bridge / desktop）
├── Cargo.toml            # Workspace 根配置
└── package.json          # 前端依赖与脚本
```

---

## 存储格式

Route 在项目目录下创建 `.route/`（类似 `.git/`）：

```
.route/
├── config.json              # 仓库配置
├── salt                     # 项目盐值（256-bit 随机数）
├── device-whitelist.json    # 设备白名单
├── responsibility-index.json # 权责索引库（Commit → 设备 → AI 归因）
├── auto-commit.json         # 智能提交配置
├── auto-commit-log.json     # 自动提交日志
├── tracking.json            # 主动跟踪配置
├── tracking-history.json    # 同步历史记录
├── llm-injection.json       # LLM 注入配置
├── HEAD                     # 当前分支
├── refs/branches/           # 分支指针
├── objects/
│   ├── manifests/           # 文件清单（增量/全量）
│   └── blobs/               # 内容寻址文件块（SHA-256 去重）
└── index.json               # AI 索引（项目结构、追踪状态）
```

**混合备份策略**：默认增量更新（仅存储变更文件），可手动触发全量快照。

---

## 开发

### 环境要求

- Rust 1.75+（`cargo`）
- Node.js 18+（`npm`）
- Tauri CLI 2.x（`cargo install tauri-cli --version "^2.0"`）

### 构建

```bash
# 前端
cd crates/route-tauri/web
npm install
npm run build

# 桌面应用
cd ..
cargo tauri build

# CLI
cd crates/route-cli
cargo build --release
```

### 开发提示

- Vite dev server 默认端口 1420，被占用时自动寻找空闲端口
- `windows_subsystem = "windows"` 已设置，不显示控制台黑框
- 深色模式为默认主题
- 所有 UI 文案支持中英双语

---

## AI 集成

AI 可通过三种方式驱动 Route：

1. **MCP** — `route-mcp` crate，stdio 传输
2. **CLI** — `route-cli` crate，命令行调用
3. **HTTP** — 桌面应用内置 HTTP 端点

三种方式委托同一后端原语，详见 [docs/ai-api.md](./docs/ai-api.md)。

### 环境变量配置

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `ROUTE_AI_KEY` | API Key（必需） | — |
| `ROUTE_AI_ENDPOINT` | API 端点 | `https://api.openai.com/v1` |
| `ROUTE_AI_MODEL` | 模型名称 | `gpt-4o` |
| `ROUTE_AI_PROVIDER` | 提供商 | `openai` |

AI 功能通过环境变量配置，未设置 `ROUTE_AI_KEY` 时 AI 功能禁用。

---

## 开源引用声明

Route 使用了以下开源组件：

| 组件 | 许可证 | 用途 |
|------|--------|------|
| [Tauri](https://tauri.app/) | Apache 2.0 / MIT | 桌面应用框架 |
| [React](https://react.dev/) | MIT | 前端 UI 框架 |
| [Rusqlite](https://github.com/rusqlite/rusqlite) | MIT | SQLite 数据库绑定 |
| [Serde](https://serde.rs/) | Apache 2.0 / MIT | 序列化/反序列化 |
| [Reqwest](https://github.com/seanmonstar/reqwest) | Apache 2.0 / MIT | HTTP 客户端 |
| [sha2](https://github.com/RustCrypto/hashes) | Apache 2.0 / MIT | SHA-256 哈希 |
| [hex](https://github.com/KokaKiwi/rust-hex) | MIT | 十六进制编解码 |
| [chrono](https://github.com/chronotope/chrono) | Apache 2.0 / MIT | 日期/时间处理 |
| [getrandom](https://github.com/rust-random/getrandom) | Apache 2.0 / MIT | 安全随机数生成 |
| [Vite](https://vitejs.dev/) | MIT | 前端构建工具 |
| [TypeScript](https://www.typescriptlang.org/) | Apache 2.0 | 类型化 JavaScript |

---

## License

Route 使用 Apache 2.0 许可证 — 见 [LICENSE](./LICENSE)。第三方组件见 [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)。