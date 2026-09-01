# Route

> **Git remembers the code. Route remembers the work.**
>
> **当前版本：v1.0 beta**（machine `1.0.0-beta`）— Experimental Beta

**Route** 是一个 **local-first、基于会话（session）的 AI 辅助开发管理系统**。
它不是 Git 的替代品，而是记录"**工作**"那一层：项目意图、连续性、任务历史、KnownGood、
checkpoint、证据、修复、重构、迁移、恢复，以及 AI/人类交接时的开发执行上下文。

```
UNDERSTAND BEFORE REWRITE.
PATCH WHEN PATCH ENOUGH.
RESTRUCTURE WHEN STRUCTURE IS THE PROBLEM.
UGLY != WRONG.  OLD != BAD.  NEW != BETTER.
PRESERVE WORKING VALUE.  VERIFY BEFORE PROMOTION.
```

---

## 为什么不是用 Git 就够了？

Git 记住**代码**——谁、何时、改了什么。Route 记住**工作**：

- 一个开发任务从哪来、到哪去、验收是什么；
- 哪些状态是 **KnownGood**，哪些 checkpoint 可以恢复；
- 记录了哪些**证据**，验证是否真的通过（而不是 AI 嘴上说"通过了"）；
- 什么时候该 **PATCH**、什么时候才该 **RESTRUCTURE**；
- AI Worker 与人类之间如何**交接**而不丢上下文。

Route 直接使用用户项目的状态，把"一次开发"作为一等公民记录下来，而不是把一堆文件变更扔进历史里。

---

## 问题 / 它解决什么

- **AI 开发缺连续性**：每次新会话都像失忆——Route 用会话 + 上下文 + 记忆把工作衔接起来。
- **AI 声称 ≠ 证据**：Route 区分"人/系统的真实证据"与"AI 的自述"，验证才有说服力。
- **回退与恢复不可靠**：KnownGood / checkpoint / 保存点 / 恢复策略让项目能被修复而不是只能重改。
- **改之前不懂**：`UNDERSTAND BEFORE REWRITE`——先理解现状，再决定 patch 还是重构。
- **人机交接断裂**：handoff / brief 生成可移交的开发上下文。

> Route **不假定项目是空白**——`existing project first-class`。进入任意既有项目：
> 理解当前状态、保留既有文件、识别结构、建立项目状态与 KnownGood，然后执行有边界的开发任务。

---

## 快速开始

### 构建（Rust workspace）

```bash
cd tool/route
cargo build -p route-cli          # CLI 二进制
cargo build --workspace           # 全部 crate
cargo test  --workspace           # 全量回归
```

仓库根目录是 Route 项目的管理边界；Rust Reference Engine 的 workspace
位于 `tool/route/`。完整目录职责见
[`docs/repository-layout.md`](docs/repository-layout.md)。

### 初始化一个既有项目并开发

```bash
route init                        # 进入/初始化既有项目
route commit -m "…"               # 提交一次"开发"（edge-centric）
route log                         # 查看开发历史
route self-sop                    # 把 route 自身约束升级为 SOP（生成 .route/sop.md）
route self-archive git-init       # 文档区本地 git 备份（无远程）
route self-archive git-commit --message "…"  # 提交整个文档区快照
```

### 既有的 CLI

- 全部命令与本文件的规范见 **`ROUTE.md`**（给任何 AI 的规范与操作手册，单一事实源）。
- 命令行手册见 [`docs/cli.md`](docs/cli.md)。

### Release 产物

发布包、安装器和临时装配内容统一写入根目录的 `Release/`。该目录已被
Git 忽略，不属于源码仓库；正式分发前只从 `Release/packages/` 取经过验证的产物。

---

## 依赖 / 约束 / 参考

Route 分别消费项目的约束资料与参考资料：

- `constraints/` — **Binding project rules** used during development（规范性，含 `constraints/ppam/`）。
- `references/` — **Optional supporting material** used for understanding and decision-making（参考性，不产生强制约束）。

约束优先于参考；参考不能覆盖约束。详见 `docs/` 与 `ROUTE.md`。

---

## 自进化能力（SOP / 自存档 / 本地备份）

Route 会**约束自身的开发**并把它沉淀成可执行、可追溯的 SOP，全部基于真实数据：

| 能力 | 命令 | 产物 / 落盘 |
|------|------|-------------|
| 升级为 **SOP**（并持久化为能力） | `route self-sop` | `<project>/.route/sop.md`（由项目记忆 + 自身 standard 生成）；同时**版本化持久化**到统一存档 `Documents/Route/route/versions/`（append-only，可回溯） |
| **自存档**（append-only） | `route self-archive {archive,list,show,apply}` | `Documents/Route/route/versions/v######/` |
| **文档区本地 git 备份**（无远程） | `route self-archive {git-init,git-commit,git-log}` | `Documents/Route/`（仅本机 git 仓库，永不推送） |
| **自改进**（只生成，不自动应用） | `route self-improve` / `route self-evolve` | pattern / workflow / memory 提案 |

> 这是 **fruit 组件**（Route 自进化）——详见
> [`tool/route/fruit/README.md`](tool/route/fruit/README.md)。

---

## 分支策略（重要）

| 分支 | 角色 |
|------|------|
| **main** | 稳定 Route 产品源 / beta release（**默认推荐，正常使用走这里**） |
| **fruit** | 自进化**组件演示**（额外推送的展示分支） |
| **sandbox** | 开发记录 / 实验 / 验证历史 |

```
MAIN    = WHAT ROUTE IS.
FRUIT   = WHAT ROUTE MAY BECOME.   （组件演示）
SANDBOX = HOW ROUTE WAS DEVELOPED.
```

### 关于 `fruit` 的重要说明

- `fruit` 分支是**额外推送上去**的一个分支，**只为了展示** route 的默认自进化结果，
  不是产品、不是正式发布。
- **正常使用时请只看 `main`**。`fruit`（以及本仓库内 `fruit` 相关产物，如
  `tools/patchbench/`）**仅为组件演示**，不要把它当作独立产品使用。
- fruit 组件的**真实内容与存储路径在 route 仓库内部的单个独立文件夹**：
  **`tool/route/fruit/`**；其自进化数据落盘于 `Documents/Route/route/` 与
  `.route/sop.md`，均为本机文件、不依赖该 git 分支。
- 推广到 `main` 只依据可验证证据，绝不靠 AI 自己决定。

---

## 文档

| 文档 | 内容 |
|------|------|
| [ROUTE.md](ROUTE.md) | 给任何 AI 的规范与操作手册（单一事实源） |
| [AGENTS.md](AGENTS.md) / [CLAUDE.md](CLAUDE.md) | 由 `route apply` 生成的有效开发上下文 |
| [docs/](docs/) | 架构、CLI、概念、回退恢复、协议、演化等 |
| [docs/repository-layout.md](docs/repository-layout.md) | 仓库根目录、源码、文档与本地 Release 产物的边界 |
| [docs/open-development-substrate.md](docs/open-development-substrate.md) | 项目级 DevelopmentEvent、全局 revision、Worker/Presence/Message 与 Evidence 边界 |
| [docs/route-cooperation-protocol.md](docs/route-cooperation-protocol.md) | `route/1` 主机/模型中立 stdio/JSONL 协议 |
| [tool/route/fruit/README.md](tool/route/fruit/README.md) | fruit 组件（自进化演示）规范与存储说明 |

---

## License

**AGPL-3.0** — 见 [LICENSE](./LICENSE)。第三方组件见 [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)。
