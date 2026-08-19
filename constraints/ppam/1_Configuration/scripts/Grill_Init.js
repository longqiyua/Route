#!/usr/bin/env node

/**
 * Grill系统初始化脚本
 * 初始化.grill工作空间和核心文件
 */

const fs = require('fs');
const path = require('path');

// Grill系统配置
const GRILL_CONFIG = {
  version: '2.0.0',
  createdAt: new Date().toISOString(),
  structure: {
    contextFile: 'context.md',
    adrsDir: 'adrs',
    sessionsDir: 'sessions',
    templatesDir: 'templates'
  }
};

// 默认的context.md内容
const DEFAULT_CONTEXT = `# 🔤 项目术语表

## 🏗️ 核心概念

### Grill系统
- **Grill**：无情的提问澄清过程，一次一个问题沿着决策树深入
- **共同理解**：AI和用户之间完全对齐的需求理解
- **决策树**：结构化的问题解决方法，沿着分支深入探索
- **HITL任务**：需要人类参与的任务（Human in the Loop）
- **AFK任务**：AI可独立完成的任务（Away From Keyboard）

### 项目结构
- **.grill/**：Grill系统工作空间
- **src/**：项目源代码
- **docs/**：项目文档
- **tasks/**：任务管理文件
- **scripts/**：工具脚本

## 🔄 工作流阶段
- **🎤 需求提出**：用户描述需求或想法
- **🔍 Grill澄清**：通过提问建立共同理解
- **📝 文档建立**：创建术语表和决策记录
- **🎯 任务创建**：从决策创建具体实施任务
- **🔧 实施开发**：AI或开发者完成任务
- **🧪 测试验证**：验证功能正确性
- **📊 完成交付**：更新状态和文档

## 📊 任务状态系统
- **🔴 紧急**：需要立即处理，今天内完成
- **🟡 重要**：优先级高，3天内完成
- **🟢 正常**：标准优先级，1周内完成
- **⚪ 低优先级**：有空时处理，无紧急时间要求

## 🎯 使用指南

### 开始第一个Grill会话
\`\`\`bash
npm run grill:start "你的第一个功能"
\`\`\`

### 创建任务
\`\`\`bash
npm run task:create "实现用户登录功能"
\`\`\`

### 验证结构
\`\`\`bash
npm run validate
\`\`\`

---

**初始化时间**：${new Date().toLocaleDateString()}
**系统版本**：${GRILL_CONFIG.version}
**维护责任**：所有项目参与者
`;

// ADR模板
const ADR_TEMPLATE = `# [ADR-001] 决策标题

## 状态
[提议|接受|拒绝|过时]

## 上下文
问题的描述，为什么需要做决策。

需要考虑的因素：
- 技术约束
- 业务需求
- 用户体验
- 维护成本

## 决策
我们决定选择 [选项]，因为 [理由]。

具体来说：
1. [具体决策点1]
2. [具体决策点2]
3. [具体决策点3]

## 后果
### 正面影响
- [影响1]
- [影响2]

### 负面影响
- [影响1]
- [影响2]

## 替代方案考虑
### 方案A：[名称]
- 优点：[优点]
- 缺点：[缺点]
- 放弃原因：[原因]

### 方案B：[名称]
- 优点：[优点]
- 缺点：[缺点]
- 放弃原因：[原因]

## 相关决策
- [相关ADR链接]
- [相关技术文档]

## 验证标准
- [标准1]
- [标准2]

---

**决策时间**：YYYY-MM-DD
**决策者**：[姓名或角色]
**记录时间**：YYYY-MM-DD
`;

// Grill会话模板
const GRILL_SESSION_TEMPLATE = `# 🍖 Grill会话：[会话主题]

## 会话信息
- **会话ID**：grill-[日期]-[序号]
- **主题**：[会话主题]
- **参与者**：AI + [用户姓名]
- **开始时间**：[时间]
- **结束时间**：[时间]
- **状态**：[进行中|已完成|已取消]

## 会话目标
[描述本次Grill会话的目标和期望产出]

## 决策树探索

### 分支1：[主要决策点]
- **问题**：[具体问题]
- **答案**：[用户回答]
- **理由**：[选择理由]
- **影响**：[对后续决策的影响]

### 分支2：[次要决策点]
- **问题**：[具体问题]
- **答案**：[用户回答]
- **理由**：[选择理由]
- **影响**：[对后续决策的影响]

## 术语澄清
| 术语 | 定义 | 上下文 | 添加时间 |
|------|------|--------|----------|
| [术语1] | [定义] | [使用场景] | [时间] |
| [术语2] | [定义] | [使用场景] | [时间] |

## 产出总结

### 新术语添加到CONTEXT.md
1. [术语1]：定义
2. [术语2]：定义

### 需要创建的ADR
1. [ADR主题]：[简要描述]
2. [ADR主题]：[简要描述]

### 生成的任务
1. [任务标题]：[优先级]
2. [任务标题]：[优先级]

## 会话回顾

### 有效的方法
- [有效点1]
- [有效点2]

### 改进建议
- [改进点1]
- [改进点2]

### 下次Grill可以优化
- [优化建议1]
- [优化建议2]

---

**会话记录时间**：[时间]
**Grill系统版本**：${GRILL_CONFIG.version}
`;

// 初始化函数
function initializeGrillSystem() {
  console.log('🍖 初始化Grill系统...\n');
  
  const grillDir = '.grill';
  const errors = [];
  const successes = [];
  
  // 1. 创建.grill目录
  if (!fs.existsSync(grillDir)) {
    fs.mkdirSync(grillDir, { recursive: true });
    successes.push('创建 .grill/ 目录');
  }
  
  // 2. 创建子目录
  const subdirs = ['adrs', 'sessions', 'templates'];
  subdirs.forEach(dir => {
    const dirPath = path.join(grillDir, dir);
    if (!fs.existsSync(dirPath)) {
      fs.mkdirSync(dirPath, { recursive: true });
      successes.push(`创建 .grill/${dir}/ 目录`);
    }
  });
  
  // 3. 创建context.md
  const contextPath = path.join(grillDir, 'context.md');
  if (!fs.existsSync(contextPath)) {
    fs.writeFileSync(contextPath, DEFAULT_CONTEXT, 'utf8');
    successes.push('创建 .grill/context.md 术语表');
  } else {
    console.log('   ⚠️  .grill/context.md 已存在，保留现有文件');
  }
  
  // 4. 创建配置文件
  const configPath = path.join(grillDir, 'config.json');
  if (!fs.existsSync(configPath)) {
    fs.writeFileSync(configPath, JSON.stringify(GRILL_CONFIG, null, 2), 'utf8');
    successes.push('创建 .grill/config.json 配置文件');
  }
  
  // 5. 创建模板文件
  const templates = {
    'adr-template.md': ADR_TEMPLATE,
    'grill-session-template.md': GRILL_SESSION_TEMPLATE,
    'quick-grill-template.md': '# 🍖 快速Grill模板\n\n用于简单的需求澄清，不生成详细文档。'
  };
  
  const templatesDir = path.join(grillDir, 'templates');
  Object.entries(templates).forEach(([filename, content]) => {
    const filePath = path.join(templatesDir, filename);
    if (!fs.existsSync(filePath)) {
      fs.writeFileSync(filePath, content, 'utf8');
      successes.push(`创建模板文件 .grill/templates/${filename}`);
    }
  });
  
  // 6. 创建README
  const readmePath = path.join(grillDir, 'README.md');
  if (!fs.existsSync(readmePath)) {
    const readmeContent = `# 🍖 Grill系统工作空间

这是Grill系统的核心工作空间，包含：
- **context.md**：项目术语表
- **adrs/**：架构决策记录
- **sessions/**：Grill会话记录
- **templates/**：Grill会话模板

## 快速开始
1. 开始新的Grill会话：\`npm run grill:start "主题"\`
2. 查看现有会话：\`npm run grill:list\`
3. 回顾会话产出：\`npm run grill:review\`

## 文件说明

### context.md
项目的普遍语言，所有参与者共享的术语定义。

### ADRs (架构决策记录)
记录难以逆转的关键技术决策，遵循标准格式。

### Grill会话记录
每次Grill会话的完整记录，包括：
- 讨论的问题和答案
- 澄清的术语
- 产生的决策
- 创建的任务

## 维护指南
- 每次Grill会话后更新context.md
- 及时创建和更新ADR
- 定期清理过时的会话记录
- 备份重要的Grill产出

---
**初始化时间**：${new Date().toLocaleDateString()}
**系统版本**：${GRILL_CONFIG.version}
`;
    fs.writeFileSync(readmePath, readmeContent, 'utf8');
    successes.push('创建 .grill/README.md');
  }
  
  // 7. 创建初始ADR示例
  const exampleAdrPath = path.join(grillDir, 'adrs', 'example-adr.md');
  if (!fs.existsSync(exampleAdrPath)) {
    const exampleAdr = ADR_TEMPLATE.replace('[ADR-001] 决策标题', '[ADR-001] 示例：选择项目结构')
      .replace('问题的描述，为什么需要做决策。', '在项目开始时，需要决定使用哪种项目组织结构。\n\n需要考虑的因素：\n- 团队成员熟悉度\n- AI协作友好性\n- 长期维护成本\n- 与现有工具的集成')
      .replace('我们决定选择 [选项]，因为 [理由]。', '我们决定采用Grill驱动的精简结构，因为：\n1. 简化了AI理解难度\n2. 强化了需求澄清过程\n3. 保持了足够的灵活性')
      .replace('**决策时间**：YYYY-MM-DD', `**决策时间**：${new Date().toISOString().split('T')[0]}`)
      .replace('**决策者**：[姓名或角色]', '**决策者**：项目启动团队')
      .replace('**记录时间**：YYYY-MM-DD', `**记录时间**：${new Date().toISOString().split('T')[0]}`);
    
    fs.writeFileSync(exampleAdrPath, exampleAdr, 'utf8');
    successes.push('创建示例ADR .grill/adrs/example-adr.md');
  }
  
  // 显示结果
  console.log('📊 初始化结果:\n');
  
  if (successes.length > 0) {
    console.log('✅ 完成的操作:');
    successes.forEach(success => console.log(`   ${success}`));
  }
  
  if (errors.length > 0) {
    console.log('\n❌ 错误:');
    errors.forEach(error => console.log(`   ${error}`));
  }
  
  // 显示Grill系统状态
  console.log('\n🍖 Grill系统状态:');
  console.log(`   工作空间：${grillDir}/`);
  console.log(`   术语表：${fs.existsSync(contextPath) ? '✅ 已创建' : '❌ 缺失'}`);
  console.log(`   ADRs目录：${fs.existsSync(path.join(grillDir, 'adrs')) ? '✅ 已创建' : '❌ 缺失'}`);
  console.log(`   会话记录：${fs.existsSync(path.join(grillDir, 'sessions')) ? '✅ 已创建' : '❌ 缺失'}`);
  console.log(`   模板文件：${fs.existsSync(templatesDir) ? '✅ 已创建' : '❌ 缺失'}`);
  
  // 显示下一步建议
  console.log('\n🎯 下一步:');
  console.log('1. 运行 npm run validate 验证项目结构');
  console.log('2. 运行 npm run grill:start "你的第一个功能" 开始Grill会话');
  console.log('3. 查看 .grill/context.md 了解项目术语');
  console.log('4. 参考 .grill/adrs/example-adr.md 了解ADR格式');
  
  console.log('\n🚀 快速命令:');
  console.log('   npm run validate          # 验证结构');
  console.log('   npm run grill:start       # 开始Grill');
  console.log('   npm run task:create       # 创建任务');
  console.log('   npm run grill:list        # 查看会话');
  
  console.log('\n💡 提示:');
  console.log('   • Grill系统已准备就绪，可以开始需求澄清');
  console.log('   • 所有术语澄清都会自动记录到context.md');
  console.log('   • 关键决策应该创建ADR记录');
  console.log('   • 任务可以从Grill会话产出自动创建');
}

// 显示帮助信息
function showHelp() {
  console.log(`
🍖 Grill系统初始化工具

用法:
  npm run grill:init    初始化Grill系统
  node scripts/grill-init.js  直接运行初始化

功能:
  1. 创建 .grill/ 工作空间
  2. 初始化 context.md 术语表
  3. 创建 adrs/ 目录用于架构决策记录
  4. 创建 sessions/ 目录用于会话记录
  5. 创建 templates/ 目录包含模板文件
  6. 创建示例文件和配置文件

创建的文件:
  .grill/
  ├── config.json          # Grill系统配置
  ├── context.md          # 项目术语表
  ├── README.md           # Grill系统说明
  ├── adrs/              # 架构决策记录
  │   └── example-adr.md # ADR示例
  ├── sessions/          # Grill会话记录
  └── templates/         # 模板文件
      ├── adr-template.md
      ├── grill-session-template.md
      └── quick-grill-template.md

设计理念:
  Grill系统通过无情的提问澄清需求，建立AI和人类之间的
  完美理解，确保项目建立在坚实、明确的基础上。
`);
}

// 主函数
function main() {
  const args = process.argv.slice(2);
  
  if (args.includes('--help') || args.includes('-h')) {
    showHelp();
    return;
  }
  
  try {
    initializeGrillSystem();
  } catch (error) {
    console.error('❌ 初始化失败:', error.message);
    process.exit(1);
  }
}

// 导出函数
module.exports = {
  initializeGrillSystem,
  GRILL_CONFIG
};

// 运行主函数
if (require.main === module) {
  main();
}