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
cargo build -p route-cli          # CLI 二进制
cargo build --workspace           # 全部 crate
cargo test  --workspace           # 全量回归
```

### 初始化一个既有项目并开发

```bash
cd /path/to/your/project
route init                         # 进入项目，保留既有文件，建立状态
route status                       # 项目概览（integrity / profile / sessions）
route task start "..."             # 启动一个有边界的开发会话
route task exec   "-- task ..."    # 有边界地执行（BranchGate/PathGate/CommandGate）
route task verify                  # 验证（真实测试证据，而非 AI 声称）
route task end    success          # 收尾，生成 Outcome
route commit -m "..."              # 把当前状态存为快照
route log                          # 查看历史
route rollback <snapshot>          # 回退到快照
route check                        # 校验仓库完整性
route repair-plan                  # 基于 check 生成修复计划
```

### Python 绑定（route-py）

```bash
cd packages/route-py
maturin build --release
# 生成的 wheel：route_vc-1.0.0b0-cp310-abi3-*.whl
python -c "import route; print(route.__version__)"   # 1.0.0-beta
```

---

## 核心安全思想

| 思想 | 含义 |
|------|------|
| **Evidence > 声称** | AI 说"测试通过"不算，Route **实际执行**测试才算。`CLAIM_EQUALS_EVIDENCE = NO` |
| **有边界的 AI Worker** | TaskSpec：目标 / 允许路径 / 禁止路径 / 测试 / 不变项 / 作用域 / 分支 |
| **多道门** | BranchGate、PathGate、CommandGate、DiffGate、TestGate、ClaimEvidenceGate |
| **禁止伪造证据** | AI 不能把自述伪装成系统证据（TestPass/Commit 由系统产生） |
| **已知好状态** | KnownGood / checkpoint / 保存点 / 恢复策略，破坏可回滚而非只能重写 |
| **UNDERSTAND BEFORE REWRITE** | 重构是大手术，先理解，能 patch 就 patch |
| **验证后才提升** | Candidate → Benchmark → Compare → 显式 Promote；AI 不能自我 promote |

---

## 让 AI / Agent 使用 Route

Route 提供 **host 中立** 的合作面，可被 Yuich、Claude/Codex 风格 Agent host、自定义 harness、
人类控制器、乃至未来系统调用。它**不要求任何特定宿主的私有 schema**。

接口包括：

- **CLI** — `route` 二进制（上表命令即公开入口）。
- **MCP** — `route-mcp`（Model Context Protocol）。
- **HTTP** — `route-http` REST API。
- **TUI** — `route-tui` 交互式 REPL。
- **Python** — `route-py`（PyO3 绑定）。
- **协议层** — `task` / `context` / `constitution` / `protocol` / `reference` / `workflow` / `apply`
  把决策上下文编译给外部 Coding AI（Claude Code、Codex……），`.route/` 才是真值源。

Route 与任意宿主**严格解耦**：**ROUTE MUST BE VALUABLE WITHOUT YUICH.** Yuich 只是其中一个高级 Host / dogfood 参与者。

---

## 当前状态：v1.0 beta

**v1.0 beta** 表示第一代 Route product semantics 已收敛、核心 development lifecycle 可运行，
并且：

- existing project first-class 已实证；
- KnownGood / checkpoint / recovery 存在；
- Evidence / verification 存在；
- AI / Agent / Harness 集成存在；
- Route 可完全独立运行；
- 可被 Yuich 或其他 Host 调用。

但 **不承诺 production-perfect**，也不会把未验证能力写成 `completed`。API / CLI / Protocol
仍可能在 beta dogfood 之后调整。

### 已实证的验证（本轮 release closure）

| 项目 | 结果 |
|------|------|
| `cargo test --workspace` | PASS（全 crate 零失败） |
| `route-basic` | PASS（323 项） |
| CLI sanity（help / version） | PASS |
| `route --version` | `route 1.0.0` |
| existing-project 测试 | PASS（init→status→commit→log，保留既有文件） |
| `route-py` `maturin build --release` | PASS（wheel 生成 + import 冒烟） |
| Route self-dogfood | PASS（route-cli 无害 unused import 清理，测试通过） |

详细开发过程记录在 **sandbox** 分支。

---

## 版本策略

| 位置 | 值 |
|------|----|
| Route 产品 release | `1.0.0-beta`（display: v1.0 beta） |
| Rust workspace 版本 | `1.0.0` |
| route-py（`route-vc`） | `1.0.0-beta` |
| TOOL.json | `1.0.0-beta` |
| CLI `--version` | `route 1.0.0` |

产品 release、protocol revision、schema revision、Handle revision **不必是同一个数字**。

---

## 项目结构

```
route/
├── Cargo.toml            # Rust workspace（12 个 crate）
├── crates/
│   ├── route-core/       # 核心原语（paths / hash / guard / schema / storage）
│   ├── route-basic/      # 核心仓库实现（会话、任务、证据、发展生命周期）
│   ├── route-cli/        # CLI 二进制
│   ├── route-mcp/        # MCP 服务器
│   ├── route-tui/        # 交互式 REPL
│   ├── route-http/       # HTTP REST API
│   ├── route-engine/     # 搜索 / 模糊匹配 / token
│   ├── route-memory/     # 记忆 / 因果链 / 会话跟踪
│   ├── route-sync/       # 同步与备份（WebDAV / S3 / SSH……）
│   ├── route-plugins/    # 插件系统
│   ├── route-stats/      # 统计
│   └── route-pyo3/       # Python 原生绑定
├── packages/route-py/    # Python 包（pip install route-vc）
├── README.md             # 本文件
├── ROUTE.md              # 给任何 AI 的规范与操作手册
├── LICENSE               # AGPL-3.0
└── THIRD_PARTY_NOTICES.md
```

---

## 分支

| 分支 | 角色 |
|------|------|
| **main** | 稳定 Route 产品源 / beta release 线（**默认推荐**） |
| **fruit** | Route 实验改进与 dogfood 结果（可能领先，但 ≠ stable ≠ release） |
| **sandbox** | Route 开发记录 / 实验 / 验证历史（开发史，不是产品状态） |

```
MAIN  = WHAT ROUTE IS.
FRUIT = WHAT ROUTE MAY BECOME.
SANDBOX = HOW ROUTE WAS DEVELOPED.
```

---

## License

**AGPL-3.0** — 见 [LICENSE](./LICENSE)。第三方组件见 [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)。