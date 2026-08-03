# Route Feature Toggle 说明

> **一个 Markdown，交给你的 AI，然后开始使用 Route。**
> **One Markdown, give it to your AI, and start using Route.**

---

## 特性开关 (Feature Toggle)

Route 采用特性开关架构，核心功能默认开启，部分高级功能需手动启用。

### 默认开启
| 功能 | CLI 命令 | 说明 |
|------|----------|------|
| CLI 模式 | `route` | 命令行界面，核心交互方式 |
| TUI 模式 | `route-tui` | 终端用户界面 |
| AI 助手 | `route ai chat` | AI 对话（需配置 API Key） |
| MCP 服务 | `route mcp` | MCP 协议支持 |
| 语义引擎 | `route base search` | 模糊匹配 + 语义索引 |

### 隐藏开关 (默认关闭)
| 功能 | 启用方式 | 说明 |
|------|----------|------|
| **GUI 桌面应用** | `route base gui enable` | Tauri 图形界面（需额外构建） |

---

## GUI 桌面应用

### 启用方式

**方式一：运行时启用（推荐）**
```bash
# 启用 GUI
route base gui enable

# 查看 GUI 状态
route base gui status

# 禁用 GUI
route base gui disable
```

**方式二：编译时启用**
```bash
# 构建 GUI 桌面应用
cd packages/desktop
npm install
npm run build
cargo tauri build
```

### 前置条件

GUI 桌面应用需要额外的前端构建环境：
- Node.js 18+
- Rust toolchain with Tauri dependencies
- 桌面平台特定依赖（WebView2 on Windows, WebKit on Linux/macOS）

### 架构说明

GUI 是一个独立的 Tauri 应用，位于 `packages/desktop/` 目录下，拥有独立的依赖树和构建配置。它通过 `desktop-bridge` 与 Route 核心交互。

---

## 扩展接口

Route 支持通过以下方式扩展：

| 接口 | 说明 |
|------|------|
| CLI | `route` 命令行，所有核心功能 |
| MCP | Model Context Protocol，AI 工具调用 |
| HTTP | REST API 服务（开发中） |
| PyO3 | Python 原生模块调用 |

---

## 协议

AGPL-3.0 — 详见 [LICENSE](./LICENSE)