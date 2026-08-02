# Route 开源发布建议

## 推荐仓库结构

- **单 monorepo** — `route` 根目录即仓库名，与文件夹一致
- **MIT License** — 对 Web Coding 生态友好
- **THIRD_PARTY_NOTICES.md** — 已包含 fast-glob、Tauri、React 等声明

## 发布前 Checklist

- [ ] 添加 CI：build + test（GitHub Actions）
- [ ] 添加 `CONTRIBUTING.md` 和 `CODE_OF_CONDUCT.md`
- [ ] 桌面端代码签名（Windows/macOS）
- [ ] CLI 发布到 npm：`@route/cli` 或全局 `route`
- [ ] 桌面 Release：Tauri 构建产物 + GitHub Releases
- [ ] 安全审计：`.route/` 不应被默认提交到用户项目（已在 ignore 规则中）
- [ ] 文档站：可考虑 VitePress / GitHub Pages

## 命名与品牌

- 项目名 **Route** — 寓意"按对话路线追踪版本"
- 元数据目录 `.route/` — 类比 `.git`，简短易记
- 避免与 npm 上已有 `route` 包冲突 → 发布时使用 scope `@route/cli`

## 社区定位

**不是 Git 替代品**，而是 **Vibe Coding 安全网**：

- 用户继续用 Git 做正式版本管理（可选）
- Route 负责 AI 对话粒度的快速回退
- 未来 Mode 2 可与 Git 共存，但需明确风险提示

## 建议的 GitHub 标签

`vibe-coding` `ai-coding` `version-control` `backup` `rollback` `tauri` `web-coding`

## 隐私说明

Mode 1 所有数据存于本地 `.route/`，不上传云端。对话内容随用户项目一起保留，开源 README 中应明确说明。

## 后续可开源的扩展

1. **Adapter 插件市场** — 社区贡献 Cursor/Codex 适配器
2. **Route Skill for Cursor** — 让 AI 自动执行 `route commit`
3. **Mode 2 Git 桥接** — 独立 feature flag，默认关闭
