# Fruit 组件（自进化演示）

> **FRUIT = WHAT ROUTE MAY BECOME.（route 自进化的演示组件）**
>
> 本文件夹是 **Route 自进化（self-evolution）组件的规范存储位置**——它位于
> route 仓库内部的一个独立文件夹里，与产品代码同级、不混联。
>
> ⚠️ **组件演示专用。正常使用 route 时，请使用 `main` 分支，不要本文件夹当成独立产品。**

---

## 定位（约定）

| 概念 | 说明 |
|------|------|
| **fruit 组件** | Route 把**自身的开发约束升级为 SOP / 自进化工具**的一组能力 |
| **存储位置** | 组件物理存放于本独立文件夹：`tool/route/fruit/` |
| **演示分支** | `fruit` git 分支是对该组件的**额外推送、仅用于展示**的副本 |
| **正常使用** | 默认不依赖 `fruit` 分支；自进化结果存储于本机独立文件（见下） |

**一句话**：`fruit` 分支是额外推送上去的**演示**，真实内容在
`tool/route/fruit/` 这个 route 内部单独文件夹里；正常用 route，只看 `main`。

---

## 组件包含的能力

- **自进化 / 升级为 SOP**：读取 route 自身的
  `constitution.md` / `protocol.md` / `reference` 与真实项目记忆，把开发约束
  提炼成可沉淀、可追溯的 **Standard Operating Procedure（SOP）**，落盘为
  `.route/sop.md`（`route self-sop`）。
- **自存档（self-archive）**：把 route 自身标准文件做版本化、append-only 存档
  （`Documents/Route/route/versions/`），`route self-archive {archive,list,show,apply}`。
- **自改进（self-improve / self-evolve）**：研究 route 自身代码库，产出
  pattern / workflow / memory 提案（只生成、不自动应用）。
- **文档区本地 git 备份（无远程）**：`Documents/Route/` 作为**只在本机的本地
  git 仓库**，`route self-archive {git-init,git-commit,git-log}`，永不上传。

> 说明：以上能力的实现位于 `crates/route-basic/src/`（`self_archive.rs`、
> `sop.rs`、`backup.rs`）与 `crates/route-cli/`；本文件夹是其**规范归属地**
> 与文档入口，避免与产品主职耦合。

---

## 自进化结果的真实存储路径

route 默认的自进化 / 存档结果**存于本机文件**，不依赖任何远程：

| 内容 | 路径（route 单独文件夹） |
|------|--------------------------|
| SOP 产物（本地引用） | `<project>/.route/sop.md` |
| SOP 持久化能力（版本化，append-only） | `Documents/Route/route/versions/v######/`（含 `sop.md` + constitution/protocol） |
| 自存档版本 | `Documents/Route/route/versions/v######/` |
| 本地 git 备份仓库 | `Documents/Route/`（无远程，仅本机） |
| 跨项目自进化输入 | `Documents/Route/projects/<project-id>/…` |

> **持久能力**：`route self-sop` 生成的 SOP 不仅是项目本地文件，还会作为
> versioned capability 追加存档到统一存档，随更新不断累积、可回溯、可被其它
> 项目参考——这就是 route 的持久能力 + 统一存档 + 跨项目协作的落地。

---

## 分支策略（请按此使用）

| 分支 | 角色 |
|------|------|
| **main** | 稳定 Route 产品源 / beta release（**默认推荐，正常使用走这里**） |
| **fruit** | 自进化**组件演示**（额外推送的展示分支，≠ stable ≠ release） |
| **sandbox** | 开发记录 / 实验 / 验证历史 |

> 推广到 `main` 只依据可验证证据，绝不靠 AI Worker 自行决定。