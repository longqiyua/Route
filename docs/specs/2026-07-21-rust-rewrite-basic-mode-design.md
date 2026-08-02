# Route 重写设计 — Rust 化 + 基础模式 + 扩展生态

**日期**：2026-07-21
**状态**：Draft（待 PM 审阅）
**作者**：AI 设计 + PM 决策

## 背景

Route 原型（TS monorepo）实现了 Mode 1（对话轮次版本管理）。PM 决定：

1. 新增 **基础模式**（图+树形 mindmap，边中心 commit 模型）
2. 新增 **扩展功能**（目录同步 sync）
3. **全 Rust 重写**核心引擎
4. 新增 Emacs 格式、WebDAV/S3 导入导出、实时统计
5. 预留 **插件接口**（事件钩子 / 命令 / UI / 存储·协议·导出 / MCP / AI CLI）

本设计文档覆盖整体架构与各模块详细设计。

---

## 1. 整体架构

### 1.1 三层架构

```
┌──────────────────────────────────────────────────────────┐
│  客户层                                                   │
│  CLI(route-rs) │ Tauri GUI │ MCP Server │ Editor Plugins │
│  Emacs插件(首版)│ Vim插件   │ AHK脚本    │ Web UI        │
├──────────────────────────────────────────────────────────┤
│  Bindings 层                                             │
│  napi-rs(Node) │ Tauri commands │ FFI │ HTTP(JSON-RPC)  │
├──────────────────────────────────────────────────────────┤
│  插件运行时 (route-plugin-host)                          │
│  事件总线 │ 命令注册 │ UI面板 │ 存储/协议/导出扩展点      │
├──────────────┬──────────────┬───────────────────────────┤
│  Mode 1      │  Basic 模式  │  扩展功能                  │
│  conversation│  graph-tree  │  sync（目录同步）          │
│  (对话轮次)  │  (图+树形)   │  stats（统计）首内置插件   │
├──────────────┴──────────────┴───────────────────────────┤
│  共享基础设施 (route-core, Rust)                         │
│  - storage: blob 内容寻址 + SQLite(rusqlite)            │
│  - transport: 本地/WebDAV/S3/P2P/自配服务器/公网         │
│  - export: JSON/Markdown/ZIP/Folder/Mermaid/Emacs org    │
│  - stats: 实时统计聚合器                                 │
└──────────────────────────────────────────────────────────┘
```

### 1.2 Rust workspace 结构

```
route/
├── Cargo.toml                    # workspace 根
├── crates/
│   ├── route-core/               # 共享基础设施
│   │   ├── storage/              #   blob + SQLite
│   │   ├── transport/            #   本地/WebDAV/S3/P2P/HTTP
│   │   ├── export/               #   所有导出格式
│   │   ├── stats/                #   统计聚合
│   │   └── plugin-host/          #   插件运行时
│   ├── route-conversation/       # Mode 1（从 TS 重写，Phase 7）
│   ├── route-basic/              # 基础模式（图+树形）
│   ├── route-sync/               # 目录同步扩展
│   ├── route-stats/              # 统计插件实现
│   ├── route-cli/                # CLI（用 clap）
│   ├── route-napi/               # napi-rs binding
│   └── route-tauri/              # Tauri 桌面（含 React UI）
├── legacy/                       # 旧 TS 代码（保留为参考，逐步废弃）
├── editors/
│   ├── emacs/                    # Emacs 插件（Emacs Lisp）
│   ├── vim/                      # Vim 插件（VimScript/Lua）
│   └── vscode/                   # VSCode 扩展（三期）
└── docs/
```

### 1.3 多端落地策略

| 端 | 核心 | 文件 I/O | 数据库 | 网络 | UI |
|----|------|---------|--------|------|-----|
| CLI | route-core | std::fs | rusqlite | reqwest | 终端 |
| 桌面(Tauri) | route-core | Tauri FS | rusqlite(原生) | reqwest | React |
| 服务器 | route-core | std::fs | rusqlite | reqwest | HTTP API |
| 浏览器 | WASM build | OPFS | sql.js | fetch/WebRTC | Web UI |
| Emacs | route-napi → CLI | Emacs 进程 | CLI 管理 | CLI 管理 | Emacs UI |

### 1.4 模式选择

`route init` 时选 `conversation` / `basic` / `hybrid`：
- `conversation` — 现有 Mode 1（对话轮次）
- `basic` — 新基础模式（图+树形）
- `hybrid` — 同项目共存两套元数据（`.route/` + `.route-basic/`）

已初始化的项目不可改模式。

### 1.5 存储目录

```
.route-basic/
├── db.sqlite              # SQLite 数据库（元数据 + 索引）
├── objects/               # blob 内容寻址
│   └── <hash前2>/<hash>
└── config.json            # 模式标识、版本号
```

---

## 2. Basic 模式数据模型

### 2.1 核心概念

| 概念 | 说明 |
|------|------|
| **Snapshot（节点）** | 某一时刻的完整文件状态（一组 manifest hash）。无元信息，纯状态。 |
| **Commit（边）** | 从一个 snapshot 到另一个 snapshot 的有向边，承载文本/时间戳/作者等元信息。**核心创新**：元信息在边上而非节点上。 |
| **Branch（子图标签）** | 一组 snapshot 的逻辑分组（main / inherited / sandbox）。 |
| **Blob** | 内容寻址的文件块（SHA-256）。 |

### 2.2 图的形态

- **底层是 DAG**（有向无环图）：snapshot 是节点，commit 是边
- **默认 UI 渲染为树形 mindmap**：选定 HEAD 为根，沿父边回溯
- **N-N 支持**：一个 snapshot 可有多入边（多 commit 汇聚）和多出边（同状态衍生多个后续）
- **折叠策略**：树形模式下重复节点折叠为"⤴ 已引用 #abc123"，按 `*` 键展开为完整 DAG

### 2.3 SQLite Schema

```sql
-- 快照节点（纯状态，无元信息）
CREATE TABLE snapshots (
  id            TEXT PRIMARY KEY,        -- ULID
  manifest_hash TEXT NOT NULL,           -- 指向 manifest 表
  created_at    INTEGER NOT NULL,
  FOREIGN KEY (manifest_hash) REFERENCES manifests(hash)
);

-- 文件清单
CREATE TABLE manifests (
  hash       TEXT PRIMARY KEY,
  content    TEXT NOT NULL               -- JSON: {"/path/file": "<blob-hash>", ...}
);

-- Commit 边（核心创新：元信息在边上）
CREATE TABLE commits (
  id            TEXT PRIMARY KEY,        -- ULID
  from_snapshot TEXT NOT NULL,
  to_snapshot   TEXT NOT NULL,
  message       TEXT NOT NULL,           -- commit 文本
  author        TEXT,
  created_at    INTEGER NOT NULL,
  branch_id     TEXT NOT NULL,
  kind          TEXT NOT NULL CHECK(kind IN (
    'incremental', 'full', 'merge', 'rollback'
  )),
  diff_summary  TEXT,                    -- JSON: {added:[], modified:[], removed:[]}
  FOREIGN KEY (from_snapshot) REFERENCES snapshots(id),
  FOREIGN KEY (to_snapshot) REFERENCES snapshots(id),
  FOREIGN KEY (branch_id) REFERENCES branches(id)
);

CREATE INDEX idx_commits_from ON commits(from_snapshot);
CREATE INDEX idx_commits_to   ON commits(to_snapshot);
CREATE INDEX idx_commits_branch ON commits(branch_id);

-- 路径注释（你的硬创新点：边上的附带信息，N-N）
CREATE TABLE commit_path_annotations (
  id          TEXT PRIMARY KEY,
  commit_id   TEXT NOT NULL,
  text        TEXT NOT NULL,
  created_at  INTEGER NOT NULL,
  FOREIGN KEY (commit_id) REFERENCES commits(id) ON DELETE CASCADE
);

-- 分支
CREATE TABLE branches (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL UNIQUE,
  kind          TEXT NOT NULL CHECK(kind IN ('main','inherited','sandbox')),
  parent_branch TEXT,                    -- inherited 的父分支；sandbox 可指向来源
  baseline_snapshot TEXT,                -- inherited 的分叉点
  head_snapshot TEXT,                    -- 当前 HEAD
  created_at    INTEGER NOT NULL,
  FOREIGN KEY (parent_branch) REFERENCES branches(id),
  FOREIGN KEY (baseline_snapshot) REFERENCES snapshots(id),
  FOREIGN KEY (head_snapshot) REFERENCES snapshots(id)
);

-- Blob 索引
CREATE TABLE blobs (
  hash    TEXT PRIMARY KEY,
  size    INTEGER NOT NULL,
  created INTEGER NOT NULL
);

-- 统计快照（route-stats 维护）
CREATE TABLE stats_snapshot (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);
```

### 2.4 继承分支 vs 独立分支

**继承分支（inherited）**：
- `parent_branch` 指向父分支，`baseline_snapshot` = 创建时的父 HEAD
- 新 commit 只记录从 baseline 到新 snapshot 的 diff 边
- 文件状态解析：`resolve(parent_baseline_manifest) ⊕ apply(inherited_diffs)`
- 联动：父分支后续推进时，继承分支可选 rebase 到父新 HEAD

**独立分支 / 沙盒（sandbox）**：
- 创建时深拷贝父分支当前 HEAD 的完整 manifest 和 blobs
- `parent_branch` 仅作血缘记录，不参与解析
- 后续 commit 完全独立，可随便改不影响父分支
- 删除沙盒只删自己的 snapshot 和 commit，blob 通过 GC 清理

### 2.5 文件状态解析算法

```rust
fn resolve_snapshot(snapshot_id: &str) -> HashMap<PathBuf, BlobHash> {
    let snapshot = snapshots.get(snapshot_id);
    let mut manifest = manifests.get(snapshot.manifest_hash);

    let branch = branches.get(snapshot.branch_id);
    if branch.kind == Inherited && snapshot_id == branch.baseline_snapshot {
        let parent_manifest = resolve_snapshot(branch.parent.head_snapshot);
        manifest = merge(parent_manifest, manifest);
    }
    manifest
}
```

### 2.6 核心操作

**增量备份 commit**：
```rust
fn commit_incremental(branch_id, message) -> Commit {
    let head = branches.get(branch_id).head_snapshot;
    let current_files = scan_project_files();
    let prev_files = resolve_snapshot(head);
    let diff = diff_manifests(prev_files, current_files);
    let new_manifest = build_manifest(current_files);
    let new_snapshot = snapshots.create(new_manifest);
    store_changed_blobs(diff.added + diff.modified);
    let commit = commits.create(from: head, to: new_snapshot, message, kind: Incremental, diff_summary: diff);
    branches.update_head(branch_id, new_snapshot);
    events.emit(CommitAfter { commit });
    commit
}
```

**手动全量备份到指定文件夹**：
```rust
fn full_backup_to_dir(branch_id, target_dir) -> PathBuf {
    let head = branches.get(branch_id).head_snapshot;
    let files = resolve_snapshot(head);
    for (path, blob_hash) in files {
        fs::copy(blob_path(blob_hash), target_dir.join(path));
    }
    // 记录 kind=full 的自环边
    commits.create(from: head, to: head, message: "Full backup to {target_dir}", kind: Full);
    target_dir
}
```

**回退**（新增回退边，不删历史）：
```rust
fn rollback(branch_id, to_snapshot_id) {
    let current_head = branches.get(branch_id).head_snapshot;
    let files = resolve_snapshot(to_snapshot_id);
    apply_files_to_project(files);
    commits.create(from: current_head, to: to_snapshot_id, message: "Rollback", kind: Rollback);
    branches.update_head(branch_id, to_snapshot_id);
}
```

---

## 3. 树形 mindmap UI

### 3.1 视图模式

| 模式 | 用途 | 渲染 |
|------|------|------|
| **树形（默认）** | 日常操作 | mindmap 从 HEAD 向父辈回溯，重复节点折叠 |
| **DAG 图** | 看分支合并/回退关系 | graph TD，显示交叉连线 |
| **文件树** | 看某 snapshot 内容 | 标准文件浏览器，挂节点详情面板 |

### 3.2 视觉

- **节点**：圆角矩形，标题=`短ID+时间`，按分支着色（main灰/inherited蓝/sandbox橙），HEAD 加粗描边
- **边**：实线=incremental/full，虚线=rollback，点划线=merge
- **边标签**：commit message 前 40 字，悬停显示完整 message + diff_summary
- **路径注释**：边下方灰色小字，点击边可编辑（N-N）

### 3.3 N-N 折叠策略

- 多入边 snapshot：树形模式只在首次出现处展开，其余位置 `⤴ 见 #abc123`
- 多出边 snapshot：并列展开所有子节点，浅色虚线框分组
- `*` 键切换展开/折叠

### 3.4 主要操作

| 操作 | 键 | 行为 |
|------|----|------|
| 增量备份 | `c` | 扫描项目 → 新 snapshot + commit 边 |
| 全量备份到文件夹 | `F` | 弹文件夹选择器 → 复制 → 记 full 自环边 |
| 回退到此 | `r` | 移动 HEAD，生成 rollback 边 |
| 新建继承分支 | `b i` | 以当前节点为 baseline |
| 新建沙盒分支 | `b s` | 深拷贝当前节点 |
| 编辑边注释 | `a` | 弹窗编辑路径注释 |
| 切换视图 | `t`/`g`/`f` | 树形/DAG/文件树 |
| 导出 | `e` | 弹导出菜单 |

### 3.5 Emacs 视图

- mindmap 在 Emacs 用 `org-mode` 大纲渲染（headline 层级 = 项目/分支/版本/文件）
- `TAB` 折叠，`RET` 进入节点，`m` 加注释
- 数据与 GUI 一致，文件一致

---

## 4. 扩展功能（sync）

### 4.1 三种模式

| 模式 | 语义 | 冲突处理 |
|------|------|---------|
| **mirror（镜像）** | 源完全覆盖目标 | 无视冲突 |
| **backup（备份）** | 只增不删 | 默认保留目标+新增源；可选"自动保留两者"（重命名 `_2`） |
| **archive（归存）** | 严格版本管理 | 每次同步在目标侧用 route-basic 记 commit 边，冲突调用 basic 合并策略 |

### 4.2 触发方式

- 手动：`route sync run <job-id>` 或 GUI 按钮
- 定时：cron 表达式 `route sync schedule "<expr>" <job-id>`
- 监听：`notify` crate 监听源目录，变化即触发
- 自定义：插件暴露 `SyncTrigger` 接口

### 4.3 传输通道

```rust
trait Transport {
    fn push(&self, src: PathBuf, dst: TransportPath) -> Result<()>;
    fn pull(&self, src: TransportPath, dst: PathBuf) -> Result<()>;
    fn list(&self, dir: TransportPath) -> Result<Vec<Entry>>;
}

enum TransportKind {
    Local, WebDAV, S3, P2P, CustomServer, PublicRelay, Plugin(String),
}
```

默认 `PublicRelay`（公网中继，二期），本地对本地用 `Local` 不需网络。

### 4.4 任务模型

```sql
CREATE TABLE sync_jobs (
  id           TEXT PRIMARY KEY,
  name         TEXT NOT NULL,
  src_path     TEXT NOT NULL,
  dst_path     TEXT NOT NULL,
  mode         TEXT NOT NULL CHECK(mode IN ('mirror','backup','archive')),
  transport    TEXT NOT NULL,
  schedule     TEXT,                    -- cron，NULL=手动
  watch        INTEGER NOT NULL DEFAULT 0,
  options      TEXT,                    -- JSON
  last_run     INTEGER,
  last_status  TEXT,
  created_at   INTEGER NOT NULL
);

CREATE TABLE sync_history (
  id           TEXT PRIMARY KEY,
  job_id       TEXT NOT NULL,
  started_at   INTEGER NOT NULL,
  finished_at  INTEGER,
  status       TEXT NOT NULL,
  files_copied INTEGER,
  bytes_copied INTEGER,
  error        TEXT,
  FOREIGN KEY (job_id) REFERENCES sync_jobs(id)
);
```

---

## 5. 插件接口

### 5.1 核心接口

```rust
trait RoutePlugin {
    fn manifest(&self) -> &PluginManifest;
    fn activate(&mut self, host: &mut PluginHost) -> Result<()>;
    fn deactivate(&mut self) -> Result<()>;
}

struct PluginManifest {
    id: String,
    name: String,
    version: String,
    api_version: u32,
    capabilities: Vec<Capability>,
}

struct PluginHost {
    events: EventBus,
    commands: CommandRegistry,
    ui: UIPanelRegistry,
    storage: StorageExtPoint,
    exporters: ExportRegistry,
    transports: TransportRegistry,
    stats: StatsRegistry,
    mcp: McpBridge,
}
```

### 5.2 事件总线主题

- `commit:before` / `commit:after`
- `rollback:before` / `rollback:after`
- `branch:create` / `branch:delete` / `branch:switch`
- `sync:job:start` / `sync:job:finish`
- `export:before` / `export:after`
- `plugin:loaded` / `plugin:unloaded`

### 5.3 插件加载来源

- 本地路径：`route plugin install /path/to/plugin.wasm`
- 注册表：`route plugin install @route/stats`
- 内置：`route-stats`、`route-sync` 编译进二进制

### 5.4 沙盒选型

`wasmtime`，host 调用 plugin 的 `activate`，plugin 通过 host import 函数操作 Route。插件语言无关（Rust/Go/C/AssemblyScript 均可）。

### 5.5 兼容性格式（分期）

1. **原生 Route 插件**（WASM，首版）
2. **MCP Server 适配**（任何 MCP server 自动成为 Route 插件，二期）
3. **VSCode 扩展适配**（三期）
4. **Shell 脚本插件**（stdin/stdout 协议，AutoHotkey/Vim 走这条，三期）

---

## 6. 导出格式

### 6.1 统一接口

```rust
trait Exporter {
    fn id(&self) -> &str;
    fn formats(&self) -> &[ExportFormat];
    fn export(&self, ctx: &ExportContext, fmt: ExportFormat, out: &mut dyn Write) -> Result<()>;
}

enum ExportFormat { Json, Markdown, Zip, Folder, Mermaid, EmacsOrg }
```

### 6.2 JSON

完整元数据（分支树、snapshot、commit 边、文件 hash），不含文件内容。

### 6.3 Markdown

```markdown
# Route 报告 — <project>

## 分支
- main (HEAD: abc123)
  - inherited: feature-x
  - sandbox: experiment-y

## 历史
| Snapshot | Commit | Branch | 时间 | Diff |
|---|---|---|---|---|
| abc123 | "添加 Hero" | main | 2026-07-21 10:00 | +5 -2 |
```

### 6.4 ZIP / Folder

完整可迁移包：`db.sqlite` + `objects/` + `config.json`。Folder 为 ZIP 的解压版。

### 6.5 Mermaid mindmap（核心创新展示）

```mermaid
mindmap
  root((项目<br/>route-demo))
    main
      abc123
        ::commit 添加 Hero 区域
          def456
            ::commit 修复移动端样式
              ghi789
                ::commit【全量备份】
      jkl012
        ::commit 初始化
    feature-x[inherited]
      ::baseline abc123
      mno345
        ::commit 实验 A
    experiment-y[sandbox]
      ::copied abc123
      pqr678
        ::commit 沙盒改动 1
```

- 每条 commit 是路径上的 `::commit <message>` 注释（非节点本身）
- 节点 = snapshot 短 ID
- 继承分支标注 `::baseline`，沙盒分支标注 `::copied`
- 多入边 snapshot 重复显示并附 `::alias of <id>`（mindmap 不支持交叉连线）

### 6.6 Emacs org-mode

```org
* Route 项目 — route-demo
:PROPERTIES:
:ROUTE_MODE: basic
:ROUTE_CREATED: 2026-07-21
:END:

** main 分支
:PROPERTIES:
:ROUTE_BRANCH_KIND: main
:ROUTE_HEAD: abc123
:END:

*** Snapshot abc123
:PROPERTIES:
:ROUTE_CREATED_AT: [2026-07-21 Mon 10:00]
:END:

**** Commit 添加 Hero 区域
:PROPERTIES:
:ROUTE_KIND: incremental
:ROUTE_FROM: jkl012
:ROUTE_TO:   abc123
:ROUTE_DIFF: +5 -2
:END:
- 路径注释: 这是 Hero 区域的起点

***** 文件
- index.html :: <blob:9a3f...>
- styles.css :: <blob:7b21...>
```

Emacs 用户在 org-mode 里 `TAB` 折叠、`C-c C-c` 触发 Route 操作。

---

## 7. 统计 + WebDAV/S3

### 7.1 route-stats（首个内置插件）

订阅事件，实时维护 `stats_snapshot` 表：

```json
{
  "branches": {"total": 3, "main": 1, "inherited": 1, "sandbox": 1},
  "snapshots": 24,
  "commits": {"total": 23, "incremental": 18, "full": 2, "rollback": 2, "merge": 1},
  "blobs": {"count": 412, "total_size": 134217728, "dedup_ratio": 0.34},
  "files": {"tracked": 87, "avg_per_snapshot": 76},
  "branches_depth": {"main": 8, "feature-x": 3, "experiment-y": 5},
  "last_activity": {"main": 1721548800, "feature-x": 1721545200},
  "sync": {"jobs": 2, "last_run": "2026-07-21T09:30:00Z", "total_bytes_synced": 52428800}
}
```

UI 展示：桌面端侧栏底部统计面板（实时刷新）/ `route stats` CLI / Emacs `M-x route-stats`。

### 7.2 WebDAV/S3 远端

**远端布局**：
```
<base>/<repo-id>/
├── db.sqlite
├── config.json
└── objects/<hash前2>/<hash>
```

**导出**：
```bash
route export --to webdav://user:pass@host/path/<repo-id>
route export --to s3://bucket/route/<repo-id>
```

**导入**：
```bash
route import --from webdav://user:pass@host/path/<repo-id>
route import --from s3://bucket/route/<repo-id>
```

**实现**：
- `WebDAVTransport`：reqwest + 基本/digest auth
- `S3Transport`：aws-sdk-s3 + STS 临时凭证
- 增量推送：只发新 blob 和 db.sqlite 差异（ETag）

---

## 8. 依赖选型

| 用途 | crate |
|------|-------|
| CLI | `clap` |
| SQLite | `rusqlite`（bundled） |
| 内容寻址 | `sha2` + `hex` |
| 文件扫描 | `ignore` |
| 文件监听 | `notify` |
| HTTP | `reqwest` |
| S3 | `aws-sdk-s3` |
| WebDAV | 基于 reqwest 自实现 |
| 序列化 | `serde` + `serde_json` |
| 时间 | `chrono` |
| ID | `ulid` |
| 错误 | `anyhow` + `thiserror` |
| 日志 | `tracing` |
| WASM 插件 | `wasmtime` |
| Node binding | `napi-rs` |
| 桌面 | `tauri` + React |
| 测试 | `rstest` + `tempfile` |

---

## 9. 实施顺序

1. **Phase 1（基础模式 MVP）**：route-core（storage + SQLite）+ route-basic + route-cli + 测试
2. **Phase 2（GUI）**：route-tauri + 树形 mindmap + 文件树详情
3. **Phase 3（扩展）**：route-sync + route-stats + 导出器全格式
4. **Phase 4（远端）**：WebDAV/S3 transport + 导入导出
5. **Phase 5（插件）**：WASM 插件运行时 + 事件总线
6. **Phase 6（生态）**：Emacs/Vim 插件 + MCP server + VSCode 扩展
7. **Phase 7（Mode 1 重写）**：route-conversation 从 TS 重写进 Rust workspace

---

## 10. 设计决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 模式共存 | 并列可选 | 互不干扰，用户按需选 |
| 树形结构 | 混合多层（项目→分支→版本→文件） | 完整层级 |
| 继承分支语义 | 基于父 baseline + diff 边，可联动 | 保留血缘、节省存储 |
| 独立分支语义 | 沙盒深拷贝，完全隔离 | 用户可"随便搞不影响" |
| 数据库 | SQLite + blob 文件 | 查询性能好，大文件不入库 |
| 导出格式 | JSON/Markdown/ZIP/Folder/Mermaid/Emacs | 覆盖人读+机读+迁移 |
| Mermaid 图类型 | mindmap | 贴合"思维导图"描述，路径文字是创新点 |
| 传输默认 | 公网中继（fallback Local） | 普通用户零配置 |
| 插件沙盒 | WASM (wasmtime) | 语言无关、安全隔离 |
| 重写语言 | Rust 全栈 | 性能、跨端、安全 |
| 旧 TS 处理 | 保留为 legacy/，不自动迁移 | 用户量小，手动重建 |

---

## 11. 未决事项（PM 后续决策）

1. **公网中继服务器**：是否自建官方中继？商业模式？
2. **WASM 插件 API 版本策略**：首版 v1，破坏性变更如何处理
3. **VSCode 扩展适配范围**：完整复刻 vscode.* API 还是子集
4. **Mode 1 是否长期保留**：还是 Phase 7 后弃用 conversation 模式
5. **P2P 协议选型**：libp2p 还是自研轻量协议

---

**等待 PM 审阅**。审阅通过后进入实施计划阶段。
