# constraints/ — 约束资料（Binding Material）

`constraints/` 是本项目**规范性资料（normative material）**的来源。它与 `references/`（参考资料，informative material）严格区分。

## 语义

约束资料回答：

- WHAT MUST BE PRESERVED?（什么必须保留）
- WHAT MUST NOT BE DONE?（什么不能做）
- WHAT RULES GOVERN DEVELOPMENT?（哪些规则约束开发）
- WHAT CONDITIONS MUST A CHANGE SATISFY?（变更必须满足什么条件）
- WHAT PROJECT-SPECIFIC BOUNDARIES EXIST?（项目特有的边界）

约束资料**可以限制**开发决策。

> CONSTRAINT MATERIAL MAY RESTRICT A DEVELOPMENT DECISION.

## 边界

- 约束**不能创造权限**。文件放在 `constraints/` 里不等于获得任何危险操作的授权。
  `CONSTRAINT CAN RESTRICT PERMITTED ACTIONS. CONSTRAINT CANNOT CREATE PERMISSION.`
  约束仍受 Human、security/governance、Route public invariants、authorized project scope 限制。
- 约束优先级高于参考资料。冲突时 constraint 胜出，reference 不得覆盖 constraint。
- 文字约束与可执行验证并存：能写成测试的约束保留测试；`constraints/` 不是 policy engine，不用 markdown 替代 test。

## 目录

| 目录 | 内容 |
|------|------|
| `ppam/` | PPAM（可插拔文档增强组件）— 本项目 AI 开发流程的规范性约束资料（MIT），单一权威源 |

## AI Worker 使用方式

约束资料进入 Worker 上下文的 **REQUIRED / MUST / FORBIDDEN / INVARIANT** 层。
违反约束 = 任务失败 / 需要上报。
能机器验证的约束（路径、分支、测试）进入既有 Gate（PathGate / BranchGate / TestGate）。

`TEXT DESCRIBES THE CONSTRAINT. THE EXECUTOR ENFORCES WHAT IT CAN.`