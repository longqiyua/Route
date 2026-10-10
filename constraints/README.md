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

约束资料是可供显式选择、审阅和投影的源材料。仅因文件位于此目录，
**不会**自动成为 Worker 指令，也不会把原文无界注入上下文。Route v1
以有界 Constraint projection 为准；具体权限、适用范围和验证结果仍由
Route 现行语义及授权流程决定。

能机器验证的约束（路径、分支、测试）可通过现有 Gate 执行，
但资料本身不构成验证通过的证据。

`TEXT DESCRIBES THE CONSTRAINT. THE EXECUTOR ENFORCES WHAT IT CAN.`
