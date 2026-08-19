# Route — Canonical Description & Operating Manual

> **One Markdown, give it to your AI, and start using Route.**
> **一个 Markdown，交给你的 AI，然后开始使用 Route。**
>
> **Route v1.0 beta**（machine `1.0.0-beta`）— Experimental Beta
>
> *Git remembers the code. Route remembers the work.*

本文件是 Route 的规范与操作手册：

- 描述 Route 是什么、能做什么、不能做什么；
- 如何构建、如何用于既有项目、如何让 AI/Agent/Host 使用；
- 版本、许可证、分支、验证状态。

> 真值来源：Route 对一个项目写入 `.route/` / `.route-basic/`，那里才是项目的真实状态。
> 本文件是给 AI / 人类阅读的规范与操作手册，不是项目的运行时真值源。

---

## 1. Route 是什么

**Route = local-first、基于会话（session）的 AI 辅助开发管理系统。**

它记录 Git 不记录的"**工作**"：

- 项目意图与连续性（constitution、protocol、reference、workflow、goal）；
- 任务/开发历史与执行上下文（task、context、agent-plan）；
- KnownGood / checkpoint / 保存点 / 恢复（savepoint、recovery、repair-plan）；
- 证据与验证（evidence、verify，区分真实证据与 AI 声称）；
- 决策路线（UNDERSTAND BEFORE REWRITE / PATCH when patch enough）；
- AI/人类交接（handoff、brief）。

**Route 不是 Git 的替代。** Route 在项目状态之上工作，`route commit` 产生快照，但核心价值在"开发如何被管理"而非"文件如何被存"。

**Route 完全独立、不依赖任何 Host。** 它可被 Yuich、Claude/Codex 风格 Agent host、自定义 harness、人类控制器和未来系统调用，但**不要求任何宿主**。

---

## 2. 让 AI 自动使用 Route（0 门槛分发）

把本文件（ROUTE.md）交给任意 AI，AI 可按以下流程让装 Route 并操作一个项目：

1. **获取源码**：`git clone https://github.com/longqiyua/route.git`。
2. **构建**：`cargo build -p route-cli --release`（生成 `route` / `route.exe`）。
3. **进入项目**：在目标项目目录 `route init`（保留既有文件，建立状态）。
4. **了解项目**：`route status` / `route context` / `route constitution` / `route goal`。
5. **启动受限开发**：`route task start "TASK"` → `route task exec -- "CMD"` → `route task verify` → `route task end success`。
6. **提交证据**：`route commit -m "msg"`。

> **验证纪律**：AI 说"测试通过"不算证据。用 `route task verify` 由 Route 实际执行验证。禁止把 AI 自述伪装成系统证据。

---

## 3. 构建 / 测试 / Python

```bash
cargo build --workspace
cargo build -p route-cli
cargo test  --workspace
cargo check --workspace

cd packages/route-py
maturin build --release        # wheel: route_vc-1.0.0b0-*.whl
```

`route-py` 可 `python -c "import route; print(route.__version__)"` → `1.0.0-beta`。

---

## 4. 命令总览（`route --help`）

```
init  status  commit  log  rollback  backup  branch  annotate  annotations
export  stats  stats-report  sync  plugin  tag  diff  changes  undo  redo
checkpoint  git  tracking  extensions  ai  project-context  mcp  permission
base  conversation  check  repair-plan  constitution  protocol  reference
workflow  plan  agent-plan  profile  curator  context  apply  learn  evolve
emerge  study  study-apply  self-improve  task  memory  strategy  experiment
agent-org  pattern  save  trajectory  capability  discover  pack  idea  failure
principle  agents  impact  health  health-history  guardian  next  goal  archive
brain  roadmap  loops  drift  maintain  brief  handoff  help
```

面向"工作管理"的核心命令：

| 命令 | 作用 |
|------|------|
| `task start / exec / verify / end` | 有边界、可验证的开发会话 |
| `context / apply` | 编译项目决策上下文给外部 Coding AI（Claude Code / Codex…） |
| `constitution / protocol / reference / workflow / profile` | 项目规则与资源 |
| `save / rollback / checkpoint / archive` | KnownGood 与恢复 |
| `check / repair-plan / health / guardian / drift` | 状态诊断与修复 |
| `learn / evolve / emerge / study` | 从开发中学习与演化（候选优先，AI 不自我 promote） |
| `memory / brain / trajectory / idea / failure / next` | 知识层 |

---

## 5. Workspace 结构

```
Cargo.toml            # Rust workspace（12 个 crate）
crates/
  route-core/         # 核心原语（paths/hash/guard/schema/storage）
  route-basic/        # 核心仓库实现（会话/任务/证据/发展生命周期）
  route-cli/          # CLI
  route-mcp/          # MCP 服务器
  route-tui/          # 交互式 REPL
  route-http/         # HTTP REST API
  route-engine/       # 搜索/模糊匹配/token
  route-memory/       # 记忆/因果链/会话
  route-sync/         # 同步备份（WebDAV/S3/SSH）
  route-plugins/      # 插件
  route-stats/        # 统计
  route-pyo3/         # Python 绑定
packages/route-py/    # pip 包 route-vc
README.md / ROUTE.md / LICENSE / THIRD_PARTY_NOTICES.md
```

**注**：项目运行时状态写入目标项目的 `.route/`（或 `.route-basic/`），不在本仓库内。

---

## 6. 合作接口（Host-Neutral）

- **CLI** `route`
- **MCP** `route-mcp`
- **HTTP** `route-http`
- **TUI** `route-tui`
- **Python** `route-py`
- **协议/上下文** `task`/`context`/`apply` 等

Route 提供 generic Host / Handle / DevelopmentIntent / Outcome / Evidence / CLI / MCP / API /
Python surface，但**不为任何特定宿主私有 schema 固化**。Yuich 是其中一个高级 Host 与 dogfood 参与者，是**参考集成**而不是**核心依赖**。

---

## 7. 版本

| 位置 | 值 |
|------|----|
| 产品 release | `1.0.0-beta`（display: v1.0 beta） |
| Rust workspace | `1.0.0-beta` |
| route-py | `1.0.0-beta` |
| TOOL.json | `1.0.0-beta` |
| CLI | `route 1.0.0-beta` |

产品 release ≠ protocol revision ≠ schema revision ≠ Handle revision。

---

## 8. 分支

| 分支 | 角色 |
|------|------|
| **main** | 稳定产品源 / beta release（默认推荐） |
| **fruit** | 实验改进 / dogfood 结果（≠ stable ≠ release） |
| **sandbox** | 开发记录 / 实验 / 验证历史 |

---

## 9. 验证状态（release closure 实证）

| 项 | 结果 |
|----|------|
| `cargo test --workspace` | PASS |
| `route-basic` | PASS（323） |
| CLI sanity | PASS |
| `--version` | `route 1.0.0` |
| existing project | PASS |
| `route-py` build | PASS |
| self-dogfood | PASS |

---

## 10. License

**AGPL-3.0** — 见 [LICENSE](./LICENSE)。第三方见 [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)。