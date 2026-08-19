# 🍖 Grill系统 - 结构化项目管理核心

Grill是一种无情的面试/提问技术，用于澄清需求、建立共同理解和记录决策。

## 🎯 核心理念

### 什么是Grill？
- **无情的提问**：一次只问一个问题，沿着决策树深入
- **共同理解**：直到AI和用户达成完全一致的理解
- **决策记录**：记录关键选择和术语定义
- **产出导向**：生成具体的任务和文档

### Grill的两种模式
1. **快速Grill**：简单的需求澄清，不保存详细记录
2. **深度Grill**：完整的决策树探索，生成CONTEXT.md和ADRs

## 📁 文件夹结构

```
.grill/
├── README.md              # 本文件
├── context.md            # 项目术语表
├── adrs/                 # 架构决策记录
├── sessions/             # Grill会话记录
└── templates/            # Grill模板
```

## 📚 核心文件

### context.md - 项目术语表
项目的普遍语言，包含：
- 核心概念定义
- 领域特定术语
- 决策上下文说明
- 共享的理解基础

### ADRs - 架构决策记录
记录难以逆转的关键决策：
- 决策上下文和问题
- 选择的方案
- 预期的后果
- 状态跟踪

## 🔄 Grill工作流

### 标准流程
```
1. 🎯 确定Grill范围
2. 🔍 启动Grill会话
3. 💬 沿着决策树提问
4. 📝 记录术语和决策
5. 📋 创建具体任务
6. ✅ 完成Grill会话
```

### Grill会话结构
```yaml
session-id: grill-20240722-001
topic: "用户认证系统设计"
participants: ["用户", "AI"]
start-time: "2024-07-22T10:00:00Z"
status: "completed"
decisions:
  - id: "decision-001"
    question: "使用哪种认证方式？"
    answer: "JWT + 刷新令牌"
    rationale: "无状态，适合微服务架构"
    adr-required: true
terms:
  - term: "JWT"
    definition: "JSON Web Token，用于无状态认证"
    context: "用户认证"
tasks-created:
  - id: "task-001"
    title: "实现JWT签发和验证"
    priority: "high"
```

## 🛠️ 使用指南

### 初始化Grill系统
```bash
# 初始化Grill工作空间
node scripts/grill-init.js

# 或使用npm脚本
npm run grill:init
```

### 开始Grill会话
```bash
# 开始新的Grill会话
node scripts/grill-start.js "用户认证系统设计"

# 交互式Grill
npm run grill:start
```

### 管理Grill产出
```bash
# 查看Grill会话记录
npm run grill:list

# 导出Grill产出
npm run grill:export

# 回顾Grill决策
npm run grill:review
```

## 🤖 AI协作规范

### AI应如何Grill
1. **主动澄清**：不确定时主动启动Grill
2. **结构化提问**：一次一个问题，按依赖顺序
3. **提供选项**：为每个问题提供建议答案
4. **记录产出**：及时更新context.md和ADRs
5. **创建任务**：从决策创建具体实施任务

### Grill问题类型
1. **事实性问题**：可以通过代码库探索回答
2. **决策性问题**：需要用户输入和选择
3. **术语性问题**：需要澄清概念定义
4. **约束性问题**：了解限制条件和要求

## 📊 Grill产出

### 1. 共享理解
- 完全对齐的需求理解
- 清晰的术语定义
- 明确的约束条件

### 2. 文档产出
- 更新的context.md术语表
- 新的ADR决策记录
- 任务描述和要求

### 3. 任务创建
- 具体的实施任务
- 明确的验收标准
- 合理的优先级分配

## 🎯 最佳实践

### 有效Grill的技巧
1. **从小开始**：从核心问题开始，逐步深入
2. **保持专注**：一次只讨论一个主题
3. **记录一切**：及时记录所有决策和术语
4. **定期回顾**：定期回顾和更新Grill产出

### 避免的陷阱
1. **避免并行提问**：一次只问一个问题
2. **不要跳过依赖**：按依赖顺序提问
3. **不要假设理解**：验证每个术语的理解
4. **不要忘记记录**：及时保存Grill产出

## 🔧 集成指南

### 与项目工作流集成
1. **需求分析阶段**：使用Grill澄清需求
2. **设计阶段**：使用Grill探索设计方案
3. **实现阶段**：参考Grill产出指导实施
4. **回顾阶段**：使用Grill记录学习经验

### 与AI工具集成
```javascript
// AI应自动检测何时需要Grill
if (requirementsUnclear || terminologyAmbiguous) {
  startGrillSession("需求澄清");
}

// Grill完成后创建任务
const tasks = createTasksFromGrillDecisions(grillSession);
```

## 📈 进阶用法

### 复杂项目的Grill策略
1. **分层Grill**：先Grill高层架构，再Grill具体实现
2. **并行Grill**：不同团队并行Grill不同模块
3. **迭代Grill**：随着项目进展迭代更新Grill产出

### Grill会话模板
创建可重用的Grill模板：
- 用户认证Grill模板
- API设计Grill模板
- 数据库设计Grill模板
- 部署架构Grill模板

## 🚀 快速开始

### 第一步：初始化
```bash
npm run grill:init
```

### 第二步：开始第一个Grill
```bash
npm run grill:start "我的第一个功能"
```

### 第三步：查看产出
```bash
# 查看术语表
cat .grill/context.md

# 查看决策记录
ls .grill/adrs/

# 查看会话记录
ls .grill/sessions/
```

### 第四步：创建任务
```bash
npm run task:create-from-grill
```

---

**设计理念**：通过结构化、无情的提问，建立AI和用户之间的完美理解桥梁，确保每个项目都建立在坚实、明确的基础上。