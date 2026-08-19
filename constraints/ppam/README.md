# PPAM — Point to Programming Auxiliary Materials（约束资料）

这是 **PPAM** 在本仓库中的 **canonical 规范来源**，位于 `constraints/` 体系内（binding material）。

## 是什么

PPAM（可插拔文档增强组件）是独立的文档增强系统（MIT License），用于增强 AI 开发流程的规范性：需求清晰度、变更可追溯、bug 可回归、功能稳中求进。

- `2_Request/Request.md` — 对 AI 的核心要求（人读中文规则说明，本包心脏）
- `.trae/skills/vibe-flow/SKILL.md` — AI 实际加载的提示词本体（与 Request.md 一致）
- `3_Prompt/Prompt_Optimization.md` — 用户提示词优化规范
- `1_Configuration/` — 系统内部配置（命名 / 整理规范、脚本、CI）
- `INDEX.md` — 顶层结构说明
- `版本规范.md` — 版本号格式规范
- `Reference/` — PPAM 自带的参考设计资料（informative）

## 范围说明

本目录为 PPAM 的**规范核心**。PPAM 原始语义与内容保持原样，未做重解释。

PPAM 自带的 `4_Expand/`（可拓展层）按 PPAM 自身定义属于**非规范参考资料**（用户自定义内容 + anthropic-skills / skills-main 外部参考，冲突只提醒不自动改），因此**不进入** Route main 的 `constraints/`，也不复制进本仓库；其规范源位于 PPAM 远程仓库（`https://github.com/longqiyua/ppam`，见 `.ppam-link` 语义）。

## 单一权威源

`constraints/ppam/` 是 PPAM 约束资料的 ONE CANONICAL SOURCE。其他分支 / 集成环境不得维护无同步语义的 PPAM 副本。