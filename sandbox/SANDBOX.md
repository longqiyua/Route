# Route Sandbox

> 沙盒环境 — 用于 Route 的自反测试（Self-Referential Testing）和演示。

---

## 说明

`sandbox/` 目录是 Route 项目的**沙盒测试环境**，包含一个真实的第三方项目[DashBoard](https://github.com/longqiyua/DashBoard)（Spring Boot 任务看板），用于：

1. **自反测试** — Route 对自身能力的全路径白盒测试
2. **演示** — 展示 Route 如何管理、分析、搜索一个真实项目
3. **回归验证** — 确保 Route 的语义引擎、RAG 管线、记忆系统等对第三方项目有效

## 测试项目

| 项目 | 说明 |
|------|------|
| [DashBoard](https://github.com/longqiyua/DashBoard) | Spring Boot 任务看板（Java, Maven） |

## 测试范围

Route 自反测试覆盖以下所有模块：

- [x] Root Base 初始化
- [x] 语义模糊匹配（双模式：精确/语义）
- [x] 六步 RAG 循环
- [x] 代码图分析（函数调用、跨文件依赖）
- [x] 向量索引（混合向量化器：Fast + ML）
- [x] 关键词索引
- [x] 项目记忆系统
- [x] 路由搜索管线
- [x] 架构建议
- [x] MCP 工具接口

## 测试结果

### 最近一次：2026-08-04

**12/12 全部通过，100%**

| # | 测试项 | 命令 | 状态 |
|---|--------|------|------|
| 1 | Root Base 状态 | `route base status` | ✅ |
| 2 | 语义模糊搜索 | `route base search "task controller"` | ✅ |
| 3 | 项目状态 | `route status` | ✅ |
| 4 | 记忆统计 | `route base memory` | ✅ |
| 5 | 自反内省 | `route base self-manage introspect` | ✅ |
| 6 | 提交日志 | `route log` | ✅ |
| 7 | GUI 状态 | `route gui status` | ✅ |
| 8 | 权限状态 | `route permission status` | ✅ |
| 9 | 项目上下文 | `route project-context` | ✅ |
| 10 | MCP 配置 | `route mcp --config` | ✅ |
| 11 | AI 配置 | `route ai config` | ✅ |
| 12 | 基准测试 | `route bench run default` | ✅ |

基准测试：100% 通过率，F1 0.908，Drift 0.000%

详细报告：[results/self-ref-test-2026-08-04.md](results/self-ref-test-2026-08-04.md)

每次测试运行后，结果记录在 `sandbox/results/` 目录下。

---

*此沙盒由 Route 自身管理。*