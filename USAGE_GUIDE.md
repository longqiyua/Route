# Route 使用指南

## 目录

- [桌面 GUI 快速上手](#桌面-gui-快速上手)
- [CLI 命令行](#cli-命令行)
- [MCP / AI 集成](#mcp--ai-集成)
- [Git 模式操作](#git-模式操作)
- [常见问题](#常见问题)

---

## 桌面 GUI 快速上手

### 启动

```bash
# 开发模式
cd crates/route-tauri
cargo tauri dev

# 或使用已构建的安装包
```

### 工作区布局

```
┌──────────────────────────────────────────────┐
│ 标题栏 (项目切换 / 主题切换 / 语言切换 / 窗口控制) │
├──────────┬───────────────────────────────────┤
│          │  工作区                             │
│ 侧边栏   │  ├─ 分支树 (branch tree)           │
│          │  ├─ 提交历史 (timeline)             │
│ 项目列表  │  ├─ 备份配置                       │
│          │  └─ 跟踪状态                       │
│          │                                   │
│ [工作区]  │  设置页                             │
│ [设置]   │  ├─ 通用设置 (语言/主题/开机自启)     │
│ [对话]   │  ├─ 外部接入 (CLI/MCP 开关)          │
│          │  ├─ AI 助手 (模式/API/模型)          │
│          │  ├─ 跟踪配置                        │
│          │  └─ 高级设置                        │
└──────────┴───────────────────────────────────┘
```

### 核心操作流程

1. **添加项目** → 点击"+"按钮选择文件夹
2. **配置备份** → 选择本地或云端备份目标
3. **开始跟踪** → 打开 Watch 开关，Route 自动追踪文件变更
4. **提交/标记** → 在 checkpoint 输入框填写标题和描述
5. **查看历史** → 分支树中展开当前分支查看提交时间线
6. **回退** → 点击时间线中的"回退"按钮

### 分支管理

- **创建分支** → 点击分支树顶部的"+"按钮，输入分支名称
- **切换分支** → 点击分支名称即可切换
- **合并分支** → 点击目标分支的"合并"按钮
- **复制到新分支** → Sandbox 类型分支可"复制到新分支"

### Git 模式

在设置页开启「Git 模式」后，所有操作转为真实 Git 命令：

- 提交 → `git add && git commit`
- 分支 → `git branch`
- 标签 → `git tag`
- 暂存 → `git stash`
- 恢复 → `git restore`

---

## CLI 命令行

### 标准模式命令

```bash
# 初始化仓库
route init

# 查看状态
route status

# 提交变更
route commit -m "添加 Hero 区域"

# 查看历史
route log

# 回退到快照
route rollback <snapshot-id>

# 创建分支
route branch create feature-x

# 切换分支
route branch switch feature-x

# 合并分支
route branch merge feature-x

# 撤销 (上一步)
route undo

# 重做 (恢复撤销)
route redo

# 创建检查点
route checkpoint -t "重构前快照" -b "准备重构数据层"

# 查看差异
route diff <from-snapshot> <to-snapshot>

# 查看工作区变更
route changes

# 导出数据
route export -f markdown -o output.md

# 查看统计
route stats

# 备份
route backup /path/to/backup/dir
```

### Git 模式命令

```bash
# 初始化 git 仓库
route git init

# 查看状态
route git status

# 暂存文件
route git add src/main.rs

# 取消暂存
route git reset src/main.rs

# 提交
route git commit -m "feat: add login page"

# 查看日志
route git log --limit 10
route git log --graph --all

# 创建分支
route git branch create feature-x

# 切换分支
route git branch switch feature-x

# 合并分支
route git merge feature-x

# 变基
route git rebase main

# 标签
route git tag create v1.0
route git tag list

# 暂存
route git stash push -m "wip"
route git stash pop

# 远程操作
route git remote add origin https://github.com/user/repo.git
route git fetch
route git pull
route git push

# 撤销提交 (安全撤销)
route git revert <commit-sha>

# Cherry-pick
route git cherry-pick <commit-sha>

# 查看差异
route git diff

# 清理未跟踪文件
route git clean --force

# 配置
route git config get user.name
route git config set user.email "user@example.com"
```

---

## MCP / AI 集成

### 启动 MCP 服务器

```bash
# 命令行启动
route-mcp --project /path/to/project

# 带权限等级
route-mcp --project /path/to/project --permission-level high
```

### 从桌面 GUI 启动

在设置页 → 外部接入 → 启动 MCP，点击开关即可启动。

### MCP 工具清单

所有工具通过 JSON-RPC 2.0 协议调用，AI 客户端可自动发现。

#### Route 操作工具

| 工具 | 描述 |
|------|------|
| `route_status` | 查看仓库状态（当前分支、HEAD 快照、模式） |
| `route_log` | 查看提交历史 |
| `route_commit` | 提交工作区变更 |
| `route_rollback` | 回退到指定快照 |
| `route_undo` | 撤销上一步提交 |
| `route_redo` | 恢复撤销的提交 |
| `route_checkpoint` | 创建命名检查点 |
| `route_branches` | 列出所有分支 |
| `route_branch_create` | 创建新分支 |
| `route_branch_switch` | 切换分支 |
| `route_merge` | 合并分支 |
| `route_diff` | 比较两个快照的差异 |
| `route_changes` | 查看工作区待提交变更 |
| `route_export` | 导出仓库数据 |
| `route_annotate` | 给提交添加注释 |
| `route_read_file` | 读取指定快照中的文件内容 |

#### Git 操作工具

| 工具 | 描述 | 权限要求 |
|------|------|----------|
| `route_git_init` | 初始化 Git 仓库 | 无 |
| `route_git_status` | 查看工作树状态 | 无 |
| `route_git_log` | 查看提交日志 | 无 |
| `route_git_log_graph` | 查看 ASCII 分支图 | 无 |
| `route_git_show` | 查看提交详情 | 无 |
| `route_git_add` | 暂存文件 | 无 |
| `route_git_reset` | 取消暂存 | 无 |
| `route_git_remote_add` | 添加远程仓库 | **Normal 受限** |
| `route_git_remote_list` | 列出远程仓库 | 无 |
| `route_git_remote_remove` | 删除远程仓库 | **Normal 受限** |
| `route_git_fetch` | 从远程拉取 | **Normal 受限** |
| `route_git_pull` | 拉取并合并 | **Normal 受限** |
| `route_git_push` | 推送到远程 | **Normal 受限** |
| `route_git_revert` | 撤销提交 | 无 |
| `route_git_cherry_pick` | Cherry-pick 提交 | 无 |
| `route_git_rebase` | 变基 | 无 |
| `route_git_stash_push` | 暂存变更 | 无 |
| `route_git_stash_pop` | 恢复暂存 | 无 |
| `route_git_stash_list` | 列出暂存 | 无 |
| `route_git_tag_create` | 创建标签 | 无 |
| `route_git_tag_list` | 列出标签 | 无 |
| `route_git_tag_delete` | 删除标签 | 无 |
| `route_git_config_get` | 读取配置 | 无 |
| `route_git_config_set` | 设置配置 | 无 |
| `route_git_clean` | 清理未跟踪文件 | 无 |

### 权限说明

- **Normal 模式**（默认）：本地操作全部允许，远程操作（push/pull/fetch/remote/clone）被阻止
- **High 模式**：所有操作完全放开

---

## Git 模式操作

### 启用 Git 模式

在设置页 → AI 助手 → Git 模式 → 开启

### GUI 中的 Git 操作

| 操作 | 位置 | 说明 |
|------|------|------|
| 查看状态 | 工作区顶部 | 显示未提交变更列表 |
| 提交 | 工作区 checkpoint 输入框 | 自动 `git add` + `git commit` |
| 切换分支 | 分支树 → 点击分支名 | `git switch` |
| 创建分支 | 分支树 → "+"按钮 | `git branch` |
| 合并分支 | 分支树 → "合并"按钮 | `git merge` |
| 标签管理 | 工作区标签区域 | `git tag create/list/delete` |
| 暂存管理 | 工作区暂存区域 | `git stash push/pop/list` |
| 恢复文件 | 工作区恢复按钮 | `git restore` (两步确认) |
| 回退提交 | 时间线 → "回退"按钮 | `git revert` |

### 标准模式 vs Git 模式

| 特性 | 标准模式 | Git 模式 |
|------|----------|----------|
| 存储引擎 | Route 原生快照 | Git 仓库 |
| 文件位置 | `.route/objects/` | `.git/` |
| 与团队协作 | 不兼容 | 兼容 |
| 远程仓库 | 不支持 | 支持 |
| 文件类型 | 任意文件 | 任意文件 |
| 操作命令 | `route commit` | `route git commit` |

---

## 常见问题

### Q: 如何选择标准模式还是 Git 模式？

**标准模式**适合个人项目、快速原型、非代码项目。无需 Git 环境，所有数据存储在 `.route/` 目录。
**Git 模式**适合需要与团队协作、使用远程仓库、需要完整 Git 工作流的项目。

### Q: MCP 权限被拒绝怎么办？

远程操作（push/pull/fetch/remote）在 Normal 模式下被阻止。有两种方式解除：

1. 在设置页 → 外部接入 → 权限等级，切换为 High
2. 重启 MCP 时添加 `--permission-level high` 参数

### Q: 如何恢复误删的快照？

Route 的 undo/redo 机制可以恢复。如果刚刚回退，使用 `undo` 命令即可恢复。如果已经过了多步，建议使用 `route log` 查找目标快照 ID，然后通过 `rollback` 回到该快照。

### Q: 备份失败了怎么办？

检查备份目标路径是否存在且有写入权限。如果是云端备份，检查网络连接和认证信息。Route 的备份是增量式的，重试不会重复存储已备份的数据。

### Q: 如何让 AI 自动提交代码？

在设置页开启 AI 模式，设置 AI 提供商的 API Key 和模型。Route 会检测 AI 参与的文件变更，自动记录 AI 归属信息。AI 可通过 MCP 工具 `route_commit` 自行提交变更。

### Q: 跟踪模式不工作？

检查设置页中跟踪开关是否已打开，备份目标是否已配置。跟踪模式需要至少一次成功的备份配置才能启动。