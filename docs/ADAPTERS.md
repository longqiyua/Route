# 对话适配器（Adapters）

Route 的最小版本单元是 **对话轮次**。适配器负责从不同 AI 工具采集对话并转为 `route commit`。

## 已内置

| ID | 名称 | 状态 |
|----|------|------|
| `manual` | 手动提交（CLI / 桌面 UI） | ✅ 可用 |
| `cursor` | Cursor Chat 日志 | 🔜 占位 |
| `codex` | Codex TUI | 🔜 占位 |
| `opencode` | OpenCode | 🔜 占位 |

## 适配器接口

```typescript
interface ConversationAdapter {
  id: string;
  name: string;
  description: string;
  isAvailable(): Promise<boolean>;
  pollLatest?(): Promise<Partial<CommitTurnInput> | null>;
  watch?(onTurn: (input: Partial<CommitTurnInput>) => void): () => void;
}
```

## 实现新适配器

1. 在 `packages/core/src/adapters/` 添加实现
2. 注册到 `BUILTIN_ADAPTERS`
3. 在 `.route/adapters/<id>.json` 写入配置（watch 路径等）
4. 桌面端 / CLI 通过 `--adapter <id>` 标记来源

## 建议的 Cursor 适配器路径（待调研）

- 监听 Cursor 项目级或全局对话存储
- 解析最后一轮 user/assistant 消息
- 自动调用 `commitTurn`，并填充 `instruction` 四点（若 AI 在回复中声明）

> 各工具日志格式可能随版本变化，适配器应版本化并允许用户手动指定路径。

## AI 声明四点的建议格式

在 AI 回复末尾附加结构化块，适配器可解析：

```yaml
route:
  start: "turn-a1b2c3d4"
  end: "hero-section-complete"
  acceptance: "首页可见 Hero，移动端正常"
  emergency_rollback: "turn-a1b2c3d4"
```

CLI 等效：

```bash
route commit -u "..." -a "..." \
  --start "turn-a1b2c3d4" \
  --end "hero-section-complete" \
  --acceptance "首页可见 Hero" \
  --emergency "turn-a1b2c3d4"
```
