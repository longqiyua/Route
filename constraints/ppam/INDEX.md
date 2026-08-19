# PPAM — Point to Programming Auxiliary Materials

指向编程辅助资料。下载解压放进任何代码仓库，AI 在 vibecoding 时自动遵循四阶段循环，增强需求清晰度、变更可追溯性、bug 可回归性、功能稳中求进。

兼容 trae / codex / claude-code / deepcode / tui / opencode 等工具——靠提示词本身的工具自适应接口，不硬编码入口。

## 顶层结构（按逻辑顺序编号）

```
1_Configuration/    系统内部配置（用户不直接接触）
  Nominate/         命名规范（Naming_Rule.md）
  Trim/             整理规范（Trim_Rule.md）
  scripts/          脚本（Grill_Init/Grill_Start/Validate_Structure）
  .github/          CI 配置
  .grill/           Grill 工作空间
2_Request/          对 AI 的核心要求（Request.md，人读的中文规则说明）
3_Prompt/           用户提示词优化规范（Prompt_Optimization.md）
4_Expand/           可拓展层（用户随便塞，AI 主动吸收，冲突主动提醒）
  CREDITS.md        参考资料版权声明
  anthropic-skills/ Anthropic 官方 skills 参考
  skills-main/      Matt Pocock skills 参考
.trae/              trae 工具自动加载路径（隐藏，提示词本体）
```

## 命名规范

详见 [1_Configuration/Nominate/Naming_Rule.md](1_Configuration/Nominate/Naming_Rule.md)。核心要点：

- 文件夹首字母大写，不用全大写
- 词组用 `_` 连接，非组合并列用 `-` 连接
- 特殊强调的单个文件用全大写（INDEX/Request/Expand）
- 全英文字符，禁止驼峰
- 顶层文件夹数字前缀按严格逻辑顺序

## 整理规范

详见 [1_Configuration/Trim/Trim_Rule.md](1_Configuration/Trim/Trim_Rule.md)。核心机制：

- PPAM 分类标准是最高优先级，用户自定义内容只能在 `4_Expand/` 内
- AI 发现散落文件/错误位置/命名不规范/隐含配置散落时自动整理
- 整理后输出报告，不删除用户文件，4_Expand 冲突只提醒不自动改

## 各文件夹职责

### 1_Configuration — 系统内部配置
最高优先级分类标准。用户日常不用碰，AI 按此规范整理整个包。
- `Nominate/` —— 命名规范，强制对齐
- `Trim/` —— 整理规范，强制归位
- `scripts/` —— 脚本（开发本包时用）
- `.github/` / `.grill/` —— CI 与 Grill 工作空间

### 2_Request — 对 AI 的核心要求
本包的心脏。告诉 AI 在 vibecoding 时必须遵循的四阶段循环（需求→写码→修 bug→加功能）与全程硬约束。给人读的中文说明，AI 实际加载的本体在 `.trae/skills/vibe-flow/SKILL.md`，两边内容一致。

### 3_Prompt — 用户提示词优化规范
AI 收到用户输入后，执行前必须先看这层。按 `2_Request/` 标准检查用户提示词，能自优化就自优化（补全缺失信息、规范术语），必须问用户时才问（歧义、决策、破坏性、越阶段）。优化标准就是符合 Request 的要求。

### 4_Expand — 可拓展层
用户随便塞任何东西（参考资料、规则、示例、领域知识、风格指南）。AI 每次对话开始主动扫描全部文件，结合固定规则增强行为。优先级：用户显式规则 > 参考示例 > 默认规则。**冲突时主动提醒用户**。无需配置，放文件即生效。自带 anthropic-skills 与 skills-main 两套参考资料。

### .trae — trae 工具自动加载路径（隐藏）
trae 系统约定路径，AI 自动发现并加载 `.trae/skills/vibe-flow/SKILL.md`。这是提示词本体。

## 散文件（根目录）

| 文件 | 性质 | 说明 |
|------|------|------|
| `package.json` | npm 约定 | 必须在根，scripts 路径指向 1_Configuration/scripts/ |
| `.gitignore` | Git 约定 | 必须在根 |

## 工作流程（AI 视角）

1. 加载 `.trae/skills/vibe-flow/SKILL.md`（四阶段固定规则）
2. 检测当前运行环境（trae/codex/claude-code/...），按工具约定加载本包规则
3. 扫描 `4_Expand/` 全部文件增强行为（冲突提醒）
4. 按 `1_Configuration/Trim/Trim_Rule.md` 检查并整理结构
5. 收到用户输入 → 过 `3_Prompt/` 优化规范 → 对照 `2_Request/` 标准
6. 在目标项目根维护 `.vibe/` 工作文件（待办/bug/变更日志/术语表）

## 使用方法

1. 下载本包，解压到你的代码仓库根目录
2. 开始 vibecoding，AI 自动遵循四阶段循环
3. 想加自定义规则/资料：往 `4_Expand/` 丢文件即可
4. 想改对 AI 的核心要求：改 `2_Request/Request.md` 与 `.trae/skills/vibe-flow/SKILL.md`（保持一致）
5. 想改提示词优化规范：改 `3_Prompt/Prompt_Optimization.md`
6. 命名与整理规则在 `1_Configuration/Nominate/` 与 `1_Configuration/Trim/`

## 多工具兼容

本包不硬编码特定工具入口文件。靠 SKILL.md 全局规则第 11 条"工具自适应"：
- AI 自行检测当前运行环境
- 按当前工具的约定入口（如 claude-code 认 CLAUDE.md，codex 认 AGENTS.md）加载本包规则
- 规则源始终是 `2_Request/Request.md`（人读）与 `.trae/skills/vibe-flow/SKILL.md`（AI 加载）
- 各工具入口文件指向它们即可，无需为每个工具维护独立规则副本

## 版权声明

详见 [4_Expand/CREDITS.md](4_Expand/CREDITS.md)。本包核心提示词为 PPAM 项目原创（MIT），`4_Expand/` 下参考资料保持原作者版权与许可。
