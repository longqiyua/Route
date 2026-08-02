# Route

**Route** — 面向 Web Coding / Vibe Coding 的轻量版本管理软件。

以「用户与 AI 的一轮对话」为最小版本单元，支持按对话回退、分支管理、混合备份策略，并提供桌面 GUI、CLI 与 MCP 三种接入方式。

> **版本 0.2.0 — BETA**
>
> 当前处于 Beta 阶段：核心功能（文件追踪 / 回退 / 分支 / 桌面 GUI / CLI / MCP）已可用，AI 协同模式与高级同步特性仍在打磨。欢迎使用并[反馈问题](https://github.com/longqiyua/route/issues)。

---

## 核心理念

- **一个指令干一个活儿** — 每条指令声明四个点：开始点、结束点、验收点、紧急回退点。
- **追踪而非替换** — Route 不替代 Git，而是在文件系统层面做增量快照，对任何文件类型都有效。
- **AI 优先** — CLI 与 MCP 接口让 AI 代理可以直接驱动版本管理，桌面 GUI 提供可视化操作。

---

## 功能

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

### AI 协同模式（BETA）

- AI 抢占 / 释放控制权
- AI 操作冲突检测与解决
- AI 索引文件（供 AI 读取项目结构）
- CLI / MCP 接口供外部 AI 代理调用

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

### MCP 集成

Route 提供 MCP 服务器（`route-mcp` crate），任何 MCP 兼容的 AI 客户端均可通过 15 个版本管理工具驱动 Route：

```
route_status, route_log, route_commit, route_rollback,
route_branch_*, route_annotate, route_export, route_diff,
route_undo_redo_status, route_files, route_checkpoint,
route_history, route_tag, route_set_remote
```

详见 [docs/ai-api.md](./docs/ai-api.md)。

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
│       ├── src/          # Rust 后端（IPC 命令、状态管理、文件追踪）
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
├── config.json          # 仓库配置
├── HEAD                 # 当前分支
├── refs/branches/       # 分支指针
├── objects/
│   ├── manifests/       # 文件清单（增量/全量）
│   └── blobs/           # 内容寻址文件块（SHA-256 去重）
└── index.json           # AI 索引（项目结构、追踪状态）
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

1. **MCP** — `route-mcp` crate，stdio 传输，15 个工具
2. **CLI** — `route-cli` crate，命令行调用
3. **HTTP** — 桌面应用内置 HTTP 端点

三种方式委托同一后端原语，详见 [docs/ai-api.md](./docs/ai-api.md)。

---

## License

Apache 2.0 — 见 [LICENSE](./LICENSE)。第三方组件见 [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)。